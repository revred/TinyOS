//! `TEST-P1-06-01-A`: the bounded decision-to-actuation path, enforced and
//! measured (`STORY-P1-06-01` — `G-PA-1`'s flagship path, `FEAT-P1-06`'s
//! mechanism half).
//!
//! **One file, two fixtures.** `--fixture=actuation` and
//! `--fixture=actuation-overrun` are the same scenario with one constant
//! changed: the control task's declared WCET budget. That is deliberate, and it
//! is [`crate::fixture_wcet`]'s own reasoning — the claim being made is
//! precisely that the *same* path produces two different, correct outcomes
//! because the *declaration* differed. Two files would let the two drift, and
//! the clean run would slowly stop being the same path the overrun run trips.
//!
//! | Task | Priority | Budget | Deadline | Policy | What it does |
//! |---|---|---|---|---|---|
//! | control | 25 | clean: generous · overrun: **12 ticks** | **2 ticks** | `TripToSafeState` | arms its window, computes a command, emits it through the output line |
//! | background | 5 | generous | — | trip (never fires) | busy-increments a counter; **is the unauthorized caller identity** |
//!
//! `background` does two jobs at once: it is a real competitor the RT task must
//! keep outranking, and it is a live [`TaskId`] that is *not* the declared
//! actuation task — so "no ambient path" is attacked with a real identity
//! rather than a fabricated one.
//!
//! ## What a number from the clean run is, and is not
//!
//! Tier 0 evidence about the **mechanism**. QEMU/TCG's TSC is a software model
//! and its port I/O is a device-model dispatch, so neither the cycle counts nor
//! anything derived from them are hardware WCET evidence. Under `ADR 0005` a
//! worst-case bound is quotable only from a platform holding a secure-world
//! qualification record and **zero platforms hold one**, so no
//! `PERF-D03-G04`/`PERF-D05-G04` row is filed here and none can be: the bound
//! is stated debt against `LE-09`. `FEAT-P1-06` draws the same line in its own
//! words — *a QEMU-measured bound is the mechanism's proof, the boards' numbers
//! are the product's*.
//!
//! ## Two properties of the measurement, stated rather than hidden
//!
//! **The timed region runs with interrupts masked.** It has to: the port's
//! state is read by the tick hook, so touching it from task context outside
//! `without_interrupts` is a data race with the deadline monitor. The
//! consequence is that **no timer tick lands inside a sample** — these
//! percentiles are the cost of the path, not of the path plus an arbitrary
//! interrupt. Ticks land between iterations, where they belong.
//!
//! **The audit stamp is inside the timed region.** Every emit and every refusal
//! stamps a [`kernel::spoor::Spoor`], and that cost is part of what is
//! reported, because it is part of what the system actually does on the way to
//! the actuator. Excluding it would report a path this kernel never takes.
//!
//! Only reachable when one of the two `fixture-actuation*` features is enabled
//! — never part of a real boot image.

#![allow(static_mut_refs, clippy::deref_addrof)]

use core::fmt::Write;
use hal::actuation::OutputLine;
use hal::time::Timebase;
use hal_x86_64::actuation::PortLine;
use hal_x86_64::interrupts;
use hal_x86_64::qemu_exit::{exit_qemu, QemuExitCode};
use hal_x86_64::serial::SerialPort;
use hal_x86_64::tsc::{self, Tsc};
use kernel::actuation::{ActuationError, ActuationPort, DeadlineStatus, DeadlineTicks};
use kernel::context::{self, Context};
use kernel::dispatch;
use kernel::measure::{
    write_result, Calibration, Environment, Metric, Report, Samples, Stopwatch, Summary,
};
use kernel::preempt::{self, TickOutcome};
use kernel::sched::{OverrunPolicy, Priority, Scheduler, TaskId, TaskState, WcetBudgetTicks};
use kernel::spoor_journal::SpoorJournal;
use kernel::wcet::{self, OverrunDisposition, TickAccounting};

/// Which claim this build is proving.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// `--fixture=actuation`: the path exists, refuses an ambient caller, and
    /// its latency distribution is measured and recorded.
    Clean,
    /// `--fixture=actuation-overrun`: the decision deliberately overruns its
    /// declared budget, the enforcement fires, and **no command reaches the
    /// line**.
    Overrun,
}

/// Selected by feature rather than declared twice, so exactly one arm can be
/// live in any build.
const MODE: Mode =
    if cfg!(feature = "fixture-actuation-overrun") { Mode::Overrun } else { Mode::Clean };

/// The fixture name reported in the `TINYOS-RESULT/1` line.
const NAME: &str = match MODE {
    Mode::Clean => "actuation",
    Mode::Overrun => "actuation-overrun",
};

const TASKS: usize = 2;
const CONTROL_SLOT: usize = 0;
const BACKGROUND_SLOT: usize = 1;

const CONTROL_PRIORITY: u8 = 25;
const BACKGROUND_PRIORITY: u8 = 5;

/// The control task's declared budget in the overrun run, in ticks.
///
/// **Twelve, not `fixture_wcet`'s four, and the gap to [`DEADLINE`] is the
/// point.** The deadline clock starts when the task *arms*; the budget clock
/// starts when the dispatcher *selects* it. Those are not the same instant, and
/// at four ticks the difference was enough to collapse the whole window between
/// the missed deadline and the trip into a single tick — the late-emit probe
/// then never ran, in the honest build and the falsified one alike, so clause
/// 7's second falsification "fired" while proving nothing.
///
/// Twelve leaves roughly ten ticks between the two events no matter where in a
/// tick period the arm lands. The margin is deliberately generous: a fixture
/// that only works when two independent clocks happen to line up is one that
/// goes intermittently red for no defect.
const OVERRUN_BUDGET: u32 = 12;
/// A budget no task in the clean run can plausibly reach.
const GENEROUS_BUDGET: u32 = 1_000_000;

/// `TEST-P1-06-01-A` clause 5's bound, fixed in that document before this
/// fixture existed: enforcement lands no later than the
/// `OVERRUN_BUDGET + MAX_TICKS_TO_ENFORCE`-th attributed tick.
const MAX_TICKS_TO_ENFORCE: u32 = 1;

