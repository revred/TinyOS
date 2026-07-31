//! `TEST-P1-04-02-A` clauses 4–7: a task that exceeds its declared WCET
//! budget is caught by real time, and the consequence it declared in advance
//! actually happens (`STORY-P1-04-02`, closing `LE-02`).
//!
//! **One file, three fixtures.** `--fixture=wcet-restart`,
//! `--fixture=wcet-degrade` and `--fixture=wcet-trip` are the same scenario
//! with one constant changed: the [`OverrunPolicy`] the offending task
//! declares at creation. That is deliberate. Three separate files would let
//! the three arms drift — one fixture growing a workaround the others never
//! got — and the claim being made is precisely that the *same* enforcement
//! path produces three different, correct outcomes because the *declaration*
//! differed. Only one of the three features is ever enabled at a time.
//!
//! **The scenario.**
//!
//! | Task | Priority | Budget | Policy | What it does |
//! |---|---|---|---|---|
//! | offender | 20 | 4 ticks | mode-dependent | busy loop, no yield of any kind |
//! | innocent | 25 | generous | trip (never fires) | short bursts of work, then blocks itself |
//! | competitor | 15 | generous | trip (never fires) | busy loop, `Ready` throughout the degrade run |
//!
//! The competitor sits **strictly between** the offender's priority (20) and
//! its declared degrade floor (5), and is `Ready` from before the overrun
//! until after it. That is what makes the degrade arm falsifiable: it loses
//! every selection to the offender before the degrade and starts winning
//! them after. Without it, "degrade" would be indistinguishable from "reset
//! the budget and carry on", which is what that arm most easily rots into.
//!
//! **Why the innocent task is in all three runs.** Clause 6: enforcement must
//! not punish the innocent. Its evidence is not the absence of a failure — it
//! is an exact equality. The tick hook keeps its *own* per-slot count of the
//! ticks it attributed, entirely independently of the scheduler's books, and
//! the fixture asserts the two agree for every task. A tick charged to the
//! wrong task, or charged twice, or charged to whoever ran last, breaks that
//! equality in one direction or the other.
//!
//! **The `Nobody` arm at Tier 0.** A tick that lands in the dispatcher is
//! attributed to nobody, and here that rule is held by returning *before the
//! scheduler is touched at all* — which is also a soundness requirement,
//! since the dispatcher is the one context that legitimately holds a
//! `&mut Scheduler`. So the same rule does two jobs, and `wcet::attribute_tick`'s
//! own `Nobody` arm is pinned by a host test rather than here. The count is
//! reported but deliberately **not** asserted non-zero: the dispatcher runs
//! with `IF` clear, so whether any tick ever lands in it is a property of
//! QEMU's interrupt delivery, not of this kernel, and gating on it would be
//! gating on the emulator.
//!
//! **Standing on the stack you are about to abandon.** Detection happens on a
//! tick, so the disposition runs inside the ISR, on the offending task's own
//! stack. For `Restart` this fixture re-initializes a [`Context`] over a stack
//! that at that moment still holds a suspended interrupt frame and this
//! hook's own frames. That is sound **only** because nothing ever resumes
//! that frame: the hook switches into [`ABANDONED_CTX`] and the task is next
//! entered through the freshly built context, whose `RSP` is the top of the
//! same stack. It is the same argument `hal_x86_64::fault`'s stubs already
//! rely on, and the same `ABANDONED_CTX` pattern `fixture_fault` established
//! — stated here rather than left implied.
//!
//! Only reachable when one of the three `fixture-wcet-*` features is enabled
//! — never part of a real boot image.

#![allow(static_mut_refs, clippy::deref_addrof)]

use core::fmt::Write;
use hal_x86_64::interrupts;
use hal_x86_64::qemu_exit::{exit_qemu, QemuExitCode};
use hal_x86_64::serial::SerialPort;
use kernel::context::{self, Context};
use kernel::dispatch;
use kernel::measure::write_result;
use kernel::preempt::{self, TickOutcome};
use kernel::sched::{OverrunPolicy, Priority, Scheduler, TaskId, TaskState, WcetBudgetTicks};
use kernel::spoor::{Action, Category};
use kernel::spoor_journal::SpoorJournal;
use kernel::wcet::{self, OverrunDisposition, TickAccounting};

/// Which arm this build is proving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// `--fixture=wcet-restart`.
    Restart,
    /// `--fixture=wcet-degrade`.
    Degrade,
    /// `--fixture=wcet-trip`.
    Trip,
}

/// Selected by feature rather than declared three times, so exactly one arm
/// can be live in any build and the other two cannot be silently compiled in.
const MODE: Mode = if cfg!(feature = "fixture-wcet-restart") {
    Mode::Restart
} else if cfg!(feature = "fixture-wcet-degrade") {
    Mode::Degrade
} else {
    Mode::Trip
};

