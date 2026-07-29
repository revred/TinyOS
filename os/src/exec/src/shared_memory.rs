//! Shared-memory region handle exchange between two tasks (`STORY-P0-07-02`).
//!
//! [`grant`] maps `pages` of one task's (the owner's) already-mapped,
//! page-aligned region into a second task's (the sharee's)
//! [`crate::address_space::AddressSpace`] at an address the owner
//! explicitly chooses — never ambiently visible to a third task, and never
//! with broader permissions on the sharee's side than the owner's own page
//! actually grants (a sharee can't be handed write/execute access the
//! owner's own mapping doesn't have). The returned [`SharedGrant`] is the
//! only way to [`revoke`] it: only the code holding both the grant *and*
//! the owner's own `TaskId` can revoke, so possessing a `SharedGrant` plus
//! matching identity is this Story's own capability token, mirroring
//! `crate::win32_shim`'s `CapabilityPolicy` precedent at a lighter weight
//! (no policy engine needed here — grant ownership is enforced by
//! `TaskId` identity directly, since there is exactly one owner per grant).

use hal_x86_64::paging::{MappedPage, PAGE_SIZE};
use kernel::sched::TaskId;

use crate::address_space::{AddressSpace, AddressSpaceError, KERNEL_RESERVED_REGION_END};
use crate::pe::Permissions;

/// Errors [`grant`]/[`revoke`] fail closed with, per
/// `agent/CODING_STANDARDS.md`'s "no stringly-typed errors" rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharedMemoryError {
    /// `owner_virt`/`sharee_virt` isn't [`PAGE_SIZE`]-aligned.
    Misaligned,
    /// `sharee_virt`'s range overlaps the kernel's own reserved region.
    KernelRegionCollision,
    /// The owner doesn't actually have every requested page mapped in its
    /// own address space — nothing to share.
    RegionNotOwned,
    /// The sharee's requested permissions exceed what the owner's own
    /// mapping grants (e.g. requesting write access to a page the owner
    /// itself only has read-only) — a grant can never escalate privilege.
    PermissionsExceedOwner,
    /// The sharee already has a mapping somewhere in the requested range —
    /// never silently overwritten.
    AlreadyGranted,
    /// The sharee's frame pool had no slot left for a new page-table level.
    FrameExhausted,
    /// [`revoke`] was called by a `TaskId` that isn't this grant's owner.
    NotOwner,
    /// [`revoke`] found part of the grant's range already unmapped —
    /// revoked (at least partially) already, or never fully granted.
    NotGranted,
    /// `grant` was asked for zero pages — not a region, nothing to share.
    ZeroPages,
    /// [`GrantRegistry`] had no free slot to record a new live grant.
    RegistryExhausted,
    /// [`revoke`] found a *different* live grant now occupying this token's
    /// `sharee_virt` (the address was revoked and re-granted since this
    /// token was issued) — the token's generation no longer matches the
    /// registry's current record, so it is rejected rather than tearing
    /// down an unrelated later grant that happens to reuse the address.
    StaleGrant,
    /// `owner_virt`/`sharee_virt` plus `pages` describes a range whose page
    /// addresses do not all fit in a 64-bit address space (`STORY-P0-07-03`).
    ///
    /// Both bases and the count are caller-chosen, so this is a *request*
    /// defect rather than a kernel-state defect: it is rejected before any
    /// page is inspected or mapped.
    RangeOverflow,
}

impl From<AddressSpaceError> for SharedMemoryError {
    fn from(err: AddressSpaceError) -> Self {
        match err {
            AddressSpaceError::FrameExhausted => SharedMemoryError::FrameExhausted,
            AddressSpaceError::NotMapped => SharedMemoryError::NotGranted,
            _ => SharedMemoryError::AlreadyGranted,
        }
    }
}

/// A live grant of `pages` pages at `sharee_virt` in some sharee's
/// [`AddressSpace`], owned by `owner` — the only value [`revoke`] accepts,
/// and only from the same `owner`.
///
/// `generation` is this grant's own identity within a [`GrantRegistry`]: it
/// distinguishes this specific `grant` call from any other (past or future)
/// grant that happens to reuse the same `sharee_virt` after this one is
/// revoked, so a stale token can never be mistaken for — and used to tear
/// down — an unrelated later grant.
///
/// **Range invariant.** [`grant`] is this type's only constructor and it
/// rejects any request whose page addresses do not all fit in a 64-bit
/// address space ([`SharedMemoryError::RangeOverflow`]). So a `SharedGrant`
/// that exists describes a representable range, and [`revoke`] may walk it
/// with plain arithmetic rather than re-deriving a guarantee that was already
/// established at issue time — a check that cannot fail is dead code, and
/// dead code on a kernel path reads like a real guard (`STORY-P0-07-03`).
pub struct SharedGrant {
    owner: TaskId,
    sharee_virt: u64,
    pages: usize,
    generation: u64,
}

impl SharedGrant {
    /// The virtual address this grant occupies in the sharee's address
    /// space.
    pub const fn sharee_virt(&self) -> u64 {
        self.sharee_virt
    }

    /// How many pages this grant spans.
    pub const fn pages(&self) -> usize {
        self.pages
    }
}

/// One [`GrantRegistry`] slot: the live record backing a still-valid
/// [`SharedGrant`] token.
#[derive(Clone, Copy)]
struct GrantRecord {
    sharee_virt: u64,
    generation: u64,
}

/// Tracks every currently-live grant into one sharee [`AddressSpace`],
/// fixed-capacity and no-heap like the rest of this crate. [`grant`] records
/// one entry here before returning a [`SharedGrant`]; [`revoke`] consults it
/// to confirm the token presented still names the registry's current
/// occupant of that `sharee_virt` (see [`SharedGrant`]'s own doc comment)
/// before removing the entry and unmapping.
pub struct GrantRegistry<const CAPACITY: usize> {
    next_generation: u64,
    entries: [Option<GrantRecord>; CAPACITY],
}

impl<const CAPACITY: usize> GrantRegistry<CAPACITY> {
    /// An empty registry with no live grants.
    pub const fn new() -> Self {
        Self { next_generation: 0, entries: [None; CAPACITY] }
    }

    fn insert(&mut self, record: GrantRecord) -> Result<(), SharedMemoryError> {
        for slot in &mut self.entries {
            if slot.is_none() {
                *slot = Some(record);
                return Ok(());
            }
        }
        Err(SharedMemoryError::RegistryExhausted)
    }

