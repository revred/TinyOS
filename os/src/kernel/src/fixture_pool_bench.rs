//! `STORY-P0-03-01`'s Tier 0 `PERF-D07` measurement fixture: drives
//! [`kernel::mem::Pool`] through the same operation shapes the host
//! diagnostic harness in `mem.rs`'s `#[cfg(test)]` module measures
//! (`perf_pool_u64x64_*`), compiled for the real `x86_64-tinyos` target and
//! run under QEMU, so every cycle count this fixture reports comes from
//! `RDTSC` executing inside the actual target binary — not a host userspace
//! process sharing a Windows dev machine with a browser and an IDE.
//!
//! Numbers leave the VM over [`hal_x86_64::serial`] (COM1), since
//! `hal_x86_64::qemu_exit`'s isa-debug-exit port only carries a single
//! pass/fail bit — `xtask qemu-x86_64 --fixture=pool-bench` still reports
//! pass/fail the normal way (this fixture's own self-consistency checks
//! below decide it), but reading the actual percentile/cycle numbers
//! requires invoking QEMU directly with `-serial file:PATH` against this
//! fixture's built ELF (see this story's own Report for the exact
//! invocation used).
//!
//! Caveat this fixture's own numbers do **not** erase: QEMU's `q35`/TCG
//! `RDTSC` model is software emulation of the timestamp counter, not a
//! passthrough read of real silicon — so while this *is* real target-compiled
//! code (not a host-native binary) executing a real `RDTSC` instruction, the
//! cycle-count magnitudes it reports are QEMU's own TSC emulation, not proof
//! of real-hardware wall-clock cost. That gap is exactly what the
//! separately-tracked HIL tier (unavailable in this environment) exists to
//! close; this fixture is Tier 0 evidence, not a HIL substitute.
//!
//! Each measurement phase below is its own `#[inline(never)]` function
//! rather than an inline block of `run`, and deliberately not written to
//! avoid it: this workspace's unoptimized dev-profile build does not reuse
//! stack slots across lexically-separate blocks within one function body
//! (`boot.rs`'s own doc comment on `boot_stack_top` documents the identical
//! trap for a different call chain), so a single monolithic `run` stacking
//! every phase's own pools/buffers/handles as same-function locals silently
//! accumulates one giant activation record — this fixture hit exactly that
//! wall during bring-up (a real triple fault/QEMU-shutdown after the
//! occupancy-pattern phase, `RSP` having walked off the 1MiB boot stack)
//! before being split into per-phase functions, each of whose frame is
//! reclaimed on return before the next phase's frame is built. `#[inline(never)]`
//! keeps the compiler from undoing that split back into one frame.
//!
//! Only reachable when the `fixture-pool-bench` feature is enabled — never
//! part of a real boot image.

use core::fmt::Write;
use hal_x86_64::serial::SerialPort;
use kernel::mem::{Pool, PoolError, PoolHandle};

/// Shared scratch buffer every phase below sorts its samples into, reused
/// sequentially (one phase finishes with it, prints, then the next phase
/// overwrites it) rather than each phase declaring its own array — keeps
/// this fixture's static footprint to one allocation instead of one per
/// phase. Lives in `.bss`, not on the boot stack, for the same
/// stack-pressure reason this module's own doc comment explains for the
/// per-phase function split.
static mut SAMPLE_BUFFER: [u64; MAX_SAMPLES] = [0; MAX_SAMPLES];

/// Upper bound on any single phase's sample count below — sized to the
/// largest phase ([`TAIL_SAMPLES`]).
const MAX_SAMPLES: usize = 50_000;

/// Primary alloc/free round-trip sample count (mirrors the host harness's
/// `perf_pool_u64x64_alloc_free_round_trip_host_diagnostic`, scaled down
/// from 100,000 — QEMU/TCG executes this loop in well under a second either
/// way, but a smaller count keeps this fixture's own runtime comfortably
/// inside `xtask`'s 15-second boot-timeout budget alongside every other
/// phase run in the same boot).
const PRIMARY_SAMPLES: usize = 10_000;
const PRIMARY_WARMUP: usize = 500;

/// Tail-focused sample count (mirrors the host harness's million-op tail
/// test, scaled down for the same reason as [`PRIMARY_SAMPLES`]).
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

