//! `TEST-P1-04-05-A`: the composed degrade/inheritance scenario run for real
//! under timer-driven preemption (`STORY-P1-04-05`, closing `LE-50`).
//!
//! **What `STORY-P1-04-04` could not claim, and this fixture does.** That
//! Story closed `LE-22` — [`kernel::lock::PriorityInheritingLock`] and
//! [`kernel::wcet`] no longer collide, because the priority the scheduler
//! reads is `max(base, inherited)`, derived on demand and stored nowhere. It
//! proved that at the **host** level, driving the overrun through
//! `wcet::account_tick`, the entry point the timer ISR calls. What it could
//! not say is that the composition holds when the ticks are real, the
//! preemption is real, and the evidence is *who actually ran*. That is this
//! file, on the template [`crate::fixture_priority_inversion`] established
//! for the un-composed case.
//!
//! **Why a second fixture rather than an extension of that one.**
//! `fixture_priority_inversion` is the evidence `STORY-P1-04-01` is Verified
//! on. Adding a budget and an overrun to it would change what a Verified
//! Story rests on in order to save a file — the same reasoning
//! `STORY-P1-04-02` used when it built [`crate::fixture_wcet`] rather than
//! growing [`crate::fixture_preempt`].
//!
//! The scenario:
//!
//! | Task | Priority | Budget | Policy | What it does |
//! |---|---|---|---|---|
//! | low | 5 | 4 ticks | `Degrade(2)` | takes the lock, releases the other two, works while holding it, unlocks, then keeps working at its floor |
//! | high | 25 | generous | trip (never fires) | preempts low, contends (boosting low to 25), blocks; resumes after the release |
//! | medium | 15 | generous | trip (never fires) | busy-increments a counter whenever it is selected |
//!
//! medium sits **strictly between** low's own priority (5) and the boost
//! (25), and strictly above low's declared floor (2). That placement is the
//! whole design: it is the task that must not run during the window, and the
//! task that must run after it.
//!
//! **Why this is a demonstration rather than a re-assertion.** A host test
//! asserts priority *values*. This asserts a scheduling *outcome*: medium,
//! `Ready` throughout and demonstrably able to run, makes no progress at all
//! across a window that contains a WCET degrade of the boosted holder. Under
//! the pre-`STORY-P1-04-04` kernel that degrade discarded the boost, low fell
//! to 2 while high was still blocked, and medium would have started winning
//! selections immediately. **The frozen counter is a claim the old kernel
//! could not have satisfied.**
//!
//! Low never yields — no `switch`, no `hlt`, no scheduler call outside the
//! two bounded critical sections in which it takes and releases the lock — so
//! whatever takes it off the CPU is always the timer. Every scheduler
//! mutation made from task context happens inside
//! `hal_x86_64::interrupts::without_interrupts`, or it would race the tick
//! hook that reads the same scheduler.
//!
//! Only reachable when the `fixture-degrade-inheritance` feature is enabled —
//! never part of a real boot image.

#![allow(static_mut_refs, clippy::deref_addrof)]

use core::fmt::Write;
use hal_x86_64::interrupts;
use hal_x86_64::serial::SerialPort;
use kernel::context::{self, Context};
use kernel::dispatch;
use kernel::lock::{LockError, PriorityInheritingLock};
use kernel::measure::write_result;
use kernel::preempt::{self, TickOutcome};
use kernel::sched::{OverrunPolicy, Priority, Scheduler, TaskId, TaskState, WcetBudgetTicks};
use kernel::spoor::{Action, Category, Spoor};
use kernel::spoor_journal::SpoorJournal;
use kernel::wcet::{self, OverrunDisposition, TickAccounting};

const NAME: &str = "degrade-inheritance";

const TASKS: usize = 3;
const LOW_SLOT: usize = 0;
const MEDIUM_SLOT: usize = 1;
const HIGH_SLOT: usize = 2;

const LOW_PRIORITY: u8 = 5;
const MEDIUM_PRIORITY: u8 = 15;
const HIGH_PRIORITY: u8 = 25;
/// The floor low declares. **Below** medium, so a degrade that actually takes
/// effect changes who wins a selection — and above zero, so it is a real
/// priority rather than a sentinel.
const DEGRADE_FLOOR: u8 = 2;

/// Low's declared budget, in ticks. Large enough that the overrun cannot land
/// before high has contended (which would be a different scenario that
/// happened to pass — clause 1 asserts it did not), small enough that it is
/// crossed early in the run.
const LOW_BUDGET: u32 = 4;
/// A budget no task in this fixture can plausibly reach.
const GENEROUS_BUDGET: u32 = 1_000_000;

const STACK_SIZE: usize = 8_192;
/// Matching `fixture_preempt`'s own empirically-chosen local-APIC reload.
const INITIAL_COUNT: u32 = 500_000;
const JOURNAL: usize = 64;
const RUN_LOG_CAPACITY: usize = 32;
const MAX_ROUNDS: u32 = 128;

