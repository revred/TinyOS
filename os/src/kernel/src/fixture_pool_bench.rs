//! `STORY-P0-03-01`'s Tier 0 `PERF-D07` measurement fixture, **refactored onto
//! the shared measurement harness by `STORY-P1-01-01`** (loose end `LE-06`).
//!
//! Drives [`kernel::mem::Pool`] through the same operation shapes the host
//! diagnostic harness in `mem.rs`'s `#[cfg(test)]` module measures
//! (`perf_pool_u64x64_*`), compiled for the real `x86_64-tinyos` target and
//! run under QEMU, so every cycle count comes from a real cycle-source read
//! executing inside the actual target binary — not a host userspace process
//! sharing a Windows dev machine with a browser and an IDE.
//!
//! **What the refactor changed, and why it matters.** This fixture used to own
//! private copies of everything measurement-shaped: its own `rdtsc()`, its own
//! `calibrate_rdtsc_overhead`, its own `percentile`, its own prose-formatted
//! `report_percentiles`, and its own bare `[u64; N]` buffer indexed by the
//! caller. All five are gone, replaced by [`kernel::measure`]:
//!
//! - the cycle source is now [`hal::time::CycleSource`] behind
//!   `hal_x86_64::tsc::Tsc`, so this fixture no longer names `RDTSC` at all
//!   and an ARM64 build of it needs no edits (`LE-09`);
//! - sampling goes through [`Samples`], which counts refused samples instead
//!   of trusting the caller's index arithmetic;
//! - percentiles come from the one shared, host-unit-tested implementation
//!   rather than a second copy that could drift from it;
//! - output is the versioned `TINYOS-MEAS/1` envelope that
//!   `cargo run -p xtask -- measure` parses and fails closed on, replacing
//!   prose lines that only a human could read — which is what made this
//!   fixture's numbers reachable *only* by invoking QEMU by hand with
//!   `-serial file:PATH`.
//!
//! The fixture's own remaining code is exactly what it should be: pool
//! workloads and their self-consistency checks.
//!
//! Each measurement phase below is its own `#[inline(never)]` function rather
//! than an inline block of `run`, and deliberately not written to avoid it:
//! this workspace's unoptimized dev-profile build does not reuse stack slots
//! across lexically-separate blocks within one function body (`boot.rs`'s own
//! doc comment on `boot_stack_top` documents the identical trap for a
//! different call chain), so a single monolithic `run` stacking every phase's
//! own pools/buffers/handles as same-function locals silently accumulates one
//! giant activation record — this fixture hit exactly that wall during
//! bring-up (a real triple fault/QEMU-shutdown after the occupancy-pattern
//! phase, `RSP` having walked off the 1MiB boot stack) before being split into
//! per-phase functions, each of whose frame is reclaimed on return before the
//! next phase's frame is built. `#[inline(never)]` keeps the compiler from
//! undoing that split back into one frame.
//!
//! Caveat this fixture's own numbers do **not** erase: QEMU's `q35`/TCG cycle
//! counter is software emulation of the timestamp counter, not a passthrough
//! read of real silicon — so while this *is* real target-compiled code (not a
//! host-native binary), the cycle-count magnitudes it reports are QEMU's own
//! emulation, not proof of real-hardware wall-clock cost. That gap is exactly
//! what the separately-tracked hardware tier (`LE-09`, Raspberry Pi 5) exists
//! to close; this fixture is Tier 0 evidence, not a hardware substitute.
//!
//! Only reachable when the `fixture-pool-bench` feature is enabled — never
//! part of a real boot image.

use core::fmt::Write;
use hal::time::{CycleSource, Timebase};
use hal_x86_64::serial::SerialPort;
use hal_x86_64::tsc::{self, Tsc};
use kernel::measure::{Calibration, Environment, Metric, Report, Samples, Stopwatch, Summary};
use kernel::mem::{Pool, PoolError, PoolHandle};

/// Upper bound on any single phase's sample count below — sized to the largest
/// phase ([`TAIL_SAMPLES`]). Lives in `.bss`, not on the boot stack, for the
/// stack-pressure reason this module's own doc comment explains, and is reused
/// sequentially by every phase ([`Samples::clear`] between them) rather than
/// one buffer per phase.
const MAX_SAMPLES: usize = 50_000;

/// Primary alloc/free round-trip sample count (mirrors the host harness's
/// `perf_pool_u64x64_alloc_free_round_trip_host_diagnostic`, scaled down from
/// 100,000 — QEMU/TCG executes this loop in well under a second either way,
/// but a smaller count keeps this fixture's own runtime comfortably inside
/// `xtask`'s 15-second boot-timeout budget alongside every other phase run in
/// the same boot).
const PRIMARY_SAMPLES: usize = 10_000;
const PRIMARY_WARMUP: usize = 500;