/// Reads the x86_64 timestamp counter.
fn rdtsc() -> u64 {
    // SAFETY: `RDTSC` is unconditionally available on every x86_64 CPU
    // (including QEMU's `q35` TCG emulation); reading it has no memory or
    // control-flow side effect this fixture needs to account for.
    unsafe { core::arch::x86_64::_rdtsc() }
}

/// Calibrates `RDTSC`'s own call overhead as the minimum back-to-back-read
/// delta — identical method to the host harness's `calibrate_rdtsc_overhead`
/// (measurement protocol #3).
fn calibrate_rdtsc_overhead() -> u64 {
    const SAMPLES: usize = 2_000;
    let mut min = u64::MAX;
    for _ in 0..SAMPLES {
        let a = rdtsc();
        let b = rdtsc();
        min = min.min(b.saturating_sub(a));
    }
    min
}

/// Nearest-rank percentile over an already-sorted slice, `num`/`den`
/// (e.g. `50, 100` for p50, `999, 1000` for p99.9) rather than a `f64`
/// fraction — this crate builds `#![no_std]` with no `libm`, and integer
/// nearest-rank arithmetic needs no floating point at all.
fn percentile(sorted: &[u64], num: usize, den: usize) -> u64 {
    let rank = (sorted.len() - 1) * num / den;
    sorted[rank]
}

/// Writes one `label`/percentile-summary line, matching the host harness's
/// own print shape (prefixed `T0` here to distinguish this tier's evidence
/// from `mem.rs`'s `Host`-tier diagnostic numbers).
fn report_percentiles(serial: &mut SerialPort, label: &str, sorted: &[u64]) {
    let p50 = percentile(sorted, 50, 100);
    let p99 = percentile(sorted, 99, 100);
    let p999 = percentile(sorted, 999, 1000);
    let max = *sorted.last().unwrap();
    let _ = writeln!(
        serial,
        "T0-PERF-D07 {} n={} cycles: p50={} p99={} p99.9={} max={}",
        label,
        sorted.len(),
        p50,
        p99,
        p999,
        max
    );
}

/// Runs one alloc/free-round-trip-shaped phase (used by both
/// [`PRIMARY_SAMPLES`] and [`TAIL_SAMPLES`] phases below): `warmup` unmeasured
/// iterations, then `samples` measured ones, writing each measured cycle
/// delta into `buf[..samples]` and returning that prefix sorted ascending.
fn run_alloc_free_phase(
    pool: &mut Pool<u64, 64>,
    rdtsc_overhead: u64,
    warmup: usize,
    samples: usize,
    buf: &mut [u64],
) -> bool {
    let mut ok = true;
    for i in 0..(warmup + samples) {
        let value = i as u64;
        let c0 = rdtsc();
        let handle = match pool.alloc(value) {
            Ok(h) => h,
            Err(_) => {
                ok = false;
                continue;
            }
        };
        let freed = pool.free(handle);
        let c1 = rdtsc();
        if freed != Ok(value) {
            ok = false;
        }
        if i >= warmup {
            buf[i - warmup] = c1.saturating_sub(c0).saturating_sub(rdtsc_overhead);
        }
    }
    buf[..samples].sort_unstable();
    ok
}

// Phase 1 (PERF-D07-G01/G02/G03/G05/G06/G07/G13/G18): primary alloc/free
// round trip.
#[inline(never)]
fn phase_primary_alloc_free(serial: &mut SerialPort, rdtsc_overhead: u64, buf: &mut [u64]) -> bool {
    let mut pool: Pool<u64, 64> = Pool::new();
    let ok = run_alloc_free_phase(&mut pool, rdtsc_overhead, PRIMARY_WARMUP, PRIMARY_SAMPLES, buf);
    report_percentiles(serial, "pool_u64x64_alloc_free[G01..G07,G13,G18]", &buf[..PRIMARY_SAMPLES]);
    ok
}

// Phase 2 (PERF-D07-G03 tail).
#[inline(never)]
fn phase_tail(serial: &mut SerialPort, rdtsc_overhead: u64, buf: &mut [u64]) -> bool {
    let mut pool: Pool<u64, 64> = Pool::new();
    let ok = run_alloc_free_phase(&mut pool, rdtsc_overhead, TAIL_WARMUP, TAIL_SAMPLES, buf);
    report_percentiles(serial, "pool_u64x64_tail[G03]", &buf[..TAIL_SAMPLES]);
    ok
}