/// How much work low does *after* the degrade before it releases the lock.
///
/// Anchored to the degrade rather than to a fixed total, so the window in
/// which medium could have stolen the CPU spans several real ticks no matter
/// how many iterations one tick happens to be worth on the host running QEMU.
/// A fixed iteration count would make the interesting half of this fixture a
/// property of the emulator's speed.
const WORK_AFTER_DEGRADE: u64 = 2_000_000;
/// How far medium must get *after* the release, to prove it was a genuine
/// competitor and not an inert task whose frozen counter meant nothing.
const MEDIUM_MIN_RUNS: u64 = 1_000;
/// Ceilings, defence in depth only — a passing run never approaches them.
const LOOP_CEILING: u64 = 4_000_000_000;
/// How many ticks medium may be given *while low still holds the contended
/// lock* before the run is declared stalled and ended.
///
/// **This bound is not decoration, and the falsification is what put it here.**
/// With the composition reverted, the degrade discards the boost, low drops to
/// 2, and medium at 15 takes the CPU and never gives it back — low can never
/// reach its own unlock, so high stays `Blocked` forever and medium's own
/// termination condition (`HIGH_COMPLETED`) can never become true. That is a
/// real deadlock, and it is *precisely* the priority inversion `G-RT-1`
/// denies. Without this bound the defect presents as a harness timeout with an
/// **empty capture** — the `LE-46` shape, where the instrument fires and
/// leaves no trace. With it, the run ends and names what happened.
///
/// The quantity counted is the inversion itself rather than elapsed time,
/// which is what makes the bound both fast and unambiguous: in a passing run
/// it is **exactly zero**, because medium is never selected before the
/// release. A wall-clock or total-tick bound would have to exceed a legitimate
/// window of some hundreds of ticks, and would therefore be slow to fire and
/// sensitive to how fast the host runs QEMU. This is neither.
const INVERSION_BOUND_TICKS: u32 = 20;

static mut SCHEDULER: Scheduler<TASKS> = Scheduler::new();
static mut JOURNAL_STORE: SpoorJournal<JOURNAL> = SpoorJournal::new();
static mut LOCK: PriorityInheritingLock = PriorityInheritingLock::new();

static mut DISPATCHER_CTX: Context = Context::zeroed();
/// Where the hook saves the registers of a task it is abandoning to end a
/// stalled run. Written and never read — `context::switch` needs a
/// destination, and a context nothing will ever resume is the honest one. The
/// same pattern `fixture_wcet` and `fixture_fault` already established.
static mut ABANDONED_CTX: Context = Context::zeroed();
static mut TASK_CTX: [Context; TASKS] = [Context::zeroed(); TASKS];
static mut TASK_STACKS: [[u8; STACK_SIZE]; TASKS] = [[0; STACK_SIZE]; TASKS];

static mut CURRENT_TASK: Option<usize> = None;
static mut TASK_IDS: [Option<TaskId>; TASKS] = [None; TASKS];

/// Which slot the dispatcher selected, in order. Both behavioural claims are
/// about this sequence, so it is recorded rather than inferred.
static mut RUN_LOG: [u8; RUN_LOG_CAPACITY] = [0; RUN_LOG_CAPACITY];
static mut RUN_LOG_LEN: usize = 0;
/// How many entries were in the log when low released the lock, so "during
/// the window" and "after it" are separable.
static mut RUN_LOG_AT_RELEASE: usize = 0;

static mut LOW_ACQUIRED: bool = false;
static mut LOW_PREEMPTED: bool = false;
static mut LOW_RELEASED: bool = false;
static mut LOW_WORK_DONE: u64 = 0;
/// Low's phase-2 counter: work done *after* the release, at its floor. Its
/// only job is to prove low stayed runnable rather than retiring, so that
/// "low is never selected again" means it was outranked rather than finished.
static mut LOW_PHASE2_DONE: u64 = 0;
static mut LOW_EXHAUSTED: bool = false;
/// Low's effective priority immediately after the unlock — clause 4's
/// quantity. Must be the floor, not the pre-boost 5.
static mut LOW_EFFECTIVE_AFTER_RELEASE: Option<u8> = None;
static mut LOW_INHERITED_AFTER_RELEASE: Option<Option<u8>> = None;
/// Clause 4's "outranked, not retired", captured at the release itself rather
/// than inferred from a counter afterwards. See the release critical section.
static mut LOW_STATE_AFTER_RELEASE: Option<TaskState> = None;
static mut MEDIUM_OUTRANKS_LOW_AFTER_RELEASE: bool = false;

/// Set by the tick hook the moment enforcement fires, and polled by low so
/// its phase-1 work is anchored to the degrade. See [`WORK_AFTER_DEGRADE`].
static mut DEGRADED: bool = false;
static mut LOW_WORK_AT_DEGRADE: u64 = 0;
/// Clause 2: both priorities, read from interrupt context on the very tick
/// the enforcement was applied.
static mut BASE_AT_DEGRADE: Option<u8> = None;
static mut EFFECTIVE_AT_DEGRADE: Option<u8> = None;
/// Clause 1: the degrade must land *after* the boost, or the composition was
/// never exercised.
static mut CONTENDED_AT_DEGRADE: bool = false;
static mut ENFORCEMENTS: u32 = 0;
static mut ENFORCED_WRONG_TASK: bool = false;
static mut WRONG_DISPOSITION: bool = false;
static mut TICKS_UNKNOWN: u32 = 0;

static mut HIGH_CONTENDED: bool = false;
static mut HIGH_SAW_BOOST: Option<u8> = None;
static mut HIGH_COMPLETED: bool = false;
/// The `COST` of the `Lock`/`Boost` spoor, sampled **at the moment it was
/// stamped** rather than read out of the journal at the end.
///
/// [`SpoorJournal`] is a fixed-capacity ring that overwrites its oldest entry,
/// and a degraded task that keeps running keeps overrunning — so this run
/// legitimately stamps tens of `Wcet` events after the single `Boost`, and
/// whether the boost is still resident at the end is a property of how many
/// ticks the host gave the window. **Asserting that the oldest entry survived
/// would be asserting about the emulator's speed**, which is exactly the class
/// of fixture that goes intermittently red for no defect. The audit claim is
/// about what was stamped, so it is sampled where it was stamped.
static mut HIGH_BOOST_SPOOR_COST: Option<u32> = None;

