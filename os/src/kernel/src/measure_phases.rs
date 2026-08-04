//! The measured phases `fixture-measure` drives, extracted from the x86_64
//! fixture binary so the AArch64 boot image (`STORY-P1-07-06`) runs the
//! *same* workloads through the *same* harness rather than a re-typed copy —
//! the arch-neutrality claim `STORY-P1-01-03` made, finally load-bearing.
//!
//! Everything here is generic over [`CycleSource`]; nothing names an
//! architecture. The two architecture-specific phases stay with their
//! fixtures: the D02 fault-latency phase (x86_64 raises `#UD`, AArch64 a
//! `BRK`) and the envelope's sink (COM1 there, PL011/canvas/wire here).
//!
//! **Per-phase `#[inline(never)]` functions, not one `run` body** — see
//! `fixture_measure.rs`'s doc comment for the stack-walking failure that
//! rule was learned from. The statics exist because a [`Context`] and its
//! stack must never move once a `switch` has been taken into them.

use crate::context::{self, Context};
use crate::dispatch;
use crate::measure::{Calibration, Samples, Stopwatch};
use crate::mem::Pool;
use crate::sched::{OverrunPolicy, Priority, Scheduler, TaskState, WcetBudgetTicks};
use hal::time::CycleSource;

/// Every phase's sample capacity — one shared buffer, cleared between
/// phases, kept in the fixture that owns the run.
pub const SAMPLES: usize = 1_000;

/// Unmeasured iterations before sampling starts, reported as `warmup=`.
pub const WARMUP: usize = 100;

/// Read pairs used to calibrate the cycle source's own overhead.
pub const CALIBRATION_SAMPLES: usize = 2_000;

/// Reads taken by the shared conformance suite against the live source.
pub const CONFORMANCE_SAMPLES: usize = 64;

/// Task stack size for the D04/D05 phases.
pub const STACK_SIZE: usize = 4_096;

/// Scheduler capacity for the D05 dispatch phase.
pub const TASKS: usize = 4;

/// Iterations inside one timed region of [`phase_reference_loop`]. **Part of
/// the reference's definition** — see that function before changing it.
pub const REFERENCE_ITERATIONS: usize = 64;

/// Operations timed inside one sample of the D07 denial phase (`LE-24`) —
/// see the x86_64 fixture's original doc for the calibration-residue defect
/// this corrects and for why the *unbatched* round trip was left alone.
pub const D07_BATCH: usize = 64;

/// Operations per timed region of the **batched** round-trip twin
/// (`STORY-P1-07-06`, the shape that closes `LE-24`). Eight, not 64: the
/// recorded 607-vs-58 superlinearity finding indicts the large batch, and
/// eight keeps the timed region ~8× the calibration pair while staying near
/// the regime where the batch-of-1 cost is known.
pub const ROUND_TRIP_BATCH: usize = 8;

// D04/D05 task state. Statics rather than locals: a `Context` and its stack
// must never move once a `switch` has been taken into them.
static mut MEASURE_CTX: Context = Context::zeroed();
static mut TASK_CTX: Context = Context::zeroed();
static mut TASK_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
static mut YIELDS: u64 = 0;

static mut DISPATCHER_CTX: Context = Context::zeroed();
static mut DISPATCH_CONTEXTS: [Context; TASKS] = [Context::zeroed(); TASKS];
static mut DISPATCH_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

/// D04's measured task: yields straight back to whoever resumed it.
extern "C" fn yield_forever() -> ! {
    loop {
        // SAFETY: single-CPU fixture; only this task writes `YIELDS`, and
        // `TASK_CTX`/`MEASURE_CTX` are switched strictly alternately.
        unsafe {
            YIELDS += 1;
            context::switch(&raw mut TASK_CTX, &raw mut MEASURE_CTX);
        }
    }
}

/// D05's dispatched task: yields back to the dispatcher context.
extern "C" fn dispatch_yield_forever() -> ! {
    loop {
        // SAFETY: see `yield_forever`; slot 0 is the only context this
        // phase ever initializes or switches into.
        unsafe {
            context::switch(&raw mut DISPATCH_CONTEXTS[0], &raw mut DISPATCHER_CTX);
        }
    }
}