/// The control task's declared relative deadline, in ticks. Deliberately
/// **below** `OVERRUN_BUDGET`, so the overrun run exercises both mechanisms in
/// their natural order: the monitor sees the window close first, and the
/// scheduler removes the offender second.
const DEADLINE: u32 = 2;

/// Sampled iterations per measured phase, and the unmeasured warmup before
/// each. Same shape and the same reason as `fixture_measure`: first-touch
/// cache effects belong outside the reported percentiles, and `warmup=` is
/// reported in the envelope rather than left implicit.
const SAMPLES: usize = 1_000;
const WARMUP: usize = 100;

/// Read pairs used to calibrate the cycle source's own overhead.
const CALIBRATION_SAMPLES: usize = 2_000;

/// Steps in the decision computation. Deliberately small and fixed: this
/// Feature is *"one task, one output, one bound — deliberately minimal, so the
/// determinism claim is attributable to the kernel rather than to application
/// structure"*. A heavy decision would report an application's cost and call it
/// the kernel's.
const DECISION_STEPS: u32 = 8;

/// Per-task stack.
///
/// **32 KiB, not the 8 KiB every other fixture in this crate uses**, and the
/// reason is worth recording because the failure it fixes is silent. This
/// fixture's control task is the only one that calls a measurement harness from
/// task context: `Stopwatch`, `Calibration`, a `without_interrupts` closure per
/// iteration and a thousand-sample `summarize`, all in an unoptimized dev
/// profile that reuses no stack slots across lexically separate blocks — on top
/// of the interrupt frame, fifteen pushed registers and the 512-byte `fxsave`
/// area the tick hook lands on the same stack.
///
/// At 8 KiB it overflowed **downward into `.bss`**, and the symptom was not a
/// fault: it was `preemptions=1311424` in a run that saw ten ticks, and
/// `overhead_cycles=1311424` in another — a spilled cycle delta appearing in
/// whichever static happened to be below the stack. Numbers, in the artifact,
/// that were plausible enough to publish. `CANARY` below is what makes the next
/// occurrence say so instead.
const STACK_SIZE: usize = 32_768;

/// Written at the low end of every task stack and checked at the end of the
/// run. A stack that grew past it has already corrupted whatever lies below,
/// and the whole run's evidence is void — so the fixture says that, rather than
/// reporting the corrupted numbers as measurements. `LE-46`'s rule one level
/// along: an instrument that can be silently wrong is worse than one that is
/// missing.
const CANARY: u64 = 0x5441_524F_4E41_4359;
/// Local-APIC reload. **A tenth of `fixture_preempt`'s 500,000**, and the
/// reason is a clause that failed rather than a preference: at 500,000 the
/// release-profile build completes all 2,200 iterations before the *first* tick
/// arrives, so the run measured the path and proved nothing at all about the
/// deadline monitor being live — `ticks=0`, caught by clause 1's own check.
///
/// A fixture whose evidence depends on the optimizer's mood is not a fixture.
/// At 50,000 both profiles see real ticks throughout, and no clean-run window
/// can expire regardless, because arm-and-emit is one interrupt-free critical
/// section: a tick cannot land inside it.
const INITIAL_COUNT: u32 = 50_000;
const JOURNAL: usize = 64;
const MAX_ROUNDS: u32 = 64;
/// Ceilings, defence in depth only — a passing run never approaches them.
const LOOP_CEILING: u64 = 4_000_000_000;

/// How many metrics the clean run emits.
const METRICS: usize = 2;

/// The output boundary, wrapped so the fixture counts writes **independently of
/// anything the port records**.
///
/// "The actuator moved" has to be observable somewhere the port does not own,
/// or the evidence for "no command reached the line" is the port's own word for
/// it. The counter is incremented *after* the real `out` retires, so this
/// wrapper can never claim a write that did not happen.
struct CountingLine {
    inner: PortLine,
    writes: u32,
    last: Option<u8>,
}

impl OutputLine for CountingLine {
    const NAME: &'static str = PortLine::NAME;

    fn write_command(&mut self, command: u8) {
        self.inner.write_command(command);
        self.writes += 1;
        self.last = Some(command);
    }
}

static mut SCHEDULER: Scheduler<TASKS> = Scheduler::new();
static mut JOURNAL_STORE: SpoorJournal<JOURNAL> = SpoorJournal::new();
/// Built in `run` rather than in a `static` initializer: the declared owner is
/// a `TaskId` the scheduler issues at runtime, and a port that could exist
/// before its owner did would be exactly the ownerless, ambient port
/// `kernel::actuation` exists to make unrepresentable.
static mut PORT: Option<ActuationPort<CountingLine>> = None;

static mut SAMPLE_BUFFER: Samples<SAMPLES> = Samples::new();
static mut CALIBRATION: Calibration = Calibration::from_overhead_cycles(0);
/// Each phase summarizes **its own** samples before the next one starts, into
/// its own slot. One shared buffer keeps this fixture's static footprint at
/// 8 KiB rather than 16, but a shared buffer summarized once at the end would
/// silently report one distribution over two different populations — which is
/// worse than either number alone, because it looks like evidence.
static mut EMIT_SUMMARY: Option<Summary> = None;
static mut DENIAL_SUMMARY: Option<Summary> = None;

static mut DISPATCHER_CTX: Context = Context::zeroed();
// No `ABANDONED_CTX` here, unlike `fixture_wcet` and `fixture_fault`: the only
// disposition this fixture's tasks declare is `TripToSafeState`, and
// `enter_safe_state` stops the machine rather than switching away from the
// offender. A context nothing will ever resume is the honest destination for a
// task the hook abandons; a fixture that never abandons one needs no such slot.
static mut TASK_CTX: [Context; TASKS] = [Context::zeroed(); TASKS];
static mut TASK_STACKS: [[u8; STACK_SIZE]; TASKS] = [[0; STACK_SIZE]; TASKS];

static mut CURRENT_TASK: Option<usize> = None;
static mut TASK_IDS: [Option<TaskId>; TASKS] = [None; TASKS];

/// Ticks the hook saw at all — evidence the timer is genuinely armed and the
/// deadline monitor is live, which matters because this scenario has no
/// higher-priority task to preempt the RT one.
static mut TICKS_SEEN: u32 = 0;
static mut TICKS_UNKNOWN: u32 = 0;
static mut PREEMPTIONS: u32 = 0;
/// Deadline misses the monitor detected, counted from the hook's own return
/// value rather than read out of the journal, which is a ring that wraps.
static mut DEADLINE_MISSES: u32 = 0;

