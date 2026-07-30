//! Bounded-capacity pool allocator (`STORY-P0-03-01`, `STORY-P0-03-03`).
//!
//! `Pool<T, N>` is the RT-path allocation primitive `agent/CODING_STANDARDS.md`'s
//! "no heap allocation in any scheduler, IPC, or interrupt-handling hot path"
//! rule requires: fixed-capacity storage for up to `N` live values of `T`,
//! backed by `[MaybeUninit<T>; N]` rather than a `Vec`/`Box`, with an
//! allocation-failure path that fails closed (`PoolError::Exhausted`) instead
//! of panicking or blocking, per Non-Negotiable #5 (fail-safe over
//! keep-trying).

use core::mem::MaybeUninit;

/// Identifies a live value previously returned by [`Pool::alloc`].
///
/// Opaque by design and generational: both the slot index and the allocation
/// incarnation must match. A handle retained after [`Pool::free`] therefore
/// cannot alias a later value that happens to reuse the same slot (the ABA
/// problem). Only [`Pool::alloc`] hands handles out, and [`Pool::free`]
/// rejects anything stale or fabricated via [`PoolError::InvalidHandle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolHandle {
    index: usize,
    generation: u64,
}

impl PoolHandle {
    /// This handle's underlying pool-slot index — exposed (read-only) so a
    /// caller can correlate a handle with an external, index-keyed array
    /// (e.g. `kernel::sched::TaskId::index`, used by `kernel::dispatch` to
    /// key a parallel `Context` array by task), without allowing a handle
    /// to be reconstructed from an arbitrary index — there is still no
    /// public `PoolHandle` constructor.
    pub const fn index(self) -> usize {
        self.index
    }
}

/// Errors [`Pool::alloc`]/[`Pool::free`] fail closed with, per
/// `agent/CODING_STANDARDS.md`'s "no stringly-typed errors" rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolError {
    /// Every slot is occupied or permanently retired after generation
    /// exhaustion; no side effects occurred.
    Exhausted,
    /// The handle does not identify a currently-occupied slot (already
    /// freed, or never issued by this pool).
    InvalidHandle,
}

struct Slot<T> {
    value: MaybeUninit<T>,
    occupied: bool,
    generation: u64,
}

/// Fixed-capacity storage for up to `N` live values of `T`.
///
/// `Pool::new` is a `const fn` so a `Pool` can live in a `static` with no
/// runtime initialization cost — every slot starts uninitialized and
/// unoccupied.
pub struct Pool<T, const N: usize> {
    slots: [Slot<T>; N],
}

impl<T, const N: usize> Pool<T, N> {
    /// Creates an empty pool. `const fn`: no heap allocation, usable in a
    /// `static` initializer.
    pub const fn new() -> Self {
        // `MaybeUninit::uninit()` never actually reads the value, so an
        // array of unoccupied slots needs no `T: Default`/`Copy` bound.
        const fn new_slot<T>() -> Slot<T> {
            Slot { value: MaybeUninit::uninit(), occupied: false, generation: 0 }
        }
        Pool { slots: [const { new_slot() }; N] }
    }

