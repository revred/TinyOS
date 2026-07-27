//! Process address-space creation and section mapping (`STORY-P0-05-02`).
//!
//! Given the sections a `pe::LoadDescriptor` (`STORY-P0-05-01`) parsed out
//! of a PE64 image, this module builds a real x86_64 page-table tree
//! (`hal_x86_64::paging`) mapping each section at its declared virtual
//! address with exactly its declared permissions — the step that turns a
//! validated-but-inert description of an image into memory a CPU could
//! actually execute out of, if this tree were ever loaded into `CR3`.
//!
//! Loading the tree into `CR3` — making it the CPU's live address space —
//! is **not** implemented here. `STORY-P0-05-02`'s acceptance criteria
//! explicitly scope full per-process page-table isolation out of this
//! Story, and inducing/catching a real `#PF` for a genuinely live mapping
//! would additionally require an IDT/exception-handling subsystem this
//! kernel does not yet have. This module instead proves permission
//! correctness by reading the constructed page-table entries back
//! (`AddressSpace::translate`, wrapping `hal_x86_64::paging::translate`) —
//! deterministic and testable on both the host and under QEMU, without
//! requiring an active mapping or a fault handler. See
//! `session/hand-2026-07-26`'s handover for this Story for the full
//! rationale.
//!
//! Callers supply the structures this module builds into — `pml4: &mut
//! PageTable`, `frame_pool: &mut Pool<PageTable, FRAMES>`, and (since
//! `STORY-P0-05-04`) `staging: &mut [u8]` — rather than this module owning
//! them, because an [`AddressSpace`] must never move once its page-table
//! cross-links are populated (the intermediate PDPT/PD/PT frames are
//! referenced from `pml4` by raw address; moving the owning struct would
//! leave those addresses pointing at stale memory). Callers place all three
//! in storage that outlives and never relocates during the `AddressSpace`'s
//! lifetime (a `static`, or a stack local that's never itself moved after
//! being borrowed) — the same discipline `context.rs`'s and
//! `context_switch_fixture.rs`'s own `static`-based task/stack storage
//! already follows for an analogous reason.
//!
//! **`STORY-P0-05-04`**: this module originally mapped pages directly out of
//! the caller's `image_bytes` buffer, no copy — but that only works when a
//! section's on-disk `file_offset` is itself page-aligned, which requires an
//! unusually large `FileAlignment` (>= 4096) no real-world linker uses by
//! default (x86-64 page tables can only ever map whole aligned physical
//! pages; there is no page-table entry that means "starting from byte 1024
//! of this buffer"). `blue-sharc.exe`'s own real section table — every
//! `file_offset` only 512-byte aligned, its standard `FileAlignment` — hits
//! exactly this wall. `create` now copies each section's bytes into
//! caller-supplied, page-aligned `staging` storage instead: one mechanism
//! that handles both the file-alignment gap and a section whose
//! `virtual_size` exceeds `file_size` (e.g. `.bss`, real loaders' own
//! demand-zero convention) identically — every mapped page is zeroed first,
//! then whatever falls within `[0, file_size)` of the section's own
//! file-backed range is copied in, so a `.bss` tail is simply the case where
//! nothing is left to copy. This trades the original zero-copy property for
//! correctness against genuinely real PE files — the same tradeoff every
//! real OS loader makes for the same hardware reason, not a TinyOS-specific
//! design gap.

use hal_x86_64::paging::{self, FrameAllocator, MappedPage, PageTable, PagingError, PAGE_SIZE};
use kernel::mem::Pool;

use crate::pe::SectionDescriptor;

/// The kernel's own identity-mapped region (`boot.rs`'s first-1GiB huge-page
/// map) — a `LoadDescriptor` requesting any virtual address inside this
/// range is rejected rather than silently remapped over kernel memory
/// (`STORY-P0-05-02` acceptance criterion 2).
pub const KERNEL_RESERVED_REGION_END: u64 = 0x4000_0000;