    /// Removes the live record matching `grant`'s `sharee_virt`, failing
    /// closed with [`SharedMemoryError::NotGranted`] if none exists or
    /// [`SharedMemoryError::StaleGrant`] if a *different* generation now
    /// occupies that address.
    fn remove_matching(&mut self, grant: &SharedGrant) -> Result<(), SharedMemoryError> {
        for slot in &mut self.entries {
            if let Some(record) = slot {
                if record.sharee_virt == grant.sharee_virt {
                    if record.generation != grant.generation {
                        return Err(SharedMemoryError::StaleGrant);
                    }
                    *slot = None;
                    return Ok(());
                }
            }
        }
        Err(SharedMemoryError::NotGranted)
    }
}

impl<const CAPACITY: usize> Default for GrantRegistry<CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}

/// The region-shaped half of a [`grant`] call — `owner_virt`, `sharee_virt`,
/// `pages`, and `sharee_permissions` bundled together so `grant` itself
/// stays under clippy's argument-count ceiling (splitting owner/sharee
/// *space* parameters, which each carry their own const-generic `FRAMES`,
/// from this plain-data region description).
pub struct GrantRequest {
    /// Start of the owner's already-mapped source region.
    pub owner_virt: u64,
    /// Where the region should appear in the sharee's address space.
    pub sharee_virt: u64,
    /// How many contiguous pages to share.
    pub pages: usize,
    /// Permissions to grant the sharee — must not exceed the owner's own.
    pub sharee_permissions: Permissions,
}

/// Grants `request.pages` pages of `owner`'s already-mapped region starting
/// at `request.owner_virt` into `sharee_space` at `request.sharee_virt`,
/// with `request.sharee_permissions` (which must not exceed what the
/// owner's own mapping already grants).
///
/// Fails closed with no partial mapping on the *validation* half (every
/// owner page's existence/permission headroom, and the sharee's whole
/// target range being currently unmapped, are checked before any page is
/// mapped into the sharee) — see this module's own doc comment for why a
/// grant token, not an ambient side effect, is the only way back in.
///
/// The *mapping* half is transactional too: if a later page in the range
/// fails to map (e.g. [`SharedMemoryError::FrameExhausted`] because the
/// sharee's frame pool runs out partway through), every page already mapped
/// by this same call is unmapped again before the error is returned — a
/// failed `grant` never leaves the sharee with a partial, half-usable
/// region. `registry` records the successful grant so [`revoke`] can later
/// confirm a token against it; a full `registry` also rolls the mapping
/// back and fails closed with [`SharedMemoryError::RegistryExhausted`].
///
/// **This function does not panic** (`STORY-P0-07-03`, closing `LE-40`). It
/// is a kernel path, and `agent/CODING_STANDARDS.md` puts fail-safe above
/// keep-trying: every reachable defect — including a caller-chosen range that
/// does not fit in the address space, [`SharedMemoryError::RangeOverflow`] —
/// returns an error with nothing mapped. A test gates the absence of explicit
/// panic constructs on this module's non-test path.
pub fn grant<
    const OWNER_FRAMES: usize,
    const SHAREE_FRAMES: usize,
    const REGISTRY_CAPACITY: usize,
>(
    owner: TaskId,
    owner_space: &AddressSpace<'_, OWNER_FRAMES>,
    sharee_space: &mut AddressSpace<'_, SHAREE_FRAMES>,
    request: GrantRequest,
    registry: &mut GrantRegistry<REGISTRY_CAPACITY>,
) -> Result<SharedGrant, SharedMemoryError> {
    let GrantRequest { owner_virt, sharee_virt, pages, sharee_permissions } = request;
    if pages == 0 {
        return Err(SharedMemoryError::ZeroPages);
    }
    if !owner_virt.is_multiple_of(PAGE_SIZE) || !sharee_virt.is_multiple_of(PAGE_SIZE) {
        return Err(SharedMemoryError::Misaligned);
    }
    if sharee_virt < KERNEL_RESERVED_REGION_END {
        return Err(SharedMemoryError::KernelRegionCollision);
    }
    // Every page address the three loops below will compute, checked before
    // any of them computes one. `owner_virt`, `sharee_virt` and `pages` are
    // all caller-chosen and the loops index off them with plain `+`/`*`;
    // before `STORY-P0-07-03` they did so unchecked, so a page-aligned
    // `sharee_virt` near the top of the address space panicked on the second
    // page in a debug build, and in a release build wrapped silently to a low
    // address that the kernel-region check immediately above had already
    // passed. `pages >= 1` holds here, so `pages - 1` cannot underflow, and
    // the last page's offset is the largest either loop will ever add.
    //
    // ESTABLISHED HERE, RELIED ON BELOW: after this point every
    // `owner_virt + i * PAGE_SIZE` and `sharee_virt + i * PAGE_SIZE` for
    // `i < pages` is representable, which is why those loops may keep using
    // plain arithmetic. This is stated rather than assumed, because an
    // unstated invariant is what `LE-40` was.
    let last_offset = ((pages - 1) as u64)
        .checked_mul(PAGE_SIZE)
        .ok_or(SharedMemoryError::RangeOverflow)?;
    owner_virt.checked_add(last_offset).ok_or(SharedMemoryError::RangeOverflow)?;
    sharee_virt.checked_add(last_offset).ok_or(SharedMemoryError::RangeOverflow)?;

    for i in 0..pages {
        grantable_owner_page(owner_space, owner_virt + i as u64 * PAGE_SIZE, sharee_permissions)?;
    }
    for i in 0..pages {
        if sharee_space.translate(sharee_virt + i as u64 * PAGE_SIZE).is_some() {
            return Err(SharedMemoryError::AlreadyGranted);
        }
    }

    let mut mapped = 0usize;
    for i in 0..pages {
        let offset = i as u64 * PAGE_SIZE;
        // Re-read rather than collected into a fixed-size buffer up front,
        // which would need a `pages`-sized scratch array for an arbitrary
        // caller-chosen count.
        //
        // WHY THE RE-READ AGREES WITH THE LOOP ABOVE, stated because it was
        // once unstated (`LE-40`): `owner_space` is held here as a SHARED
        // borrow (`&AddressSpace<'_, OWNER_FRAMES>`), so no `&mut` can alias
        // it, and this kernel is single-core with no page-table mutation from
        // an interrupt path. There is no window between check and use — this
        // is not a TOCTOU, and it was once reported as one.
        //
        // TWO THINGS WOULD INVALIDATE THAT, SILENTLY:
        //   1. SMP. Another core mutating the owner's tables.
        //   2. Page-table structure shared between the two spaces covering
        //      `owner_virt` — `attach_shared_pd` makes that possible in
        //      principle. No reachable case has been constructed, and Rust's
        //      borrow rules already prevent the obvious one.
        //
        // So the re-read is checked, not asserted (`STORY-P0-07-03`). It runs
        // the SAME grantability verdict as the loop above — presence AND
        // authority — because under either condition the permission half is
        // as stale as the presence half, and the old code panicked on the
        // first while mapping the page regardless of the second. A rejected
        // re-read rolls this call's mapping back and fails closed, exactly as
        // frame exhaustion does two lines below. `LE-40` required this to
        // land BEFORE any SMP work; it has.
        let owner_page =
            match grantable_owner_page(owner_space, owner_virt + offset, sharee_permissions) {
                Ok(page) => page,
                Err(err) => {
                    unmap_prefix(sharee_space, sharee_virt, mapped);
                    return Err(err);
                }
            };
        if let Err(err) =
            sharee_space.map_page(sharee_virt + offset, owner_page.phys, sharee_permissions)
        {
            unmap_prefix(sharee_space, sharee_virt, mapped);
            return Err(err.into());
        }
        mapped += 1;
    }

    let generation = registry.next_generation;
    // The last arithmetic on this path. Generations are never reused — that
    // is the whole basis of `StaleGrant` — so the counter must not wrap
    // (a wrapped generation makes a stale token match a later grant) and must
    // not saturate (every subsequent grant would share one generation, with
    // the same effect). Refusing the grant is the only disposition that keeps
    // the token's meaning intact, so an exhausted counter is a full registry.
    let Some(next_generation) = generation.checked_add(1) else {
        unmap_prefix(sharee_space, sharee_virt, mapped);
        return Err(SharedMemoryError::RegistryExhausted);
    };
    if let Err(err) = registry.insert(GrantRecord { sharee_virt, generation }) {
        unmap_prefix(sharee_space, sharee_virt, mapped);
        return Err(err);
    }
    registry.next_generation = next_generation;

    Ok(SharedGrant { owner, sharee_virt, pages, generation })
}