static mut ENFORCEMENTS: u32 = 0;
static mut ENFORCED_WRONG_TASK: bool = false;
static mut WRONG_DISPOSITION: bool = false;
static mut TICKS_ATTRIBUTED_CONTROL: u32 = 0;
static mut TICKS_AT_FIRST_ENFORCE: u32 = 0;

/// Clean-run bookkeeping.
static mut EMIT_FAILURES: u32 = 0;
static mut DENIAL_FAILURES: u32 = 0;
static mut EMIT_PHASE_DONE: bool = false;
static mut DENIAL_PHASE_DONE: bool = false;
/// The line's write count at the start and end of the denial phase. Equal in a
/// passing run: an unauthorized identity never reaches the line.
static mut WRITES_BEFORE_DENIAL: u32 = 0;
static mut WRITES_AFTER_DENIAL: u32 = 0;

/// Overrun-run bookkeeping.
static mut DECISION_WORK: u64 = 0;
static mut DECISION_EXHAUSTED: bool = false;
/// The **late-emit probe**: one command presented after the deadline has
/// closed and before the budget has run out, from inside the still-running
/// decision.
///
/// This is clause 4's Tier 0 evidence, and it exists because without it the
/// clause has no instrument. The trip removes the offender before it can reach
/// any emit of its own, so *"a late command is prevented"* would rest entirely
/// on the task never running again — and the falsification clause 7 demands
/// (remove the expiry check, watch a late command reach the line) would have
/// nothing to fire against. The probe is the difference between prevention that
/// is enforced and prevention that is merely unreachable.
static mut LATE_PROBE_DONE: bool = false;
static mut LATE_PROBE_RESULT: Option<Result<(), ActuationError>> = None;
static mut LATE_PROBE_WRITES: u32 = u32::MAX;
/// Set only if the deliberately-overrunning decision *finished* — which it can
/// only do if the enforcement never fired.
static mut LATE_EMIT_REACHED: bool = false;
static mut LATE_EMIT_RESULT: Option<Result<(), ActuationError>> = None;

static mut BACKGROUND_COUNTER: u64 = 0;
static mut DONE: bool = false;

fn task_id(scheduler: &Scheduler<TASKS>, slot: usize) -> Option<TaskId> {
    scheduler.iter_tasks().map(|(task, _)| task).find(|task| task.index() == slot)
}

fn priority(value: u8) -> Option<Priority> {
    Priority::try_new(value).ok()
}

/// The decision half of the path: a fixed, deterministic computation producing
/// one command word.
///
/// `black_box` per iteration rather than only on the result, for the reason
/// `fixture_measure::phase_reference_loop` records at length: with the barrier
/// only at the ends, the optimizer closed-forms the whole recurrence and the
/// "decision" measured nothing.
#[inline(never)]
fn decide(index: u32) -> u8 {
    let mut accumulator = core::hint::black_box(index as u64 ^ 0x9E37_79B9_7F4A_7C15);
    for step in 0..DECISION_STEPS {
        accumulator = core::hint::black_box(
            accumulator.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(step as u64 | 1),
        );
    }
    (accumulator >> 24) as u8
}

/// Clause 1: the measured path. One iteration is one activation — arm, decide,
/// emit — with the timed region running from the start of the decision to
/// immediately after the line write returns.
///
/// `arm` is outside the timed region and inside the critical section: arming is
/// the *release* of an activation (in a real system, a sensor sample arriving),
/// not part of the decision-to-actuation latency the Goal states.
#[inline(never)]
fn phase_emit(control: TaskId) -> bool {
    let mut ok = true;
    for index in 0..(WARMUP + SAMPLES) {
        // SAFETY: single-CPU fixture. The port and the journal are also touched
        // by the tick hook, so every access from task context is inside one
        // bounded, interrupt-free critical section; the closure takes no lock
        // and allocates nothing.
        let cycles = unsafe {
            interrupts::without_interrupts(|| {
                let scheduler = &*(&raw const SCHEDULER);
                let journal = &mut *(&raw mut JOURNAL_STORE);
                let port = (&mut *(&raw mut PORT)).as_mut()?;
                port.arm();
                let watch = Stopwatch::start(&Tsc);
                let command = decide(index as u32);
                let emitted = port.emit(scheduler, journal, control, command);
                let cycles = watch.stop(&*(&raw const CALIBRATION));
                if emitted.is_err() {
                    EMIT_FAILURES += 1;
                }
                Some(cycles)
            })
        };
        let Some(cycles) = cycles else {
            return false;
        };
        if index >= WARMUP {
            // SAFETY: the buffer is touched only from this task's phases, never
            // from the hook, and the phases run strictly one after another.
            unsafe { (&mut *(&raw mut SAMPLE_BUFFER)).record(cycles) };
        }
    }
    // SAFETY: read after the loop, on the same single CPU; the buffer is
    // touched only by the phases, which run strictly one after another.
    unsafe {
        let samples = &mut *(&raw mut SAMPLE_BUFFER);
        EMIT_SUMMARY = samples.summarize();
        samples.clear();
        ok &= EMIT_FAILURES == 0 && EMIT_SUMMARY.is_some();
        EMIT_PHASE_DONE = true;
    }
    ok
}

