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
/// Opaque by design (a newtype over the slot index, per the newtype style
/// note in `agent/CODING_STANDARDS.md`) so callers can't construct a valid
/// handle out of thin air — only [`Pool::alloc`] hands one out, and
/// [`Pool::free`] rejects anything else that reaches it via
/// [`PoolError::InvalidHandle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PoolHandle {
    index: usize,
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
    /// Every slot is occupied; no side effects occurred.
    Exhausted,
    /// The handle does not identify a currently-occupied slot (already
    /// freed, or never issued by this pool).
    InvalidHandle,
}

struct Slot<T> {
    value: MaybeUninit<T>,
    occupied: bool,
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
            Slot { value: MaybeUninit::uninit(), occupied: false }
        }
        Pool { slots: [const { new_slot() }; N] }
    }

    /// Claims the first free slot and moves `value` into it.
    ///
    /// Fails closed with [`PoolError::Exhausted`] and no side effects if
    /// every slot is occupied — never panics, never blocks.
    pub fn alloc(&mut self, value: T) -> Result<PoolHandle, PoolError> {
        for (index, slot) in self.slots.iter_mut().enumerate() {
            if !slot.occupied {
                slot.value.write(value);
                slot.occupied = true;
                return Ok(PoolHandle { index });
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
        if !slot.occupied {
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
            (PoolHandle { index }, unsafe { slot.value.assume_init_ref() })
        })
    }

    /// Returns ownership of the value at `handle` and frees its slot.
    ///
    /// Fails closed with [`PoolError::InvalidHandle`] (never a panic or a
    /// stale/aliased read) if `handle` is out of range or already free.
    pub fn free(&mut self, handle: PoolHandle) -> Result<T, PoolError> {
        let slot = self.slots.get_mut(handle.index).ok_or(PoolError::InvalidHandle)?;
        if !slot.occupied {
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
        assert_eq!(pool.free(handle2), Ok(2));
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
        let bogus = PoolHandle { index: 99 };
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
        assert_eq!(pool.get_mut(PoolHandle { index: 99 }), None);
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