/// Errors [`validate_sections`] and [`AddressSpace::create`] fail closed
/// with, per `agent/CODING_STANDARDS.md`'s "no stringly-typed errors" rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressSpaceError {
    /// Two sections claim overlapping virtual address ranges.
    SectionOverlap,
    /// A section's virtual address range overlaps
    /// [`KERNEL_RESERVED_REGION_END`]'s reserved region.
    KernelRegionCollision,
    /// A section's virtual address, or the `staging`/`image_bytes` buffer
    /// itself, is not [`PAGE_SIZE`]-aligned — every *mapped* page must sit
    /// on a page boundary, even though (since `STORY-P0-05-04`) a section's
    /// on-disk `file_offset` no longer needs to.
    Misaligned,
    /// A section's virtual address or size overflows a 64-bit address
    /// range once added to `image_base`.
    InvalidRange,
    /// The frame pool had no slot left for a new page-table level.
    FrameExhausted,
    /// Internal inconsistency: a virtual page within a just-validated,
    /// non-overlapping section set was already mapped. Unreachable in
    /// practice (kept exhaustive rather than assumed away, matching
    /// `sched.rs::TaskCreateError`'s precedent).
    AlreadyMapped,
    /// A section's declared `(file_offset, file_size)` range falls outside
    /// `image_bytes`'s actual length (`STORY-P0-05-04`) — re-validated here
    /// rather than trusted from the caller, since `AddressSpace::create` is
    /// directly host/QEMU-testable with hand-built `SectionDescriptor`s that
    /// never passed through `pe::parse`'s own bounds checking.
    SectionDataOutOfBounds,
    /// `staging` isn't large enough to hold every section's mapped pages
    /// (`STORY-P0-05-04`) — sized by the caller, checked here rather than
    /// silently truncated.
    StagingExhausted,
    /// [`AddressSpace::unmap_page`] (`STORY-P0-07-02`) was called against a
    /// `virt` with no present mapping.
    NotMapped,
}

impl From<PagingError> for AddressSpaceError {
    fn from(err: PagingError) -> Self {
        match err {
            PagingError::FrameExhausted => AddressSpaceError::FrameExhausted,
            PagingError::AlreadyMapped => AddressSpaceError::AlreadyMapped,
            PagingError::Misaligned => AddressSpaceError::Misaligned,
            PagingError::NotMapped => AddressSpaceError::NotMapped,
        }
    }
}

/// A section's `[start, end)` virtual address range, validated against
/// overflow and [`PAGE_SIZE`] alignment (of the *virtual* address only —
/// see this module's own doc comment for why `file_offset` no longer needs
/// to be page-aligned) but not yet against other sections or the kernel's
/// reserved region.
fn section_range(
    section: &SectionDescriptor,
    image_base: u64,
) -> Result<(u64, u64), AddressSpaceError> {
    let start = image_base
        .checked_add(u64::from(section.virtual_address))
        .ok_or(AddressSpaceError::InvalidRange)?;
    if !start.is_multiple_of(PAGE_SIZE) {
        return Err(AddressSpaceError::Misaligned);
    }
    let size = u64::from(section.virtual_size.max(section.file_size));
    let end = start.checked_add(size).ok_or(AddressSpaceError::InvalidRange)?;
    Ok((start, end))
}

fn ranges_overlap(a: (u64, u64), b: (u64, u64)) -> bool {
    a.0 < b.1 && b.0 < a.1
}

/// Validates `sections` (mapped at `image_base`) against overlap with each
/// other and collision with the kernel's own reserved region, without
/// touching any page table — pure and host-testable in isolation, mirroring
/// `pe::parse`'s own split between validation and the effectful step it
/// precedes (`STORY-P0-05-01`'s established pattern).
///
/// Fails closed with the *first* violation found in section order; never
/// partially validates (a caller that gets `Ok(())` back has a fully
/// consistent section set to map).
pub fn validate_sections(
    sections: &[SectionDescriptor],
    image_base: u64,
) -> Result<(), AddressSpaceError> {
    for i in 0..sections.len() {
        let a = section_range(&sections[i], image_base)?;
        if a.0 < KERNEL_RESERVED_REGION_END {
            return Err(AddressSpaceError::KernelRegionCollision);
        }
        for section_j in &sections[i + 1..] {
            let b = section_range(section_j, image_base)?;
            if ranges_overlap(a, b) {
                return Err(AddressSpaceError::SectionOverlap);
            }
        }
    }
    Ok(())
}