static mut MEDIUM_COUNTER: u64 = 0;
static mut MEDIUM_AT_BLOCK: u64 = 0;
static mut MEDIUM_AT_DEGRADE: u64 = 0;
static mut MEDIUM_AT_RELEASE: u64 = 0;
static mut MEDIUM_EXHAUSTED: bool = false;
/// Medium's `Ready`-ness at each of the window's three points. A frozen
/// counter for a task that was never runnable would prove nothing at all,
/// which is exactly how this fixture could have been written to pass for the
/// wrong reason.
static mut MEDIUM_READY_AT_BLOCK: bool = false;
static mut MEDIUM_READY_AT_DEGRADE: bool = false;
static mut MEDIUM_READY_AT_RELEASE: bool = false;

static mut PREEMPTIONS: u32 = 0;
static mut DONE: bool = false;
/// Ticks charged to medium while low still held the contended lock and high
/// was still blocked on it — the priority inversion, counted. **Zero in a
/// passing run**, and the flag set if it exceeds [`INVERSION_BOUND_TICKS`].
static mut MEDIUM_TICKS_IN_WINDOW: u32 = 0;
static mut STALLED: bool = false;

fn task_id(scheduler: &Scheduler<TASKS>, slot: usize) -> Option<TaskId> {
    scheduler.iter_tasks().map(|(task, _)| task).find(|task| task.index() == slot)
}

fn priority(value: u8) -> Option<Priority> {
    Priority::try_new(value).ok()
}

/// Spelled once rather than inlined three times, so all three of clause 3's
/// readiness samples are literally the same check taken at three moments.
fn is_ready(scheduler: &Scheduler<TASKS>, slot: usize) -> bool {
    // SAFETY: `TASK_IDS` is written once during setup, before interrupts are
    // armed, and only read afterwards.
    let task = unsafe { TASK_IDS[slot] };
    task.and_then(|task| scheduler.state_of(task)) == Some(TaskState::Ready)
}

/// The lock holder and the offender, in one task. Takes the lock, releases
/// the competitors, works while holding it until well past its own degrade,
/// unlocks, and then keeps working at its floor.
///
/// **No yield of any kind** in either work loop: whatever takes this task off
/// the CPU is the timer.
extern "C" fn low_task() -> ! {
    // SAFETY: single-CPU fixture. Every scheduler touch is inside
    // `without_interrupts`, so the tick hook cannot observe a half-applied
    // mutation; the closure is bounded and takes no lock of its own.
    unsafe {
        let acquired = interrupts::without_interrupts(|| {
            let scheduler = &mut *(&raw mut SCHEDULER);
            let journal = &mut *(&raw mut JOURNAL_STORE);
            let lock = &mut *(&raw mut LOCK);
            let (Some(low), Some(p)) = (TASK_IDS[LOW_SLOT], priority(LOW_PRIORITY)) else {
                return false;
            };
            if lock.try_lock(scheduler, journal, low, p).is_err() {
                return false;
            }
            // **Recorded inside the critical section, deliberately.** The very
            // next statement makes a priority-25 task `Ready`, so from the
            // moment interrupts come back on this task can be preempted
            // between any two instructions — and if it is never selected
            // again, anything written after the closure returns is simply
            // lost. Evidence the checker reads is written where it cannot be.
            LOW_ACQUIRED = true;
            // Release the competitors only *after* the lock is held —
            // otherwise high could contend before there is a holder to boost,
            // and the scenario would never form.
            for slot in [MEDIUM_SLOT, HIGH_SLOT] {
                if let Some(task) = TASK_IDS[slot] {
                    scheduler.set_state(task, TaskState::Ready);
                }
            }
            true
        });
        if !acquired {
            DONE = true;
            context::switch(&raw mut TASK_CTX[LOW_SLOT], &raw mut DISPATCHER_CTX);
            unreachable!("the fixture fails before this task runs again")
        }
    }

    // Phase 1: work while holding the lock, past the degrade and well past it.
    loop {
        // SAFETY: only this task writes these two statics; `DEGRADED` and
        // `LOW_WORK_AT_DEGRADE` are written only by the tick hook, on this
        // same core, with this task suspended in the ISR.
        //
        // Read through `read_volatile` because the compiler cannot see the
        // interrupt handler that writes them and would otherwise be entitled
        // to hoist the load out of this loop entirely.
        unsafe {
            LOW_WORK_DONE += 1;
            if LOW_WORK_DONE >= LOOP_CEILING {
                LOW_EXHAUSTED = true;
                break;
            }
            if core::ptr::read_volatile(&raw const DEGRADED) {
                let at = core::ptr::read_volatile(&raw const LOW_WORK_AT_DEGRADE);
                if LOW_WORK_DONE >= at + WORK_AFTER_DEGRADE {
                    break;
                }
            }
        }
    }

    // Release. The window closes here.
    //
    // SAFETY: as above — one bounded, interrupt-free critical section.
    unsafe {
        let released = interrupts::without_interrupts(|| {
            let scheduler = &mut *(&raw mut SCHEDULER);
            let journal = &mut *(&raw mut JOURNAL_STORE);
            let lock = &mut *(&raw mut LOCK);
            let Some(low) = TASK_IDS[LOW_SLOT] else { return false };
            // Read before the unlock, so the window's closing sample is taken
            // while it is still open.
            MEDIUM_AT_RELEASE = MEDIUM_COUNTER;
            MEDIUM_READY_AT_RELEASE = is_ready(scheduler, MEDIUM_SLOT);
            RUN_LOG_AT_RELEASE = RUN_LOG_LEN;
            if lock.unlock(scheduler, journal, low).is_err() {
                return false;
            }
            // All of clause 4's evidence is captured here, inside the critical
            // section, for the same reason `LOW_ACQUIRED` is: the unlock has
            // just dropped this task to 2 and is about to make a task at 25
            // `Ready`, so it becomes preemptible the instant interrupts return
            // and may never be selected again. Writing `LOW_RELEASED` after the
            // closure made this fixture fail three runs in twelve — with the
            // kernel behaving perfectly and the *fixture* losing its own
            // record of it.
            LOW_RELEASED = true;
            LOW_EFFECTIVE_AFTER_RELEASE = scheduler.live_priority_of(low).map(Priority::value);
            LOW_INHERITED_AFTER_RELEASE =
                scheduler.inherited_priority_of(low).map(|p| p.map(Priority::value));
            // "Outranked, not retired", as two facts that do not depend on
            // this task ever running again: it is still runnable, and medium
            // now sits above it. A phase-2 counter cannot carry this claim —
            // whether low gets a single further iteration before the next tick
            // is a race, and asserting on it would be asserting about timing.
            LOW_STATE_AFTER_RELEASE = scheduler.state_of(low);
            MEDIUM_OUTRANKS_LOW_AFTER_RELEASE = match (
                TASK_IDS[MEDIUM_SLOT].and_then(|task| scheduler.live_priority_of(task)),
                scheduler.live_priority_of(low),
            ) {
                (Some(medium), Some(low)) => medium > low,
                _ => false,
            };
            // The waiter this task was boosted for is runnable again.
            if let Some(high) = TASK_IDS[HIGH_SLOT] {
                scheduler.set_state(high, TaskState::Ready);
            }
            true
        });
        if !released {
            DONE = true;
            context::switch(&raw mut TASK_CTX[LOW_SLOT], &raw mut DISPATCHER_CTX);
            unreachable!("the fixture fails before this task runs again")
        }
    }

    // Phase 2: still `Ready`, still busy, now at its floor. This task does not
    // retire, which is what makes "its slot never appears in the log again"
    // mean *outranked* rather than *finished*. It is medium that ends the run.
    loop {
        // SAFETY: only this task writes these statics.
        unsafe {
            LOW_PHASE2_DONE += 1;
            if LOW_PHASE2_DONE >= LOOP_CEILING {
                LOW_EXHAUSTED = true;
                DONE = true;
                context::switch(&raw mut TASK_CTX[LOW_SLOT], &raw mut DISPATCHER_CTX);
            }
        }
    }
}