/// The measurement reference (`STORY-P1-01-04`).
///
/// # Do not change this function
///
/// The timing gate compares every metric's **same-run ratio to this loop**.
/// Editing its body silently re-points every committed baseline ratio in
/// `goals/performance/baselines/`, and the gate cannot tell that from a
/// regression in everything at once. If it genuinely has to change,
/// re-record the baselines in the same commit and say so. The per-iteration
/// [`core::hint::black_box`] is load-bearing: without it the release profile
/// closed-forms the recurrence and the reference measured 16 cycles.
#[inline(never)]
pub fn phase_reference_loop<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    let mut last: u64 = 0;
    for index in 0..(WARMUP + SAMPLES) {
        let watch = Stopwatch::start(source);
        let mut accumulator = core::hint::black_box(0x9E37_79B9_7F4A_7C15u64);
        for step in 0..REFERENCE_ITERATIONS {
            accumulator = core::hint::black_box(
                accumulator.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(step as u64 | 1),
            );
        }
        let cycles = watch.stop(calibration);
        if index >= WARMUP {
            samples.record(cycles);
        }
        last = accumulator;
    }
    last != 0
}

/// D07: `Pool<u64, 64>` alloc/free round trip. **Unbatched, deliberately** —
/// see [`D07_BATCH`].
#[inline(never)]
pub fn phase_pool_alloc_free<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    let mut pool: Pool<u64, 64> = Pool::new();
    let mut ok = true;
    for index in 0..(WARMUP + SAMPLES) {
        let value = index as u64;
        let watch = Stopwatch::start(source);
        let handle = match pool.alloc(value) {
            Ok(handle) => handle,
            Err(_) => {
                ok = false;
                continue;
            }
        };
        let freed = pool.free(handle);
        let cycles = watch.stop(calibration);
        if freed != Ok(value) {
            ok = false;
        }
        if index >= WARMUP {
            samples.record(cycles);
        }
    }
    ok
}

/// D07, the batched round-trip twin (`LE-24`): the same alloc/free pair,
/// timed [`ROUND_TRIP_BATCH`] operations per sample and divided — the
/// host-independent shape `LE-24`'s register row asks for by name.
#[inline(never)]
pub fn phase_pool_alloc_free_batched<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    let mut pool: Pool<u64, 64> = Pool::new();
    let mut ok = true;
    for index in 0..(WARMUP + SAMPLES) {
        let value = index as u64;
        let mut round_trips = 0usize;
        let watch = Stopwatch::start(source);
        for _ in 0..ROUND_TRIP_BATCH {
            let Ok(handle) = pool.alloc(value) else {
                break;
            };
            if pool.free(handle) == Ok(value) {
                round_trips += 1;
            }
        }
        let cycles = watch.stop(calibration);
        if round_trips != ROUND_TRIP_BATCH {
            ok = false;
        }
        if index >= WARMUP {
            samples.record(cycles / ROUND_TRIP_BATCH as u64);
        }
    }
    ok
}

/// D07: the denial path over an exhausted `Pool<u64, 4>`, batched
/// [`D07_BATCH`] operations per sample.
#[inline(never)]
pub fn phase_pool_denial<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    let mut pool: Pool<u64, 4> = Pool::new();
    for index in 0..4 {
        if pool.alloc(index as u64).is_err() {
            return false;
        }
    }
    let mut ok = true;
    for index in 0..(WARMUP + SAMPLES) {
        let mut denials = 0usize;
        let watch = Stopwatch::start(source);
        for _ in 0..D07_BATCH {
            if pool.alloc(0xDEAD).is_err() {
                denials += 1;
            }
        }
        let cycles = watch.stop(calibration);
        if denials != D07_BATCH {
            ok = false;
        }
        if index >= WARMUP {
            samples.record(cycles / D07_BATCH as u64);
        }
    }
    let mut occupied = 0;
    for (handle, value) in pool.iter_occupied() {
        occupied += 1;
        if *value != handle.index() as u64 {
            ok = false;
        }
    }
    ok && occupied == 4
}