/// Tail-focused sample count (mirrors the host harness's million-op tail test,
/// scaled down for the same reason as [`PRIMARY_SAMPLES`]).
const TAIL_SAMPLES: usize = 50_000;
const TAIL_WARMUP: usize = 500;

/// Occupancy-pattern / denial-path trial count per case (mirrors the host
/// harness's `TRIALS`, scaled down for the same reason).
const OCCUPANCY_TRIALS: usize = 2_000;

/// Exhaustion/drain cycle count (mirrors the host harness's
/// `perf_pool_exhaustion_drain_cycles_drop_accounting`, scaled down for the
/// same reason).
const DRAIN_CYCLES: usize = 200;
const DRAIN_N: usize = 16;

/// Percentile-reporting phases in this fixture — the fixed capacity of the
/// collected-summary array in [`run`].
const METRICS: usize = 9;

static mut SAMPLE_BUFFER: Samples<MAX_SAMPLES> = Samples::new();

/// One measured phase, held until every phase has run so the envelope is
/// emitted in one piece (a `Report` borrows its sink for its whole lifetime,
/// and this fixture also prints non-percentile self-check lines).
struct Measured {
    name: &'static str,
    warmup: usize,
    summary: Summary,
}

/// Summarizes the current phase's samples into `collected` and clears the
/// buffer for the next phase. A phase that recorded nothing collects nothing
/// and fails the run: silence is not a fast pass.
fn collect(
    collected: &mut [Option<Measured>; METRICS],
    slot: usize,
    name: &'static str,
    warmup: usize,
    samples: &mut Samples<MAX_SAMPLES>,
) -> bool {
    let summarized = samples.summarize();
    samples.clear();
    match summarized {
        Some(summary) => {
            collected[slot] = Some(Measured { name, warmup, summary });
            true
        }
        None => false,
    }
}

/// Runs one alloc/free-round-trip-shaped phase (used by both
/// [`PRIMARY_SAMPLES`] and [`TAIL_SAMPLES`] phases below): `warmup` unmeasured
/// iterations, then `samples` measured ones, recorded into `buffer`.
fn run_alloc_free_phase<S: CycleSource>(
    pool: &mut Pool<u64, 64>,
    source: &S,
    calibration: &Calibration,
    warmup: usize,
    samples: usize,
    buffer: &mut Samples<MAX_SAMPLES>,
) -> bool {
    let mut ok = true;
    for index in 0..(warmup + samples) {
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
        if index >= warmup {
            buffer.record(cycles);
        }
    }
    ok
}

// Phase 1 (PERF-D07-G01/G02/G03/G05/G06/G07/G13/G18): primary alloc/free
// round trip.
#[inline(never)]
fn phase_primary_alloc_free<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    buffer: &mut Samples<MAX_SAMPLES>,
) -> bool {
    let mut pool: Pool<u64, 64> = Pool::new();
    run_alloc_free_phase(&mut pool, source, calibration, PRIMARY_WARMUP, PRIMARY_SAMPLES, buffer)
}

// Phase 2 (PERF-D07-G03 tail).
#[inline(never)]
fn phase_tail<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    buffer: &mut Samples<MAX_SAMPLES>,
) -> bool {
    let mut pool: Pool<u64, 64> = Pool::new();
    run_alloc_free_phase(&mut pool, source, calibration, TAIL_WARMUP, TAIL_SAMPLES, buffer)
}

/// Occupancy-pattern phases 3a-3c: `prefill` slots permanently occupied, then
/// the measured `alloc` lands at slot `prefill` — isolating the linear-scan
/// cost of the first-free-slot search at three different occupancy depths.
fn run_occupancy_phase<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    prefill: usize,
    buffer: &mut Samples<MAX_SAMPLES>,
) -> bool {
    let mut pool: Pool<u64, 64> = Pool::new();
    for index in 0..prefill {
        if pool.alloc(index as u64).is_err() {
            return false;
        }
    }
    let mut ok = true;
    for index in 0..OCCUPANCY_TRIALS {
        let watch = Stopwatch::start(source);
        let handle = pool.alloc(index as u64);
        let cycles = watch.stop(calibration);
        match handle {
            Ok(handle) => {
                if pool.free(handle).is_err() {
                    ok = false;
                }
            }
            Err(_) => ok = false,
        }
        buffer.record(cycles);
    }
    ok
}

