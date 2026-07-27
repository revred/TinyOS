//! `TEST-P1-04-01-A` clause 6: the classic three-task priority-inversion
//! scenario, run for real under timer-driven preemption
//! (`STORY-P1-04-01`).
//!
//! **What `STORY-P0-02-03` could not claim, and this fixture does.** That
//! Story built `kernel::lock::PriorityInheritingLock` and proved its
//! *bookkeeping*: a contended lock boosts its holder, an unlock restores it
//! exactly. Its own doc comment says plainly that the *behavioral* half —
//! that a real, running medium-priority task is actually kept from starving
//! the boosted holder — needed a dispatcher this kernel did not have. That
//! caveat closes here.
//!
//! The scenario:
//!
//! | Task | Priority | What it does |
//! |---|---|---|
//! | low | 5 | takes the lock, makes the other two Ready, then works while holding it |
//! | high | 25 | preempts low on a tick, fails the lock (boosting low to 25), blocks |
//! | medium | 15 | busy-increments a counter whenever it gets the CPU |
//!
//! **Why the counter is the evidence, and the run-log alone is not.**
//! Without inheritance, low at priority 5 loses every selection to medium at
//! 15: medium's counter climbs and high never runs at all. With it, low is
//! boosted above medium, finishes, unlocks, and high proceeds. So the test
//! asserts *both*: the dispatch order actually taken (`low → high → low →
//! high`) **and** that medium's counter did not move between high blocking
//! and high resuming — and, critically, that medium **was `Ready`
//! throughout that window and does run afterwards**. A frozen counter for a
//! task that was never runnable would prove nothing at all, which is exactly
//! the way this test could have been written to pass for the wrong reason.
//!
//! Every scheduler mutation made from task context here happens inside
//! `hal_x86_64::interrupts::without_interrupts` — clause 3's Tier 0 half.
//! Without it these tasks would be racing the tick hook that reads the same
//! scheduler.
//!
//! Only reachable when the `fixture-priority-inversion` feature is enabled —
//! never part of a real boot image.

#![allow(static_mut_refs, clippy::deref_addrof)]

use core::fmt::Write;
use hal_x86_64::interrupts;
use hal_x86_64::serial::SerialPort;
use kernel::context::{self, Context};
use kernel::dispatch;
use kernel::lock::{LockError, PriorityInheritingLock};
use kernel::measure::write_result;
use kernel::preempt;
use kernel::sched::{Priority, Scheduler, TaskId, TaskState, WcetBudgetTicks};
use kernel::spoor_journal::SpoorJournal;

const TASKS: usize = 3;
const LOW_SLOT: usize = 0;
const MEDIUM_SLOT: usize = 1;
const HIGH_SLOT: usize = 2;

const LOW_PRIORITY: u8 = 5;
const MEDIUM_PRIORITY: u8 = 15;
const HIGH_PRIORITY: u8 = 25;

const STACK_SIZE: usize = 8_192;
const INITIAL_COUNT: u32 = 500_000;
const JOURNAL: usize = 16;
const RUN_LOG_CAPACITY: usize = 16;
const MAX_ROUNDS: u32 = 64;

/// How much work low does while holding the lock. Long enough to span
/// several ticks, so the window in which medium could have stolen the CPU is
/// a real one rather than an instant.
const LOW_WORK_ITERATIONS: u64 = 40_000_000;
/// How far medium's counter must get *after* the window, to prove it was a
/// genuine competitor and not an inert task whose frozen counter meant
/// nothing.
const MEDIUM_MIN_RUNS: u64 = 1_000;
/// Ceilings, defence in depth only — a passing run never approaches them.
const LOW_CEILING: u64 = 4_000_000_000;
const MEDIUM_CEILING: u64 = 4_000_000_000;

static mut SCHEDULER: Scheduler<TASKS> = Scheduler::new();
static mut JOURNAL_STORE: SpoorJournal<JOURNAL> = SpoorJournal::new();
static mut LOCK: PriorityInheritingLock = PriorityInheritingLock::new();

static mut DISPATCHER_CTX: Context = Context::zeroed();
static mut TASK_CTX: [Context; TASKS] = [Context::zeroed(); TASKS];
static mut TASK_STACKS: [[u8; STACK_SIZE]; TASKS] = [[0; STACK_SIZE]; TASKS];