/// A [`Pool`]-backed [`FrameAllocator`]: each call to `allocate_frame`
/// claims a fresh pool slot and returns that slot's own (stable, per this
/// module's doc comment) address.
struct PoolFrameAllocator<'a, const FRAMES: usize> {
    pool: &'a mut Pool<PageTable, FRAMES>,
}

impl<const FRAMES: usize> FrameAllocator for PoolFrameAllocator<'_, FRAMES> {
    fn allocate_frame(&mut self) -> Option<paging::FrameAddr> {
        let handle = self.pool.alloc(PageTable::new()).ok()?;
        let table = self.pool.get_mut(handle)?;
        Some(core::ptr::from_mut(table) as u64)
    }
}

/// A process address space: an x86_64 page-table tree mapping each of a
/// validated section set at its declared virtual address with its declared
/// permissions, and the [`Pool`] backing every intermediate page-table
/// frame this tree needed.
///
/// Not `Send`/`Sync` beyond what its `&mut` fields already require, and
/// never `Clone`/`Copy` — an `AddressSpace` uniquely owns the mapping state
/// in `pml4` and `frame_pool` for as long as it lives.
pub struct AddressSpace<'a, const FRAMES: usize> {
    pml4: &'a mut PageTable,
    frame_pool: &'a mut Pool<PageTable, FRAMES>,
}

impl<'a, const FRAMES: usize> AddressSpace<'a, FRAMES> {
    /// Validates `sections` (see [`validate_sections`]) and, if valid, maps
    /// each one into `pml4`/`frame_pool`. Each mapped page is copied out of
    /// `image_bytes`/`staging` (`STORY-P0-05-04`, see this module's own doc
    /// comment for why a copy is unavoidable) — `staging` must itself begin
    /// on a [`PAGE_SIZE`] boundary and be large enough to hold every
    /// section's mapped page range (`sum` over sections of
    /// `ceil(max(virtual_size, file_size) / PAGE_SIZE)` pages), or this
    /// fails closed with [`AddressSpaceError::StagingExhausted`].
    ///
    /// Fails closed with a typed [`AddressSpaceError`] and no partial
    /// mapping: [`validate_sections`] and a bounds check against
    /// `image_bytes` both run first over the whole section set, so a
    /// rejection never leaves `pml4`/`frame_pool`/`staging` touched.
    pub fn create(
        pml4: &'a mut PageTable,
        frame_pool: &'a mut Pool<PageTable, FRAMES>,
        sections: &[SectionDescriptor],
        image_base: u64,
        image_bytes: &[u8],
        staging: &mut [u8],
    ) -> Result<Self, AddressSpaceError> {
        if !(staging.as_ptr() as u64).is_multiple_of(PAGE_SIZE) {
            return Err(AddressSpaceError::Misaligned);
        }
        validate_sections(sections, image_base)?;
        for section in sections {
            let file_end = (section.file_offset as usize)
                .checked_add(section.file_size as usize)
                .ok_or(AddressSpaceError::SectionDataOutOfBounds)?;
            if file_end > image_bytes.len() {
                return Err(AddressSpaceError::SectionDataOutOfBounds);
            }
        }

        let mut this = AddressSpace { pml4, frame_pool };
        let mut staging_cursor = 0usize;
        for section in sections {
            staging_cursor =
                this.map_section(image_base, section, image_bytes, staging, staging_cursor)?;
        }
        Ok(this)
    }