// Phase 3a (PERF-D07-G04/G12/G14): best case — slot 0 immediately free.
#[inline(never)]
fn phase_occupancy_best<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    buffer: &mut Samples<MAX_SAMPLES>,
) -> bool {
    run_occupancy_phase(source, calibration, 0, buffer)
}

// Phase 3b: slots 0..31 permanently occupied, alloc lands at slot 32.
#[inline(never)]
fn phase_occupancy_middle<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    buffer: &mut Samples<MAX_SAMPLES>,
) -> bool {
    run_occupancy_phase(source, calibration, 31, buffer)
}

// Phase 3c: slots 0..62 permanently occupied — the full 64-slot linear-scan
// cost.
#[inline(never)]
fn phase_occupancy_last<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    buffer: &mut Samples<MAX_SAMPLES>,
) -> bool {
    run_occupancy_phase(source, calibration, 63, buffer)
}

// Phase 3d: free-path timing, O(1) regardless of index.
#[inline(never)]
fn phase_occupancy_free<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    buffer: &mut Samples<MAX_SAMPLES>,
) -> bool {
    let mut pool: Pool<u64, 64> = Pool::new();
    let mut ok = true;
    for index in 0..OCCUPANCY_TRIALS {
        let Ok(handle) = pool.alloc(index as u64) else {
            return false;
        };
        let watch = Stopwatch::start(source);
        let freed = pool.free(handle);
        let cycles = watch.stop(calibration);
        if freed.is_err() {
            ok = false;
        }
        buffer.record(cycles);
    }
    ok
}

// Phase 3e: exhaustion-then-recovery, one round trip per trial.
#[inline(never)]
fn phase_occupancy_deny_then_recover<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    buffer: &mut Samples<MAX_SAMPLES>,
) -> bool {
    let mut pool: Pool<u64, 64> = Pool::new();
    let mut handles: [Option<PoolHandle>; 64] = [None; 64];
    for (index, slot) in handles.iter_mut().enumerate() {
        let Ok(handle) = pool.alloc(index as u64) else {
            return false;
        };
        *slot = Some(handle);
    }
    let mut top = 64usize;
    let mut ok = true;
    for _ in 0..OCCUPANCY_TRIALS {
        let watch = Stopwatch::start(source);
        let denied = pool.alloc(0);
        top -= 1;
        let Some(handle) = handles[top].take() else {
            return false;
        };
        let freed = pool.free(handle);
        let recovered = pool.alloc(999);
        let cycles = watch.stop(calibration);
        if denied != Err(PoolError::Exhausted) || freed.is_err() {
            ok = false;
        }
        let Ok(recovered) = recovered else {
            return false;
        };
        buffer.record(cycles);
        handles[top] = Some(recovered);
        top += 1;
    }
    ok
}

/// State snapshot (occupied bitmap + values) of a `Pool<u64, 4>`, used by both
/// Phase 4 denial-path cases below to prove a denied call produces zero
/// observable state change. No test-only back door into `mem.rs`'s production
/// API exists (nor should one be added just for this fixture) —
/// `iter_occupied` is the only public, read-only way to observe every slot's
/// occupied/value state at once, keyed by `PoolHandle::index()` (also public,
/// read-only), so this snapshot is built from that.
fn snapshot4(pool: &Pool<u64, 4>) -> ([bool; 4], [u64; 4]) {
    let mut occupied = [false; 4];
    let mut values = [0u64; 4];
    for (handle, value) in pool.iter_occupied() {
        occupied[handle.index()] = true;
        values[handle.index()] = *value;
    }
    (occupied, values)
}

/// Element-wise comparison of two [`snapshot4`] results, deliberately *not*
/// `a != b` on the tuple/array types directly: this target's `core`/
/// `compiler_builtins` combination lowers array/slice equality on
/// byte-comparable element types (here, `[bool; 4]`/`[u64; 4]`) to a
/// `memcmp`/`bcmp` call, and this fixture's own bring-up hit a real triple
/// fault at exactly that comparison under this custom JSON target spec — a
/// linkage/ABI gap in that lowering path, not a `Pool` bug (the identical
/// host-side comparison in `mem.rs`'s test harness, built against a normal
/// host target, has never shown this). Looping element-by-element sidesteps
/// the specialization entirely and is the only comparison this fixture now
/// performs on these arrays.
fn snapshots4_equal(a: &([bool; 4], [u64; 4]), b: &([bool; 4], [u64; 4])) -> bool {
    for k in 0..4 {
        if a.0[k] != b.0[k] || a.1[k] != b.1[k] {
            return false;
        }
    }
    true
}

