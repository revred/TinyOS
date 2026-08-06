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
use crate::spoor::Outcome;
use crate::spoor_stream::{Rung, SpoorStream, ANNOUNCE_EVERY, BOARD_STREAM_CAPACITY};
use crate::spoor_wire::MAX_PAYLOAD;
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

/// `PERF-D07-G23`'s **spoor-enabled arm** — byte-for-byte
/// [`phase_pool_alloc_free_batched`] with one stamp per round trip.
///
/// # Why this phase exists, and why it is a *pair*
///
/// `G23` reads *"spoor enabled adds <= 2% p99 and <= 2% CPU cycles;
/// allocations = 0; torn records = 0"* — and every spoor number this project
/// has ever measured is **absolute**: stamp 136 cycles, announce 3099, drain
/// 122005. A percentage needs two arms and there was only ever one, so the
/// gate could not be computed from any measurement taken, however precise.
/// That is handover `09A` §5's general finding in its sharpest instance: *a
/// measurement taken without first reading the gate's `target` column is a
/// measurement that will need retaking.* This is the retake, done as a pair so
/// the units come out right the first time.
///
/// **The disabled arm is [`phase_pool_alloc_free_batched`] and nothing else.**
/// Not a copy — the same function, already committed, already carrying `D07`'s
/// `G01`/`G02`/`G03` rows. Two arms that share a loop shape but not a loop
/// cannot be differenced honestly, because any edit to one silently changes
/// the ratio. The body below is that function's body with exactly one line
/// added, and [`tests::the_two_g23_arms_differ_by_exactly_the_stamp`] is what
/// holds that claim to something.
///
/// **One stamp per round trip is the deliberate worst case.** Real
/// instrumentation density is lower, so a ratio computed from these two arms
/// over-states the overhead rather than flattering it — which is the direction
/// a safety gate should err in, and must be said with the number rather than
/// after it.
#[inline(never)]
pub fn phase_pool_alloc_free_batched_spoored<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    let mut pool: Pool<u64, 64> = Pool::new();
    let mut stream: SpoorStream<BOARD_STREAM_CAPACITY> = SpoorStream::new();
    stream.seed_epoch(0x5EED_0000_0000_0001);
    // Close the certificate before the timed region, exactly as
    // `phase_spoor_stamp` does: the once-per-boot retain path is not what any
    // steady-state ratio should be carrying.
    stream.stamp(Rung::ParkIteration, Outcome::Ok, 0);
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
            stream.stamp(Rung::ParkIteration, Outcome::Ok, 0);
        }
        let cycles = watch.stop(calibration);
        if round_trips != ROUND_TRIP_BATCH {
            ok = false;
        }
        if index >= WARMUP {
            samples.record(cycles / ROUND_TRIP_BATCH as u64);
        }
    }
    // Every stamp accounted: one closing the certificate, then one per round
    // trip. A disagreeing sequence means the arm measured something other than
    // the workload plus exactly one stamp per operation, which would make the
    // ratio meaningless rather than merely imprecise.
    ok && stream.next_sequence() == (1 + (WARMUP + SAMPLES) * ROUND_TRIP_BATCH) as u64
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

/// Stamps timed per sample of [`phase_spoor_stamp`], divided out — the same
/// eight as [`ROUND_TRIP_BATCH`], for the same reason: a single stamp is a
/// ring append and costs less than the calibrated subtraction resolves, so an
/// unbatched figure would be mostly residue (`LE-24`'s lesson, applied before
/// the number is ever quoted rather than after).
pub const SPOOR_STAMP_BATCH: usize = 8;

/// `STORY-P1-10-02` criterion 6: the per-stamp cost of the spoor substrate,
/// measured through the same harness as everything else instead of asserted.
///
/// Steady state, deliberately: the certificate is closed by a first untimed
/// stamp so the once-per-boot retain path is not what gets measured, and the
/// ring is allowed to wrap because on a running board it does. The timed
/// region is [`SpoorStream::stamp`] alone — what a call site pays at the
/// moment it stamps, with no drain and no wire in it.
#[inline(never)]
pub fn phase_spoor_stamp<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    let mut stream: SpoorStream<BOARD_STREAM_CAPACITY> = SpoorStream::new();
    stream.seed_epoch(0x5EED_0000_0000_0001);
    // Close the certificate: a park rung is not a boot rung, so from here on
    // every stamp takes the steady-state path the park loop pays.
    stream.stamp(Rung::ParkIteration, Outcome::Ok, 0);
    for index in 0..(WARMUP + SAMPLES) {
        let watch = Stopwatch::start(source);
        for _ in 0..SPOOR_STAMP_BATCH {
            stream.stamp(Rung::ParkIteration, Outcome::Ok, 0);
        }
        let cycles = watch.stop(calibration);
        if index >= WARMUP {
            samples.record(cycles / SPOOR_STAMP_BATCH as u64);
        }
    }
    // The stream must have accounted every stamp: one to close the
    // certificate, then the batches — a sequence that disagrees means the
    // phase measured something other than what it claims.
    stream.next_sequence() == (1 + (WARMUP + SAMPLES) * SPOOR_STAMP_BATCH) as u64
}