/// The uninvolved competitor: busy-increments a counter whenever it runs. It
/// touches neither the lock nor any other task until the evidence is complete.
extern "C" fn medium_task() -> ! {
    loop {
        // SAFETY: only this task writes `MEDIUM_COUNTER`/`MEDIUM_EXHAUSTED`;
        // `HIGH_COMPLETED` is written by high, on this same core.
        let finished = unsafe {
            MEDIUM_COUNTER += 1;
            if MEDIUM_COUNTER >= LOOP_CEILING {
                MEDIUM_EXHAUSTED = true;
                true
            } else {
                core::ptr::read_volatile(&raw const HIGH_COMPLETED)
                    && MEDIUM_COUNTER >= MEDIUM_MIN_RUNS
            }
        };
        if finished {
            // The evidence is complete: end the run. Retiring low too is a
            // *fixture* decision about when to stop, not a policy one — a
            // degraded task is left runnable forever by design.
            //
            // SAFETY: bounded, interrupt-free critical section, as elsewhere.
            unsafe {
                DONE = true;
                interrupts::without_interrupts(|| {
                    let scheduler = &mut *(&raw mut SCHEDULER);
                    for slot in [MEDIUM_SLOT, LOW_SLOT, HIGH_SLOT] {
                        if let Some(task) = TASK_IDS[slot] {
                            scheduler.set_state(task, TaskState::Finished);
                        }
                    }
                });
                context::switch(&raw mut TASK_CTX[MEDIUM_SLOT], &raw mut DISPATCHER_CTX);
            }
            unreachable!("a Finished task is never selected again")
        }
    }
}

/// The high-priority waiter: contends for the lock low holds, blocks, and
/// resumes once low releases it.
extern "C" fn high_task() -> ! {
    // First run: contend, observe the boost, and block. The window opens here.
    //
    // SAFETY: single-CPU fixture; one bounded, interrupt-free critical section
    // over the scheduler, lock and journal.
    unsafe {
        interrupts::without_interrupts(|| {
            let scheduler = &mut *(&raw mut SCHEDULER);
            let journal = &mut *(&raw mut JOURNAL_STORE);
            let lock = &mut *(&raw mut LOCK);
            let (Some(high), Some(low), Some(p)) =
                (TASK_IDS[HIGH_SLOT], TASK_IDS[LOW_SLOT], priority(HIGH_PRIORITY))
            else {
                return;
            };
            // Contention is what boosts the holder. `AlreadyLocked` is the
            // expected, correct answer — this lock reports contention rather
            // than parking the caller itself.
            HIGH_CONTENDED =
                lock.try_lock(scheduler, journal, high, p) == Err(LockError::AlreadyLocked);
            HIGH_SAW_BOOST = scheduler.live_priority_of(low).map(Priority::value);
            // Clause 5's boost half, sampled here rather than at the end —
            // see `HIGH_BOOST_SPOOR_COST` for why the journal cannot be
            // trusted to still hold its oldest entry by then.
            HIGH_BOOST_SPOOR_COST =
                last(journal, Category::Lock, Action::Boost).map(|spoor| spoor.cost());
            MEDIUM_READY_AT_BLOCK = is_ready(scheduler, MEDIUM_SLOT);
            MEDIUM_AT_BLOCK = MEDIUM_COUNTER;
            scheduler.set_state(high, TaskState::Blocked);
        });
        context::switch(&raw mut TASK_CTX[HIGH_SLOT], &raw mut DISPATCHER_CTX);
    }

    // Second run: low released the lock and made this task Ready again.
    //
    // SAFETY: as above.
    unsafe {
        core::ptr::write_volatile(&raw mut HIGH_COMPLETED, true);
        interrupts::without_interrupts(|| {
            if let Some(high) = TASK_IDS[HIGH_SLOT] {
                (*(&raw mut SCHEDULER)).set_state(high, TaskState::Finished);
            }
        });
        context::switch(&raw mut TASK_CTX[HIGH_SLOT], &raw mut DISPATCHER_CTX);
    }
    unreachable!("a Finished task is never selected again")
}