// Phase 3a (PERF-D07-G04/G12/G14): best-case (slot 0 immediately free).
#[inline(never)]
fn phase_occupancy_best(serial: &mut SerialPort, rdtsc_overhead: u64, buf: &mut [u64]) {
    let mut pool: Pool<u64, 64> = Pool::new();
    for i in 0..OCCUPANCY_TRIALS {
        let c0 = rdtsc();
        let h = pool.alloc(i as u64).unwrap();
        let c1 = rdtsc();
        buf[i] = c1.saturating_sub(c0).saturating_sub(rdtsc_overhead);
        pool.free(h).unwrap();
    }
    buf[..OCCUPANCY_TRIALS].sort_unstable();
    report_percentiles(
        serial,
        "pool_u64x64_occupancy[best_slot0][G04,G12,G14]",
        &buf[..OCCUPANCY_TRIALS],
    );
}

// Phase 3b: slots 0..31 permanently occupied, alloc lands at slot 32.
#[inline(never)]
fn phase_occupancy_middle(serial: &mut SerialPort, rdtsc_overhead: u64, buf: &mut [u64]) {
    let mut pool: Pool<u64, 64> = Pool::new();
    for i in 0..31 {
        pool.alloc(i as u64).unwrap();
    }
    for i in 0..OCCUPANCY_TRIALS {
        let c0 = rdtsc();
        let h = pool.alloc(i as u64).unwrap();
        let c1 = rdtsc();
        buf[i] = c1.saturating_sub(c0).saturating_sub(rdtsc_overhead);
        pool.free(h).unwrap();
    }
    buf[..OCCUPANCY_TRIALS].sort_unstable();
    report_percentiles(
        serial,
        "pool_u64x64_occupancy[middle_slot32][G04,G12,G14]",
        &buf[..OCCUPANCY_TRIALS],
    );
}

// Phase 3c: slots 0..62 permanently occupied — the full 64-slot linear-scan
// cost.
#[inline(never)]
fn phase_occupancy_last(serial: &mut SerialPort, rdtsc_overhead: u64, buf: &mut [u64]) {
    let mut pool: Pool<u64, 64> = Pool::new();
    for i in 0..63 {
        pool.alloc(i as u64).unwrap();
    }
    for i in 0..OCCUPANCY_TRIALS {
        let c0 = rdtsc();
        let h = pool.alloc(i as u64).unwrap();
        let c1 = rdtsc();
        buf[i] = c1.saturating_sub(c0).saturating_sub(rdtsc_overhead);
        pool.free(h).unwrap();
    }
    buf[..OCCUPANCY_TRIALS].sort_unstable();
    report_percentiles(
        serial,
        "pool_u64x64_occupancy[last_slot63][G04,G12,G14]",
        &buf[..OCCUPANCY_TRIALS],
    );
}

// Phase 3d: free-path timing, O(1) regardless of index.
#[inline(never)]
fn phase_occupancy_free(serial: &mut SerialPort, rdtsc_overhead: u64, buf: &mut [u64]) {
    let mut pool: Pool<u64, 64> = Pool::new();
    for i in 0..OCCUPANCY_TRIALS {
        let h = pool.alloc(i as u64).unwrap();
        let c0 = rdtsc();
        pool.free(h).unwrap();
        let c1 = rdtsc();
        buf[i] = c1.saturating_sub(c0).saturating_sub(rdtsc_overhead);
    }
    buf[..OCCUPANCY_TRIALS].sort_unstable();
    report_percentiles(
        serial,
        "pool_u64x64_occupancy[free_o1][G04,G12,G14]",
        &buf[..OCCUPANCY_TRIALS],
    );
}