/// `STORY-P1-10-02` criterion 6: the per-drain cost, at the drain's bounded
/// worst case — a full ring of [`BOARD_STREAM_CAPACITY`] records packed into
/// one maximum-size frame.
///
/// The timed region is [`SpoorStream::drain`] alone: journal walk, record
/// packing and header encode into a RAM buffer. **The GEM transmit is not in
/// it** — what the wire costs is `hal-arm64`'s to measure and is not claimed
/// here. Worst case rather than steady state because the park loop's budget
/// has to survive the worst drain, and a one-record average would understate
/// exactly the pass that matters.
#[inline(never)]
pub fn phase_spoor_drain<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    let mut stream: SpoorStream<BOARD_STREAM_CAPACITY> = SpoorStream::new();
    stream.seed_epoch(0x5EED_0000_0000_0001);
    let mut frame = [0u8; MAX_PAYLOAD];
    let mut ok = true;
    for index in 0..(WARMUP + SAMPLES) {
        for _ in 0..BOARD_STREAM_CAPACITY {
            stream.stamp(Rung::ParkIteration, Outcome::Ok, 0);
        }
        let watch = Stopwatch::start(source);
        let drained = stream.drain(&mut frame);
        let cycles = watch.stop(calibration);
        // A full ring must produce exactly the maximum frame; anything else
        // means the phase did not measure the worst case it names.
        if drained != Ok(Some(MAX_PAYLOAD)) {
            ok = false;
        }
        if index >= WARMUP {
            samples.record(cycles);
        }
    }
    ok
}

/// `STORY-P1-10-04`'s follow-on cost (`STORY-P1-10-02` criterion 6's third
/// number): re-announcing the retained boot certificate.
///
/// The stream carries the real boot prologue — the three rungs every board
/// verdict since 10 has shown — and only the **emitting** call is timed; the
/// [`ANNOUNCE_EVERY`]` - 1` refusals between announcements are walked
/// untimed, because the park loop pays them too but they are a counter
/// decrement, not the cost this metric names.
#[inline(never)]
pub fn phase_spoor_announce<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    let mut stream: SpoorStream<BOARD_STREAM_CAPACITY> = SpoorStream::new();
    stream.seed_epoch(0x5EED_0000_0000_0001);
    stream.stamp(Rung::MmuEnabled, Outcome::Ok, 0);
    stream.stamp(Rung::GicRouted, Outcome::Ok, 0);
    stream.stamp(Rung::TickArmed, Outcome::Ok, 0);
    // Close the certificate the way a real boot does: with the first park
    // pass. Three retained records, exactly the shape on the wire.
    stream.stamp(Rung::ParkIteration, Outcome::Ok, 0);
    let mut frame = [0u8; MAX_PAYLOAD];
    // The very first call emits (a fresh stream owes its announcement);
    // consume it so every iteration below starts at the top of the period.
    let mut ok = matches!(stream.announce(&mut frame), Ok(Some(_)));
    for index in 0..(WARMUP + SAMPLES) {
        for _ in 0..(ANNOUNCE_EVERY - 1) {
            if !matches!(stream.announce(&mut frame), Ok(None)) {
                ok = false;
            }
        }
        let watch = Stopwatch::start(source);
        let announced = stream.announce(&mut frame);
        let cycles = watch.stop(calibration);
        if !matches!(announced, Ok(Some(_))) {
            ok = false;
        }
        if index >= WARMUP {
            samples.record(cycles);
        }
    }
    ok
}

#[cfg(test)]
mod spoor_phase_tests {
    use super::*;
    use core::cell::Cell;

    /// Deterministic monotone source: every read advances a fixed step, so
    /// calibration is exact and a phase's control flow — not the host's
    /// clock — decides whether these tests pass.
    struct SteppingSource {
        now: Cell<u64>,
    }

    impl CycleSource for SteppingSource {
        fn read_cycles(&self) -> u64 {
            let value = self.now.get();
            self.now.set(value + 7);
            value
        }
    }