/// Clause 2: the denial path, measured on the same harness — an emit presented
/// with `background`'s real `TaskId`.
///
/// Measured rather than merely asserted because a denial that is *slow* is its
/// own failure (`PERF-D03-G20`'s shape: a denial must be cheap and must change
/// nothing), and because an unmeasured denial path is one nobody notices
/// growing.
#[inline(never)]
fn phase_denial(intruder: TaskId) -> bool {
    // **The intruder is placed in `Running` for this phase, deliberately.**
    //
    // The first attempt at clause 7's falsification proved this necessary: with
    // the owner check removed from `ActuationPort::emit`, this fixture still
    // *passed*, because the background task is `Ready` and the port's separate
    // "the caller must be running" check refused it anyway. The fixture was
    // therefore testing the running check and reporting it as evidence for the
    // authority check — an instrument that could not fail for the reason it
    // claimed to be watching.
    //
    // With the intruder marked `Running`, the **only** thing distinguishing it
    // from the declared owner is its identity, which is exactly the attack
    // clause 2 describes. Two `Running` tasks is not a state this scheduler
    // would produce on its own; it is a fixture placing the system in the most
    // favourable possible state for the attacker, which is what an adversarial
    // test is for. The dispatcher is unaffected: a `Running` task is not
    // `Ready`, so it is never selected, and it is retired with everything else
    // at the end of the run.
    //
    // SAFETY: single-CPU; one bounded, interrupt-free critical section, and no
    // other user of the port at this point.
    unsafe {
        interrupts::without_interrupts(|| {
            (*(&raw mut SCHEDULER)).set_state(intruder, TaskState::Running);
        });
        WRITES_BEFORE_DENIAL = (&*(&raw const PORT)).as_ref().map_or(0, |port| port.line().writes);
    }
    for index in 0..(WARMUP + SAMPLES) {
        // SAFETY: as `phase_emit`.
        let cycles = unsafe {
            interrupts::without_interrupts(|| {
                let scheduler = &*(&raw const SCHEDULER);
                let journal = &mut *(&raw mut JOURNAL_STORE);
                let port = (&mut *(&raw mut PORT)).as_mut()?;
                // Armed, so the refusal cannot be an accident of a closed
                // window: the *only* reason this is refused is who asked.
                port.arm();
                let watch = Stopwatch::start(&Tsc);
                let command = decide(index as u32);
                let denied = port.emit(scheduler, journal, intruder, command);
                let cycles = watch.stop(&*(&raw const CALIBRATION));
                if denied != Err(ActuationError::NotAuthorized) {
                    DENIAL_FAILURES += 1;
                }
                Some(cycles)
            })
        };
        let Some(cycles) = cycles else {
            return false;
        };
        if index >= WARMUP {
            // SAFETY: as `phase_emit`.
            unsafe { (&mut *(&raw mut SAMPLE_BUFFER)).record(cycles) };
        }
    }
    // SAFETY: as `phase_emit`.
    unsafe {
        // **Disarmed first, before anything slow.** The last activation this
        // phase armed was never satisfied — by design, since every emit in it
        // was refused — and left armed it expires a few ticks later, reporting
        // a deadline miss belonging to no decision anybody took. That is not
        // hypothetical: summarizing before disarming sorts a thousand samples
        // inside the open window, which is several ticks' worth of work, and
        // the first run of this fixture failed exactly there. A cancelled
        // activation has to be cancelled *immediately*, not eventually.
        interrupts::without_interrupts(|| {
            if let Some(port) = (&mut *(&raw mut PORT)).as_mut() {
                port.disarm();
            }
        });
        WRITES_AFTER_DENIAL = (&*(&raw const PORT)).as_ref().map_or(0, |port| port.line().writes);
        let samples = &mut *(&raw mut SAMPLE_BUFFER);
        DENIAL_SUMMARY = samples.summarize();
        samples.clear();
        DENIAL_PHASE_DONE = true;
        DENIAL_FAILURES == 0
            && WRITES_AFTER_DENIAL == WRITES_BEFORE_DENIAL
            && DENIAL_SUMMARY.is_some()
    }
}

/// The RT control task.
///
/// In the clean run it drives both measured phases and then retires the run.
/// In the overrun run it arms one activation and enters a decision it cannot
/// finish inside its declared budget — **no `switch`, no `hlt`, no scheduler
/// call** — so whatever takes it off the CPU is the enforcement.
extern "C" fn control_task() -> ! {
    // SAFETY: single-CPU fixture; `TASK_IDS` is written once during setup,
    // before interrupts were armed.
    let (control, background) = unsafe { (TASK_IDS[CONTROL_SLOT], TASK_IDS[BACKGROUND_SLOT]) };
    let (Some(control), Some(background)) = (control, background) else {
        // SAFETY: ending the run is the only safe response to a setup that did
        // not complete; the checker reports it as a failure.
        unsafe {
            DONE = true;
            context::switch(&raw mut TASK_CTX[CONTROL_SLOT], &raw mut DISPATCHER_CTX);
        }
        unreachable!("the fixture fails before this task runs again")
    };

    match MODE {
        Mode::Clean => {
            let ok = phase_emit(control) && phase_denial(background);
            // SAFETY: single-CPU; the run is over as far as this task is
            // concerned, and every scheduler touch is interrupt-free.
            unsafe {
                if !ok {
                    EMIT_FAILURES = EMIT_FAILURES.max(1);
                }
                DONE = true;
                interrupts::without_interrupts(|| {
                    let scheduler = &mut *(&raw mut SCHEDULER);
                    for slot in [CONTROL_SLOT, BACKGROUND_SLOT] {
                        if let Some(task) = TASK_IDS[slot] {
                            scheduler.set_state(task, TaskState::Finished);
                        }
                    }
                });
                context::switch(&raw mut TASK_CTX[CONTROL_SLOT], &raw mut DISPATCHER_CTX);
            }
            unreachable!("a Finished task is never selected again")
        }
        Mode::Overrun => {
            // One activation, armed — and then a decision that cannot finish
            // in time. Both the deadline and the budget are about to be
            // exceeded, in that order.
            //
            // SAFETY: one bounded, interrupt-free critical section over the
            // port, as everywhere else in this file.
            unsafe {
                interrupts::without_interrupts(|| {
                    if let Some(port) = (&mut *(&raw mut PORT)).as_mut() {
                        port.arm();
                    }
                });
            }
            loop {
                // SAFETY: only this task writes these statics; `DEADLINE_MISSES`
                // is written only by the tick hook, on this same core, with
                // this task suspended in the ISR. Read through `read_volatile`
                // because the compiler cannot see that handler and would
                // otherwise be entitled to hoist the load out of this loop.
                let done = unsafe {
                    DECISION_WORK += 1;
                    if DECISION_WORK >= LOOP_CEILING {
                        DECISION_EXHAUSTED = true;
                        true
                    } else if core::ptr::read_volatile(&raw const DEADLINE_MISSES) >= 1
                        && !LATE_PROBE_DONE
                    {
                        // The window has closed and the budget has not yet run
                        // out. Present a command anyway — the port must refuse
                        // it, and the line must not move.
                        interrupts::without_interrupts(|| {
                            let scheduler = &*(&raw const SCHEDULER);
                            let journal = &mut *(&raw mut JOURNAL_STORE);
                            if let Some(port) = (&mut *(&raw mut PORT)).as_mut() {
                                LATE_PROBE_RESULT =
                                    Some(port.emit(scheduler, journal, control, 0xC0));
                                LATE_PROBE_WRITES = port.line().writes;
                            }
                        });
                        LATE_PROBE_DONE = true;
                        false
                    } else {
                        false
                    }
                };
                if done {
                    break;
                }
            }
            // Reached only if the enforcement never fired — the decision ran to
            // its own ceiling with the kernel doing nothing about it. Recorded
            // rather than assumed: the port must refuse this late command too,
            // and the difference between "prevented by enforcement" and
            // "prevented by the port alone" is exactly what clause 5 is about.
            //
            // SAFETY: as above.
            unsafe {
                LATE_EMIT_REACHED = true;
                interrupts::without_interrupts(|| {
                    let scheduler = &*(&raw const SCHEDULER);
                    let journal = &mut *(&raw mut JOURNAL_STORE);
                    if let Some(port) = (&mut *(&raw mut PORT)).as_mut() {
                        LATE_EMIT_RESULT = Some(port.emit(scheduler, journal, control, 0xFF));
                    }
                });
                DONE = true;
                interrupts::without_interrupts(|| {
                    let scheduler = &mut *(&raw mut SCHEDULER);
                    for slot in [CONTROL_SLOT, BACKGROUND_SLOT] {
                        if let Some(task) = TASK_IDS[slot] {
                            scheduler.set_state(task, TaskState::Finished);
                        }
                    }
                });
                context::switch(&raw mut TASK_CTX[CONTROL_SLOT], &raw mut DISPATCHER_CTX);
            }
            unreachable!("a Finished task is never selected again")
        }
    }
}