/// The timer-tick consumer. Runs in interrupt context on the interrupted
/// task's own stack with `IF` clear. Bounded and allocation-free throughout.
///
/// It does exactly two things: drive the real enforcement path
/// ([`wcet::account_tick`]) and take the preemption decision. The degrade
/// disposition needs nothing further from the caller — the priority is
/// already lowered — and, unlike `fixture_wcet`'s degrade arm, this hook
/// deliberately does **not** leave the CPU afterwards: low still holds the
/// lock and is still boosted, so it must keep running, and that it does is
/// half of what this fixture proves.
extern "C" fn on_tick() {
    // Checked *first*, before the scheduler is touched at all: the dispatcher
    // legitimately holds a `&mut Scheduler` and a tick already in flight when
    // it cleared `IF` can still land here.
    //
    // SAFETY: written only by the dispatcher, with interrupts disabled.
    let Some(slot) = (unsafe { CURRENT_TASK }) else {
        return;
    };

    // SAFETY: a task is running, so the dispatcher holds no borrow; this is
    // the only code touching the scheduler for the duration, and the `&mut` is
    // formed and dropped inside this block, before any switch is taken.
    unsafe {
        let scheduler = &mut *(&raw mut SCHEDULER);
        let journal = &mut *(&raw mut JOURNAL_STORE);
        let running = task_id(scheduler, slot);

        // The whole enforcement path in one call: attribute the tick, charge
        // it, and apply whatever the task declared if it crossed its budget.
        match wcet::account_tick(scheduler, journal, running) {
            TickAccounting::UnknownTask => TICKS_UNKNOWN += 1,
            TickAccounting::Unattributed | TickAccounting::WithinBudget(_) => {}
            TickAccounting::Enforced { task, disposition } => {
                ENFORCEMENTS += 1;
                if task.index() != LOW_SLOT {
                    ENFORCED_WRONG_TASK = true;
                }
                // Checked against what the task *declared*, not against
                // whatever came back — otherwise this would assert only that
                // the enumeration round-trips.
                let as_declared = match disposition {
                    OverrunDisposition::DegradeTo(floor) => Some(floor) == priority(DEGRADE_FLOOR),
                    _ => false,
                };
                if !as_declared {
                    WRONG_DISPOSITION = true;
                }
                if ENFORCEMENTS == 1 {
                    // Clause 2, both halves, at the same instant and from
                    // inside the ISR that applied the decision.
                    BASE_AT_DEGRADE = scheduler.base_priority_of(task).map(Priority::value);
                    EFFECTIVE_AT_DEGRADE = scheduler.live_priority_of(task).map(Priority::value);
                    // Clause 3's middle sample.
                    MEDIUM_AT_DEGRADE = MEDIUM_COUNTER;
                    MEDIUM_READY_AT_DEGRADE = is_ready(scheduler, MEDIUM_SLOT);
                    // Clause 1: the boost was already in place.
                    CONTENDED_AT_DEGRADE = HIGH_CONTENDED;
                    core::ptr::write_volatile(&raw mut LOW_WORK_AT_DEGRADE, LOW_WORK_DONE);
                    core::ptr::write_volatile(&raw mut DEGRADED, true);
                }
            }
        }
    }

    // The stall bound. A run that cannot progress from the degrade to the
    // release is ended here, with every task retired, so the dispatcher loop
    // drains and `run` reports a named failure — rather than spinning until
    // the harness times it out and leaves nothing to read. See
    // `INVERSION_BOUND_TICKS`: this is the path the deliberate falsification
    // actually takes.
    //
    // SAFETY: a task is running, so the dispatcher holds no borrow; the
    // accounting borrow above was dropped. `ABANDONED_CTX` is never resumed.
    unsafe {
        if slot == MEDIUM_SLOT && HIGH_CONTENDED && !LOW_RELEASED && !STALLED {
            MEDIUM_TICKS_IN_WINDOW += 1;
            if MEDIUM_TICKS_IN_WINDOW > INVERSION_BOUND_TICKS {
                STALLED = true;
                DONE = true;
                let scheduler = &mut *(&raw mut SCHEDULER);
                for slot in [LOW_SLOT, MEDIUM_SLOT, HIGH_SLOT] {
                    if let Some(task) = TASK_IDS[slot] {
                        scheduler.set_state(task, TaskState::Finished);
                    }
                }
                CURRENT_TASK = None;
                context::switch(&raw mut ABANDONED_CTX, &raw mut DISPATCHER_CTX);
                unreachable!("an abandoned task is never switched back into")
            }
        }
    }

    // The preemption decision, unchanged from `STORY-P1-04-01`. Enforcement
    // and preemption share this hook, and neither may perturb the other.
    //
    // SAFETY: `slot` is the task this interrupt is executing on, so its
    // context slot is its own; `DISPATCHER_CTX` is suspended at the
    // dispatcher's own `run_once` call site. The borrow above was dropped.
    let outcome = unsafe {
        let running = task_id(&*(&raw const SCHEDULER), slot);
        preempt::on_timer_tick(
            &raw mut SCHEDULER,
            running,
            &raw mut TASK_CTX[slot],
            &raw mut DISPATCHER_CTX,
        )
    };
    if matches!(outcome, TickOutcome::Preempt(_)) {
        // SAFETY: reached only once the preempted task is resumed.
        unsafe {
            PREEMPTIONS += 1;
            if slot == LOW_SLOT {
                LOW_PREEMPTED = true;
            }
        }
    }
}