/// D04: one `context::switch` out to a task that immediately switches back —
/// two switches per sample, as the metric name states.
#[inline(never)]
pub fn phase_context_switch<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    // SAFETY: this fixture is the only code running; `TASK_STACK` is used by
    // exactly one `Context` for the whole phase and never moves, and
    // `TASK_CTX`/`MEASURE_CTX` are switched strictly alternately below.
    unsafe {
        let stack = core::slice::from_raw_parts_mut((&raw mut TASK_STACK).cast::<u8>(), STACK_SIZE);
        let Ok(task) = Context::new(stack, yield_forever) else {
            return false;
        };
        TASK_CTX = task;
        YIELDS = 0;
    }

    for index in 0..(WARMUP + SAMPLES) {
        let watch = Stopwatch::start(source);
        // SAFETY: `TASK_CTX` was initialized above and is suspended at its
        // entry point or its own `switch` call site; `MEASURE_CTX` is this
        // context's own slot. Exactly `switch`'s documented contract.
        unsafe { context::switch(&raw mut MEASURE_CTX, &raw mut TASK_CTX) };
        let cycles = watch.stop(calibration);
        if index >= WARMUP {
            samples.record(cycles);
        }
    }

    // SAFETY: read after every switch above has returned; single-CPU.
    let yields = unsafe { YIELDS };
    yields == (WARMUP + SAMPLES) as u64
}

/// D05: ready-queue selection alone — no switch in the timed region.
#[inline(never)]
pub fn phase_dispatch_select<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    let mut scheduler: Scheduler<TASKS> = Scheduler::new();
    let mut created = 0;
    for priority in [3u8, 9, 17, 25] {
        let Ok(priority) = Priority::try_new(priority) else {
            return false;
        };
        if scheduler
            .create_task(
                priority,
                WcetBudgetTicks(1_000),
                OverrunPolicy::TripToSafeState,
                dispatch_yield_forever,
            )
            .is_ok()
        {
            created += 1;
        }
    }
    if created != TASKS {
        return false;
    }

    let mut ok = true;
    for index in 0..(WARMUP + SAMPLES) {
        let watch = Stopwatch::start(source);
        let selected = scheduler.highest_priority_ready();
        // The deliberate regression (`fixture-measure-regression`, never in a
        // real image): seven extra selections, so the gate has something
        // real to catch.
        #[cfg(feature = "fixture-measure-regression")]
        let selected = {
            let mut last = selected;
            for _ in 0..7 {
                last = scheduler.highest_priority_ready();
            }
            last
        };
        let cycles = watch.stop(calibration);
        if selected.map(|task| task.index()) != Some(3) {
            ok = false;
        }
        if index >= WARMUP {
            samples.record(cycles);
        }
    }
    ok
}

/// D05: a whole cooperative dispatch round — select, switch in, the task
/// yields, book-keep back to Ready.
#[inline(never)]
pub fn phase_dispatch_round<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    let mut scheduler: Scheduler<TASKS> = Scheduler::new();
    let Ok(priority) = Priority::try_new(11) else {
        return false;
    };
    let Ok(task) = scheduler.create_task(
        priority,
        WcetBudgetTicks(1_000),
        OverrunPolicy::TripToSafeState,
        dispatch_yield_forever,
    ) else {
        return false;
    };
    if task.index() != 0 {
        return false;
    }

    // SAFETY: slot 0 is the only context this phase initializes or switches
    // into; `DISPATCH_STACK` is a never-moving static owned solely by it.
    unsafe {
        let stack =
            core::slice::from_raw_parts_mut((&raw mut DISPATCH_STACK).cast::<u8>(), STACK_SIZE);
        let Ok(context) = Context::new(stack, dispatch_yield_forever) else {
            return false;
        };
        DISPATCH_CONTEXTS[0] = context;
    }

    let mut ok = true;
    for index in 0..(WARMUP + SAMPLES) {
        let watch = Stopwatch::start(source);
        // SAFETY: `DISPATCH_CONTEXTS[0]` was initialized above and is
        // suspended at its entry point or its own `switch` call site;
        // `DISPATCHER_CTX` is this context's own slot — `run_once`'s
        // documented contract.
        let ran = unsafe {
            dispatch::run_once(&mut scheduler, &raw mut DISPATCHER_CTX, &raw mut DISPATCH_CONTEXTS)
        };
        let cycles = watch.stop(calibration);
        if ran != Some(task) || scheduler.state_of(task) != Some(TaskState::Ready) {
            ok = false;
        }
        if index >= WARMUP {
            samples.record(cycles);
        }
    }
    ok
}