/// The background competitor, and the unauthorized caller identity. It touches
/// neither the port nor any other task.
extern "C" fn background_task() -> ! {
    loop {
        // SAFETY: only this task writes `BACKGROUND_COUNTER`; `DONE` is written
        // by the control task on this same core.
        let stop = unsafe {
            BACKGROUND_COUNTER += 1;
            BACKGROUND_COUNTER >= LOOP_CEILING || core::ptr::read_volatile(&raw const DONE)
        };
        if stop {
            // SAFETY: one bounded, interrupt-free critical section.
            unsafe {
                interrupts::without_interrupts(|| {
                    if let Some(task) = TASK_IDS[BACKGROUND_SLOT] {
                        (*(&raw mut SCHEDULER)).set_state(task, TaskState::Finished);
                    }
                });
                context::switch(&raw mut TASK_CTX[BACKGROUND_SLOT], &raw mut DISPATCHER_CTX);
            }
            unreachable!("a Finished task is never selected again")
        }
    }
}

/// Ends the run the moment the declared `TripToSafeState` is taken.
///
/// The system's declared safe state at Tier 0 is a reported, fail-closed stop,
/// so this reports and exits with QEMU's **failure** code — which is this
/// fixture's *correct* outcome, exactly as `broken-boot`, `idt-apic-unrouted`
/// and `wcet-trip` already establish. A safe state that returned to the
/// dispatcher would not be a safe state.
///
/// The exit code cannot distinguish a correct trip from a broken one — both are
/// failures — so the `TINYOS-RESULT/1` line is the only thing that can, and
/// every clause below is what makes it able to.
fn enter_safe_state(task: TaskId, attributed: u32) -> ! {
    // SAFETY: this function never returns, so re-initializing COM1 cannot race
    // any other user of it on this single-CPU path.
    let mut serial = unsafe { SerialPort::init() };
    let mut ok = true;

    let within_bound = attributed <= OVERRUN_BUDGET + MAX_TICKS_TO_ENFORCE;
    let after_budget = attributed > OVERRUN_BUDGET;

    // SAFETY: interrupt context on a single CPU with the run over; the
    // accounting borrow was dropped before this call.
    let (finished, writes, emitted, refused, misses, wrong_task, wrong_disposition, reached) = unsafe {
        let scheduler = &*(&raw const SCHEDULER);
        let port = (&*(&raw const PORT)).as_ref();
        (
            scheduler.state_of(task) == Some(TaskState::Finished),
            port.map_or(u32::MAX, |port| port.line().writes),
            port.map_or(u32::MAX, ActuationPort::emitted),
            port.map_or(0, ActuationPort::refused),
            DEADLINE_MISSES,
            ENFORCED_WRONG_TASK,
            WRONG_DISPOSITION,
            LATE_EMIT_REACHED,
        )
    };

    // Same guard as the clean run's, and it matters more here: this path
    // reports its verdict from inside an ISR running on the offender's own
    // stack, which is the stack most likely to have grown.
    //
    // SAFETY: interrupt context on a single CPU with the run over.
    let canaries_intact = unsafe {
        (0..TASKS)
            .all(|slot| (&raw const TASK_STACKS[slot]).cast::<u64>().read_unaligned() == CANARY)
    };
    check(
        &mut serial,
        &mut ok,
        "clause 8: both task stacks are intact — a stack that overflowed into .bss makes every \
         number in this run a fabrication, not a measurement",
        canaries_intact,
    );

    // **The claim.** Nothing reached the actuator line, on the fixture's own
    // independent count, and the port agrees.
    check(&mut serial, &mut ok, "clause 5: no command reached the actuator line", writes == 0);
    check(&mut serial, &mut ok, "clause 5: the port emitted nothing", emitted == 0);
    check(
        &mut serial,
        &mut ok,
        "clause 5: the enforcement fired within budget+1 attributed ticks",
        within_bound && after_budget,
    );
    check(&mut serial, &mut ok, "clause 5: the enforcement named the control task", !wrong_task);
    check(
        &mut serial,
        &mut ok,
        "clause 5: the disposition was the declared TripToSafeState",
        !wrong_disposition,
    );
    check(
        &mut serial,
        &mut ok,
        "clause 5: the kernel left the offender Finished — the trip is a state change, not a \
         returned value the fixture acted on",
        finished,
    );
    check(
        &mut serial,
        &mut ok,
        "clause 5: the decision never finished, so the enforcement is what stopped it",
        !reached,
    );
    check(
        &mut serial,
        &mut ok,
        "clause 5: the deadline monitor saw the window close before the budget ran out",
        misses >= 1,
    );

    // Clause 4, at Tier 0: a command presented *after* the window closed and
    // *before* the trip — by the owner, while it was still running — is
    // refused, and the line does not move. Without this the prevention claim
    // would rest on the offender never reaching an emit at all.
    //
    // SAFETY: interrupt context on a single CPU with the run over.
    let (probe_done, probe_result, probe_writes) =
        unsafe { (LATE_PROBE_DONE, LATE_PROBE_RESULT, LATE_PROBE_WRITES) };
    check(
        &mut serial,
        &mut ok,
        "clause 4: the late-emit probe ran — the decision was still on the CPU after the \
         deadline closed, so the refusal was actually exercised",
        probe_done,
    );
    check(
        &mut serial,
        &mut ok,
        "clause 4: the late command was REFUSED as DeadlineMissed, by the owner's own identity, \
         while the owner was still running — prevented, not merely unreachable",
        probe_result == Some(Err(ActuationError::DeadlineMissed)),
    );
    check(
        &mut serial,
        &mut ok,
        "clause 4: the late command did not reach the line",
        probe_writes == 0,
    );

    // The last emit attempt, made from the safe-state path with the owner's own
    // identity: refused, because the owner is no longer `Running`. Prevention
    // does not rest on the task never being scheduled again.
    //
    // SAFETY: as above. The port is left in whatever state this leaves it; the
    // machine stops on the next line but one.
    let last = unsafe {
        let scheduler = &*(&raw const SCHEDULER);
        let journal = &mut *(&raw mut JOURNAL_STORE);
        (&mut *(&raw mut PORT))
            .as_mut()
            .map(|port| (port.emit(scheduler, journal, task, 0xEE), port.line().writes))
    };
    check(
        &mut serial,
        &mut ok,
        "clause 5: a final emit with the OWNER's identity is still refused after the trip",
        last == Some((Err(ActuationError::NotAuthorized), 0)),
    );

    let _ = writeln!(
        serial,
        "fixture-{NAME}: TRIP task={} attributed_ticks={attributed} budget={OVERRUN_BUDGET} \
         (bound {}) finished={finished} within_bound={within_bound}",
        task.index(),
        OVERRUN_BUDGET + MAX_TICKS_TO_ENFORCE
    );
    let _ = writeln!(
        serial,
        "fixture-{NAME}: line_writes={writes} emitted={emitted} refused={refused} \
         deadline_misses={misses} deadline={DEADLINE} decision_finished={reached} \
         last_emit={last:?} late_probe={probe_result:?} probe_writes={probe_writes} \
         probe_ran={probe_done}"
    );
    let _ = writeln!(
        serial,
        "fixture-{NAME}: entering declared safe state — fail-closed stop is this fixture's pass \
         condition, and the RESULT line above is what distinguishes a correct trip from a broken one"
    );
    let _ = write_result(&mut serial, NAME, ok);
    exit_qemu(QemuExitCode::Failure)
}