// Phase 3e: exhaustion-then-recovery, one round trip per trial.
#[inline(never)]
fn phase_occupancy_deny_then_recover(
    serial: &mut SerialPort,
    rdtsc_overhead: u64,
    buf: &mut [u64],
) -> bool {
    let mut pool: Pool<u64, 64> = Pool::new();
    let mut handles: [Option<PoolHandle>; 64] = [None; 64];
    for (i, slot) in handles.iter_mut().enumerate() {
        *slot = Some(pool.alloc(i as u64).unwrap());
    }
    let mut top = 64usize;
    let mut denial_ok = true;
    for i in 0..OCCUPANCY_TRIALS {
        let c0 = rdtsc();
        let denied = pool.alloc(0);
        top -= 1;
        let h = handles[top].take().unwrap();
        pool.free(h).unwrap();
        let recovered = pool.alloc(999);
        let c1 = rdtsc();
        if denied != Err(PoolError::Exhausted) {
            denial_ok = false;
        }
        let recovered = recovered.unwrap();
        buf[i] = c1.saturating_sub(c0).saturating_sub(rdtsc_overhead);
        handles[top] = Some(recovered);
        top += 1;
    }
    buf[..OCCUPANCY_TRIALS].sort_unstable();
    report_percentiles(
        serial,
        "pool_u64x64_occupancy[deny_then_recover][G04,G12,G14]",
        &buf[..OCCUPANCY_TRIALS],
    );
    denial_ok
}

/// State snapshot (occupied bitmap + values) of a `Pool<u64, 4>`, used by
/// both Phase 4 denial-path cases below to prove a denied call produces zero
/// observable state change. No test-only back door into `mem.rs`'s
/// production API exists (nor should one be added just for this fixture) —
/// `iter_occupied` is the only public, read-only way to observe every slot's
/// occupied/value state at once, keyed by `PoolHandle::index()` (also
/// public, read-only), so this snapshot is built from that.
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
fn phase_denial_exhausted(serial: &mut SerialPort, rdtsc_overhead: u64, buf: &mut [u64]) -> bool {
    let mut pool: Pool<u64, 4> = Pool::new();
    for i in 0..4 {
        pool.alloc(i as u64).unwrap();
    }
    let before = snapshot4(&pool);
    let mut state_changed = false;
    for i in 0..OCCUPANCY_TRIALS {
        let c0 = rdtsc();
        let r = pool.alloc(0xDEAD);
        let c1 = rdtsc();
        if r != Err(PoolError::Exhausted) {
            state_changed = true;
        }
        buf[i] = c1.saturating_sub(c0).saturating_sub(rdtsc_overhead);
    }
    let after = snapshot4(&pool);
    if !snapshots4_equal(&before, &after) {
        state_changed = true;
    }
    buf[..OCCUPANCY_TRIALS].sort_unstable();
    report_percentiles(serial, "pool_denial[exhausted][G20]", &buf[..OCCUPANCY_TRIALS]);
    let _ =
        writeln!(serial, "T0-PERF-D07 pool_denial[exhausted][G20] state_changed={}", state_changed);
    !state_changed
}

// Phase 4b (PERF-D07-G20): `InvalidHandle`-denial (double-free of an
// already-freed handle) latency plus the same state-change verification.
#[inline(never)]
fn phase_denial_invalid_handle(
    serial: &mut SerialPort,
    rdtsc_overhead: u64,
    buf: &mut [u64],
) -> bool {
    let mut pool: Pool<u64, 4> = Pool::new();
    let h = pool.alloc(1).unwrap();
    pool.free(h).unwrap();
    let before = snapshot4(&pool);
    let mut state_changed = false;
    for i in 0..OCCUPANCY_TRIALS {
        let c0 = rdtsc();
        let r = pool.free(h);
        let c1 = rdtsc();
        if r != Err(PoolError::InvalidHandle) {
            state_changed = true;
        }
        buf[i] = c1.saturating_sub(c0).saturating_sub(rdtsc_overhead);
    }
    let after = snapshot4(&pool);
    if !snapshots4_equal(&before, &after) {
        state_changed = true;
    }
    buf[..OCCUPANCY_TRIALS].sort_unstable();
    report_percentiles(serial, "pool_denial[invalid_handle][G20]", &buf[..OCCUPANCY_TRIALS]);
    let _ = writeln!(
        serial,
        "T0-PERF-D07 pool_denial[invalid_handle][G20] state_changed={}",
        state_changed
    );
    !state_changed
}