    /// Maps one section's pages, copying each page's file-backed bytes (if
    /// any) out of `image_bytes` into the next unused slice of `staging`
    /// starting at `staging_cursor`, zero-filling the rest — see this
    /// module's own doc comment. Returns the advanced cursor for the next
    /// section to continue from.
    fn map_section(
        &mut self,
        image_base: u64,
        section: &SectionDescriptor,
        image_bytes: &[u8],
        staging: &mut [u8],
        staging_cursor: usize,
    ) -> Result<usize, AddressSpaceError> {
        let (virt_start, virt_end) = section_range(section, image_base)?;
        let file_offset = section.file_offset as usize;
        let file_size = section.file_size as usize;
        let page_size = PAGE_SIZE as usize;
        let mut allocator = PoolFrameAllocator { pool: self.frame_pool };

        let mut offset = 0u64;
        let mut cursor = staging_cursor;
        while virt_start + offset < virt_end {
            let page_end =
                cursor.checked_add(page_size).ok_or(AddressSpaceError::StagingExhausted)?;
            if page_end > staging.len() {
                return Err(AddressSpaceError::StagingExhausted);
            }
            let page = &mut staging[cursor..page_end];
            page.fill(0);

            let section_offset = offset as usize;
            if section_offset < file_size {
                let copy_len = page_size.min(file_size - section_offset);
                let src_start = file_offset + section_offset;
                page[..copy_len].copy_from_slice(&image_bytes[src_start..src_start + copy_len]);
            }

            let phys = page.as_ptr() as u64;
            paging::map_4k(
                self.pml4,
                &mut allocator,
                virt_start + offset,
                phys,
                section.permissions.write,
                section.permissions.execute,
            )
            .map_err(AddressSpaceError::from)?;

            offset += PAGE_SIZE;
            cursor = page_end;
        }
        Ok(cursor)
    }

    /// Looks up `virt`'s current mapping, if any — the read-back mechanism
    /// this module's own doc comment describes as standing in for a live
    /// CPU fault: a page this returns as non-`writable` is, per
    /// `hal_x86_64::paging`'s own contract, a page whose PTE never had the
    /// writable bit set in the first place.
    pub fn translate(&self, virt: u64) -> Option<MappedPage> {
        paging::translate(self.pml4, virt)
    }

    /// Maps a single page at `virt` to physical frame `phys` with
    /// `permissions` — the primitive `STORY-P0-07-02`'s cross-address-space
    /// shared-memory grant builds on, exposed publicly since granting a
    /// region into a *different* task's `AddressSpace` needs to add a
    /// mapping that space's own section set never described at `create`
    /// time.
    ///
    /// Fails closed the same way `create`'s own section mapping does:
    /// [`AddressSpaceError::Misaligned`] if `virt`/`phys` aren't
    /// page-aligned, [`AddressSpaceError::AlreadyMapped`] if `virt` is
    /// already mapped, [`AddressSpaceError::FrameExhausted`] if the frame
    /// pool runs out building a missing intermediate level.
    pub fn map_page(
        &mut self,
        virt: u64,
        phys: u64,
        permissions: crate::pe::Permissions,
    ) -> Result<(), AddressSpaceError> {
        let mut allocator = PoolFrameAllocator { pool: self.frame_pool };
        paging::map_4k(
            self.pml4,
            &mut allocator,
            virt,
            phys,
            permissions.write,
            permissions.execute,
        )
        .map_err(AddressSpaceError::from)
    }

    /// Unmaps a single page at `virt` — the counterpart to [`map_page`],
    /// deterministic revocation for `STORY-P0-07-02`'s shared-memory
    /// grants: no path leaves a stale mapping the sharee could still
    /// read/write after the owner revokes it.
    ///
    /// Fails closed with [`AddressSpaceError::Misaligned`] if `virt` isn't
    /// page-aligned, or [`AddressSpaceError::NotMapped`] if `virt` has no
    /// present mapping.
    ///
    /// [`map_page`]: AddressSpace::map_page
    pub fn unmap_page(&mut self, virt: u64) -> Result<(), AddressSpaceError> {
        paging::unmap_4k(self.pml4, virt).map_err(AddressSpaceError::from)
    }
}