/// The timer-tick consumer. Runs in interrupt context on the interrupted task's
/// own stack with `IF` clear. Bounded and allocation-free throughout.
///
/// **The deadline monitor runs before the budget enforcement**, and the order
/// is a decision rather than an accident: the monitor *observes* (it takes no
/// scheduler borrow at all), the enforcement *acts*. Observing first means the
/// window's state is already correct on the tick the trip is taken, so the
/// safe-state report can say whether the deadline had closed — which, on the
/// run where both fire, is the difference between two mechanisms agreeing and
/// one of them being invisible.
extern "C" fn on_tick() {
    // Checked first, before the scheduler is touched at all: the dispatcher
    // legitimately holds a `&mut Scheduler` and a tick already in flight when
    // it cleared `IF` can still land here.
    //
    // SAFETY: written only by the dispatcher, with interrupts disabled.
    let Some(slot) = (unsafe { CURRENT_TASK }) else {
        return;
    };

    // SAFETY: a task is running, so the dispatcher holds no borrow, and task
    // code touches the port only inside `without_interrupts` — so this is the
    // only code touching any of this state for the duration.
    let trip = unsafe {
        TICKS_SEEN += 1;

        let journal = &mut *(&raw mut JOURNAL_STORE);
        if let Some(port) = (&mut *(&raw mut PORT)).as_mut() {
            // Counted on the *transition*, so this is one per activation rather
            // than one per tick spent past the deadline — the same distinction
            // the monitor's own single stamp draws.
            let before = port.status();
            let after = port.on_tick(journal);
            if after == DeadlineStatus::Missed && before != DeadlineStatus::Missed {
                DEADLINE_MISSES += 1;
            }
        }

        let scheduler = &mut *(&raw mut SCHEDULER);
        let running = task_id(scheduler, slot);
        match wcet::account_tick(scheduler, journal, running) {
            TickAccounting::UnknownTask => {
                TICKS_UNKNOWN += 1;
                None
            }
            TickAccounting::Unattributed => None,
            TickAccounting::WithinBudget(task) => {
                if task.index() == CONTROL_SLOT {
                    TICKS_ATTRIBUTED_CONTROL += 1;
                }
                None
            }
            TickAccounting::Enforced { task, disposition } => {
                if task.index() == CONTROL_SLOT {
                    TICKS_ATTRIBUTED_CONTROL += 1;
                } else {
                    ENFORCED_WRONG_TASK = true;
                }
                ENFORCEMENTS += 1;
                // Checked against what the task *declared*, not against
                // whatever came back — otherwise this would assert only that
                // the enumeration round-trips.
                if disposition != OverrunDisposition::TripToSafeState {
                    WRONG_DISPOSITION = true;
                }
                if ENFORCEMENTS == 1 {
                    TICKS_AT_FIRST_ENFORCE = TICKS_ATTRIBUTED_CONTROL;
                }
                Some((task, TICKS_AT_FIRST_ENFORCE))
            }
        }
    };

    if let Some((task, attributed)) = trip {
        // The declared safe state. Every task in this fixture declares
        // `TripToSafeState`, so there is one arm and it does not return.
        enter_safe_state(task, attributed);
    }

    // The preemption decision, unchanged from `STORY-P1-04-01`. Enforcement,
    // the deadline monitor and preemption share this hook, and none may perturb
    // the others.
    //
    // SAFETY: `slot` is the task this interrupt is executing on, so its context
    // slot is its own; `DISPATCHER_CTX` is suspended at the dispatcher's own
    // `run_once` call site. Every borrow above was dropped.
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
        unsafe { PREEMPTIONS += 1 };
    }
}

