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

use hal_x86_64::paging::PAGE_SIZE;
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

    for i in 0..pages {
        let owner_page = owner_space
            .translate(owner_virt + i as u64 * PAGE_SIZE)
            .ok_or(SharedMemoryError::RegionNotOwned)?;
        if sharee_permissions.write && !owner_page.writable {
            return Err(SharedMemoryError::PermissionsExceedOwner);
        }
        if sharee_permissions.execute && !owner_page.executable {
            return Err(SharedMemoryError::PermissionsExceedOwner);
        }
    }
    for i in 0..pages {
        if sharee_space.translate(sharee_virt + i as u64 * PAGE_SIZE).is_some() {
            return Err(SharedMemoryError::AlreadyGranted);
        }
    }

    let mut mapped = 0usize;
    for i in 0..pages {
        let offset = i as u64 * PAGE_SIZE;
        // Already validated present above; re-reading here (rather than
        // collecting into a fixed-size buffer up front) avoids needing a
        // `pages`-sized scratch array for an arbitrary caller-chosen count.
        let owner_page = owner_space
            .translate(owner_virt + offset)
            .expect("validated present in the loop above");
        if let Err(err) =
            sharee_space.map_page(sharee_virt + offset, owner_page.phys, sharee_permissions)
        {
            unmap_prefix(sharee_space, sharee_virt, mapped);
            return Err(err.into());
        }
        mapped += 1;
    }

    let generation = registry.next_generation;
    if let Err(err) = registry.insert(GrantRecord { sharee_virt, generation }) {
        unmap_prefix(sharee_space, sharee_virt, mapped);
        return Err(err);
    }
    registry.next_generation += 1;

    Ok(SharedGrant { owner, sharee_virt, pages, generation })
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
}