/// The fixture name reported in the `TOS64-RESULT/1` line.
const NAME: &str = match MODE {
    Mode::Restart => "wcet-restart",
    Mode::Degrade => "wcet-degrade",
    Mode::Trip => "wcet-trip",
};

const TASKS: usize = 3;
const OFFENDER_SLOT: usize = 0;
const INNOCENT_SLOT: usize = 1;
const COMPETITOR_SLOT: usize = 2;

const OFFENDER_PRIORITY: u8 = 20;
const INNOCENT_PRIORITY: u8 = 25;
/// Strictly between the offender's priority and its declared floor — the
/// whole point of this task's existence. See the module doc.
const COMPETITOR_PRIORITY: u8 = 15;
/// The floor the offender declares. Below the competitor, so a degrade
/// actually changes who wins a selection.
const DEGRADE_FLOOR: u8 = 5;

/// The offender's declared budget, in ticks. Small enough that it is crossed
/// early in the run, large enough that crossing it takes several real ticks
/// rather than being an artefact of the first one.
const OFFENDER_BUDGET: u32 = 4;
/// A budget no task in this fixture can plausibly reach.
const GENEROUS_BUDGET: u32 = 1_000_000;

/// `TEST-P1-04-02-A` clause 4's bound, fixed in that document before this
/// fixture existed: enforcement must land no later than the offender's
/// `OFFENDER_BUDGET + MAX_TICKS_TO_ENFORCE`-th attributed tick.
const MAX_TICKS_TO_ENFORCE: u32 = 1;

/// How many restarts the restart run demands before it retires the task. Two
/// rather than one because a restart that can only happen once is a special
/// case, not a policy.
const RESTART_TARGET: u32 = 2;

/// How far the competitor must get *after* the degrade to count as a genuine
/// competitor rather than an inert task whose zero counter meant nothing.
/// Large enough to span several ticks, so the innocent task is re-armed and
/// demonstrably keeps making progress *after* the enforcement rather than the
/// run ending the instant the degrade lands.
const COMPETITOR_TARGET: u64 = 20_000_000;

/// Iterations of work the innocent task does per burst before blocking
/// itself. Long enough to span at least one tick, short enough that it does
/// not monopolize the CPU.
const INNOCENT_BURST: u64 = 2_000_000;
/// How often (in ticks) the hook re-arms the innocent task.
const INNOCENT_PERIOD: u32 = 4;
/// How much progress the innocent task must make across the whole run.
const INNOCENT_MIN_PROGRESS: u64 = INNOCENT_BURST;

/// Per-task stack. The ISR runs on the interrupted task's own stack, which
/// must therefore also hold the interrupt frame, fifteen pushed registers,
/// the 512-byte `fxsave` area and this hook's own frames.
const STACK_SIZE: usize = 8_192;

/// Matching `fixture_preempt`'s own empirically-chosen local-APIC reload.
const INITIAL_COUNT: u32 = 500_000;

const JOURNAL: usize = 64;
const RUN_LOG_CAPACITY: usize = 64;
/// Bound on dispatcher rounds, so a scheduling defect ends the run instead of
/// spinning until the harness kills it.
const MAX_ROUNDS: u32 = 512;
/// Ceilings on the busy loops — defence in depth only; a passing run never
/// approaches them.
const LOOP_CEILING: u64 = 4_000_000_000;

static mut SCHEDULER: Scheduler<TASKS> = Scheduler::new();
static mut JOURNAL_STORE: SpoorJournal<JOURNAL> = SpoorJournal::new();

static mut DISPATCHER_CTX: Context = Context::zeroed();
/// Where the hook saves the registers of a task it is retiring or rewinding.
/// Written and never read — `context::switch` needs a destination, and a
/// context nothing will ever resume is the honest one.
static mut ABANDONED_CTX: Context = Context::zeroed();
static mut TASK_CTX: [Context; TASKS] = [Context::zeroed(); TASKS];
static mut TASK_STACKS: [[u8; STACK_SIZE]; TASKS] = [[0; STACK_SIZE]; TASKS];

/// Which task the dispatcher last switched into, or `None` while the
/// dispatcher itself is running. The hook reads nothing else to decide
/// whether it may touch the scheduler at all.
static mut CURRENT_TASK: Option<usize> = None;
static mut TASK_IDS: [Option<TaskId>; TASKS] = [None; TASKS];

/// The hook's own tick count per slot, kept independently of the scheduler's
/// books so the two can be compared. This is clause 6's evidence.
static mut TICKS_ATTRIBUTED: [u32; TASKS] = [0; TASKS];
static mut TICKS_UNATTRIBUTED: u32 = 0;
/// A tick attributed to a task the scheduler does not know. Must stay 0.
static mut TICKS_UNKNOWN: u32 = 0;