/// # Safety
/// `slot` must be the next unused scheduler slot, with an unused stack.
unsafe fn create(
    slot: usize,
    priority_value: u8,
    budget: u32,
    entry: extern "C" fn() -> !,
) -> Option<TaskId> {
    // SAFETY: per this function's own contract.
    unsafe {
        let scheduler = &mut *(&raw mut SCHEDULER);
        let task = scheduler
            .create_task(
                priority(priority_value)?,
                WcetBudgetTicks(budget),
                // Every task in this fixture declares the same policy: the
                // control task because tripping is the claim, the background
                // task because a task with a budget and no declared consequence
                // cannot exist (`STORY-P1-04-02` criterion 2).
                OverrunPolicy::TripToSafeState,
                entry,
            )
            .ok()?;
        if task.index() != slot {
            return None;
        }
        let stack =
            core::slice::from_raw_parts_mut((&raw mut TASK_STACKS[slot]).cast::<u8>(), STACK_SIZE);
        TASK_CTX[slot] = Context::new(stack, entry).ok()?;
        // The canary goes in *after* `Context::new`, which builds its initial
        // frame at the top of the slice and never touches the bottom.
        (&raw mut TASK_STACKS[slot]).cast::<u64>().write_unaligned(CANARY);
        TASK_IDS[slot] = Some(task);
        Some(task)
    }
}

/// Records a failed clause by name rather than folding it into a bare `false`.
/// `TEST-P1-06-01-A` clause 7 asks for this specifically: a fixture whose only
/// output on failure is a non-zero exit is the `LE-46` shape — armed to detect
/// and not to explain — and for `actuation-overrun`, whose *pass* condition is
/// already a non-zero exit, it is the only diagnostic that exists at all.
fn check(serial: &mut SerialPort, ok: &mut bool, clause: &str, condition: bool) {
    if !condition {
        let _ = writeln!(serial, "fixture-{NAME}: FAILED {clause}");
        *ok = false;
    }
}

/// One measured phase, held until both have run — `fixture_measure`'s own
/// pattern, for its own reason: a fixture that dies mid-measurement then
/// produces no envelope at all rather than a half-open one, which `xtask`
/// rejects as truncated instead of silently reading a short report.
struct Measured {
    domain: &'static str,
    name: &'static str,
    summary: Summary,
}

/// Writes the whole envelope from the collected summaries.
fn emit_all<W: Write>(
    sink: &mut W,
    environment: &Environment<'_>,
    collected: &[Option<Measured>; METRICS],
) -> Option<usize> {
    let mut report = Report::begin(sink, environment).ok()?;
    for measured in collected.iter().flatten() {
        report
            .metric(&Metric {
                domain: measured.domain,
                name: measured.name,
                warmup: WARMUP,
                summary: measured.summary,
            })
            .ok()?;
    }
    report.end().ok()
}