// Phase 4a (PERF-D07-G20): exhausted-denial latency plus byte-for-byte
// state-change verification.
#[inline(never)]
fn phase_denial_exhausted<S: CycleSource>(
    serial: &mut SerialPort,
    source: &S,
    calibration: &Calibration,
    buffer: &mut Samples<MAX_SAMPLES>,
) -> bool {
    let mut pool: Pool<u64, 4> = Pool::new();
    for index in 0..4 {
        if pool.alloc(index as u64).is_err() {
            return false;
        }
    }
    let before = snapshot4(&pool);
    let mut state_changed = false;
    for _ in 0..OCCUPANCY_TRIALS {
        let watch = Stopwatch::start(source);
        let denied = pool.alloc(0xDEAD);
        let cycles = watch.stop(calibration);
        if denied != Err(PoolError::Exhausted) {
            state_changed = true;
        }
        buffer.record(cycles);
    }
    let after = snapshot4(&pool);
    if !snapshots4_equal(&before, &after) {
        state_changed = true;
    }
    let _ = writeln!(serial, "pool-bench denial[exhausted][G20] state_changed={state_changed}");
    !state_changed
}

// Phase 4b (PERF-D07-G20): `InvalidHandle`-denial (double-free of an
// already-freed handle) latency plus the same state-change verification.
#[inline(never)]
fn phase_denial_invalid_handle<S: CycleSource>(
    serial: &mut SerialPort,
    source: &S,
    calibration: &Calibration,
    buffer: &mut Samples<MAX_SAMPLES>,
) -> bool {
    let mut pool: Pool<u64, 4> = Pool::new();
    let Ok(handle) = pool.alloc(1) else {
        return false;
    };
    if pool.free(handle).is_err() {
        return false;
    }
    let before = snapshot4(&pool);
    let mut state_changed = false;
    for _ in 0..OCCUPANCY_TRIALS {
        let watch = Stopwatch::start(source);
        let denied = pool.free(handle);
        let cycles = watch.stop(calibration);
        if denied != Err(PoolError::InvalidHandle) {
            state_changed = true;
        }
        buffer.record(cycles);
    }
    let after = snapshot4(&pool);
    if !snapshots4_equal(&before, &after) {
        state_changed = true;
    }
    let _ =
        writeln!(serial, "pool-bench denial[invalid_handle][G20] state_changed={state_changed}");
    !state_changed
}

// Phase 5 (PERF-D07-G21): fill-to-exhaustion/drain cycles, drop accounting.
// Not a percentile phase — its evidence is an equality, not a latency.
#[inline(never)]
fn phase_drain_cycles(serial: &mut SerialPort) -> bool {
    use core::cell::Cell;
    struct DropCounter<'a>(&'a Cell<u32>);
    impl Drop for DropCounter<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    let alloc_count = Cell::new(0u32);
    let drop_count = Cell::new(0u32);
    let mut cycle_ok = true;

    for _ in 0..DRAIN_CYCLES {
        let mut pool: Pool<DropCounter<'_>, DRAIN_N> = Pool::new();
        let mut handles: [Option<PoolHandle>; DRAIN_N] = [None; DRAIN_N];
        for slot in handles.iter_mut() {
            match pool.alloc(DropCounter(&drop_count)) {
                Ok(handle) => *slot = Some(handle),
                Err(_) => return false,
            }
            alloc_count.set(alloc_count.get() + 1);
        }
        // A denied `alloc` still drops the by-value argument it was handed
        // (never stored) — expected, not a leak; see the identical accounting
        // note in `mem.rs`'s host harness.
        if pool.alloc(DropCounter(&drop_count)) != Err(PoolError::Exhausted) {
            cycle_ok = false;
        }
        alloc_count.set(alloc_count.get() + 1);
        for slot in handles.iter_mut() {
            if let Some(handle) = slot.take() {
                if pool.free(handle).is_err() {
                    cycle_ok = false;
                }
            }
        }
        if drop_count.get() != alloc_count.get() {
            cycle_ok = false;
        }
    }
    cycle_ok &= alloc_count.get() == drop_count.get();
    let _ = writeln!(
        serial,
        "pool-bench exhaustion_drain_cycles[G21] cycles={} n_per_cycle={} total_allocs={} \
         total_drops={} ok={}",
        DRAIN_CYCLES,
        DRAIN_N,
        alloc_count.get(),
        drop_count.get(),
        cycle_ok
    );
    cycle_ok
}