static mut CURRENT_TASK: Option<usize> = None;
static mut TASK_IDS: [Option<TaskId>; TASKS] = [None; TASKS];

/// Which slot the dispatcher selected, in order. The behavioural claim is
/// about this sequence, so it is recorded rather than inferred.
static mut RUN_LOG: [u8; RUN_LOG_CAPACITY] = [0; RUN_LOG_CAPACITY];
static mut RUN_LOG_LEN: usize = 0;

static mut MEDIUM_COUNTER: u64 = 0;
static mut MEDIUM_AT_BLOCK: u64 = 0;
static mut MEDIUM_AT_RESUME: u64 = 0;
static mut MEDIUM_READY_IN_WINDOW: bool = false;
static mut MEDIUM_EXHAUSTED: bool = false;

static mut LOW_ACQUIRED: bool = false;
static mut LOW_WORK_DONE: u64 = 0;
static mut LOW_PREEMPTED: bool = false;
static mut LOW_RELEASED: bool = false;
static mut LOW_EXHAUSTED: bool = false;

static mut HIGH_CONTENDED: bool = false;
static mut HIGH_SAW_BOOST: Option<u8> = None;
static mut LOW_PRIORITY_AFTER_RELEASE: Option<u8> = None;
static mut HIGH_COMPLETED: bool = false;
static mut PREEMPTIONS: u32 = 0;

fn task_id(scheduler: &Scheduler<TASKS>, slot: usize) -> Option<TaskId> {
    scheduler.iter_tasks().map(|(task, _)| task).find(|task| task.index() == slot)
}

fn priority(value: u8) -> Option<Priority> {
    Priority::try_new(value).ok()
}

/// The lock holder. Takes the lock, releases the other two tasks, works
/// while holding it, then unlocks and retires.
extern "C" fn low_task() -> ! {
    // SAFETY: single-CPU fixture. Every scheduler touch below is inside
    // `without_interrupts`, so the tick hook cannot observe a half-applied
    // mutation; the closures are bounded and take no lock of their own.
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
            // Release the competitors only *after* the lock is held —
            // otherwise high could contend before there is a holder to
            // boost, and the scenario would never form.
            for slot in [MEDIUM_SLOT, HIGH_SLOT] {
                if let Some(task) = TASK_IDS[slot] {
                    scheduler.set_state(task, TaskState::Ready);
                }
            }
            true
        });
        LOW_ACQUIRED = acquired;
        if !acquired {
            context::switch(&raw mut TASK_CTX[LOW_SLOT], &raw mut DISPATCHER_CTX);
            unreachable!("the fixture fails before this task runs again")
        }
    }

    // Work while holding the lock. No yield: whatever takes this task off
    // the CPU is the timer.
    loop {
        // SAFETY: only this task writes these statics.
        unsafe {
            LOW_WORK_DONE += 1;
            if LOW_WORK_DONE >= LOW_WORK_ITERATIONS || LOW_WORK_DONE >= LOW_CEILING {
                LOW_EXHAUSTED = LOW_WORK_DONE >= LOW_CEILING;
                break;
            }
        }
    }

    // SAFETY: as above — one bounded, interrupt-free critical section.
    unsafe {
        let released = interrupts::without_interrupts(|| {
            let scheduler = &mut *(&raw mut SCHEDULER);
            let journal = &mut *(&raw mut JOURNAL_STORE);
            let lock = &mut *(&raw mut LOCK);
            let Some(low) = TASK_IDS[LOW_SLOT] else { return false };
            if lock.unlock(scheduler, journal, low).is_err() {
                return false;
            }
            LOW_PRIORITY_AFTER_RELEASE = scheduler.live_priority_of(low).map(Priority::value);
            // The waiter this task was boosted for is now runnable again.
            if let Some(high) = TASK_IDS[HIGH_SLOT] {
                scheduler.set_state(high, TaskState::Ready);
            }
            scheduler.set_state(low, TaskState::Finished);
            true
        });
        LOW_RELEASED = released;
        context::switch(&raw mut TASK_CTX[LOW_SLOT], &raw mut DISPATCHER_CTX);
    }
    unreachable!("a Finished task is never selected again")
}