/// Reads `virt`'s mapping in `owner_space` and decides whether it may back a
/// grant carrying `sharee_permissions` — the **single** definition of "this
/// page is grantable", called by both of [`grant`]'s owner-facing loops.
///
/// One function rather than two because the two loops previously disagreed
/// about what they were checking: the first asked both questions, and the
/// second re-read the translation, `.expect`ed its presence and re-checked
/// its permissions **not at all** (`LE-40`). Presence and authority are one
/// verdict about one page, so they are read together or not at all.
///
/// Fails closed with [`SharedMemoryError::RegionNotOwned`] if `virt` is not
/// mapped in `owner_space`, or [`SharedMemoryError::PermissionsExceedOwner`]
/// if the owner's own page does not already carry the write/execute
/// authority the sharee is asking for — a grant can never escalate.
fn grantable_owner_page<const OWNER_FRAMES: usize>(
    owner_space: &AddressSpace<'_, OWNER_FRAMES>,
    virt: u64,
    sharee_permissions: Permissions,
) -> Result<MappedPage, SharedMemoryError> {
    let owner_page = owner_space.translate(virt).ok_or(SharedMemoryError::RegionNotOwned)?;
    if sharee_permissions.write && !owner_page.writable {
        return Err(SharedMemoryError::PermissionsExceedOwner);
    }
    if sharee_permissions.execute && !owner_page.executable {
        return Err(SharedMemoryError::PermissionsExceedOwner);
    }
    Ok(owner_page)
}

/// Unmaps the first `count` pages of a grant-in-progress starting at
/// `sharee_virt` — the rollback [`grant`] runs when a later page (or the
/// registry insert) fails partway through, so a failed call never leaves a
/// partial mapping behind.
fn unmap_prefix<const SHAREE_FRAMES: usize>(
    sharee_space: &mut AddressSpace<'_, SHAREE_FRAMES>,
    sharee_virt: u64,
    count: usize,
) {
    for i in 0..count {
        // Every one of these was just mapped by this same call, so
        // unmapping it back out cannot fail; if it somehow did, there is no
        // safer alternative action than leaving the rest of the rollback to
        // run and surfacing the original error, so the result is ignored.
        let _ = sharee_space.unmap_page(sharee_virt + i as u64 * PAGE_SIZE);
    }
}