/// # Safety
/// `slot` must be the next unused scheduler slot, with an unused stack.
unsafe fn create(
    slot: usize,
    priority_value: u8,
    budget: u32,
    policy: OverrunPolicy,
    entry: extern "C" fn() -> !,
) -> Option<TaskId> {
    // SAFETY: per this function's own contract.
    unsafe {
        let scheduler = &mut *(&raw mut SCHEDULER);
        let task = scheduler
            .create_task(priority(priority_value)?, WcetBudgetTicks(budget), policy, entry)
            .ok()?;
        if task.index() != slot {
            return None;
        }
        let stack =
            core::slice::from_raw_parts_mut((&raw mut TASK_STACKS[slot]).cast::<u8>(), STACK_SIZE);
        TASK_CTX[slot] = Context::new(stack, entry).ok()?;
        TASK_IDS[slot] = Some(task);
        Some(task)
    }
}

/// Records a failed clause by name rather than folding it into a bare `false`.
///
/// `TEST-P1-04-05-A` clause 6 asks for this specifically: a fixture whose only
/// output on failure is a non-zero exit is the `LE-46` shape — armed to detect
/// and not to explain.
fn check(serial: &mut SerialPort, ok: &mut bool, clause: &str, condition: bool) {
    if !condition {
        let _ = writeln!(serial, "fixture-{NAME}: FAILED {clause}");
        *ok = false;
    }
}

/// Counts spoors carrying `category`/`action`.
fn count(journal: &SpoorJournal<JOURNAL>, category: Category, action: Action) -> usize {
    journal.iter().filter(|spoor| spoor.category() == category && spoor.action() == action).count()
}

/// The last spoor carrying `category`/`action`, if any.
fn last(journal: &SpoorJournal<JOURNAL>, category: Category, action: Action) -> Option<Spoor> {
    journal.iter().filter(|spoor| spoor.category() == category && spoor.action() == action).last()
}