    /// Claims the first free slot and moves `value` into it.
    ///
    /// Fails closed with [`PoolError::Exhausted`] and no side effects if
    /// every slot is occupied or generation-retired — never panics, never
    /// blocks.
    pub fn alloc(&mut self, value: T) -> Result<PoolHandle, PoolError> {
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if !slot.occupied {
                // A wrapped generation would make the first handle for this
                // slot valid again. Retire that slot permanently instead.
                let Some(generation) = slot.generation.checked_add(1) else {
                    continue;
                };
                slot.value.write(value);
                slot.occupied = true;
                slot.generation = generation;
                return Ok(PoolHandle { index, generation });
            }
        }
        Err(PoolError::Exhausted)
    }

    /// Returns a mutable reference to the value at `handle`, without
    /// freeing its slot.
    ///
    /// `None` if `handle` is out of range or its slot is currently free —
    /// mirrors [`Pool::free`]'s validity check, just without consuming the
    /// value. Used by callers (e.g. `exec::address_space`'s
    /// `Pool`-backed page-table frame allocator, `STORY-P0-05-02`) that
    /// need a stable address for a pool-owned value without taking
    /// ownership of it.
    pub fn get_mut(&mut self, handle: PoolHandle) -> Option<&mut T> {
        let slot = self.slots.get_mut(handle.index)?;
        if !slot.occupied || slot.generation != handle.generation {
            return None;
        }
        // SAFETY: `slot.occupied` is only `true` for a slot `alloc` fully
        // initialized via `slot.value.write(_)`, and no other path
        // invalidates it while `occupied` remains `true` — so the value is
        // guaranteed initialized here, and this shared/exclusive borrow of
        // `slot.value` doesn't overlap any other live reference into it.
        Some(unsafe { slot.value.assume_init_mut() })
    }

    /// Iterates over every currently-occupied slot, yielding each one's
    /// handle alongside a shared reference to its value — used by
    /// `kernel::sched::Scheduler::iter_tasks` to scan every live task
    /// without taking ownership of any of them.
    pub fn iter_occupied(&self) -> impl Iterator<Item = (PoolHandle, &T)> {
        self.slots.iter().enumerate().filter(|(_, slot)| slot.occupied).map(|(index, slot)| {
            // SAFETY: `occupied` is only `true` for a slot `alloc` fully
            // initialized via `slot.value.write(_)`, mirroring `get_mut`'s
            // identical proof for the same invariant.
            (PoolHandle { index, generation: slot.generation }, unsafe {
                slot.value.assume_init_ref()
            })
        })
    }

    /// Returns ownership of the value at `handle` and frees its slot.
    ///
    /// Fails closed with [`PoolError::InvalidHandle`] (never a panic or a
    /// stale/aliased read) if `handle` is out of range or already free.
    pub fn free(&mut self, handle: PoolHandle) -> Result<T, PoolError> {
        let slot = self.slots.get_mut(handle.index).ok_or(PoolError::InvalidHandle)?;
        if !slot.occupied || slot.generation != handle.generation {
            return Err(PoolError::InvalidHandle);
        }
        slot.occupied = false;
        // SAFETY: `slot.occupied` was `true`, which this type only sets
        // after a prior `slot.value.write(_)` fully initialized it, and no
        // other path re-reads or drops `slot.value` while `occupied` is
        // `true` — so the value is guaranteed initialized here.
        Ok(unsafe { slot.value.assume_init_read() })
    }
}