    fn harness() -> (SteppingSource, Calibration) {
        let source = SteppingSource { now: Cell::new(0) };
        let calibration = Calibration::measure(&source, CALIBRATION_SAMPLES);
        (source, calibration)
    }

    #[test]
    fn spoor_stamp_phase_fills_the_sample_set_and_reports_ok() {
        let (source, calibration) = harness();
        let mut samples: Samples<SAMPLES> = Samples::new();
        assert!(phase_spoor_stamp(&source, &calibration, &mut samples));
        assert_eq!(samples.len(), SAMPLES);
        assert_eq!(samples.dropped(), 0);
    }

    #[test]
    fn spoor_drain_phase_drains_a_full_ring_every_sample() {
        let (source, calibration) = harness();
        let mut samples: Samples<SAMPLES> = Samples::new();
        assert!(phase_spoor_drain(&source, &calibration, &mut samples));
        assert_eq!(samples.len(), SAMPLES);
        assert_eq!(samples.dropped(), 0);
    }

    #[test]
    fn spoor_announce_phase_times_only_the_emitting_call() {
        let (source, calibration) = harness();
        let mut samples: Samples<SAMPLES> = Samples::new();
        assert!(phase_spoor_announce(&source, &calibration, &mut samples));
        assert_eq!(samples.len(), SAMPLES);
        assert_eq!(samples.dropped(), 0);
    }

    #[test]
    fn the_spoor_enabled_g23_arm_fills_the_sample_set_and_accounts_every_stamp() {
        let (source, calibration) = harness();
        let mut samples: Samples<SAMPLES> = Samples::new();
        assert!(
            phase_pool_alloc_free_batched_spoored(&source, &calibration, &mut samples),
            "a false return means the stamp count disagreed and the ratio would be meaningless"
        );
        assert_eq!(samples.len(), SAMPLES);
        assert_eq!(samples.dropped(), 0);
    }

    /// The claim `PERF-D07-G23`'s ratio rests on: **inside the timed region
    /// the two arms differ by exactly one stamp, and by nothing else.**
    ///
    /// Checked against the source text, because the behaviour cannot show it.
    /// Two arms that had quietly diverged — a different pool capacity, a
    /// different batch, one extra check inside the stopwatch — would both
    /// still run, both still fill their sample sets, and produce a difference
    /// that would be reported as *spoor overhead*. A ratio is only as good as
    /// the sameness of what it divides, and nothing else in this file was in a
    /// position to notice that going wrong.
    ///
    /// **The timed region specifically**, from `Stopwatch::start` to
    /// `watch.stop`, because that is the only part of either arm the
    /// measurement can see. Setup outside it — the enabled arm's stream, its
    /// certificate-closing stamp, its sequence assertion — is allowed to
    /// differ and must, or there would be nothing to stamp with.
    #[test]
    fn the_two_g23_arms_differ_by_exactly_one_stamp_inside_the_timed_region() {
        const SOURCE: &str = include_str!("measure_phases.rs");
        const STAMP: &str = "stream.stamp(Rung::ParkIteration, Outcome::Ok, 0);";

        fn timed_region_of(function: &str) -> Vec<String> {
            let body = SOURCE
                .split_once(&format!("pub fn {function}<S: CycleSource>"))
                .expect("the phase exists")
                .1;
            let region = body
                .split_once("let watch = Stopwatch::start(source);")
                .expect("the phase times something")
                .1;
            let region = region
                .split_once("let cycles = watch.stop(calibration);")
                .expect("the phase stops its watch")
                .0;
            region
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty() && !line.starts_with("//"))
                .map(str::to_string)
                .collect()
        }

        let disabled = timed_region_of("phase_pool_alloc_free_batched");
        let enabled = timed_region_of("phase_pool_alloc_free_batched_spoored");

        assert_eq!(
            enabled.iter().filter(|line| *line == STAMP).count(),
            1,
            "the enabled arm's timed region must carry exactly one stamp"
        );
        assert!(
            !disabled.contains(&STAMP.to_string()),
            "the disabled arm's timed region must carry none"
        );
        let enabled_without_stamp: Vec<&String> =
            enabled.iter().filter(|line| *line != STAMP).collect();
        let disabled_lines: Vec<&String> = disabled.iter().collect();
        assert_eq!(
            disabled_lines, enabled_without_stamp,
            "the G23 arms have diverged inside the timed region by something other than the \
             spoor stamp, so any percentage computed from them is not spoor overhead"
        );
    }
}