/// Runs the fixture.
pub fn run() -> bool {
    // SAFETY: this fixture is the only code running; `init` is called once,
    // before any other `SerialPort` method.
    let mut serial = unsafe { SerialPort::init() };
    let mut ok = true;

    let Some(floor) = priority(DEGRADE_FLOOR) else {
        let _ = writeln!(serial, "fixture-{NAME}: could not build the declared floor");
        return false;
    };

    // SAFETY: single-CPU fixture, each slot used exactly once.
    unsafe {
        let created =
            create(LOW_SLOT, LOW_PRIORITY, LOW_BUDGET, OverrunPolicy::Degrade(floor), low_task)
                .is_some()
                && create(
                    MEDIUM_SLOT,
                    MEDIUM_PRIORITY,
                    GENEROUS_BUDGET,
                    OverrunPolicy::TripToSafeState,
                    medium_task,
                )
                .is_some()
                && create(
                    HIGH_SLOT,
                    HIGH_PRIORITY,
                    GENEROUS_BUDGET,
                    OverrunPolicy::TripToSafeState,
                    high_task,
                )
                .is_some();
        if !created {
            let _ = writeln!(serial, "fixture-{NAME}: task creation failed");
            return false;
        }
        // Only low starts runnable: the holder must exist before anything can
        // contend for what it holds.
        let scheduler = &mut *(&raw mut SCHEDULER);
        for slot in [MEDIUM_SLOT, HIGH_SLOT] {
            if let Some(task) = TASK_IDS[slot] {
                scheduler.set_state(task, TaskState::Blocked);
            }
        }
    }

    // SAFETY: registered before interrupts are armed, so no tick can arrive
    // between arming and installation. `on_tick` is bounded, allocation-free,
    // and leaves the interrupt frame intact.
    unsafe { interrupts::set_tick_hook(on_tick) };
    // SAFETY: called exactly once, before anything depends on interrupts being
    // armed. Ends with `sti`.
    unsafe { interrupts::init(INITIAL_COUNT) };
    // From here the dispatcher runs with `IF` clear and never re-enables it: a
    // task's own saved `RFLAGS` is what turns interrupts back on across the
    // switch into it. That, not a convention, is what stops the hook ever
    // observing a scheduler this loop is mid-mutation of.
    //
    // SAFETY: every re-enable happens via a context switch's own `popfq`.
    let _ = unsafe { interrupts::disable_interrupts() };

    let mut rounds: u32 = 0;
    loop {
        // SAFETY: interrupts masked, so this is the only code touching the
        // scheduler; each `TASK_CTX` slot is owned by one task.
        let ran = unsafe {
            let scheduler = &mut *(&raw mut SCHEDULER);
            let Some(next) = scheduler.highest_priority_ready() else {
                break;
            };
            if RUN_LOG_LEN < RUN_LOG_CAPACITY {
                RUN_LOG[RUN_LOG_LEN] = next.index() as u8;
                RUN_LOG_LEN += 1;
            }
            CURRENT_TASK = Some(next.index());
            let ran = dispatch::run_once(scheduler, &raw mut DISPATCHER_CTX, &raw mut TASK_CTX);
            CURRENT_TASK = None;
            check(
                &mut serial,
                &mut ok,
                "the dispatcher ran the task it selected",
                ran == Some(next),
            );
            ran
        };
        if ran.is_none() {
            break;
        }
        rounds += 1;
        if rounds > MAX_ROUNDS {
            let _ = writeln!(serial, "fixture-{NAME}: exceeded {MAX_ROUNDS} dispatcher rounds");
            ok = false;
            break;
        }
    }

    // SAFETY: read after every switch has returned, with interrupts masked;
    // nothing else can be running.
    let (
        log_len,
        log_at_release,
        acquired,
        preempted,
        preemptions,
        released,
        low_exhausted,
        phase2,
        effective_after,
        inherited_after,
        contended,
        boost,
        completed,
        enforcements,
        wrong_task,
        wrong_disposition,
        unknown,
        contended_at_degrade,
        base_at_degrade,
        effective_at_degrade,
        medium_total,
        at_block,
        at_degrade,
        at_release,
        ready_at_block,
        ready_at_degrade,
        ready_at_release,
        medium_exhausted,
    ) = unsafe {
        (
            RUN_LOG_LEN,
            RUN_LOG_AT_RELEASE,
            LOW_ACQUIRED,
            LOW_PREEMPTED,
            PREEMPTIONS,
            LOW_RELEASED,
            LOW_EXHAUSTED,
            LOW_PHASE2_DONE,
            LOW_EFFECTIVE_AFTER_RELEASE,
            LOW_INHERITED_AFTER_RELEASE,
            HIGH_CONTENDED,
            HIGH_SAW_BOOST,
            HIGH_COMPLETED,
            ENFORCEMENTS,
            ENFORCED_WRONG_TASK,
            WRONG_DISPOSITION,
            TICKS_UNKNOWN,
            CONTENDED_AT_DEGRADE,
            BASE_AT_DEGRADE,
            EFFECTIVE_AT_DEGRADE,
            MEDIUM_COUNTER,
            MEDIUM_AT_BLOCK,
            MEDIUM_AT_DEGRADE,
            MEDIUM_AT_RELEASE,
            MEDIUM_READY_AT_BLOCK,
            MEDIUM_READY_AT_DEGRADE,
            MEDIUM_READY_AT_RELEASE,
            MEDIUM_EXHAUSTED,
        )
    };
    // Copied out rather than borrowed: taking a reference to a `static mut` is
    // what `static_mut_refs` exists to prevent, and the log is 32 bytes.
    let mut log_copy = [0u8; RUN_LOG_CAPACITY];
    // SAFETY: read after every switch has returned, with interrupts masked;
    // both sides are `RUN_LOG_CAPACITY` bytes and do not overlap.
    unsafe {
        core::ptr::copy_nonoverlapping(
            (&raw const RUN_LOG).cast::<u8>(),
            log_copy.as_mut_ptr(),
            RUN_LOG_CAPACITY,
        );
    }
    let log = &log_copy[..log_len.min(RUN_LOG_CAPACITY)];
    let split = log_at_release.min(log.len());

    // SAFETY: interrupts masked, run over.
    let (stalled, medium_ticks_in_window, low_state_after, medium_outranks) = unsafe {
        (
            STALLED,
            MEDIUM_TICKS_IN_WINDOW,
            LOW_STATE_AFTER_RELEASE,
            MEDIUM_OUTRANKS_LOW_AFTER_RELEASE,
        )
    };
    check(
        &mut serial,
        &mut ok,
        "clause 3/7: medium was given CPU time while low still held the contended lock. This \
         IS the defect: with the boost discarded, low sits at its floor of 2, medium at 15 \
         never yields, and the holder can never reach its own unlock — the inversion G-RT-1 \
         denies, and the run had to be ended to report it",
        !stalled,
    );

    // Clause 1: the composed scenario formed at all, under real ticks.
    check(&mut serial, &mut ok, "clause 1: low acquired the lock", acquired);
    check(&mut serial, &mut ok, "clause 1: high contended for it", contended);
    check(
        &mut serial,
        &mut ok,
        "clause 1: the boost reached the waiter's priority",
        boost == Some(HIGH_PRIORITY),
    );
    check(
        &mut serial,
        &mut ok,
        "clause 1: low was preempted by a real tick",
        preempted && preemptions >= 1,
    );
    check(&mut serial, &mut ok, "clause 1: low was degraded", enforcements >= 1);
    check(&mut serial, &mut ok, "clause 1: every enforcement named low", !wrong_task);
    check(
        &mut serial,
        &mut ok,
        "clause 1: every disposition was the declared DegradeTo(2)",
        !wrong_disposition,
    );
    check(&mut serial, &mut ok, "clause 1: no tick was charged to an unknown task", unknown == 0);
    check(
        &mut serial,
        &mut ok,
        "clause 1: the degrade landed AFTER the boost, not before",
        contended_at_degrade,
    );
    check(&mut serial, &mut ok, "clause 1: low released the lock", released);
    check(&mut serial, &mut ok, "clause 1: high ran to completion", completed);
    check(
        &mut serial,
        &mut ok,
        "clause 7: no loop reached its ceiling",
        !low_exhausted && !medium_exhausted,
    );

    // Clause 2: the degrade landed and the boost survived it, both read from
    // inside the ISR on the tick the enforcement was applied.
    check(
        &mut serial,
        &mut ok,
        "clause 2: low's BASE priority became the declared floor 2",
        base_at_degrade == Some(DEGRADE_FLOOR),
    );
    check(
        &mut serial,
        &mut ok,
        "clause 2: low's EFFECTIVE priority was still 25 — the blocked waiter kept its boost \
         across the degrade (this is LE-22's boost-then-degrade half)",
        effective_at_degrade == Some(HIGH_PRIORITY),
    );

    // Clause 3: medium made no progress across the window, and was a genuine
    // competitor at every point in it.
    check(
        &mut serial,
        &mut ok,
        "clause 3: medium made no progress between high blocking and the degrade",
        at_block == at_degrade,
    );
    check(
        &mut serial,
        &mut ok,
        "clause 3: medium made no progress between the degrade and the release — the degrade \
         did NOT hand it the CPU",
        at_degrade == at_release,
    );
    check(&mut serial, &mut ok, "clause 3: medium was Ready when high blocked", ready_at_block);
    check(
        &mut serial,
        &mut ok,
        "clause 3: medium was Ready when the degrade fired",
        ready_at_degrade,
    );
    check(&mut serial, &mut ok, "clause 3: medium was Ready when low released", ready_at_release);
    check(
        &mut serial,
        &mut ok,
        "clause 3: medium's slot appears nowhere in the selection log before the release",
        !log[..split].iter().any(|slot| *slot as usize == MEDIUM_SLOT),
    );

    // Clause 4: low left the lock at its floor, and the dispatcher acted on it.
    check(
        &mut serial,
        &mut ok,
        "clause 4: low's effective priority after the unlock is the degraded floor 2, NOT the \
         pre-boost 5 (this is LE-22's degrade-then-unlock half)",
        effective_after == Some(DEGRADE_FLOOR),
    );
    check(
        &mut serial,
        &mut ok,
        "clause 4: nothing is left inherited after the release",
        inherited_after == Some(None),
    );
    check(
        &mut serial,
        &mut ok,
        "clause 4: low was still runnable at the release rather than retiring, so being \
         unselected afterwards means outranked",
        low_state_after == Some(TaskState::Running) || low_state_after == Some(TaskState::Ready),
    );
    check(
        &mut serial,
        &mut ok,
        "clause 4: medium at 15 outranks low the moment it lands on its floor of 2",
        medium_outranks,
    );
    check(
        &mut serial,
        &mut ok,
        "clause 4: low is never selected again after the release — at 2 it is outranked by \
         medium at 15",
        !log[split..].iter().any(|slot| *slot as usize == LOW_SLOT),
    );
    check(
        &mut serial,
        &mut ok,
        "clause 4: medium is selected after the release",
        log[split..].iter().any(|slot| *slot as usize == MEDIUM_SLOT),
    );
    check(
        &mut serial,
        &mut ok,
        "clause 4: medium made real progress afterwards, so its frozen counter meant something",
        medium_total >= MEDIUM_MIN_RUNS,
    );

    // Clause 5: the audit trail says what happened.
    //
    // SAFETY: interrupts masked, run over.
    let (boost_cost, degrades, restore) = unsafe {
        let journal = &*(&raw const JOURNAL_STORE);
        (
            HIGH_BOOST_SPOOR_COST,
            count(journal, Category::Wcet, Action::Degrade),
            last(journal, Category::Lock, Action::Restore),
        )
    };
    check(
        &mut serial,
        &mut ok,
        "clause 5: the boost was stamped, naming the waiter's priority",
        boost_cost == Some(HIGH_PRIORITY as u32),
    );
    check(&mut serial, &mut ok, "clause 5: the degrade was stamped", degrades >= 1);
    check(
        &mut serial,
        &mut ok,
        "clause 5: the restore spoor records the floor low landed on, not the 5 it left",
        restore.map(|spoor| spoor.cost()) == Some(DEGRADE_FLOOR as u32),
    );

    let _ = writeln!(
        serial,
        "fixture-{NAME}: acquired={acquired} contended={contended} boost={boost:?} \
         released={released} high_completed={completed}"
    );
    let _ = writeln!(
        serial,
        "fixture-{NAME}: enforcements={enforcements} contended_at_degrade={contended_at_degrade} \
         base_at_degrade={base_at_degrade:?} effective_at_degrade={effective_at_degrade:?} \
         wrong_task={wrong_task} wrong_disposition={wrong_disposition} unknown={unknown}"
    );
    let _ = writeln!(
        serial,
        "fixture-{NAME}: low effective_after_release={effective_after:?} \
         inherited_after_release={inherited_after:?} state_after_release={low_state_after:?} \
         medium_outranks={medium_outranks} phase2={phase2} preemptions={preemptions} \
         stalled={stalled} medium_ticks_in_window={medium_ticks_in_window} \
         (bound {INVERSION_BOUND_TICKS}, and 0 in a passing run)"
    );
    let _ = writeln!(
        serial,
        "fixture-{NAME}: medium at_block={at_block} at_degrade={at_degrade} \
         at_release={at_release} final={medium_total} (min {MEDIUM_MIN_RUNS}) \
         ready=[{ready_at_block},{ready_at_degrade},{ready_at_release}]"
    );
    let _ = writeln!(
        serial,
        "fixture-{NAME}: dispatch order={log:?} (0=low 1=medium 2=high), release_split={split}"
    );
    let _ = writeln!(
        serial,
        "fixture-{NAME}: spoors boost_cost={boost_cost:?} degrade={degrades} restore_cost={:?} \
         (the journal is a {JOURNAL}-entry ring; a run this long evicts its oldest entries)",
        restore.map(|spoor| spoor.cost())
    );

    let _ = write_result(&mut serial, NAME, ok);
    ok
}