/// Revokes `grant`, unmapping its whole range from `sharee_space` — the
/// deterministic teardown `STORY-P0-07-02` acceptance criterion 2 requires:
/// no path leaves a stale mapping the sharee could still read/write
/// afterward.
///
/// Takes `grant` by reference, not by value: a rejected call (wrong
/// `caller`) must not consume the caller's only token, or the real owner
/// would lose the ability to revoke on a future, correctly-authorized
/// attempt. Fails closed with [`SharedMemoryError::NotOwner`] if `caller`
/// isn't this grant's own owner, or [`SharedMemoryError::StaleGrant`] if
/// `registry` shows a *different* grant now occupies this token's
/// `sharee_virt` (see [`SharedGrant`]'s own doc comment).
pub fn revoke<const SHAREE_FRAMES: usize, const REGISTRY_CAPACITY: usize>(
    caller: TaskId,
    grant: &SharedGrant,
    sharee_space: &mut AddressSpace<'_, SHAREE_FRAMES>,
    registry: &mut GrantRegistry<REGISTRY_CAPACITY>,
) -> Result<(), SharedMemoryError> {
    if caller != grant.owner {
        return Err(SharedMemoryError::NotOwner);
    }
    registry.remove_matching(grant)?;
    for i in 0..grant.pages {
        sharee_space.unmap_page(grant.sharee_virt + i as u64 * PAGE_SIZE)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pe::SectionDescriptor;
    use hal_x86_64::paging::PageTable;
    use kernel::mem::Pool;
    use kernel::sched::{OverrunPolicy, Priority, Scheduler, WcetBudgetTicks};

    const IMAGE_BASE: u64 = 0x1_4000_0000;
    const SHAREE_VIRT: u64 = 0x1_5000_0000;
    const RW: Permissions = Permissions { read: true, write: true, execute: false };
    const RX: Permissions = Permissions { read: true, write: false, execute: true };
    const RO: Permissions = Permissions { read: true, write: false, execute: false };

    #[repr(C, align(4096))]
    struct AlignedPages([u8; 8192]);

    #[allow(clippy::empty_loop)]
    extern "C" fn dummy_entry() -> ! {
        loop {}
    }

    fn priority(value: u8) -> Priority {
        Priority::try_new(value).expect("value is in range")
    }

    fn two_tasks() -> (TaskId, TaskId) {
        let mut sched: Scheduler<4> = Scheduler::new();
        let owner = sched
            .create_task(
                priority(1),
                WcetBudgetTicks(1000),
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();
        let sharee = sched
            .create_task(
                priority(1),
                WcetBudgetTicks(1000),
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();
        (owner, sharee)
    }

    fn owner_space_with_one_rw_page<'a>(
        bytes: &'a AlignedPages,
        staging: &'a mut AlignedPages,
        pml4: &'a mut PageTable,
        frame_pool: &'a mut Pool<PageTable, 8>,
    ) -> AddressSpace<'a, 8> {
        let sections = [SectionDescriptor {
            virtual_address: 0,
            virtual_size: PAGE_SIZE as u32,
            file_offset: 0,
            file_size: PAGE_SIZE as u32,
            permissions: RW,
        }];
        AddressSpace::create(pml4, frame_pool, &sections, IMAGE_BASE, &bytes.0, &mut staging.0)
            .unwrap()
    }

    /// An empty `AddressSpace` (no sections) — `AddressSpace` has no public
    /// constructor other than `create`, so an empty section set is the
    /// standard way to build a fresh, otherwise-untouched space to grant
    /// into (or, in one test, to prove a region can't be shared out of).
    fn empty_space<'a>(
        bytes: &'a AlignedPages,
        staging: &'a mut AlignedPages,
        pml4: &'a mut PageTable,
        frame_pool: &'a mut Pool<PageTable, 8>,
    ) -> AddressSpace<'a, 8> {
        AddressSpace::create(pml4, frame_pool, &[], IMAGE_BASE, &bytes.0, &mut staging.0).unwrap()
    }

    // STORY-P0-07-02 AC1: a well-formed grant maps the sharee's page to the
    // owner's own backing frame, with the requested (not-broader)
    // permissions.
    #[test]
    fn a_well_formed_grant_maps_the_sharee_to_the_owners_backing_frame() {
        let (owner, _sharee) = two_tasks();
        let bytes = AlignedPages([0; 8192]);
        let mut owner_staging = AlignedPages([0; 8192]);
        let mut owner_pml4 = PageTable::new();
        let mut owner_frames: Pool<PageTable, 8> = Pool::new();
        let owner_space = owner_space_with_one_rw_page(
            &bytes,
            &mut owner_staging,
            &mut owner_pml4,
            &mut owner_frames,
        );

        let sharee_bytes = AlignedPages([0; 8192]);
        let mut sharee_staging = AlignedPages([0; 8192]);
        let mut sharee_pml4 = PageTable::new();
        let mut sharee_frames: Pool<PageTable, 8> = Pool::new();
        let mut sharee_space =
            empty_space(&sharee_bytes, &mut sharee_staging, &mut sharee_pml4, &mut sharee_frames);

        let mut registry: GrantRegistry<4> = GrantRegistry::new();
        let result = grant(
            owner,
            &owner_space,
            &mut sharee_space,
            GrantRequest {
                owner_virt: IMAGE_BASE,
                sharee_virt: SHAREE_VIRT,
                pages: 1,
                sharee_permissions: RO,
            },
            &mut registry,
        );
        assert!(result.is_ok());

        let owner_page = owner_space.translate(IMAGE_BASE).unwrap();
        let sharee_page = sharee_space.translate(SHAREE_VIRT).unwrap();
        assert_eq!(sharee_page.phys, owner_page.phys);
        assert!(!sharee_page.writable, "requested RO must not become writable");
    }

    // STORY-P0-07-02 AC1: a grant requesting broader permissions than the
    // owner's own page has is rejected.
    #[test]
    fn a_grant_requesting_write_beyond_the_owners_read_only_page_is_rejected() {
        let (owner, _sharee) = two_tasks();
        let bytes = AlignedPages([0; 8192]);
        let mut owner_staging = AlignedPages([0; 8192]);
        let mut owner_pml4 = PageTable::new();
        let mut owner_frames: Pool<PageTable, 8> = Pool::new();
        let sections = [SectionDescriptor {
            virtual_address: 0,
            virtual_size: PAGE_SIZE as u32,
            file_offset: 0,
            file_size: PAGE_SIZE as u32,
            permissions: RX,
        }];
        let owner_space = AddressSpace::create(
            &mut owner_pml4,
            &mut owner_frames,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut owner_staging.0,
        )
        .unwrap();

        let sharee_bytes = AlignedPages([0; 8192]);
        let mut sharee_staging = AlignedPages([0; 8192]);
        let mut sharee_pml4 = PageTable::new();
        let mut sharee_frames: Pool<PageTable, 8> = Pool::new();
        let mut sharee_space =
            empty_space(&sharee_bytes, &mut sharee_staging, &mut sharee_pml4, &mut sharee_frames);

        let mut registry: GrantRegistry<4> = GrantRegistry::new();
        let result = grant(
            owner,
            &owner_space,
            &mut sharee_space,
            GrantRequest {
                owner_virt: IMAGE_BASE,
                sharee_virt: SHAREE_VIRT,
                pages: 1,
                sharee_permissions: RW,
            },
            &mut registry,
        );
        assert_eq!(result.err(), Some(SharedMemoryError::PermissionsExceedOwner));
        assert_eq!(sharee_space.translate(SHAREE_VIRT), None, "a rejected grant maps nothing");
    }

    // A region the owner doesn't actually have mapped can't be shared.
    #[test]
    fn granting_an_unmapped_owner_region_is_rejected() {
        let (owner, _sharee) = two_tasks();
        let owner_bytes = AlignedPages([0; 8192]);
        let mut owner_staging = AlignedPages([0; 8192]);
        let mut owner_pml4 = PageTable::new();
        let mut owner_frames: Pool<PageTable, 8> = Pool::new();
        let owner_space =
            empty_space(&owner_bytes, &mut owner_staging, &mut owner_pml4, &mut owner_frames);

        let sharee_bytes = AlignedPages([0; 8192]);
        let mut sharee_staging = AlignedPages([0; 8192]);
        let mut sharee_pml4 = PageTable::new();
        let mut sharee_frames: Pool<PageTable, 8> = Pool::new();
        let mut sharee_space =
            empty_space(&sharee_bytes, &mut sharee_staging, &mut sharee_pml4, &mut sharee_frames);

        let mut registry: GrantRegistry<4> = GrantRegistry::new();
        let result = grant(
            owner,
            &owner_space,
            &mut sharee_space,
            GrantRequest {
                owner_virt: IMAGE_BASE,
                sharee_virt: SHAREE_VIRT,
                pages: 1,
                sharee_permissions: RO,
            },
            &mut registry,
        );
        assert_eq!(result.err(), Some(SharedMemoryError::RegionNotOwned));
    }

    // A region already granted (the sharee already has something mapped
    // there) is never silently overwritten.
    #[test]
    fn granting_into_an_already_mapped_sharee_region_is_rejected() {
        let (owner, _sharee) = two_tasks();
        let bytes = AlignedPages([0; 8192]);
        let mut owner_staging = AlignedPages([0; 8192]);
        let mut owner_pml4 = PageTable::new();
        let mut owner_frames: Pool<PageTable, 8> = Pool::new();
        let owner_space = owner_space_with_one_rw_page(
            &bytes,
            &mut owner_staging,
            &mut owner_pml4,
            &mut owner_frames,
        );

        let sharee_bytes = AlignedPages([0; 8192]);
        let mut sharee_staging = AlignedPages([0; 8192]);
        let mut sharee_pml4 = PageTable::new();
        let mut sharee_frames: Pool<PageTable, 8> = Pool::new();
        let mut sharee_space =
            empty_space(&sharee_bytes, &mut sharee_staging, &mut sharee_pml4, &mut sharee_frames);
        sharee_space.map_page(SHAREE_VIRT, 0xdead_b000, RO).unwrap();

        let mut registry: GrantRegistry<4> = GrantRegistry::new();
        let result = grant(
            owner,
            &owner_space,
            &mut sharee_space,
            GrantRequest {
                owner_virt: IMAGE_BASE,
                sharee_virt: SHAREE_VIRT,
                pages: 1,
                sharee_permissions: RO,
            },
            &mut registry,
        );
        assert_eq!(result.err(), Some(SharedMemoryError::AlreadyGranted));
    }

    // A `sharee_virt` inside the kernel's own reserved region is rejected.
    #[test]
    fn granting_into_the_kernel_reserved_region_is_rejected() {
        let (owner, _sharee) = two_tasks();
        let bytes = AlignedPages([0; 8192]);
        let mut owner_staging = AlignedPages([0; 8192]);
        let mut owner_pml4 = PageTable::new();
        let mut owner_frames: Pool<PageTable, 8> = Pool::new();
        let owner_space = owner_space_with_one_rw_page(
            &bytes,
            &mut owner_staging,
            &mut owner_pml4,
            &mut owner_frames,
        );

        let sharee_bytes = AlignedPages([0; 8192]);
        let mut sharee_staging = AlignedPages([0; 8192]);
        let mut sharee_pml4 = PageTable::new();
        let mut sharee_frames: Pool<PageTable, 8> = Pool::new();
        let mut sharee_space =
            empty_space(&sharee_bytes, &mut sharee_staging, &mut sharee_pml4, &mut sharee_frames);

        let mut registry: GrantRegistry<4> = GrantRegistry::new();
        let result = grant(
            owner,
            &owner_space,
            &mut sharee_space,
            GrantRequest {
                owner_virt: IMAGE_BASE,
                sharee_virt: 0,
                pages: 1,
                sharee_permissions: RO,
            },
            &mut registry,
        );
        assert_eq!(result.err(), Some(SharedMemoryError::KernelRegionCollision));
    }

    // STORY-P0-07-02 AC2: revoking a grant unmaps it from the sharee
    // deterministically — no stale mapping survives.
    #[test]
    fn revoking_a_grant_unmaps_it_from_the_sharee() {
        let (owner, _sharee) = two_tasks();
        let bytes = AlignedPages([0; 8192]);
        let mut owner_staging = AlignedPages([0; 8192]);
        let mut owner_pml4 = PageTable::new();
        let mut owner_frames: Pool<PageTable, 8> = Pool::new();
        let owner_space = owner_space_with_one_rw_page(
            &bytes,
            &mut owner_staging,
            &mut owner_pml4,
            &mut owner_frames,
        );

        let sharee_bytes = AlignedPages([0; 8192]);
        let mut sharee_staging = AlignedPages([0; 8192]);
        let mut sharee_pml4 = PageTable::new();
        let mut sharee_frames: Pool<PageTable, 8> = Pool::new();
        let mut sharee_space =
            empty_space(&sharee_bytes, &mut sharee_staging, &mut sharee_pml4, &mut sharee_frames);
        let mut registry: GrantRegistry<4> = GrantRegistry::new();
        let live_grant = grant(
            owner,
            &owner_space,
            &mut sharee_space,
            GrantRequest {
                owner_virt: IMAGE_BASE,
                sharee_virt: SHAREE_VIRT,
                pages: 1,
                sharee_permissions: RO,
            },
            &mut registry,
        )
        .unwrap();
        assert!(sharee_space.translate(SHAREE_VIRT).is_some());

        assert_eq!(revoke(owner, &live_grant, &mut sharee_space, &mut registry), Ok(()));
        assert_eq!(sharee_space.translate(SHAREE_VIRT), None);
    }

    // STORY-P0-07-02 AC3: revocation by a non-owner is rejected, and the
    // mapping survives the rejected attempt.
    #[test]
    fn revocation_by_a_non_owner_is_rejected() {
        let (owner, sharee) = two_tasks();
        let bytes = AlignedPages([0; 8192]);
        let mut owner_staging = AlignedPages([0; 8192]);
        let mut owner_pml4 = PageTable::new();
        let mut owner_frames: Pool<PageTable, 8> = Pool::new();
        let owner_space = owner_space_with_one_rw_page(
            &bytes,
            &mut owner_staging,
            &mut owner_pml4,
            &mut owner_frames,
        );

        let sharee_bytes = AlignedPages([0; 8192]);
        let mut sharee_staging = AlignedPages([0; 8192]);
        let mut sharee_pml4 = PageTable::new();
        let mut sharee_frames: Pool<PageTable, 8> = Pool::new();
        let mut sharee_space =
            empty_space(&sharee_bytes, &mut sharee_staging, &mut sharee_pml4, &mut sharee_frames);
        let mut registry: GrantRegistry<4> = GrantRegistry::new();
        let live_grant = grant(
            owner,
            &owner_space,
            &mut sharee_space,
            GrantRequest {
                owner_virt: IMAGE_BASE,
                sharee_virt: SHAREE_VIRT,
                pages: 1,
                sharee_permissions: RO,
            },
            &mut registry,
        )
        .unwrap();

        assert_eq!(
            revoke(sharee, &live_grant, &mut sharee_space, &mut registry).err(),
            Some(SharedMemoryError::NotOwner)
        );
        assert!(sharee_space.translate(SHAREE_VIRT).is_some(), "rejected revoke must not unmap");
    }

    // A `grant` for zero pages is not a region and is rejected outright.
    #[test]
    fn granting_zero_pages_is_rejected() {
        let (owner, _sharee) = two_tasks();
        let bytes = AlignedPages([0; 8192]);
        let mut owner_staging = AlignedPages([0; 8192]);
        let mut owner_pml4 = PageTable::new();
        let mut owner_frames: Pool<PageTable, 8> = Pool::new();
        let owner_space = owner_space_with_one_rw_page(
            &bytes,
            &mut owner_staging,
            &mut owner_pml4,
            &mut owner_frames,
        );

        let sharee_bytes = AlignedPages([0; 8192]);
        let mut sharee_staging = AlignedPages([0; 8192]);
        let mut sharee_pml4 = PageTable::new();
        let mut sharee_frames: Pool<PageTable, 8> = Pool::new();
        let mut sharee_space =
            empty_space(&sharee_bytes, &mut sharee_staging, &mut sharee_pml4, &mut sharee_frames);

        let mut registry: GrantRegistry<4> = GrantRegistry::new();
        let result = grant(
            owner,
            &owner_space,
            &mut sharee_space,
            GrantRequest {
                owner_virt: IMAGE_BASE,
                sharee_virt: SHAREE_VIRT,
                pages: 0,
                sharee_permissions: RO,
            },
            &mut registry,
        );
        assert_eq!(result.err(), Some(SharedMemoryError::ZeroPages));
    }

    // A `FrameExhausted` failure partway through a multi-page grant rolls
    // back every page that call already mapped — no partial region is left
    // behind for the sharee to read. `sharee_page_0` is deliberately the
    // last 4KiB page of one 2MiB page-table block and `sharee_page_1` the
    // first page of the next: mapping page 0 into a brand-new sharee space
    // allocates a fresh PDPT+PD+PT (3 frames); mapping page 1 needs one more
    // PT (a 4th frame) since it falls in a different PD entry. A 3-frame
    // sharee pool lets page 0 succeed and starves page 1.
    #[test]
    fn grant_rolls_back_partial_mapping_on_frame_exhaustion() {
        const BLOCK: u64 = 0x1_6000_0000;
        const TWO_MIB: u64 = 0x20_0000;
        let sharee_page_0 = BLOCK + TWO_MIB - PAGE_SIZE;
        let sharee_page_1 = BLOCK + TWO_MIB;

        let (owner, _sharee) = two_tasks();
        let bytes = AlignedPages([0; 8192]);
        let mut owner_staging = AlignedPages([0; 8192]);
        let mut owner_pml4 = PageTable::new();
        let mut owner_frames: Pool<PageTable, 8> = Pool::new();
        let sections = [SectionDescriptor {
            virtual_address: 0,
            virtual_size: 2 * PAGE_SIZE as u32,
            file_offset: 0,
            file_size: 2 * PAGE_SIZE as u32,
            permissions: RW,
        }];
        let owner_space = AddressSpace::create(
            &mut owner_pml4,
            &mut owner_frames,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut owner_staging.0,
        )
        .unwrap();

        let sharee_bytes = AlignedPages([0; 8192]);
        let mut sharee_staging = AlignedPages([0; 8192]);
        let mut sharee_pml4 = PageTable::new();
        let mut sharee_frames: Pool<PageTable, 3> = Pool::new();
        let mut sharee_space = AddressSpace::create(
            &mut sharee_pml4,
            &mut sharee_frames,
            &[],
            IMAGE_BASE,
            &sharee_bytes.0,
            &mut sharee_staging.0,
        )
        .unwrap();

        let mut registry: GrantRegistry<4> = GrantRegistry::new();
        let result = grant(
            owner,
            &owner_space,
            &mut sharee_space,
            GrantRequest {
                owner_virt: IMAGE_BASE,
                sharee_virt: sharee_page_0,
                pages: 2,
                sharee_permissions: RO,
            },
            &mut registry,
        );
        assert_eq!(result.err(), Some(SharedMemoryError::FrameExhausted));
        assert_eq!(
            sharee_space.translate(sharee_page_0),
            None,
            "the already-mapped first page must be rolled back"
        );
        assert_eq!(sharee_space.translate(sharee_page_1), None);
    }

    // A full registry rolls back the mapping too, rather than leaving a page
    // mapped with no way for `revoke` to ever find it again.
    #[test]
    fn grant_rolls_back_the_mapping_when_the_registry_is_full() {
        let (owner, _sharee) = two_tasks();
        let bytes = AlignedPages([0; 8192]);
        let mut owner_staging = AlignedPages([0; 8192]);
        let mut owner_pml4 = PageTable::new();
        let mut owner_frames: Pool<PageTable, 8> = Pool::new();
        let owner_space = owner_space_with_one_rw_page(
            &bytes,
            &mut owner_staging,
            &mut owner_pml4,
            &mut owner_frames,
        );

        let sharee_bytes = AlignedPages([0; 8192]);
        let mut sharee_staging = AlignedPages([0; 8192]);
        let mut sharee_pml4 = PageTable::new();
        let mut sharee_frames: Pool<PageTable, 8> = Pool::new();
        let mut sharee_space =
            empty_space(&sharee_bytes, &mut sharee_staging, &mut sharee_pml4, &mut sharee_frames);

        let mut registry: GrantRegistry<0> = GrantRegistry::new();
        let result = grant(
            owner,
            &owner_space,
            &mut sharee_space,
            GrantRequest {
                owner_virt: IMAGE_BASE,
                sharee_virt: SHAREE_VIRT,
                pages: 1,
                sharee_permissions: RO,
            },
            &mut registry,
        );
        assert_eq!(result.err(), Some(SharedMemoryError::RegistryExhausted));
        assert_eq!(sharee_space.translate(SHAREE_VIRT), None, "a rolled-back grant maps nothing");
    }

    // A stale token from a revoked grant must not be able to tear down an
    // unrelated later grant that happens to reuse the same `sharee_virt`.
    #[test]
    fn revoke_rejects_a_stale_token_whose_address_was_regranted() {
        let (owner, _sharee) = two_tasks();
        let bytes = AlignedPages([0; 8192]);
        let mut owner_staging = AlignedPages([0; 8192]);
        let mut owner_pml4 = PageTable::new();
        let mut owner_frames: Pool<PageTable, 8> = Pool::new();
        let owner_space = owner_space_with_one_rw_page(
            &bytes,
            &mut owner_staging,
            &mut owner_pml4,
            &mut owner_frames,
        );

        let sharee_bytes = AlignedPages([0; 8192]);
        let mut sharee_staging = AlignedPages([0; 8192]);
        let mut sharee_pml4 = PageTable::new();
        let mut sharee_frames: Pool<PageTable, 8> = Pool::new();
        let mut sharee_space =
            empty_space(&sharee_bytes, &mut sharee_staging, &mut sharee_pml4, &mut sharee_frames);

        let mut registry: GrantRegistry<4> = GrantRegistry::new();
        let first_grant = grant(
            owner,
            &owner_space,
            &mut sharee_space,
            GrantRequest {
                owner_virt: IMAGE_BASE,
                sharee_virt: SHAREE_VIRT,
                pages: 1,
                sharee_permissions: RO,
            },
            &mut registry,
        )
        .unwrap();
        revoke(owner, &first_grant, &mut sharee_space, &mut registry).unwrap();

        let second_grant = grant(
            owner,
            &owner_space,
            &mut sharee_space,
            GrantRequest {
                owner_virt: IMAGE_BASE,
                sharee_virt: SHAREE_VIRT,
                pages: 1,
                sharee_permissions: RO,
            },
            &mut registry,
        )
        .unwrap();

        assert_eq!(
            revoke(owner, &first_grant, &mut sharee_space, &mut registry).err(),
            Some(SharedMemoryError::StaleGrant),
            "the first grant's now-stale token must not revoke the second grant"
        );
        assert!(
            sharee_space.translate(SHAREE_VIRT).is_some(),
            "the second grant's mapping must survive the rejected stale revoke"
        );

        assert_eq!(revoke(owner, &second_grant, &mut sharee_space, &mut registry), Ok(()));
        assert_eq!(sharee_space.translate(SHAREE_VIRT), None);
    }

    // ---------------------------------------------------------------------
    // STORY-P0-07-03 (`LE-40`) — `grant` fails closed on every path it takes.
    // ---------------------------------------------------------------------

    /// An owner space with a two-page RW region at `IMAGE_BASE` — the
    /// smallest region that makes `grant`'s *per-page* loops take more than
    /// one iteration, which is where every arithmetic defect below lives.
    fn owner_space_with_two_rw_pages<'a>(
        bytes: &'a AlignedPages,
        staging: &'a mut AlignedPages,
        pml4: &'a mut PageTable,
        frame_pool: &'a mut Pool<PageTable, 8>,
    ) -> AddressSpace<'a, 8> {
        let sections = [SectionDescriptor {
            virtual_address: 0,
            virtual_size: 2 * PAGE_SIZE as u32,
            file_offset: 0,
            file_size: 2 * PAGE_SIZE as u32,
            permissions: RW,
        }];
        AddressSpace::create(pml4, frame_pool, &sections, IMAGE_BASE, &bytes.0, &mut staging.0)
            .unwrap()
    }

    /// The highest page-aligned address in a 64-bit address space. Above
    /// `KERNEL_RESERVED_REGION_END` and `PAGE_SIZE`-aligned, so `grant`'s
    /// kernel-collision and alignment checks both pass it — it is a
    /// *well-formed* request by every check that existed before this Story.
    const TOP_PAGE: u64 = 0xFFFF_FFFF_FFFF_F000;

    // TEST-P0-07-03-A §3: the reachable one. Two legitimately-mapped owner
    // pages and a sharee range that runs off the top of the address space.
    // Before this Story the sharee-range loop computed `TOP_PAGE + PAGE_SIZE`
    // with a plain `+` and panicked in a debug build; in a release build it
    // wrapped silently to 0 — inside the kernel-reserved region whose check
    // the function had already finished running.
    #[test]
    fn a_grant_whose_sharee_range_overflows_the_address_space_is_rejected() {
        let (owner, _sharee) = two_tasks();
        let bytes = AlignedPages([0; 8192]);
        let mut owner_staging = AlignedPages([0; 8192]);
        let mut owner_pml4 = PageTable::new();
        let mut owner_frames: Pool<PageTable, 8> = Pool::new();
        let owner_space = owner_space_with_two_rw_pages(
            &bytes,
            &mut owner_staging,
            &mut owner_pml4,
            &mut owner_frames,
        );

        let sharee_bytes = AlignedPages([0; 8192]);
        let mut sharee_staging = AlignedPages([0; 8192]);
        let mut sharee_pml4 = PageTable::new();
        let mut sharee_frames: Pool<PageTable, 8> = Pool::new();
        let mut sharee_space =
            empty_space(&sharee_bytes, &mut sharee_staging, &mut sharee_pml4, &mut sharee_frames);

        let mut registry: GrantRegistry<4> = GrantRegistry::new();
        let result = grant(
            owner,
            &owner_space,
            &mut sharee_space,
            GrantRequest {
                owner_virt: IMAGE_BASE,
                sharee_virt: TOP_PAGE,
                pages: 2,
                sharee_permissions: RO,
            },
            &mut registry,
        );

        assert_eq!(result.err(), Some(SharedMemoryError::RangeOverflow));
        assert_eq!(sharee_space.translate(TOP_PAGE), None, "a rejected grant maps nothing");
        assert_eq!(sharee_space.translate(0), None, "and must not have wrapped to address 0");
    }

    // TEST-P0-07-03-A §3: the same defect on the owner side. Rejected as an
    // unrepresentable *range* rather than as an unmapped page, because the
    // check runs before either loop — the range is malformed regardless of
    // what happens to be mapped.
    #[test]
    fn a_grant_whose_owner_range_overflows_the_address_space_is_rejected() {
        let (owner, _sharee) = two_tasks();
        let bytes = AlignedPages([0; 8192]);
        let mut owner_staging = AlignedPages([0; 8192]);
        let mut owner_pml4 = PageTable::new();
        let mut owner_frames: Pool<PageTable, 8> = Pool::new();
        let owner_space = owner_space_with_two_rw_pages(
            &bytes,
            &mut owner_staging,
            &mut owner_pml4,
            &mut owner_frames,
        );

        let sharee_bytes = AlignedPages([0; 8192]);
        let mut sharee_staging = AlignedPages([0; 8192]);
        let mut sharee_pml4 = PageTable::new();
        let mut sharee_frames: Pool<PageTable, 8> = Pool::new();
        let mut sharee_space =
            empty_space(&sharee_bytes, &mut sharee_staging, &mut sharee_pml4, &mut sharee_frames);

        let mut registry: GrantRegistry<4> = GrantRegistry::new();
        let result = grant(
            owner,
            &owner_space,
            &mut sharee_space,
            GrantRequest {
                owner_virt: TOP_PAGE,
                sharee_virt: SHAREE_VIRT,
                pages: 2,
                sharee_permissions: RO,
            },
            &mut registry,
        );

        assert_eq!(result.err(), Some(SharedMemoryError::RangeOverflow));
        assert_eq!(sharee_space.translate(SHAREE_VIRT), None, "a rejected grant maps nothing");
    }

    // TEST-P0-07-03-A §3: `pages` is caller-chosen too, and it is multiplied
    // by `PAGE_SIZE`. Before this Story a nonsense count was rejected only
    // incidentally — by the first unmapped page it happened to reach — which
    // is not the same as rejecting it for being nonsense.
    #[test]
    fn a_grant_whose_page_count_overflows_the_address_space_is_rejected() {
        let (owner, _sharee) = two_tasks();
        let bytes = AlignedPages([0; 8192]);
        let mut owner_staging = AlignedPages([0; 8192]);
        let mut owner_pml4 = PageTable::new();
        let mut owner_frames: Pool<PageTable, 8> = Pool::new();
        let owner_space = owner_space_with_two_rw_pages(
            &bytes,
            &mut owner_staging,
            &mut owner_pml4,
            &mut owner_frames,
        );

        let sharee_bytes = AlignedPages([0; 8192]);
        let mut sharee_staging = AlignedPages([0; 8192]);
        let mut sharee_pml4 = PageTable::new();
        let mut sharee_frames: Pool<PageTable, 8> = Pool::new();
        let mut sharee_space =
            empty_space(&sharee_bytes, &mut sharee_staging, &mut sharee_pml4, &mut sharee_frames);

        let mut registry: GrantRegistry<4> = GrantRegistry::new();
        let result = grant(
            owner,
            &owner_space,
            &mut sharee_space,
            GrantRequest {
                owner_virt: IMAGE_BASE,
                sharee_virt: SHAREE_VIRT,
                pages: usize::MAX,
                sharee_permissions: RO,
            },
            &mut registry,
        );

        assert_eq!(result.err(), Some(SharedMemoryError::RangeOverflow));
        assert_eq!(sharee_space.translate(SHAREE_VIRT), None, "a rejected grant maps nothing");
    }

    // TEST-P0-07-03-A §3: the last arithmetic on the path. An exhausted
    // generation counter must refuse the grant rather than wrap (a wrapped
    // generation makes a stale token match a later grant, defeating
    // `StaleGrant`) or panic on the `+= 1`.
    #[test]
    fn a_grant_that_would_exhaust_the_generation_counter_is_rejected() {
        let (owner, _sharee) = two_tasks();
        let bytes = AlignedPages([0; 8192]);
        let mut owner_staging = AlignedPages([0; 8192]);
        let mut owner_pml4 = PageTable::new();
        let mut owner_frames: Pool<PageTable, 8> = Pool::new();
        let owner_space = owner_space_with_one_rw_page(
            &bytes,
            &mut owner_staging,
            &mut owner_pml4,
            &mut owner_frames,
        );

        let sharee_bytes = AlignedPages([0; 8192]);
        let mut sharee_staging = AlignedPages([0; 8192]);
        let mut sharee_pml4 = PageTable::new();
        let mut sharee_frames: Pool<PageTable, 8> = Pool::new();
        let mut sharee_space =
            empty_space(&sharee_bytes, &mut sharee_staging, &mut sharee_pml4, &mut sharee_frames);

        let mut registry: GrantRegistry<4> = GrantRegistry::new();
        registry.next_generation = u64::MAX;

        let result = grant(
            owner,
            &owner_space,
            &mut sharee_space,
            GrantRequest {
                owner_virt: IMAGE_BASE,
                sharee_virt: SHAREE_VIRT,
                pages: 1,
                sharee_permissions: RO,
            },
            &mut registry,
        );

        assert_eq!(result.err(), Some(SharedMemoryError::RegistryExhausted));
        assert_eq!(sharee_space.translate(SHAREE_VIRT), None, "a rejected grant maps nothing");
    }

    // TEST-P0-07-03-A §1/§2: the single grantability decision, exercised
    // directly. `grant`'s mapping loop can only reject what this function
    // rejects, and before this Story the mapping loop's own re-read was an
    // `.expect` that checked presence by panicking and permissions not at all.
    #[test]
    fn the_grantability_check_rejects_an_unmapped_owner_page() {
        let bytes = AlignedPages([0; 8192]);
        let mut staging = AlignedPages([0; 8192]);
        let mut pml4 = PageTable::new();
        let mut frames: Pool<PageTable, 8> = Pool::new();
        let space = owner_space_with_one_rw_page(&bytes, &mut staging, &mut pml4, &mut frames);

        assert_eq!(
            grantable_owner_page(&space, IMAGE_BASE + 0x10_0000, RO).err(),
            Some(SharedMemoryError::RegionNotOwned),
            "an unmapped page must be an error, not a panic"
        );
    }

    #[test]
    fn the_grantability_check_rejects_authority_the_owner_does_not_have() {
        let bytes = AlignedPages([0; 8192]);
        let mut staging = AlignedPages([0; 8192]);
        let mut pml4 = PageTable::new();
        let mut frames: Pool<PageTable, 8> = Pool::new();
        // The region is RW: writable, and (W^X) not executable.
        let space = owner_space_with_one_rw_page(&bytes, &mut staging, &mut pml4, &mut frames);

        assert_eq!(
            grantable_owner_page(&space, IMAGE_BASE, RX).err(),
            Some(SharedMemoryError::PermissionsExceedOwner),
            "execute must not be grantable out of a non-executable page"
        );
    }

    #[test]
    fn the_grantability_check_accepts_a_request_within_the_owners_authority() {
        let bytes = AlignedPages([0; 8192]);
        let mut staging = AlignedPages([0; 8192]);
        let mut pml4 = PageTable::new();
        let mut frames: Pool<PageTable, 8> = Pool::new();
        let space = owner_space_with_one_rw_page(&bytes, &mut staging, &mut pml4, &mut frames);

        let page = grantable_owner_page(&space, IMAGE_BASE, RO).expect("RO is within RW");
        assert_eq!(page.phys, space.translate(IMAGE_BASE).unwrap().phys);
        assert_eq!(
            grantable_owner_page(&space, IMAGE_BASE, RW).map(|p| p.phys),
            Ok(page.phys),
            "RW is exactly the owner's own authority and must be grantable"
        );
    }

    // TEST-P0-07-03-A §5: the gate. A prose rule with a machine behind it,
    // in the spirit of `LE-33`/`LE-35`/`LE-36`/`LE-44`. This scans the half
    // of this file above `#[cfg(test)]`, with comment lines stripped so the
    // module's own prose about panics does not trip its own gate.
    //
    // It does NOT claim the absence of implicit panics (indexing, unchecked
    // arithmetic) — `TEST-P0-07-03-A` §5 says so, and `LE-52` carries the
    // generalisation of this gate beyond one module.
    #[test]
    fn this_modules_non_test_source_contains_no_panic_constructs() {
        const SOURCE: &str = include_str!("shared_memory.rs");
        const MARKER: &str = "#[cfg(test)]";
        const BANNED: [&str; 6] =
            [".unwrap()", ".expect(", "panic!", "unreachable!", "todo!", "unimplemented!"];

        let non_test = SOURCE.split(MARKER).next().expect("split always yields one part");
        assert!(non_test.len() < SOURCE.len(), "the marker must actually be present");

        for (number, line) in non_test.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            for banned in BANNED {
                assert!(
                    !line.contains(banned),
                    "`{banned}` on this module's non-test path, line {}: `grant` is a kernel \
                     path and `agent/CODING_STANDARDS.md` puts fail-safe above keep-trying \
                     (LE-40)",
                    number + 1
                );
            }
        }
    }
}