// Phase 5 (PERF-D07-G21): fill-to-exhaustion/drain cycles, drop accounting.
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
            *slot = Some(pool.alloc(DropCounter(&drop_count)).unwrap());
            alloc_count.set(alloc_count.get() + 1);
        }
        // A denied `alloc` still drops the by-value argument it was handed
        // (never stored) — expected, not a leak; see the identical
        // accounting note in `mem.rs`'s host harness.
        if pool.alloc(DropCounter(&drop_count)) != Err(PoolError::Exhausted) {
            cycle_ok = false;
        }
        alloc_count.set(alloc_count.get() + 1);
        for slot in handles.iter_mut() {
            if let Some(h) = slot.take() {
                pool.free(h).unwrap();
            }
        }
        if drop_count.get() != alloc_count.get() {
            cycle_ok = false;
        }
    }
    cycle_ok &= alloc_count.get() == drop_count.get();
    let _ = writeln!(
        serial,
        "T0-PERF-D07 pool_exhaustion_drain_cycles[G21] cycles={} n_per_cycle={} total_allocs={} \
         total_drops={} ok={}",
        DRAIN_CYCLES,
        DRAIN_N,
        alloc_count.get(),
        drop_count.get(),
        cycle_ok
    );
    cycle_ok
}

// Phase 6 (PERF-D07-G10): static-pool footprint.
#[inline(never)]
fn phase_size_of(serial: &mut SerialPort) -> bool {
    let bytes = core::mem::size_of::<Pool<u64, 64>>();
    let fits = bytes <= 8 * 1024;
    let _ = writeln!(
        serial,
        "T0-PERF-D07 size_of::<Pool<u64,64>>()[G10] = {} bytes fits_8kib_budget={}",
        bytes, fits
    );
    fits
}

/// Runs every measurement phase and reports whether every phase's own
/// self-consistency check held (the pass/fail bit `xtask` reads back via
/// isa-debug-exit) — the printed numbers themselves are diagnostic evidence,
/// not a threshold this function enforces (measurement protocol #7,
/// identical framing to the host harness).
pub fn run() -> bool {
    // SAFETY: this fixture is the only code running (single-CPU boot path,
    // no other UART user), and `init` is called exactly once, before any
    // other `SerialPort` method — `init`'s own documented contract.
    let mut serial = unsafe { SerialPort::init() };
    let _ = writeln!(serial, "T0-PERF-D07 fixture-pool-bench starting");

    let rdtsc_overhead = calibrate_rdtsc_overhead();
    let _ = writeln!(serial, "T0-PERF-D07 rdtsc_overhead_cycles={}", rdtsc_overhead);

    // SAFETY: this fixture is single-threaded and non-reentrant (no
    // interrupts armed, no recursion into `run`), and every phase function
    // below runs to completion (borrowing this slice for its own duration
    // only) before the next one starts — no two phases ever hold an
    // overlapping borrow of `SAMPLE_BUFFER`, and this is the only function
    // in the binary that ever touches it.
    let buf: &mut [u64; MAX_SAMPLES] = unsafe { &mut *core::ptr::addr_of_mut!(SAMPLE_BUFFER) };

    let mut overall_ok = true;
    overall_ok &= phase_primary_alloc_free(&mut serial, rdtsc_overhead, buf);
    overall_ok &= phase_tail(&mut serial, rdtsc_overhead, buf);
    phase_occupancy_best(&mut serial, rdtsc_overhead, buf);
    phase_occupancy_middle(&mut serial, rdtsc_overhead, buf);
    phase_occupancy_last(&mut serial, rdtsc_overhead, buf);
    phase_occupancy_free(&mut serial, rdtsc_overhead, buf);
    overall_ok &= phase_occupancy_deny_then_recover(&mut serial, rdtsc_overhead, buf);
    overall_ok &= phase_denial_exhausted(&mut serial, rdtsc_overhead, buf);
    overall_ok &= phase_denial_invalid_handle(&mut serial, rdtsc_overhead, buf);
    overall_ok &= phase_drain_cycles(&mut serial);
    overall_ok &= phase_size_of(&mut serial);

    let _ = writeln!(serial, "T0-PERF-D07 fixture-pool-bench overall_ok={}", overall_ok);
    overall_ok
}