impl<const FRAMES: usize> Drop for AddressSpace<'_, FRAMES> {
    /// Symmetric teardown (`STORY-P0-05-02` acceptance criterion 3): resets
    /// `pml4` to empty and replaces `*frame_pool` with a fresh, empty
    /// `Pool` — the assignment drops the old `Pool` in place first (running
    /// [`Pool`]'s own `Drop`, which frees every occupied slot), so no
    /// mapped frame or page-table level survives past this call. Mirrors
    /// `Pool`'s own reclaim-on-drop discipline (`STORY-P0-03-01`), applied
    /// one level up at the address-space boundary. `staging` is
    /// caller-owned and reused verbatim by the next `create` call, which
    /// already overwrites every byte it maps, so it needs no reset here.
    fn drop(&mut self) {
        *self.pml4 = PageTable::new();
        *self.frame_pool = Pool::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pe::Permissions;

    const IMAGE_BASE: u64 = 0x1_4000_0000;

    fn section(virtual_address: u32, size: u32, permissions: Permissions) -> SectionDescriptor {
        SectionDescriptor {
            virtual_address,
            virtual_size: size,
            file_offset: 0,
            file_size: size,
            permissions,
        }
    }

    const RX: Permissions = Permissions { read: true, write: false, execute: true };
    const RW: Permissions = Permissions { read: true, write: true, execute: false };

    // STORY-P0-05-02 AC2: non-overlapping, non-colliding sections validate.
    #[test]
    fn validate_accepts_non_overlapping_sections_above_the_kernel_region() {
        let sections =
            [section(0, PAGE_SIZE as u32, RX), section(PAGE_SIZE as u32, PAGE_SIZE as u32, RW)];
        assert_eq!(validate_sections(&sections, IMAGE_BASE), Ok(()));
    }

    // STORY-P0-05-02 AC2: overlapping virtual ranges are rejected.
    #[test]
    fn validate_rejects_overlapping_sections() {
        let sections =
            [section(0, 2 * PAGE_SIZE as u32, RX), section(PAGE_SIZE as u32, PAGE_SIZE as u32, RW)];
        assert_eq!(
            validate_sections(&sections, IMAGE_BASE),
            Err(AddressSpaceError::SectionOverlap)
        );
    }

    // STORY-P0-05-02 AC2: a section landing inside the kernel's own
    // identity-mapped region is rejected, never silently mapped over it.
    #[test]
    fn validate_rejects_a_section_colliding_with_the_kernel_region() {
        let sections = [section(0, PAGE_SIZE as u32, RX)];
        // image_base 0: the section's virtual range starts at 0, squarely
        // inside [0, KERNEL_RESERVED_REGION_END).
        assert_eq!(validate_sections(&sections, 0), Err(AddressSpaceError::KernelRegionCollision));
    }

    // A misaligned virtual address is rejected before any overlap check.
    #[test]
    fn validate_rejects_misaligned_section_virtual_address() {
        let sections = [section(1, PAGE_SIZE as u32, RX)];
        assert_eq!(validate_sections(&sections, IMAGE_BASE), Err(AddressSpaceError::Misaligned));
    }

    /// A page-aligned scratch buffer usable as `AddressSpace::create`'s
    /// `image_bytes`/`staging` arguments.
    #[repr(C, align(4096))]
    struct AlignedPages([u8; 8192]);

    // STORY-P0-05-02 AC1: mapped pages carry exactly the declared
    // permissions, readable back via `translate` — a read-only section is
    // never left writable, and vice versa.
    #[test]
    fn created_address_space_maps_sections_with_exact_declared_permissions() {
        let bytes = AlignedPages([0xAA; 8192]);
        let mut staging = AlignedPages([0; 8192]);
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 16> = Pool::new();
        let sections = [
            SectionDescriptor {
                virtual_address: 0,
                virtual_size: PAGE_SIZE as u32,
                file_offset: 0,
                file_size: PAGE_SIZE as u32,
                permissions: RX,
            },
            SectionDescriptor {
                virtual_address: PAGE_SIZE as u32,
                virtual_size: PAGE_SIZE as u32,
                file_offset: PAGE_SIZE as u32,
                file_size: PAGE_SIZE as u32,
                permissions: RW,
            },
        ];

        let space = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        )
        .expect("valid, non-colliding sections should map");

        let code_page = space.translate(IMAGE_BASE).expect("code page should be mapped");
        assert!(!code_page.writable);
        assert!(code_page.executable);

        let data_page =
            space.translate(IMAGE_BASE + PAGE_SIZE).expect("data page should be mapped");
        assert!(data_page.writable);
        assert!(!data_page.executable);
    }

    // STORY-P0-05-04: a section's file-backed bytes are actually copied
    // (and readable back) through the mapped page, not just permission
    // bits — proving the copy-based mapper preserves real content.
    #[test]
    fn mapped_page_content_matches_the_sections_source_bytes() {
        let mut bytes = AlignedPages([0; 8192]);
        bytes.0[PAGE_SIZE as usize] = 0x42;
        let mut staging = AlignedPages([0; 8192]);
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 16> = Pool::new();
        let sections = [SectionDescriptor {
            virtual_address: PAGE_SIZE as u32,
            virtual_size: PAGE_SIZE as u32,
            file_offset: PAGE_SIZE as u32,
            file_size: PAGE_SIZE as u32,
            permissions: RW,
        }];

        let space = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        )
        .expect("valid section should map");
        let mapped = space.translate(IMAGE_BASE + PAGE_SIZE).expect("page should be mapped");
        // SAFETY: `mapped.phys` is a page this call itself just mapped
        // from `staging`, still alive for as long as `space` (and thus
        // `staging`, borrowed for `space`'s whole lifetime) is.
        let byte = unsafe { *(mapped.phys as *const u8) };
        assert_eq!(byte, 0x42);
    }

    // STORY-P0-05-04: a section whose `virtual_size` exceeds `file_size`
    // (the real `.bss` shape) maps its file-backed prefix faithfully and
    // demand-zeros the rest, rather than exposing whatever staging bytes
    // happened to follow.
    #[test]
    fn a_section_with_virtual_size_larger_than_file_size_demand_zeros_the_tail() {
        let mut bytes = AlignedPages([0xFF; 8192]);
        bytes.0[0] = 0x11;
        let mut staging = AlignedPages([0xAA; 8192]);
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 16> = Pool::new();
        // 16 real file-backed bytes, section otherwise covers a whole page
        // — the classic `.bss` shape (some real data, then demand-zero).
        let sections = [SectionDescriptor {
            virtual_address: 0,
            virtual_size: PAGE_SIZE as u32,
            file_offset: 0,
            file_size: 16,
            permissions: RW,
        }];

        let space = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        )
        .expect("a virtual_size > file_size section should still map");
        let mapped = space.translate(IMAGE_BASE).expect("page should be mapped");
        // SAFETY: see `mapped_page_content_matches_the_sections_source_bytes`.
        let page = unsafe { core::slice::from_raw_parts(mapped.phys as *const u8, 4096) };
        assert_eq!(page[0], 0x11, "file-backed byte should be copied faithfully");
        assert_eq!(page[16], 0, "byte just past file_size should be demand-zeroed");
        assert_eq!(page[4095], 0, "the whole bss tail should be demand-zeroed");
    }

    // STORY-P0-05-04: `staging` too small for the section set fails closed
    // rather than writing past its end.
    #[test]
    fn staging_too_small_for_the_section_set_fails_closed() {
        let bytes = AlignedPages([0; 8192]);
        #[repr(C, align(4096))]
        struct OnePage([u8; 4096]);
        let mut staging = OnePage([0; 4096]);
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 16> = Pool::new();
        let sections = [
            SectionDescriptor {
                virtual_address: 0,
                virtual_size: PAGE_SIZE as u32,
                file_offset: 0,
                file_size: PAGE_SIZE as u32,
                permissions: RX,
            },
            SectionDescriptor {
                virtual_address: PAGE_SIZE as u32,
                virtual_size: PAGE_SIZE as u32,
                file_offset: PAGE_SIZE as u32,
                file_size: PAGE_SIZE as u32,
                permissions: RW,
            },
        ];

        let result = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        );
        assert_eq!(result.err(), Some(AddressSpaceError::StagingExhausted));
    }

    // STORY-P0-05-04: a section whose file range runs past the end of
    // `image_bytes` is rejected before any mapping — re-validated here
    // rather than trusted from a caller that bypassed `pe::parse`.
    #[test]
    fn a_section_whose_file_range_exceeds_image_bytes_is_rejected() {
        let bytes = AlignedPages([0; 8192]);
        let mut staging = AlignedPages([0; 8192]);
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 16> = Pool::new();
        let sections = [SectionDescriptor {
            virtual_address: 0,
            virtual_size: PAGE_SIZE as u32,
            file_offset: 8192 - 16,
            file_size: PAGE_SIZE as u32,
            permissions: RX,
        }];

        let result = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        );
        assert_eq!(result.err(), Some(AddressSpaceError::SectionDataOutOfBounds));
    }

    // STORY-P0-05-02 AC3: dropping an AddressSpace reclaims every frame it
    // allocated — repeated create/drop cycles against the same pool never
    // exhaust it.
    #[test]
    fn dropping_an_address_space_reclaims_its_frame_pool_for_reuse() {
        let bytes = AlignedPages([0; 8192]);
        let mut staging = AlignedPages([0; 8192]);
        let mut frame_pool: Pool<PageTable, 4> = Pool::new();
        let sections = [SectionDescriptor {
            virtual_address: 0,
            virtual_size: PAGE_SIZE as u32,
            file_offset: 0,
            file_size: PAGE_SIZE as u32,
            permissions: RX,
        }];

        for _ in 0..10 {
            let mut pml4 = PageTable::new();
            let space = AddressSpace::create(
                &mut pml4,
                &mut frame_pool,
                &sections,
                IMAGE_BASE,
                &bytes.0,
                &mut staging.0,
            )
            .expect("pool should never stay exhausted across create/drop cycles");
            assert!(space.translate(IMAGE_BASE).is_some());
        }
    }

    // A rejected creation (overlap) never touches pml4/frame_pool.
    #[test]
    fn rejected_creation_leaves_pml4_and_frame_pool_untouched() {
        let bytes = AlignedPages([0; 8192]);
        let mut staging = AlignedPages([0; 8192]);
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 4> = Pool::new();
        let sections =
            [section(0, 2 * PAGE_SIZE as u32, RX), section(PAGE_SIZE as u32, PAGE_SIZE as u32, RW)];

        let result = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        );
        assert_eq!(result.err(), Some(AddressSpaceError::SectionOverlap));
        assert_eq!(paging::translate(&pml4, IMAGE_BASE), None);
        assert!(frame_pool.alloc(PageTable::new()).is_ok());
    }

    // STORY-P0-07-02: `map_page` adds a mapping `create`'s own section set
    // never described, and `unmap_page` deterministically clears it again.
    #[test]
    fn map_page_and_unmap_page_round_trip() {
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 4> = Pool::new();
        let mut space = AddressSpace { pml4: &mut pml4, frame_pool: &mut frame_pool };

        assert_eq!(space.map_page(IMAGE_BASE, 0x9000, RW), Ok(()));
        let mapped = space.translate(IMAGE_BASE).expect("just-mapped page should translate");
        assert_eq!(mapped.phys, 0x9000);
        assert!(mapped.writable);

        assert_eq!(space.unmap_page(IMAGE_BASE), Ok(()));
        assert_eq!(space.translate(IMAGE_BASE), None);
    }

    // Unmapping a page that was never mapped fails closed.
    #[test]
    fn unmap_page_of_an_unmapped_address_fails_closed() {
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 4> = Pool::new();
        let mut space = AddressSpace { pml4: &mut pml4, frame_pool: &mut frame_pool };
        assert_eq!(space.unmap_page(IMAGE_BASE), Err(AddressSpaceError::NotMapped));
    }

    // Mapping the same page twice via `map_page` fails closed rather than
    // silently overwriting the first mapping.
    #[test]
    fn map_page_rejects_an_already_mapped_address() {
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 4> = Pool::new();
        let mut space = AddressSpace { pml4: &mut pml4, frame_pool: &mut frame_pool };
        space.map_page(IMAGE_BASE, 0x9000, RW).unwrap();
        assert_eq!(space.map_page(IMAGE_BASE, 0xa000, RW), Err(AddressSpaceError::AlreadyMapped));
    }
}