// Phase 6 (PERF-D07-G10): static-pool footprint. Also not a percentile phase.
#[inline(never)]
fn phase_size_of(serial: &mut SerialPort) -> bool {
    let bytes = core::mem::size_of::<Pool<u64, 64>>();
    let fits = bytes <= 8 * 1024;
    let _ = writeln!(
        serial,
        "pool-bench size_of::<Pool<u64,64>>()[G10] = {bytes} bytes fits_8kib_budget={fits}"
    );
    fits
}

/// Runs every measurement phase and reports whether every phase's own
/// self-consistency check held (the pass/fail bit `xtask` reads back via
/// isa-debug-exit) — the printed numbers themselves are diagnostic evidence,
/// not a threshold this function enforces (measurement protocol #7, identical
/// framing to the host harness).
pub fn run() -> bool {
    // SAFETY: this fixture is the only code running (single-CPU boot path, no
    // other UART user), and `init` is called exactly once, before any other
    // `SerialPort` method — `init`'s own documented contract.
    let mut serial = unsafe { SerialPort::init() };
    let _ = writeln!(serial, "pool-bench starting");

    let source = Tsc;
    // SAFETY: nothing else in this fixture uses PIT channel 2 or port 0x61,
    // and this fixture never enables interrupts — `calibrate_cycles_per_us`'s
    // documented contract.
    let timebase = unsafe { tsc::calibrate_cycles_per_us() };
    let calibration = Calibration::measure(&source, 2_000);

    // SAFETY: this fixture is single-threaded and non-reentrant (no interrupts
    // armed, no recursion into `run`), and every phase function below runs to
    // completion (borrowing this buffer for its own duration only) before the
    // next one starts — no two phases ever hold an overlapping borrow, and
    // this is the only function in the binary that ever touches it.
    let buffer: &mut Samples<MAX_SAMPLES> = unsafe { &mut *core::ptr::addr_of_mut!(SAMPLE_BUFFER) };

    let mut ok = true;
    let mut collected: [Option<Measured>; METRICS] =
        [None, None, None, None, None, None, None, None, None];

    ok &= phase_primary_alloc_free(&source, &calibration, buffer);
    ok &= collect(&mut collected, 0, "pool_u64x64_alloc_free", PRIMARY_WARMUP, buffer);

    ok &= phase_tail(&source, &calibration, buffer);
    ok &= collect(&mut collected, 1, "pool_u64x64_tail", TAIL_WARMUP, buffer);

    ok &= phase_occupancy_best(&source, &calibration, buffer);
    ok &= collect(&mut collected, 2, "pool_u64x64_occupancy_best_slot0", 0, buffer);

    ok &= phase_occupancy_middle(&source, &calibration, buffer);
    ok &= collect(&mut collected, 3, "pool_u64x64_occupancy_middle_slot32", 0, buffer);

    ok &= phase_occupancy_last(&source, &calibration, buffer);
    ok &= collect(&mut collected, 4, "pool_u64x64_occupancy_last_slot63", 0, buffer);

    ok &= phase_occupancy_free(&source, &calibration, buffer);
    ok &= collect(&mut collected, 5, "pool_u64x64_free_o1", 0, buffer);

    ok &= phase_occupancy_deny_then_recover(&source, &calibration, buffer);
    ok &= collect(&mut collected, 6, "pool_u64x64_deny_then_recover", 0, buffer);

    ok &= phase_denial_exhausted(&mut serial, &source, &calibration, buffer);
    ok &= collect(&mut collected, 7, "pool_u64x4_denial_exhausted", 0, buffer);

    ok &= phase_denial_invalid_handle(&mut serial, &source, &calibration, buffer);
    ok &= collect(&mut collected, 8, "pool_u64x4_denial_invalid_handle", 0, buffer);

    ok &= phase_drain_cycles(&mut serial);
    ok &= phase_size_of(&mut serial);

    let environment = Environment {
        tier: "T0",
        arch: "x86_64",
        cycle_source: Tsc::NAME,
        overhead_cycles: calibration.overhead_cycles(),
        cycles_per_us: timebase.cycles_per_us(),
    };
    let Ok(mut report) = Report::begin(&mut serial, &environment) else {
        return false;
    };
    for measured in collected.iter().flatten() {
        if report
            .metric(&Metric {
                domain: "D07",
                name: measured.name,
                warmup: measured.warmup,
                summary: measured.summary,
            })
            .is_err()
        {
            return false;
        }
    }
    let Ok(metrics) = report.end() else {
        return false;
    };

    let _ = writeln!(serial, "pool-bench metrics={metrics}");
    // See `fixture_measure`'s note: the verdict is a sentinel line so a board
    // with no isa-debug-exit port can still report pass/fail.
    let _ = kernel::measure::write_result(&mut serial, "pool-bench", ok);
    ok && metrics == METRICS
}