impl<T, const N: usize> Default for Pool<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> Drop for Pool<T, N> {
    fn drop(&mut self) {
        for slot in self.slots.iter_mut() {
            if slot.occupied {
                // SAFETY: same invariant as `free` — `occupied` is only
                // `true` for a fully-initialized slot, and dropping the pool
                // is the last read of this slot's storage.
                unsafe { slot.value.assume_init_drop() };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TEST-P0-03-01-A: alloc/free round-trip and invalid-handle rejection.
    #[test]
    fn alloc_free_round_trip_returns_stored_value() {
        let mut pool: Pool<u32, 4> = Pool::new();
        let handle = pool.alloc(42).expect("pool has free slots");
        assert_eq!(pool.free(handle), Ok(42));
    }

    #[test]
    fn freed_slot_is_reusable_by_a_later_alloc() {
        let mut pool: Pool<u32, 1> = Pool::new();
        let handle = pool.alloc(1).unwrap();
        pool.free(handle).unwrap();
        let handle2 = pool.alloc(2).expect("freed slot should be reusable");
        assert_ne!(handle, handle2, "slot reuse must advance its generation");
        assert_eq!(pool.free(handle2), Ok(2));
    }

    #[test]
    fn stale_handle_cannot_alias_a_reused_slot() {
        let mut pool: Pool<u32, 1> = Pool::new();
        let stale = pool.alloc(1).unwrap();
        assert_eq!(pool.free(stale), Ok(1));
        let current = pool.alloc(2).unwrap();

        assert_eq!(stale.index(), current.index(), "the same physical slot was reused");
        assert_ne!(stale, current, "the allocation incarnation is part of identity");
        assert_eq!(pool.get_mut(stale), None);
        assert_eq!(pool.free(stale), Err(PoolError::InvalidHandle));
        assert_eq!(pool.free(current), Ok(2));
    }

    #[test]
    fn generation_wrap_retires_a_slot_instead_of_revalidating_an_old_handle() {
        let mut pool: Pool<u32, 1> = Pool::new();
        pool.slots[0].generation = u64::MAX;
        assert_eq!(pool.alloc(1), Err(PoolError::Exhausted));
        assert!(!pool.slots[0].occupied);
        assert_eq!(pool.slots[0].generation, u64::MAX);
    }

    #[test]
    fn double_free_is_rejected_not_panicking() {
        let mut pool: Pool<u32, 4> = Pool::new();
        let handle = pool.alloc(7).unwrap();
        assert_eq!(pool.free(handle), Ok(7));
        assert_eq!(pool.free(handle), Err(PoolError::InvalidHandle));
    }

    #[test]
    fn out_of_range_handle_is_rejected_not_panicking() {
        let mut pool: Pool<u32, 4> = Pool::new();
        let bogus = PoolHandle { index: 99, generation: 1 };
        assert_eq!(pool.free(bogus), Err(PoolError::InvalidHandle));
    }

    #[test]
    fn get_mut_returns_a_reference_to_the_stored_value_without_freeing_it() {
        let mut pool: Pool<u32, 4> = Pool::new();
        let handle = pool.alloc(42).unwrap();
        *pool.get_mut(handle).expect("handle is occupied") += 1;
        assert_eq!(pool.free(handle), Ok(43));
    }

    #[test]
    fn get_mut_rejects_freed_or_out_of_range_handles() {
        let mut pool: Pool<u32, 4> = Pool::new();
        let handle = pool.alloc(1).unwrap();
        pool.free(handle).unwrap();
        assert_eq!(pool.get_mut(handle), None);
        assert_eq!(pool.get_mut(PoolHandle { index: 99, generation: 1 }), None);
    }

    // TEST-P0-03-03-A: exhaustion fails closed, then recovers after a free.
    #[test]
    fn exhausted_pool_fails_closed_without_side_effects() {
        let mut pool: Pool<u32, 2> = Pool::new();
        let a = pool.alloc(1).unwrap();
        let _b = pool.alloc(2).unwrap();

        assert_eq!(pool.alloc(3), Err(PoolError::Exhausted));
        // Repeated exhaustion fails the same way every time, not just once.
        assert_eq!(pool.alloc(4), Err(PoolError::Exhausted));

        // Freeing one slot proves exhaustion was transient occupancy state,
        // not a poisoned/latched pool.
        assert_eq!(pool.free(a), Ok(1));
        let c = pool.alloc(5).expect("a freed slot should be allocatable again");
        assert_eq!(pool.free(c), Ok(5));
    }

    // ------------------------------------------------------------------
    // STORY-P0-03-01 / PERF-D07 performance-measurement harness.
    //
    // Host-tier, diagnostic-only benchmarks (`goals/performance/README.md`
    // measurement protocol #7: "Shared public CI checks catalogue integrity
    // and benchmark shape; threshold enforcement belongs on controlled
    // QEMU/HIL runners. Noisy shared-host timings are diagnostic only.").
    // This dev machine is a real x86_64 CPU (RDTSC reads real hardware
    // cycles, not QEMU/TCG-emulated ones), but the process is not
    // core-pinned, RT-prioritized, or isolated from the rest of the host
    // OS's scheduling noise, so none of the numbers these tests print gate
    // release on their own. They exist to produce real, traceable evidence
    // to sit alongside the Tier-0 (QEMU) fixture the full PERF-D07
    // measurement plan also calls for (not implemented in this pass — see
    // this story's Report for what remains open and why).
    //
    // Every number these tests print comes from an actual sample loop run
    // on this call to `cargo test` — nothing here is a canned/precomputed
    // figure.

    /// Reads the x86_64 timestamp counter.
    fn rdtsc() -> u64 {
        // SAFETY: `RDTSC` is unconditionally available on every x86_64 CPU;
        // reading the timestamp counter has no memory or control-flow side
        // effect this benchmark needs to account for.
        unsafe { core::arch::x86_64::_rdtsc() }
    }

    /// Calibrates RDTSC's own call overhead (measurement protocol #3:
    /// "Subtract calibrated timer/PMU read overhead") as the minimum
    /// back-to-back-read delta over `SAMPLES` samples — the minimum, not the
    /// mean, because any delta above the true minimum is itself overhead
    /// noise the calibration should not average away.
    fn calibrate_rdtsc_overhead() -> u64 {
        const SAMPLES: usize = 10_000;
        let mut min = u64::MAX;
        for _ in 0..SAMPLES {
            let a = rdtsc();
            let b = rdtsc();
            min = min.min(b.saturating_sub(a));
        }
        min
    }

    /// Nearest-rank percentile over an already-sorted sample slice.
    fn percentile(sorted: &[u64], p: f64) -> u64 {
        assert!(!sorted.is_empty());
        let rank = ((sorted.len() - 1) as f64 * p).round() as usize;
        sorted[rank]
    }

    // TEST-P0-03-01-PERF-A (PERF-D07-G01/G02/G03/G05/G06/G07/G13/G18): the
    // canonical work unit (measurement plan's choice) is `Pool<u64, 64>`
    // driven through 100,000 alloc/free round trips after a warm-up, IRQs
    // not applicable on host (no interrupts in a userspace test process).
    // G13 (queue residence) and G18 (warm reuse) are degenerate cases of
    // this identical series, per the plan: `Pool::alloc` is synchronous, so
    // enqueue-to-service collapses to call latency, and `free`-then-`alloc`
    // of the same slot is exactly what every iteration of this loop does.
    #[test]
    fn perf_pool_u64x64_alloc_free_round_trip_host_diagnostic() {
        const WARMUP: usize = 1_000;
        const SAMPLES: usize = 100_000;

        let rdtsc_overhead = calibrate_rdtsc_overhead();

        let mut pool: Pool<u64, 64> = Pool::new();
        let mut cycles: Vec<u64> = Vec::with_capacity(SAMPLES);
        let mut nanos: Vec<u64> = Vec::with_capacity(SAMPLES);

        for i in 0..(WARMUP + SAMPLES) {
            let value = i as u64;
            let t0 = std::time::Instant::now();
            let c0 = rdtsc();
            let handle = pool
                .alloc(value)
                .expect("Pool<u64,64> alloc immediately followed by free never exhausts");
            let freed = pool.free(handle).expect("handle just returned by alloc is always valid");
            let c1 = rdtsc();
            let t1 = std::time::Instant::now();
            assert_eq!(freed, value);

            if i >= WARMUP {
                cycles.push(c1.saturating_sub(c0).saturating_sub(rdtsc_overhead));
                nanos.push(t1.duration_since(t0).as_nanos() as u64);
            }
        }

        cycles.sort_unstable();
        nanos.sort_unstable();

        let cyc_p50 = percentile(&cycles, 0.50);
        let cyc_p99 = percentile(&cycles, 0.99);
        let cyc_p999 = percentile(&cycles, 0.999);
        let cyc_max = *cycles.last().unwrap();

        let ns_p50 = percentile(&nanos, 0.50);
        let ns_p99 = percentile(&nanos, 0.99);
        let ns_p999 = percentile(&nanos, 0.999);
        let ns_max = *nanos.last().unwrap();

        println!(
            "PERF-D07-G01..G07/G13/G18 pool_u64x64_alloc_free samples={} rdtsc_overhead_cycles={}",
            SAMPLES, rdtsc_overhead
        );
        println!(
            "  cycles: p50={} p99={} p99.9={} max={} jitter_p99_minus_p50={}",
            cyc_p50,
            cyc_p99,
            cyc_p999,
            cyc_max,
            cyc_p99.saturating_sub(cyc_p50)
        );
        println!("  nanos:  p50={} p99={} p99.9={} max={}", ns_p50, ns_p99, ns_p999, ns_max);
    }

    // TEST-P0-03-01-PERF-B (PERF-D07-G03 tail, raised to >=1,000,000 ops per
    // measurement protocol #4).
    #[test]
    fn perf_pool_u64x64_million_op_tail_host_diagnostic() {
        const WARMUP: usize = 1_000;
        const SAMPLES: usize = 1_000_000;

        let rdtsc_overhead = calibrate_rdtsc_overhead();
        let mut pool: Pool<u64, 64> = Pool::new();
        let mut cycles: Vec<u64> = Vec::with_capacity(SAMPLES);

        for i in 0..(WARMUP + SAMPLES) {
            let value = i as u64;
            let c0 = rdtsc();
            let handle = pool.alloc(value).unwrap();
            let freed = pool.free(handle).unwrap();
            let c1 = rdtsc();
            assert_eq!(freed, value);
            if i >= WARMUP {
                cycles.push(c1.saturating_sub(c0).saturating_sub(rdtsc_overhead));
            }
        }
        cycles.sort_unstable();

        println!(
            "PERF-D07-G03 pool_u64x64_million_op_tail samples={} rdtsc_overhead_cycles={} \
             (note: this is a userspace host process, not a bare-metal/QEMU environment — cache \
             state and IRQ-mask state are not applicable here the way they are in the planned T0 \
             fixture)",
            SAMPLES, rdtsc_overhead
        );
        println!(
            "  cycles: p50={} p99={} p99.9={} max={}",
            percentile(&cycles, 0.50),
            percentile(&cycles, 0.99),
            percentile(&cycles, 0.999),
            cycles.last().unwrap()
        );
    }

    // TEST-P0-03-01-PERF-C (PERF-D07-G04/G12/G14): occupancy-pattern
    // matrix — `Pool::alloc`'s cost is dominated by scan position, not just
    // call count, so best/middle/last-slot-free cases are timed separately
    // (measurement protocol #6: "never average them together"), plus the
    // O(1) free path and the exhaustion/recovery denial-then-retry path.
    #[test]
    fn perf_pool_u64x64_occupancy_pattern_host_diagnostic() {
        const TRIALS: usize = 10_000;
        let rdtsc_overhead = calibrate_rdtsc_overhead();

        // Case "best": slot 0 immediately free.
        let mut best_cycles: Vec<u64> = Vec::with_capacity(TRIALS);
        {
            let mut pool: Pool<u64, 64> = Pool::new();
            for i in 0..TRIALS {
                let c0 = rdtsc();
                let h = pool.alloc(i as u64).unwrap();
                let c1 = rdtsc();
                best_cycles.push(c1.saturating_sub(c0).saturating_sub(rdtsc_overhead));
                pool.free(h).unwrap();
            }
        }

        // Case "middle": slots 0..31 permanently occupied, alloc lands at 32.
        let mut middle_cycles: Vec<u64> = Vec::with_capacity(TRIALS);
        {
            let mut pool: Pool<u64, 64> = Pool::new();
            for i in 0..31 {
                pool.alloc(i as u64).unwrap();
            }
            for i in 0..TRIALS {
                let c0 = rdtsc();
                let h = pool.alloc(i as u64).unwrap();
                let c1 = rdtsc();
                middle_cycles.push(c1.saturating_sub(c0).saturating_sub(rdtsc_overhead));
                pool.free(h).unwrap();
            }
        }

        // Case "last": slots 0..62 permanently occupied, only slot 63 free
        // — the full 64-slot linear-scan cost.
        let mut last_cycles: Vec<u64> = Vec::with_capacity(TRIALS);
        {
            let mut pool: Pool<u64, 64> = Pool::new();
            for i in 0..63 {
                pool.alloc(i as u64).unwrap();
            }
            for i in 0..TRIALS {
                let c0 = rdtsc();
                let h = pool.alloc(i as u64).unwrap();
                let c1 = rdtsc();
                last_cycles.push(c1.saturating_sub(c0).saturating_sub(rdtsc_overhead));
                pool.free(h).unwrap();
            }
        }

        // Free-path timing: O(1) regardless of index, single series
        // suffices per the plan.
        let mut free_cycles: Vec<u64> = Vec::with_capacity(TRIALS);
        {
            let mut pool: Pool<u64, 64> = Pool::new();
            for i in 0..TRIALS {
                let h = pool.alloc(i as u64).unwrap();
                let c0 = rdtsc();
                pool.free(h).unwrap();
                let c1 = rdtsc();
                free_cycles.push(c1.saturating_sub(c0).saturating_sub(rdtsc_overhead));
            }
        }

        // Exhaustion-then-recovery: alloc into a full pool (denied), then
        // free one slot and alloc again (recovery), timed as one round
        // trip per protocol #6's "exhaustion" case.
        let mut recovery_cycles: Vec<u64> = Vec::with_capacity(TRIALS);
        {
            let mut pool: Pool<u64, 64> = Pool::new();
            let mut handles = Vec::with_capacity(64);
            for i in 0..64 {
                handles.push(pool.alloc(i as u64).unwrap());
            }
            for _ in 0..TRIALS {
                let c0 = rdtsc();
                let denied = pool.alloc(0);
                let h = handles.pop().unwrap();
                pool.free(h).unwrap();
                let recovered = pool.alloc(999);
                let c1 = rdtsc();
                assert_eq!(denied, Err(PoolError::Exhausted));
                let recovered = recovered.unwrap();
                recovery_cycles.push(c1.saturating_sub(c0).saturating_sub(rdtsc_overhead));
                handles.push(recovered);
            }
        }

        for (label, mut series) in [
            ("best_slot0", best_cycles),
            ("middle_slot32", middle_cycles),
            ("last_slot63", last_cycles),
            ("free_o1", free_cycles),
            ("deny_then_recover", recovery_cycles),
        ] {
            series.sort_unstable();
            println!(
                "PERF-D07-G04/G12/G14 pool_u64x64_occupancy[{}] n={} cycles: p50={} p99={} max={}",
                label,
                series.len(),
                percentile(&series, 0.50),
                percentile(&series, 0.99),
                series.last().unwrap()
            );
        }
    }

    // TEST-P0-03-01-PERF-D (PERF-D07-G20): denial-path latency plus
    // byte-for-byte state-change verification (occupied-bitmap and stored
    // values, snapshotted immediately before/after each denied call).
    #[test]
    fn perf_pool_u64x64_denial_paths_host_diagnostic() {
        const TRIALS: usize = 10_000;
        let rdtsc_overhead = calibrate_rdtsc_overhead();

        fn snapshot<const N: usize>(pool: &Pool<u64, N>) -> ([bool; N], [u64; N]) {
            let mut occupied = [false; N];
            let mut values = [0u64; N];
            for (i, slot) in pool.slots.iter().enumerate() {
                occupied[i] = slot.occupied;
                if slot.occupied {
                    // SAFETY: identical invariant to `Pool::get_mut` — this
                    // slot is occupied, so `slot.value` was fully
                    // initialized by a prior `alloc` and never invalidated.
                    values[i] = unsafe { *slot.value.assume_init_ref() };
                }
            }
            (occupied, values)
        }

        // Exhausted-denial case.
        let mut exhausted_cycles: Vec<u64> = Vec::with_capacity(TRIALS);
        {
            let mut pool: Pool<u64, 4> = Pool::new();
            for i in 0..4 {
                pool.alloc(i as u64).unwrap();
            }
            let mut state_changed = false;
            for _ in 0..TRIALS {
                let before = snapshot(&pool);
                let c0 = rdtsc();
                let r = pool.alloc(0xDEAD);
                let c1 = rdtsc();
                let after = snapshot(&pool);
                assert_eq!(r, Err(PoolError::Exhausted));
                if before != after {
                    state_changed = true;
                }
                exhausted_cycles.push(c1.saturating_sub(c0).saturating_sub(rdtsc_overhead));
            }
            assert!(!state_changed, "Exhausted denial must produce zero state changes");
        }

        // InvalidHandle-denial case (double-free of an already-freed
        // handle).
        let mut invalid_cycles: Vec<u64> = Vec::with_capacity(TRIALS);
        {
            let mut pool: Pool<u64, 4> = Pool::new();
            let h = pool.alloc(1).unwrap();
            pool.free(h).unwrap();
            let mut state_changed = false;
            for _ in 0..TRIALS {
                let before = snapshot(&pool);
                let c0 = rdtsc();
                let r = pool.free(h);
                let c1 = rdtsc();
                let after = snapshot(&pool);
                assert_eq!(r, Err(PoolError::InvalidHandle));
                if before != after {
                    state_changed = true;
                }
                invalid_cycles.push(c1.saturating_sub(c0).saturating_sub(rdtsc_overhead));
            }
            assert!(!state_changed, "InvalidHandle denial must produce zero state changes");
        }

        for (label, mut series) in
            [("exhausted", exhausted_cycles), ("invalid_handle", invalid_cycles)]
        {
            series.sort_unstable();
            println!(
                "PERF-D07-G20 pool_denial[{}] n={} state_changes=0 cycles: p50={} p99={} max={}",
                label,
                series.len(),
                percentile(&series, 0.50),
                percentile(&series, 0.99),
                series.last().unwrap()
            );
        }
    }

    // TEST-P0-03-01-PERF-E (PERF-D07-G21): 1,000 fill-to-exhaustion/drain
    // cycles against a drop-counting payload, asserting drop count exactly
    // matches alloc count every cycle (no leak, no double-drop) — extends
    // `dropping_a_pool_with_occupied_slots_drops_their_values`.
    #[test]
    fn perf_pool_exhaustion_drain_cycles_drop_accounting() {
        use core::cell::Cell;

        struct DropCounter<'a>(&'a Cell<u32>);
        impl Drop for DropCounter<'_> {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }

        const CYCLES: usize = 1_000;
        const N: usize = 16;
        let alloc_count = Cell::new(0u32);
        let drop_count = Cell::new(0u32);

        for _ in 0..CYCLES {
            let mut pool: Pool<DropCounter<'_>, N> = Pool::new();
            let mut handles = Vec::with_capacity(N);
            for _ in 0..N {
                handles.push(pool.alloc(DropCounter(&drop_count)).unwrap());
                alloc_count.set(alloc_count.get() + 1);
            }
            // The pool is now exhausted. `alloc` takes its argument by
            // value, so a *rejected* alloc still drops the value it was
            // handed right here (it was never stored, so this call frame is
            // the value's only owner) — this is `Pool::alloc`'s documented
            // "no side effects" contract holding for the *pool's own
            // internal state*, not a claim that the caller's argument
            // survives a denied call. Counted as one more value-in/drop-out
            // pair, not a leak.
            assert_eq!(pool.alloc(DropCounter(&drop_count)), Err(PoolError::Exhausted));
            alloc_count.set(alloc_count.get() + 1);
            for h in handles {
                pool.free(h).unwrap();
            }
            assert_eq!(drop_count.get(), alloc_count.get(), "no leak/double-drop mid-cycle");
        }

        assert_eq!(alloc_count.get(), drop_count.get());
        println!(
            "PERF-D07-G21 pool_exhaustion_drain_cycles cycles={} n_per_cycle={} total_allocs={} \
             total_drops={} leaks=0 double_drops=0",
            CYCLES,
            N,
            alloc_count.get(),
            drop_count.get()
        );
    }

    // TEST-P0-03-01-PERF-F (PERF-D07-G10): static-pool footprint via
    // `size_of` — direct evidence for the working-memory guardrail's
    // static-pool contribution.
    #[test]
    fn perf_pool_u64x64_size_of_host_diagnostic() {
        let bytes = core::mem::size_of::<Pool<u64, 64>>();
        println!("PERF-D07-G10 size_of::<Pool<u64,64>>() = {} bytes", bytes);
        assert!(bytes <= 8 * 1024, "static-pool contribution alone must fit the 8 KiB budget");
    }

    #[test]
    fn dropping_a_pool_with_occupied_slots_drops_their_values() {
        use core::cell::Cell;

        struct DropCounter<'a>(&'a Cell<u32>);
        impl Drop for DropCounter<'_> {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }

        let count = Cell::new(0);
        {
            let mut pool: Pool<DropCounter<'_>, 2> = Pool::new();
            let _a = pool.alloc(DropCounter(&count)).unwrap();
            let _b = pool.alloc(DropCounter(&count)).unwrap();
        }
        assert_eq!(count.get(), 2);
    }
}