static mut ENFORCEMENTS: u32 = 0;
/// The offender's attributed-tick count at the moment enforcement first
/// fired — clause 4's measured quantity.
static mut TICKS_AT_FIRST_ENFORCE: u32 = 0;
static mut FIRST_ENFORCE_TICK: u32 = 0;
/// Set if any enforcement ever named a task other than the offender.
static mut ENFORCED_WRONG_TASK: bool = false;
/// Set if any enforcement produced a disposition other than the declared
/// policy's own.
static mut WRONG_DISPOSITION: bool = false;

static mut OFFENDER_ENTRIES: u64 = 0;
/// Reset to 0 every time the offender's entry point runs — so a value that
/// survives a restart would prove the task was resumed, not restarted.
static mut OFFENDER_ACCUMULATOR: u64 = 0;
static mut OFFENDER_TOTAL: u64 = 0;
static mut OFFENDER_ACC_AT_FIRST_ENFORCE: u64 = 0;
/// What the entry point found in the accumulator on a re-entry and threw
/// away. Nonzero is the proof that a restart discarded real work rather than
/// resuming it.
static mut OFFENDER_ACC_ON_REENTRY: u64 = 0;
static mut OFFENDER_EXHAUSTED: bool = false;
static mut RESTARTS: u32 = 0;

static mut INNOCENT_COUNTER: u64 = 0;
static mut INNOCENT_AT_FIRST_ENFORCE: u64 = 0;
static mut INNOCENT_AFTER_ENFORCE: u64 = 0;
static mut INNOCENT_ARMS: u32 = 0;
static mut INNOCENT_EXHAUSTED: bool = false;

static mut COMPETITOR_COUNTER: u64 = 0;
static mut COMPETITOR_AT_FIRST_ENFORCE: u64 = 0;
static mut COMPETITOR_EXHAUSTED: bool = false;

/// Which slot the dispatcher selected, in order. The degrade claim is about
/// this sequence, so it is recorded rather than inferred.
static mut RUN_LOG: [u8; RUN_LOG_CAPACITY] = [0; RUN_LOG_CAPACITY];
static mut RUN_LOG_LEN: usize = 0;
/// How many entries were in the run log when enforcement first fired, so
/// "before the degrade" and "after the degrade" are separable.
static mut RUN_LOG_AT_FIRST_ENFORCE: usize = 0;

static mut DONE: bool = false;
static mut PREEMPTIONS: u32 = 0;

fn task_id(scheduler: &Scheduler<TASKS>, slot: usize) -> Option<TaskId> {
    scheduler.iter_tasks().map(|(task, _)| task).find(|task| task.index() == slot)
}

/// The offender's declared policy — the single constant that separates the
/// three fixtures.
fn offender_policy() -> Option<OverrunPolicy> {
    Some(match MODE {
        Mode::Restart => OverrunPolicy::Restart,
        Mode::Degrade => OverrunPolicy::Degrade(Priority::try_new(DEGRADE_FLOOR).ok()?),
        Mode::Trip => OverrunPolicy::TripToSafeState,
    })
}

/// The offending task: **no `switch`, no `hlt`, no scheduler call.** It
/// cannot cooperate with its own budget running out, which is the point.
///
/// It stamps its arrival and then accumulates. `OFFENDER_ACCUMULATOR` is
/// zeroed here and nowhere else, so a restart is distinguishable from a
/// resume by inspection of that one value.
extern "C" fn offender_task() -> ! {
    // SAFETY: single-CPU fixture; only this task writes these statics, and
    // the tick hook only reads them.
    unsafe {
        OFFENDER_ENTRIES += 1;
        // What the previous activation had accumulated and is about to lose.
        // Recorded *before* the reset, because "the work is gone" is only
        // evidence if there was work to lose.
        if OFFENDER_ENTRIES > 1 {
            OFFENDER_ACC_ON_REENTRY = OFFENDER_ACCUMULATOR;
        }
        OFFENDER_ACCUMULATOR = 0;
    }
    loop {
        // SAFETY: as above.
        unsafe {
            OFFENDER_ACCUMULATOR += 1;
            OFFENDER_TOTAL += 1;
            if OFFENDER_TOTAL >= LOOP_CEILING {
                // Defence in depth: enforcement never happened. Report it as
                // a failed run rather than as a harness timeout.
                OFFENDER_EXHAUSTED = true;
                DONE = true;
                context::switch(&raw mut TASK_CTX[OFFENDER_SLOT], &raw mut DISPATCHER_CTX);
            }
        }
    }
}