/// The uninvolved competitor: busy-increments a counter whenever it runs.
/// It touches neither the lock nor any other task.
extern "C" fn medium_task() -> ! {
    loop {
        // SAFETY: only this task writes `MEDIUM_COUNTER`/`MEDIUM_EXHAUSTED`.
        let finished = unsafe {
            MEDIUM_COUNTER += 1;
            if MEDIUM_COUNTER >= MEDIUM_CEILING {
                MEDIUM_EXHAUSTED = true;
                true
            } else {
                HIGH_COMPLETED && MEDIUM_COUNTER >= MEDIUM_MIN_RUNS
            }
        };
        if finished {
            // SAFETY: bounded, interrupt-free critical section, as elsewhere
            // in this fixture.
            unsafe {
                interrupts::without_interrupts(|| {
                    if let Some(task) = TASK_IDS[MEDIUM_SLOT] {
                        (*(&raw mut SCHEDULER)).set_state(task, TaskState::Finished);
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
    // First run: contend, observe the boost, and block.
    //
    // SAFETY: single-CPU fixture; one bounded, interrupt-free critical
    // section over the scheduler, lock and journal.
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
            // The window opens here: medium must be genuinely runnable, or a
            // frozen counter would prove nothing.
            MEDIUM_READY_IN_WINDOW = TASK_IDS[MEDIUM_SLOT]
                .and_then(|task| scheduler.state_of(task))
                == Some(TaskState::Ready);
            MEDIUM_AT_BLOCK = MEDIUM_COUNTER;
            scheduler.set_state(high, TaskState::Blocked);
        });
        context::switch(&raw mut TASK_CTX[HIGH_SLOT], &raw mut DISPATCHER_CTX);
    }

    // Second run: low released the lock and made this task Ready again. The
    // window closes here.
    //
    // SAFETY: as above.
    unsafe {
        MEDIUM_AT_RESUME = MEDIUM_COUNTER;
        HIGH_COMPLETED = true;
        interrupts::without_interrupts(|| {
            if let Some(high) = TASK_IDS[HIGH_SLOT] {
                (*(&raw mut SCHEDULER)).set_state(high, TaskState::Finished);
            }
        });
        context::switch(&raw mut TASK_CTX[HIGH_SLOT], &raw mut DISPATCHER_CTX);
    }
    unreachable!("a Finished task is never selected again")
}

/// The timer-tick consumer. Identical in shape to `fixture_preempt`'s: check
/// that a task is running *before* touching the scheduler, then take the one
/// decision.
extern "C" fn on_tick() {
    // SAFETY: written only by the dispatcher, with interrupts disabled.
    let Some(slot) = (unsafe { CURRENT_TASK }) else {
        return;
    };
    // SAFETY: `slot` is the task this interrupt is executing on, so its
    // context and extended-state slots are its own; `DISPATCHER_CTX` is
    // suspended at the dispatcher's own `run_once` call site.
    let outcome = unsafe {
        let running = task_id(&*(&raw const SCHEDULER), slot);
        preempt::on_timer_tick(
            &raw mut SCHEDULER,
            running,
            &raw mut TASK_CTX[slot],
            &raw mut DISPATCHER_CTX,
        )
    };
    if matches!(outcome, preempt::TickOutcome::Preempt(_)) {
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
unsafe fn create(slot: usize, priority_value: u8, entry: extern "C" fn() -> !) -> Option<TaskId> {
    // SAFETY: per this function's own contract.
    unsafe {
        let scheduler = &mut *(&raw mut SCHEDULER);
        let task = scheduler
            .create_task(priority(priority_value)?, WcetBudgetTicks(1_000_000), entry)
            .ok()?;
        if task.index() != slot {
            return None;
        }
        let stack =
            core::slice::from_raw_parts_mut((&raw mut TASK_STACKS[slot]).cast::<u8>(), STACK_SIZE);
        TASK_CTX[slot] = Context::new(stack, entry).ok()?;
        Some(task)
    }
}

/// Runs the fixture.
pub fn run() -> bool {
    // SAFETY: this fixture is the only code running.
    let mut serial = unsafe { SerialPort::init() };
    let mut ok = true;

    // SAFETY: single-CPU fixture, each slot used exactly once.
    unsafe {
        let Some(low) = create(LOW_SLOT, LOW_PRIORITY, low_task) else { return false };
        let Some(medium) = create(MEDIUM_SLOT, MEDIUM_PRIORITY, medium_task) else { return false };
        let Some(high) = create(HIGH_SLOT, HIGH_PRIORITY, high_task) else { return false };
        TASK_IDS = [Some(low), Some(medium), Some(high)];
        // Only low starts runnable: the holder must exist before anything
        // can contend for what it holds.
        let scheduler = &mut *(&raw mut SCHEDULER);
        scheduler.set_state(medium, TaskState::Blocked);
        scheduler.set_state(high, TaskState::Blocked);
    }

    // SAFETY: registered before interrupts are armed; `on_tick` is bounded,
    // allocation-free, and leaves the interrupt frame intact.
    unsafe { interrupts::set_tick_hook(on_tick) };
    // SAFETY: called exactly once, before anything depends on interrupts
    // being armed. Ends with `sti`.
    unsafe { interrupts::init(INITIAL_COUNT) };
    // The dispatcher runs with `IF` clear from here on; a task's own saved
    // `RFLAGS` re-enables interrupts across the switch into it.
    //
    // SAFETY: every re-enable happens via a context switch's `popfq`.
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
            ok &= ran == Some(next);
            ran
        };
        if ran.is_none() {
            break;
        }
        rounds += 1;
        if rounds > MAX_ROUNDS {
            let _ = writeln!(serial, "fixture-inversion: exceeded {MAX_ROUNDS} dispatcher rounds");
            ok = false;
            break;
        }
    }

    // SAFETY: read after every switch has returned, interrupts masked.
    let (
        log_len,
        acquired,
        preempted,
        contended,
        boost,
        released,
        after_release,
        completed,
        medium_total,
        at_block,
        at_resume,
        medium_ready,
        preemptions,
        low_exhausted,
        medium_exhausted,
    ) = unsafe {
        (
            RUN_LOG_LEN,
            LOW_ACQUIRED,
            LOW_PREEMPTED,
            HIGH_CONTENDED,
            HIGH_SAW_BOOST,
            LOW_RELEASED,
            LOW_PRIORITY_AFTER_RELEASE,
            HIGH_COMPLETED,
            MEDIUM_COUNTER,
            MEDIUM_AT_BLOCK,
            MEDIUM_AT_RESUME,
            MEDIUM_READY_IN_WINDOW,
            PREEMPTIONS,
            LOW_EXHAUSTED,
            MEDIUM_EXHAUSTED,
        )
    };
    // Copied out rather than borrowed: taking a reference to a `static mut`
    // is exactly what `static_mut_refs` exists to prevent, and the log is
    // sixteen bytes.
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
    let log = &log_copy[..log_len];

    // The scenario formed at all.
    ok &= acquired && contended && released && completed;
    ok &= !low_exhausted && !medium_exhausted;
    // Low left the CPU because of a tick, not because it yielded.
    ok &= preempted && preemptions >= 1;
    // Inheritance boosted the holder to the waiter's priority, and released
    // it again — the bookkeeping `STORY-P0-02-03` already proved, re-checked
    // here because the behavioural claim depends on it.
    ok &= boost == Some(HIGH_PRIORITY);
    ok &= after_release == Some(LOW_PRIORITY);
    // The behavioural claim: the dispatch order actually taken.
    ok &= log.len() >= 4;
    ok &= log.first().copied() == Some(LOW_SLOT as u8);
    ok &= log.get(1).copied() == Some(HIGH_SLOT as u8);
    ok &= log.get(2).copied() == Some(LOW_SLOT as u8);
    ok &= log.get(3).copied() == Some(HIGH_SLOT as u8);
    // ... and its counterpart: medium was a genuine competitor that made no
    // progress during the window, then plenty afterwards. Both halves are
    // required — a frozen counter for a task that was never Ready would
    // prove nothing at all.
    ok &= medium_ready;
    ok &= at_block == at_resume;
    ok &= medium_total >= MEDIUM_MIN_RUNS;

    let _ = writeln!(
        serial,
        "fixture-inversion: acquired={acquired} contended={contended} boost={boost:?} \
         released={released} priority_after_release={after_release:?} high_completed={completed}"
    );
    let _ = writeln!(
        serial,
        "fixture-inversion: dispatch order={log:?} (0=low 1=medium 2=high), preemptions={preemptions}, low_preempted={preempted}"
    );
    let _ = writeln!(
        serial,
        "fixture-inversion: medium ready_in_window={medium_ready} counter_at_block={at_block} \
         counter_at_resume={at_resume} counter_final={medium_total} (min {MEDIUM_MIN_RUNS})"
    );

    let _ = write_result(&mut serial, "priority-inversion", ok);
    ok
}