/// Runs the fixture.
pub fn run() -> bool {
    // SAFETY: this fixture is the only code running; `init` is called once,
    // before any other `SerialPort` method.
    let mut serial = unsafe { SerialPort::init() };
    let mut ok = true;

    let control_budget = match MODE {
        Mode::Clean => GENEROUS_BUDGET,
        Mode::Overrun => OVERRUN_BUDGET,
    };

    // SAFETY: single-CPU fixture, each slot used exactly once.
    let control = unsafe {
        let control = create(CONTROL_SLOT, CONTROL_PRIORITY, control_budget, control_task);
        let background =
            create(BACKGROUND_SLOT, BACKGROUND_PRIORITY, GENEROUS_BUDGET, background_task);
        match (control, background) {
            (Some(control), Some(_)) => control,
            _ => {
                let _ = writeln!(serial, "fixture-{NAME}: task creation failed");
                return false;
            }
        }
    };

    // The port is declared over the control task and nothing else, before any
    // task runs. There is no later call that could re-point it.
    //
    // SAFETY: written once, here, before interrupts are armed.
    unsafe {
        PORT = Some(ActuationPort::declare(
            CountingLine { inner: PortLine, writes: 0, last: None },
            control,
            DeadlineTicks(DEADLINE),
        ));
    }

    // Calibrated **before** the APIC timer is armed: `calibrate_cycles_per_us`
    // documents that no timer may be armed on this path, and an interrupt
    // landing inside the PIT gate would inflate the factor.
    //
    // SAFETY: nothing else in this fixture uses PIT channel 2 or port 0x61, and
    // no timer is armed yet — the function's own documented contract.
    let timebase = unsafe { tsc::calibrate_cycles_per_us() };
    // SAFETY: written once, before any phase reads it.
    unsafe { CALIBRATION = Calibration::measure(&Tsc, CALIBRATION_SAMPLES) };

    // SAFETY: registered before interrupts are armed, so no tick can arrive
    // between arming and installation. `on_tick` is bounded, allocation-free,
    // and leaves the interrupt frame intact.
    unsafe { interrupts::set_tick_hook(on_tick) };
    // SAFETY: called exactly once, before anything depends on interrupts being
    // armed. Ends with `sti`.
    unsafe { interrupts::init(INITIAL_COUNT) };
    // From here the dispatcher runs with `IF` clear and never re-enables it: a
    // task's own saved `RFLAGS` is what turns interrupts back on across the
    // switch into it.
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

    // The overrun run never reaches here: `enter_safe_state` does not return.
    // Arriving here at all means the system did *not* stop, which is the
    // failure that run exists to rule out.
    if MODE == Mode::Overrun {
        // SAFETY: interrupts masked, run over.
        let (reached, result, work, exhausted, enforcements) = unsafe {
            (LATE_EMIT_REACHED, LATE_EMIT_RESULT, DECISION_WORK, DECISION_EXHAUSTED, ENFORCEMENTS)
        };
        let _ = writeln!(
            serial,
            "fixture-{NAME}: FAILED clause 5: the deliberately-overrunning decision was never \
             stopped — enforcements={enforcements} work={work} exhausted={exhausted} \
             late_emit={result:?} (reached={reached})"
        );
        let _ = write_result(&mut serial, NAME, false);
        return false;
    }

    // SAFETY: interrupts masked, every switch has returned.
    let (
        ticks,
        unknown,
        preemptions,
        misses,
        enforcements,
        emit_failures,
        denial_failures,
        emit_done,
        denial_done,
        writes_before,
        writes_after,
        background,
    ) = unsafe {
        (
            TICKS_SEEN,
            TICKS_UNKNOWN,
            PREEMPTIONS,
            DEADLINE_MISSES,
            ENFORCEMENTS,
            EMIT_FAILURES,
            DENIAL_FAILURES,
            EMIT_PHASE_DONE,
            DENIAL_PHASE_DONE,
            WRITES_BEFORE_DENIAL,
            WRITES_AFTER_DENIAL,
            BACKGROUND_COUNTER,
        )
    };
    // SAFETY: as above.
    let (writes, emitted, refused, last, status) = unsafe {
        let port = (&*(&raw const PORT)).as_ref();
        (
            port.map_or(0, |port| port.line().writes),
            port.map_or(0, ActuationPort::emitted),
            port.map_or(0, ActuationPort::refused),
            port.and_then(|port| port.line().last),
            port.map_or(DeadlineStatus::Idle, ActuationPort::status),
        )
    };

    // Before any number below is read as evidence: did either stack grow into
    // whatever lies beneath it? If so, nothing here is a measurement.
    //
    // SAFETY: interrupts masked, every switch has returned.
    let canaries_intact = unsafe {
        (0..TASKS)
            .all(|slot| (&raw const TASK_STACKS[slot]).cast::<u64>().read_unaligned() == CANARY)
    };
    check(
        &mut serial,
        &mut ok,
        "clause 8: both task stacks are intact — a stack that overflowed into .bss makes every \
         number in this run a fabrication, not a measurement",
        canaries_intact,
    );

    let expected = (WARMUP + SAMPLES) as u32;

    // Clause 1: the path ran, end to end, for every sampled iteration.
    check(&mut serial, &mut ok, "clause 1: the emit phase completed", emit_done);
    check(&mut serial, &mut ok, "clause 1: no emit was refused", emit_failures == 0);
    check(
        &mut serial,
        &mut ok,
        "clause 1: every measured iteration reached the line — the fixture's own write count \
         and the port's emit count agree, and both equal the iteration count",
        writes == expected && emitted == expected,
    );
    check(&mut serial, &mut ok, "clause 1: the last command word was recorded", last.is_some());
    check(
        &mut serial,
        &mut ok,
        "clause 1: real timer ticks arrived, so the deadline monitor was live throughout",
        ticks >= 1,
    );
    check(&mut serial, &mut ok, "clause 1: no tick was charged to an unknown task", unknown == 0);
    check(
        &mut serial,
        &mut ok,
        "clause 1: no enforcement fired in the clean run — a generous budget must not trip",
        enforcements == 0,
    );
    check(
        &mut serial,
        &mut ok,
        "clause 1: no deadline was missed in the clean run",
        misses == 0 && status != DeadlineStatus::Missed,
    );

    // Clause 2: the ambient path does not exist.
    check(&mut serial, &mut ok, "clause 2: the denial phase completed", denial_done);
    check(
        &mut serial,
        &mut ok,
        "clause 2: every unauthorized emit was refused with NotAuthorized",
        denial_failures == 0,
    );
    check(
        &mut serial,
        &mut ok,
        "clause 2: the line was never written during the denial phase — an unauthorized \
         identity does not reach the actuator",
        writes_after == writes_before,
    );
    check(&mut serial, &mut ok, "clause 2: every refusal was counted", refused == expected);

    // The envelope. Two metrics, in the order they were measured — each
    // summarized by its own phase, over its own population.
    //
    // SAFETY: interrupts masked, both phases finished.
    let (emit_summary, denial_summary) = unsafe { (EMIT_SUMMARY, DENIAL_SUMMARY) };
    let collected: [Option<Measured>; METRICS] = [
        emit_summary.map(|summary| Measured {
            domain: "D03",
            name: "decision_to_actuation_emit",
            summary,
        }),
        denial_summary.map(|summary| Measured {
            domain: "D03",
            name: "actuation_refused_unauthorized",
            summary,
        }),
    ];
    let environment = Environment {
        tier: "T0",
        arch: "x86_64",
        platform: "qemu-tcg-x86_64",
        qualification: kernel::measure::UNQUALIFIED,
        cycle_source: "rdtsc",
        overhead_cycles: unsafe { (&*(&raw const CALIBRATION)).overhead_cycles() },
        cycles_per_us: timebase.cycles_per_us(),
    };
    let Some(metrics) = emit_all(&mut serial, &environment, &collected) else {
        let _ = write_result(&mut serial, NAME, false);
        return false;
    };

    let _ = writeln!(
        serial,
        "fixture-{NAME}: iterations={expected} line_writes={writes} emitted={emitted} \
         refused={refused} last_command={last:?} emit_failures={emit_failures} \
         denial_failures={denial_failures}"
    );
    let _ = writeln!(
        serial,
        "fixture-{NAME}: denial_phase_writes before={writes_before} after={writes_after} \
         (equal in a passing run)"
    );
    let _ = writeln!(
        serial,
        "fixture-{NAME}: ticks={ticks} unknown={unknown} preemptions={preemptions} \
         enforcements={enforcements} deadline={DEADLINE} deadline_misses={misses} \
         status={status:?} background={background}"
    );
    let _ = writeln!(
        serial,
        "fixture-{NAME}: metrics={metrics} — Tier 0 mechanism evidence only; the WCET bound is \
         stated debt against LE-09 (ADR 0005: zero platforms hold a qualification record)"
    );

    let verdict = ok && metrics == METRICS;
    let _ = write_result(&mut serial, NAME, verdict);
    verdict
}