/// The innocent RT task: bursts of work, then blocks itself and yields. The
/// hook re-arms it. It is charged for the ticks it ran and must be charged
/// for no others.
extern "C" fn innocent_task() -> ! {
    loop {
        let mut done = 0u64;
        while done < INNOCENT_BURST {
            done += 1;
            // SAFETY: single-CPU fixture; only this task writes this static.
            unsafe {
                INNOCENT_COUNTER += 1;
                if INNOCENT_COUNTER >= LOOP_CEILING {
                    INNOCENT_EXHAUSTED = true;
                    break;
                }
            }
        }
        // A task mutating the scheduler must do so with interrupts masked,
        // or it races the very tick hook that reads the same scheduler
        // (`TEST-P1-04-01-A` clause 3's Tier 0 half).
        //
        // SAFETY: the closure is bounded (one pool write) and takes no lock;
        // the dispatcher runs only while this task does not.
        unsafe {
            interrupts::without_interrupts(|| {
                if let Some(task) = TASK_IDS[INNOCENT_SLOT] {
                    (*(&raw mut SCHEDULER)).set_state(task, TaskState::Blocked);
                }
            });
            context::switch(&raw mut TASK_CTX[INNOCENT_SLOT], &raw mut DISPATCHER_CTX);
        }
    }
}

/// The competitor: `Ready` throughout, at a priority strictly between the
/// offender's and its declared floor. It should make no progress at all until
/// the offender is degraded beneath it.
extern "C" fn competitor_task() -> ! {
    loop {
        // SAFETY: single-CPU fixture; only this task writes these statics.
        unsafe {
            COMPETITOR_COUNTER += 1;
            if COMPETITOR_COUNTER >= LOOP_CEILING {
                COMPETITOR_EXHAUSTED = true;
                DONE = true;
            }
            if COMPETITOR_COUNTER >= COMPETITOR_TARGET && !DONE {
                // The evidence is complete: end the run. Retiring the
                // offender too is a *fixture* decision about when to stop,
                // not a policy decision — the degrade arm deliberately
                // leaves a degraded task runnable forever.
                DONE = true;
                interrupts::without_interrupts(|| {
                    let scheduler = &mut *(&raw mut SCHEDULER);
                    for slot in [COMPETITOR_SLOT, OFFENDER_SLOT, INNOCENT_SLOT] {
                        if let Some(task) = TASK_IDS[slot] {
                            scheduler.set_state(task, TaskState::Finished);
                        }
                    }
                });
                context::switch(&raw mut TASK_CTX[COMPETITOR_SLOT], &raw mut DISPATCHER_CTX);
            }
            if DONE {
                context::switch(&raw mut TASK_CTX[COMPETITOR_SLOT], &raw mut DISPATCHER_CTX);
            }
        }
    }
}

/// Ends the run the moment a `TripToSafeState` disposition is taken.
///
/// The system's declared safe state at Tier 0 is a reported, fail-closed
/// stop, so this reports and exits with QEMU's failure code — which is this
/// fixture's *correct* outcome, exactly as `broken-boot` and
/// `idt-apic-unrouted` already establish for their own paths. A safe state
/// that returned to the dispatcher would not be a safe state.
fn enter_safe_state(task: TaskId, ticks: u32, tick: u32) -> ! {
    // SAFETY: this function never returns, so re-initializing COM1 here
    // cannot race any other user of it on this single-CPU path.
    let mut serial = unsafe { SerialPort::init() };
    let within_bound = ticks <= OFFENDER_BUDGET + MAX_TICKS_TO_ENFORCE;
    // The kernel's own half of the trip, checked rather than assumed. Without
    // this the fixture would prove only that the *decision* reached the
    // caller — and since this arm's pass condition is a failure exit code, a
    // kernel that decided correctly and did nothing would be indistinguishable
    // from one that worked. A deliberate falsification proved exactly that.
    //
    // SAFETY: a task is running, so the dispatcher holds no borrow; the
    // accounting borrow above was dropped before this call.
    let finished = unsafe { (*(&raw const SCHEDULER)).state_of(task) == Some(TaskState::Finished) };
    let _ = writeln!(
        serial,
        "fixture-{NAME}: TRIP task={} attributed_ticks={ticks} budget={OFFENDER_BUDGET} \
         (bound {}) tick={tick} within_bound={within_bound} task_finished={finished}",
        task.index(),
        OFFENDER_BUDGET + MAX_TICKS_TO_ENFORCE
    );
    // SAFETY: read from interrupt context on a single-CPU path with the run
    // already over; nothing else can be executing.
    let (innocent, arms, attributed, unattributed) =
        unsafe { (INNOCENT_COUNTER, INNOCENT_ARMS, TICKS_ATTRIBUTED, TICKS_UNATTRIBUTED) };
    let _ = writeln!(
        serial,
        "fixture-{NAME}: innocent_counter={innocent} innocent_arms={arms} \
         ticks_attributed={attributed:?} ticks_unattributed={unattributed}"
    );
    let _ = writeln!(
        serial,
        "fixture-{NAME}: entering declared safe state — fail-closed stop is this fixture's \
         pass condition"
    );
    // Reported as `ok` because the trip is the correct outcome; the *exit
    // code* is what makes the run distinguishable, and it is a failure code
    // on purpose.
    let _ = write_result(&mut serial, NAME, within_bound && finished && innocent > 0);
    exit_qemu(QemuExitCode::Failure)
}

/// The timer-tick consumer. Runs in interrupt context on the interrupted
/// task's own stack with `IF` clear. Bounded and allocation-free throughout.
extern "C" fn on_tick() {
    // The dispatcher legitimately holds a `&mut Scheduler` and runs with `IF`
    // clear, but a tick already in flight when it cleared the flag can still
    // land here. Checking this *first*, before touching the scheduler at all,
    // is what makes that harmless — and it is simultaneously the attribution
    // rule's `Nobody` arm. See the module doc.
    //
    // SAFETY: single-CPU fixture; `CURRENT_TASK` is written only by the
    // dispatcher, with interrupts disabled.
    let Some(slot) = (unsafe { CURRENT_TASK }) else {
        // SAFETY: as above.
        unsafe { TICKS_UNATTRIBUTED += 1 };
        return;
    };

    let tick = interrupts::tick_count();

    // SAFETY: a task is running, so the dispatcher holds no borrow; this is
    // the only code touching the scheduler for the duration. The `&mut` is
    // formed and dropped inside this block, before any switch is taken.
    let disposition = unsafe {
        let scheduler = &mut *(&raw mut SCHEDULER);
        let journal = &mut *(&raw mut JOURNAL_STORE);
        let running = task_id(scheduler, slot);

        // The whole Story, in one call: attribute the tick, charge it, and
        // apply whatever the task declared if it crossed its budget.
        let accounting = wcet::account_tick(scheduler, journal, running);

        // Re-arm the innocent task on a fixed period, so it keeps making
        // progress across the detection and the disposition.
        if !DONE && tick.is_multiple_of(INNOCENT_PERIOD) {
            if let Some(task) = TASK_IDS[INNOCENT_SLOT] {
                if scheduler.state_of(task) == Some(TaskState::Blocked) {
                    scheduler.set_state(task, TaskState::Ready);
                    INNOCENT_ARMS += 1;
                }
            }
        }

        match accounting {
            TickAccounting::Unattributed => {
                // Unreachable: `slot` is `Some` here, so `running` is `Some`
                // for any live task. Counted rather than ignored.
                TICKS_UNATTRIBUTED += 1;
                None
            }
            TickAccounting::UnknownTask => {
                TICKS_UNKNOWN += 1;
                None
            }
            TickAccounting::WithinBudget(task) => {
                TICKS_ATTRIBUTED[task.index()] += 1;
                None
            }
            TickAccounting::Enforced { task, disposition } => {
                TICKS_ATTRIBUTED[task.index()] += 1;
                ENFORCEMENTS += 1;
                if task.index() != OFFENDER_SLOT {
                    ENFORCED_WRONG_TASK = true;
                }
                // The disposition must be the one the *declared* policy maps
                // to, checked against the mode rather than against whatever
                // came back — otherwise this would assert only that the
                // enumeration round-trips.
                let as_declared = match (MODE, disposition) {
                    (Mode::Restart, OverrunDisposition::RestartTask) => true,
                    (Mode::Degrade, OverrunDisposition::DegradeTo(floor)) => {
                        Ok(floor) == Priority::try_new(DEGRADE_FLOOR)
                    }
                    (Mode::Trip, OverrunDisposition::TripToSafeState) => true,
                    _ => false,
                };
                if !as_declared {
                    WRONG_DISPOSITION = true;
                }
                if ENFORCEMENTS == 1 {
                    TICKS_AT_FIRST_ENFORCE = TICKS_ATTRIBUTED[task.index()];
                    FIRST_ENFORCE_TICK = tick;
                    OFFENDER_ACC_AT_FIRST_ENFORCE = OFFENDER_ACCUMULATOR;
                    INNOCENT_AT_FIRST_ENFORCE = INNOCENT_COUNTER;
                    COMPETITOR_AT_FIRST_ENFORCE = COMPETITOR_COUNTER;
                    RUN_LOG_AT_FIRST_ENFORCE = RUN_LOG_LEN;
                }
                Some((task, disposition))
            }
        }
    };

    if let Some((task, disposition)) = disposition {
        // SAFETY: `slot` is the task this interrupt is executing on, so
        // `TASK_CTX[slot]` is its own; `DISPATCHER_CTX` is suspended at the
        // dispatcher's `run_once` call site. The scheduler borrow above has
        // been dropped.
        unsafe {
            match disposition {
                // The caller's half of a restart: rewind the instruction
                // pointer. `wcet::account_tick` has already reset the budget
                // window and returned the task to `Ready`; only this part
                // needs a stack and an entry point, which is why it lives
                // here. See the module doc for why building a fresh
                // `Context` over this very stack is sound.
                OverrunDisposition::RestartTask => {
                    RESTARTS += 1;
                    let retire = RESTARTS > RESTART_TARGET;
                    if retire {
                        DONE = true;
                        (*(&raw mut SCHEDULER)).set_state(task, TaskState::Finished);
                    } else {
                        let stack = core::slice::from_raw_parts_mut(
                            (&raw mut TASK_STACKS[OFFENDER_SLOT]).cast::<u8>(),
                            STACK_SIZE,
                        );
                        match Context::new(stack, offender_task) {
                            Ok(fresh) => TASK_CTX[OFFENDER_SLOT] = fresh,
                            Err(_) => {
                                DONE = true;
                                (*(&raw mut SCHEDULER)).set_state(task, TaskState::Finished);
                            }
                        }
                    }
                    CURRENT_TASK = None;
                    context::switch(&raw mut ABANDONED_CTX, &raw mut DISPATCHER_CTX);
                    unreachable!("a rewound or retired task is never switched back into")
                }
                // Nothing further is required of the caller — the priority is
                // already lowered. Leaving the CPU here is a fixture choice
                // that makes the consequence immediate: the very next
                // selection is the one the offender must lose.
                OverrunDisposition::DegradeTo(_) => {
                    CURRENT_TASK = None;
                    context::switch(&raw mut TASK_CTX[OFFENDER_SLOT], &raw mut DISPATCHER_CTX);
                }
                OverrunDisposition::TripToSafeState => {
                    enter_safe_state(task, TICKS_AT_FIRST_ENFORCE, tick)
                }
            }
        }
    }

    // The preemption decision, unchanged from `STORY-P1-04-01`. Enforcement
    // and preemption share this hook, and neither may perturb the other.
    //
    // SAFETY: as above; the scheduler borrow is dropped.
    let outcome = unsafe {
        let Some(slot) = CURRENT_TASK else { return };
        let running = task_id(&*(&raw const SCHEDULER), slot);
        preempt::on_timer_tick(
            &raw mut SCHEDULER,
            running,
            &raw mut TASK_CTX[slot],
            &raw mut DISPATCHER_CTX,
        )
    };
    if matches!(outcome, TickOutcome::Preempt(_)) {
        // SAFETY: reached only after the preempted task is resumed.
        unsafe { PREEMPTIONS += 1 };
    }
}

/// Creates one task in `slot`, initializing its context.
///
/// # Safety
/// `slot` must be the next unused scheduler slot, and its stack must not be
/// in use by any other context.
unsafe fn create(
    slot: usize,
    priority: u8,
    budget: u32,
    policy: OverrunPolicy,
    entry: extern "C" fn() -> !,
) -> Option<TaskId> {
    // SAFETY: per this function's own contract.
    unsafe {
        let scheduler = &mut *(&raw mut SCHEDULER);
        let priority = Priority::try_new(priority).ok()?;
        let task = scheduler.create_task(priority, WcetBudgetTicks(budget), policy, entry).ok()?;
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

/// Counts how many spoors in the journal carry `action` under
/// `Category::Wcet`.
fn count_wcet(journal: &SpoorJournal<JOURNAL>, action: Action) -> usize {
    journal
        .iter()
        .filter(|spoor| spoor.category() == Category::Wcet && spoor.action() == action)
        .count()
}

/// Runs the fixture.
pub fn run() -> bool {
    // SAFETY: this fixture is the only code running; `init` is called once,
    // before any other `SerialPort` method.
    let mut serial = unsafe { SerialPort::init() };
    let mut ok = true;

    let Some(policy) = offender_policy() else {
        let _ = writeln!(serial, "fixture-{NAME}: could not build the declared policy");
        return false;
    };

    // SAFETY: single-CPU fixture, each slot used exactly once.
    unsafe {
        if create(OFFENDER_SLOT, OFFENDER_PRIORITY, OFFENDER_BUDGET, policy, offender_task)
            .is_none()
        {
            let _ = writeln!(serial, "fixture-{NAME}: offender creation failed");
            return false;
        }
        let innocent = create(
            INNOCENT_SLOT,
            INNOCENT_PRIORITY,
            GENEROUS_BUDGET,
            OverrunPolicy::TripToSafeState,
            innocent_task,
        );
        if innocent.is_none() {
            let _ = writeln!(serial, "fixture-{NAME}: innocent creation failed");
            return false;
        }
        let competitor = create(
            COMPETITOR_SLOT,
            COMPETITOR_PRIORITY,
            GENEROUS_BUDGET,
            OverrunPolicy::TripToSafeState,
            competitor_task,
        );
        let Some(competitor) = competitor else {
            let _ = writeln!(serial, "fixture-{NAME}: competitor creation failed");
            return false;
        };
        // The competitor is `Ready` throughout only in the degrade run, where
        // its progress is the evidence. In the other two it would merely be
        // noise, so it is parked.
        if MODE != Mode::Degrade {
            (*(&raw mut SCHEDULER)).set_state(competitor, TaskState::Blocked);
        }
    }

    // SAFETY: registered before interrupts are armed, so no tick can arrive
    // between arming and installation.
    unsafe { interrupts::set_tick_hook(on_tick) };
    // SAFETY: called exactly once, before anything here depends on interrupts
    // being armed — `init`'s own documented contract. It ends with `sti`.
    unsafe { interrupts::init(INITIAL_COUNT) };
    // From here the dispatcher runs with `IF` clear and never re-enables it:
    // a task's own saved `RFLAGS` is what turns interrupts back on across the
    // switch into it. That, and not a convention, is what stops the hook ever
    // observing a scheduler this loop is mid-mutation of.
    //
    // SAFETY: every subsequent re-enable happens via a context switch's own
    // `popfq`, so interrupts are not lost.
    let _ = unsafe { interrupts::disable_interrupts() };

    let mut rounds: u32 = 0;
    loop {
        // SAFETY: interrupts are masked, so this is the only code touching
        // the scheduler; `TASK_CTX` slots are each owned by one task.
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
            let _ = writeln!(serial, "fixture-{NAME}: dispatcher exceeded {MAX_ROUNDS} rounds");
            ok = false;
            break;
        }
    }

    // SAFETY: read after every switch has returned and with interrupts
    // masked; nothing else can be running.
    unsafe {
        INNOCENT_AFTER_ENFORCE = INNOCENT_COUNTER;
    }

    // SAFETY: as above.
    let (
        enforcements,
        ticks_at_first,
        first_tick,
        wrong_task,
        wrong_disposition,
        entries,
        acc_at_enforce,
        acc_on_reentry,
        offender_exhausted,
        restarts,
        innocent,
        innocent_at,
        innocent_after,
        arms,
        innocent_exhausted,
        competitor,
        competitor_at,
        competitor_exhausted,
        attributed,
        unattributed,
        unknown,
        preemptions,
        log_at_enforce,
    ) = unsafe {
        (
            ENFORCEMENTS,
            TICKS_AT_FIRST_ENFORCE,
            FIRST_ENFORCE_TICK,
            ENFORCED_WRONG_TASK,
            WRONG_DISPOSITION,
            OFFENDER_ENTRIES,
            OFFENDER_ACC_AT_FIRST_ENFORCE,
            OFFENDER_ACC_ON_REENTRY,
            OFFENDER_EXHAUSTED,
            RESTARTS,
            INNOCENT_COUNTER,
            INNOCENT_AT_FIRST_ENFORCE,
            INNOCENT_AFTER_ENFORCE,
            INNOCENT_ARMS,
            INNOCENT_EXHAUSTED,
            COMPETITOR_COUNTER,
            COMPETITOR_AT_FIRST_ENFORCE,
            COMPETITOR_EXHAUSTED,
            TICKS_ATTRIBUTED,
            TICKS_UNATTRIBUTED,
            TICKS_UNKNOWN,
            PREEMPTIONS,
            RUN_LOG_AT_FIRST_ENFORCE,
        )
    };

    // Clause 4: the overrun was detected, once, on the offending task, within
    // the bound this test fixed before the fixture existed.
    ok &= enforcements >= 1;
    ok &= !wrong_task;
    ok &= !wrong_disposition;
    ok &= ticks_at_first <= OFFENDER_BUDGET + MAX_TICKS_TO_ENFORCE;
    ok &= ticks_at_first > OFFENDER_BUDGET;
    ok &= !offender_exhausted;
    ok &= unknown == 0;

    // **The budget window was reset by the enforcement**, and this is the
    // assertion a deliberate falsification proved was missing. Without it the
    // restart run passed with every scheduler mutation removed from
    // `wcet::apply`, because the fixture's own context rewind plus
    // `dispatch::run_once`'s ordinary `Running -> Ready` transition together
    // reproduced the *visible* effects of a restart. What they cannot
    // reproduce is the spacing: a task whose window is never reset overruns
    // again on the very next tick, so enforcements pile up one per tick.
    //
    // Every enforcement must therefore be a full `budget + 1` attributed
    // ticks after the previous one — no more enforcements than the budget
    // permits, and no fewer.
    let offender_ticks = attributed[OFFENDER_SLOT];
    let window = OFFENDER_BUDGET + 1;
    ok &= offender_ticks >= enforcements * window;
    ok &= offender_ticks < (enforcements + 1) * window;

    // Clause 6: enforcement did not punish the innocent. The equality is the
    // claim — the scheduler's books and the hook's independent count agree,
    // task by task.
    //
    // SAFETY: interrupts masked, run over.
    let books_agree = unsafe {
        let scheduler = &mut *(&raw mut SCHEDULER);
        let mut agree = true;
        for slot in [INNOCENT_SLOT, COMPETITOR_SLOT] {
            if let Some(task) = TASK_IDS[slot] {
                let consumed = scheduler.wcet_state(task).map(|(consumed, _)| consumed);
                agree &= consumed == Some(attributed[slot]);
            }
        }
        agree
    };
    ok &= books_agree;
    ok &= innocent >= INNOCENT_MIN_PROGRESS;
    ok &= innocent_after > innocent_at;
    ok &= !innocent_exhausted;
    ok &= arms >= 1;

    // Clause 7: the decisions are audited, and the audit says what happened.
    //
    // SAFETY: as above.
    let (overruns, restarts_logged, degrades_logged, terminates_logged) = unsafe {
        let journal = &*(&raw const JOURNAL_STORE);
        (
            count_wcet(journal, Action::Overrun),
            count_wcet(journal, Action::Restart),
            count_wcet(journal, Action::Degrade),
            count_wcet(journal, Action::Terminate),
        )
    };
    ok &= overruns >= 1;

    // Clause 5, per arm.
    match MODE {
        Mode::Restart => {
            // The task ran from its entry point again, and what it had
            // accumulated before the overrun is gone.
            // The initial activation plus one per restart.
            ok &= entries > RESTART_TARGET as u64;
            ok &= acc_at_enforce > 0;
            // Real accumulated work was found and discarded on re-entry —
            // "it was marked Ready" is not the claim.
            ok &= acc_on_reentry > 0;
            ok &= restarts >= RESTART_TARGET;
            ok &= restarts_logged >= RESTART_TARGET as usize;
        }
        Mode::Degrade => {
            // The competitor was Ready throughout and made no progress at all
            // before the degrade, then started winning selections.
            ok &= competitor_at == 0;
            ok &= competitor >= COMPETITOR_TARGET;
            ok &= !competitor_exhausted;
            ok &= degrades_logged >= 1;
            // And the run log shows it: the competitor's slot appears only
            // after the enforcement, never before.
            // SAFETY: interrupts masked, run over.
            let (before, after) = unsafe {
                let log = &RUN_LOG[..RUN_LOG_LEN];
                let split = log_at_enforce.min(log.len());
                (
                    log[..split].iter().any(|slot| *slot as usize == COMPETITOR_SLOT),
                    log[split..].iter().any(|slot| *slot as usize == COMPETITOR_SLOT),
                )
            };
            ok &= !before;
            ok &= after;
        }
        // The trip arm never reaches here: `enter_safe_state` does not
        // return. Arriving here at all means the system did *not* stop, which
        // is the failure that arm exists to rule out.
        Mode::Trip => ok = false,
    }

    let _ = writeln!(
        serial,
        "fixture-{NAME}: enforcements={enforcements} ticks_at_first_enforce={ticks_at_first} \
         budget={OFFENDER_BUDGET} (bound {}) first_enforce_tick={first_tick} \
         wrong_task={wrong_task} wrong_disposition={wrong_disposition}",
        OFFENDER_BUDGET + MAX_TICKS_TO_ENFORCE
    );
    let _ = writeln!(
        serial,
        "fixture-{NAME}: offender entries={entries} acc_at_enforce={acc_at_enforce} \
         acc_on_reentry={acc_on_reentry} restarts={restarts} exhausted={offender_exhausted}"
    );
    let _ = writeln!(
        serial,
        "fixture-{NAME}: innocent counter={innocent} at_enforce={innocent_at} \
         after={innocent_after} arms={arms} books_agree={books_agree}"
    );
    let _ = writeln!(
        serial,
        "fixture-{NAME}: competitor counter={competitor} at_enforce={competitor_at} \
         (target {COMPETITOR_TARGET})"
    );
    let _ = writeln!(
        serial,
        "fixture-{NAME}: ticks_attributed={attributed:?} unattributed={unattributed} \
         unknown={unknown} preemptions={preemptions}"
    );
    let _ = writeln!(
        serial,
        "fixture-{NAME}: spoors overrun={overruns} restart={restarts_logged} \
         degrade={degrades_logged} terminate={terminates_logged}"
    );
    // SAFETY: interrupts masked, run over.
    let _ =
        writeln!(serial, "fixture-{NAME}: dispatch order={:?}", unsafe { &RUN_LOG[..RUN_LOG_LEN] });

    let _ = write_result(&mut serial, NAME, ok);
    ok
}
