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

/// Exclusive end of x86-64's lower canonical virtual-address half.
///
/// Executable images live below this boundary. Accepting a non-canonical
/// address would let page-table indexing appear to succeed while the CPU
/// later raises `#GP` on the first instruction fetch.
pub const LOWER_CANONICAL_END: u64 = 0x0000_8000_0000_0000;

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
    /// A section falls outside x86-64's lower canonical address half.
    NonCanonical,
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
    if start >= LOWER_CANONICAL_END || end > LOWER_CANONICAL_END {
        return Err(AddressSpaceError::NonCanonical);
    }
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
    /// The page-rounded `[start, end)` hull of every section `create`
    /// mapped, or `None` for a space assembled without `create` —
    /// what [`AddressSpace::teardown`] revokes and what
    /// [`AddressSpace::seal_kernel_alias`] walks (`STORY-P1-03-02`).
    image_range: Option<(u64, u64)>,
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

        let mut image_range: Option<(u64, u64)> = None;
        for section in sections {
            let (start, end) = section_range(section, image_base)?;
            let end = end.div_ceil(PAGE_SIZE) * PAGE_SIZE;
            image_range = Some(match image_range {
                None => (start, end),
                Some((lo, hi)) => (lo.min(start), hi.max(end)),
            });
        }

        let mut this = AddressSpace { pml4, frame_pool, image_range };
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
            paging::map_user_4k(
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

    /// This address space's `CR3` value — the physical address of its PML4
    /// (`STORY-P1-03-01`), i.e. what a caller loads into the register to
    /// make this space the CPU's live address space.
    ///
    /// This kernel's current no-higher-half-split memory model means the
    /// address of the caller-owned `pml4` this struct borrows already **is**
    /// that physical address (`hal_x86_64::paging::FrameAddr`'s own doc
    /// comment; the same assumption [`PoolFrameAllocator::allocate_frame`]
    /// already relies on).
    pub fn cr3(&self) -> u64 {
        core::ptr::from_ref(&*self.pml4) as u64
    }

    /// Looks up `virt`'s current mapping, if any — the read-back mechanism
    /// this module's own doc comment describes as standing in for a live
    /// CPU fault: a page this returns as non-`writable` is, per
    /// `hal_x86_64::paging`'s own contract, a page whose PTE never had the
    /// writable bit set in the first place.
    pub fn translate(&self, virt: u64) -> Option<MappedPage> {
        paging::translate(self.pml4, virt)
    }

    /// Maps a single user-accessible page at `virt` to physical frame `phys`
    /// with `permissions` — the primitive `STORY-P0-07-02`'s
    /// cross-address-space shared-memory grant builds on. Kernel directories
    /// attached to this tree remain supervisor-only at their lower entries.
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
        paging::map_user_4k(
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
        paging::unmap_4k(self.pml4, virt).map_err(AddressSpaceError::from)?;
        // If this is the active address space, invalidate its cached leaf
        // immediately. If it is inactive, this is harmless and the CR3 load
        // required to resume that task flushes all non-global entries. TinyOS
        // is single-core; SMP will require an inter-processor shootdown here.
        //
        // `LE-102`: `target_os = "none"`, NOT `not(target_os = "windows")`.
        // `invlpg` is a ring-0 instruction. On a Linux host the old gate is
        // satisfied, so this line was compiled into the `exec` host test
        // binary and `unmap_page` is safe and called by ordinary unit tests —
        // the suite died with `SIGSEGV` after five tests on the first Linux
        // run that got as far as executing anything.
        #[cfg(target_os = "none")]
        paging::invalidate_page(virt);
        Ok(())
    }

    /// Links a shared kernel page directory (built once by
    /// [`crate::kernel_map::build_shared_directories`]) into this space's
    /// tree, serving the 1GiB region at `virt_base` (`STORY-P1-03-02`
    /// acceptance criterion A4) — the replacement for `STORY-P1-03-01`'s
    /// per-space, all-RWX identity replica: every space references the same
    /// W^X-correct directory rather than owning a copy.
    pub fn attach_shared_pd(&mut self, virt_base: u64, pd: u64) -> Result<(), AddressSpaceError> {
        let mut allocator = PoolFrameAllocator { pool: self.frame_pool };
        paging::install_shared_pd(self.pml4, &mut allocator, virt_base, pd)
            .map_err(AddressSpaceError::from)
    }

    /// Seals the loader's writable aliases (`STORY-P1-03-02` acceptance
    /// criterion A3, review D5): for every page `create` mapped
    /// *non-writable* in this space (image text/rodata), the identity-view
    /// mapping of its backing frame in `kernel_pml4` — the staging bytes the
    /// loader copied through — is re-protected RO-NX, so no executable or
    /// read-only frame keeps a writable view anywhere. Without this, a
    /// per-entry W^X audit passes while W^X is defeated through the alias.
    ///
    /// Fails closed if `kernel_pml4` doesn't map a frame at 4KiB
    /// granularity (the kernel tree must cover the staging storage).
    pub fn seal_kernel_alias(&self, kernel_pml4: &mut PageTable) -> Result<(), AddressSpaceError> {
        self.reprotect_kernel_alias(kernel_pml4, false)
    }

    /// The inverse of [`AddressSpace::seal_kernel_alias`], restoring the
    /// identity view to RW-NX — required before [`AddressSpace::teardown`]
    /// can wipe the frames at all once `CR0.WP` is enabled (review D8).
    pub fn unseal_kernel_alias(
        &self,
        kernel_pml4: &mut PageTable,
    ) -> Result<(), AddressSpaceError> {
        self.reprotect_kernel_alias(kernel_pml4, true)
    }

    fn reprotect_kernel_alias(
        &self,
        kernel_pml4: &mut PageTable,
        writable: bool,
    ) -> Result<(), AddressSpaceError> {
        let Some((start, end)) = self.image_range else {
            return Ok(());
        };
        let mut virt = start;
        while virt < end {
            if let Some(mapped) = self.translate(virt) {
                if !mapped.writable {
                    paging::protect_4k(kernel_pml4, mapped.phys, writable, false)
                        .map_err(AddressSpaceError::from)?;
                    // `LE-102`, same reason as `unmap_page`'s: ring-0 only.
                    #[cfg(target_os = "none")]
                    paging::invalidate_page(mapped.phys);
                }
            }
            virt += PAGE_SIZE;
        }
        Ok(())
    }

    /// Generation-safe teardown, per the charter's `PD-13` and review D8's
    /// protocol (`STORY-P1-03-02` acceptance criterion A2): revokes every
    /// image mapping `create` built, wipes the staged frames (`staging` is
    /// the same storage `create` copied into — the caller passes it back
    /// rather than this struct holding a second long-lived borrow of it),
    /// and advances `generation` — in that order, so the generation is
    /// observably new before any frame reuse.
    ///
    /// What deliberately *survives*: the shared kernel directories linked by
    /// [`AddressSpace::attach_shared_pd`] (so the torn tree remains loadable
    /// and a stale-mapping probe is even executable), and the intermediate
    /// page-table frames in `frame_pool` (present-but-empty, reclaimed only
    /// when the pool itself is reset — documented, not counted as wiped).
    /// `Drop` does **not** run — its reset would unlink the kernel
    /// directories and free the tables the still-loadable tree references.
    ///
    /// The caller must ensure this tree is not the live `CR3`'s while its
    /// own mappings are being revoked out from under it in a way the
    /// currently executing code depends on — in practice: tear down from
    /// the supervisor, never from the task being torn down.
    pub fn teardown(
        self,
        staging: &mut [u8],
        generation: &mut TeardownGeneration,
    ) -> Result<(), TeardownError> {
        let mut this = core::mem::ManuallyDrop::new(self);
        if let Some((start, end)) = this.image_range {
            let mut virt = start;
            while virt < end {
                // `NotMapped` tolerated: the hull may span gaps between
                // sections; everything present is revoked.
                let _ = paging::unmap_4k(this.pml4, virt);
                virt += PAGE_SIZE;
            }
        }
        staging.fill(0);
        generation.advance()
    }
}

/// A teardown completed its revoke/wipe phase but the arena cannot be issued
/// another distinct generation and therefore must be retired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeardownError {
    /// Advancing would wrap and revalidate an ancient generation.
    GenerationExhausted,
}

/// A monotonic teardown generation (`PD-13`): advanced after mappings are
/// revoked and frames wiped, *before* any frame reuse — so "this frame
/// belongs to generation N" is checkable evidence rather than convention.
/// Owned by the caller (typically one per staging arena), not by any single
/// `AddressSpace`, since it must outlive every space whose teardown it
/// witnesses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeardownGeneration(u64);

impl TeardownGeneration {
    /// Generation 0: nothing torn down yet. `const fn` for `static` use.
    pub const fn new() -> Self {
        TeardownGeneration(0)
    }

    /// Advances the generation by one. Exhaustion is explicit: saturation
    /// would assign the same identity to repeated lifetimes, while wrapping
    /// would revalidate an ancient one.
    pub fn advance(&mut self) -> Result<(), TeardownError> {
        let next = self.0.checked_add(1).ok_or(TeardownError::GenerationExhausted)?;
        self.0 = next;
        Ok(())
    }

    /// The current generation number.
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl Default for TeardownGeneration {
    fn default() -> Self {
        Self::new()
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

    /// `LE-102`. Every ring-0 helper reached from this module must be gated on
    /// `target_os = "none"` and never on `not(target_os = "windows")`.
    ///
    /// The two are the same condition read from this project's Windows bench
    /// and are not the same condition on a Linux runner, where the old gate is
    /// SATISFIED. `invalidate_page` is `invlpg`; `write_cr3`/`read_cr3` are
    /// `mov` to and from `CR3`. Executing any of them in a userspace test
    /// process is a `#GP` the process sees as `SIGSEGV` — which is exactly how
    /// the `exec` suite died on the first CI run that linked far enough to run
    /// a test, after five tests had already passed. `unmap_page` is safe, is
    /// public, and is called by ordinary unit tests; nothing about the crash
    /// was reachable from here.
    ///
    /// TWO exclusions, and both were earned rather than anticipated. Comment
    /// lines are skipped because the explanations of this very gate name
    /// `invalidate_page` in prose. And the scan STOPS at `#[cfg(test)]`,
    /// because the first version of this test failed on its own needle list —
    /// the four string literals below are lines containing `write_cr3(` and
    /// the rest, so a whole-file scan reported them as ungated ring-0 calls.
    /// That is `metric_labels.rs`'s self-match for the third time in this
    /// repository, and stopping at the test boundary is also the honest rule:
    /// this module's shipped code is what can reach a ring-0 instruction.
    #[test]
    fn every_ring0_helper_call_is_gated_to_the_bare_metal_target() {
        const RING0: [&str; 4] =
            ["invalidate_page(", "write_cr3(", "read_cr3(", "enable_nx_and_wp("];
        const GATE: &str = "#[cfg(target_os = \"none\")]";
        let source = include_str!("address_space.rs");
        let shipped = source.split("\n#[cfg(test)]\n").next().unwrap_or(source);
        let lines: Vec<&str> = shipped.lines().map(str::trim).collect();
        assert!(
            lines.len() < source.lines().count(),
            "the scan must stop at the test module; if this file's `#[cfg(test)]` marker \
             moves or is reindented, the scan silently covers its own needle list again"
        );
        let mut offenders = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            if line.starts_with("//") || !RING0.iter().any(|needle| line.contains(needle)) {
                continue;
            }
            let gated = index.checked_sub(1).is_some_and(|i| lines[i] == GATE);
            if !gated {
                offenders.push(format!("line {}: {line}", index + 1));
            }
        }
        assert!(
            offenders.is_empty(),
            "LE-102: these ring-0 instructions are reachable from a hosted test binary, \
             where they raise #GP and the process dies with SIGSEGV: {offenders:?}"
        );
    }

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

    #[test]
    fn validate_rejects_non_canonical_section_range() {
        let sections = [section(0, PAGE_SIZE as u32, RX)];
        assert_eq!(
            validate_sections(&sections, LOWER_CANONICAL_END),
            Err(AddressSpaceError::NonCanonical)
        );
        assert_eq!(
            validate_sections(
                &[section(0, 2 * PAGE_SIZE as u32, RX)],
                LOWER_CANONICAL_END - PAGE_SIZE
            ),
            Err(AddressSpaceError::NonCanonical)
        );
        assert_eq!(validate_sections(&sections, LOWER_CANONICAL_END - PAGE_SIZE), Ok(()));
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
        assert!(code_page.user_accessible);

        let data_page =
            space.translate(IMAGE_BASE + PAGE_SIZE).expect("data page should be mapped");
        assert!(data_page.writable);
        assert!(!data_page.executable);
        assert!(data_page.user_accessible);
    }

    // STORY-P1-03-01: `cr3()` reports the same address the caller's own
    // `pml4` binding already lives at — the no-higher-half-split assumption
    // stated in `cr3()`'s own doc comment, pinned rather than just asserted.
    #[test]
    fn cr3_reports_the_pml4s_own_address() {
        let mut pml4 = PageTable::new();
        let pml4_addr = core::ptr::from_ref(&pml4) as u64;
        let mut frame_pool: Pool<PageTable, 4> = Pool::new();
        let space =
            AddressSpace { pml4: &mut pml4, frame_pool: &mut frame_pool, image_range: None };
        assert_eq!(space.cr3(), pml4_addr);
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
        let mut space =
            AddressSpace { pml4: &mut pml4, frame_pool: &mut frame_pool, image_range: None };

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
        let mut space =
            AddressSpace { pml4: &mut pml4, frame_pool: &mut frame_pool, image_range: None };
        assert_eq!(space.unmap_page(IMAGE_BASE), Err(AddressSpaceError::NotMapped));
    }

    // Mapping the same page twice via `map_page` fails closed rather than
    // silently overwriting the first mapping.
    #[test]
    fn map_page_rejects_an_already_mapped_address() {
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 4> = Pool::new();
        let mut space =
            AddressSpace { pml4: &mut pml4, frame_pool: &mut frame_pool, image_range: None };
        space.map_page(IMAGE_BASE, 0x9000, RW).unwrap();
        assert_eq!(space.map_page(IMAGE_BASE, 0xa000, RW), Err(AddressSpaceError::AlreadyMapped));
    }

    /// One RX page then one RW page — the smallest section set exercising
    /// both the sealed (non-writable) and unsealed (writable) alias cases.
    fn rx_then_rw_sections() -> [SectionDescriptor; 2] {
        [
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
        ]
    }

    /// A stand-in kernel tree identity-mapping `staging`'s pages RW-NX —
    /// what `exec::kernel_map`'s real directories provide under QEMU.
    fn kernel_tree_over(
        staging: &AlignedPages,
        pml4: &mut PageTable,
        frame_pool: &mut Pool<PageTable, 16>,
    ) {
        struct PoolAlloc<'a>(&'a mut Pool<PageTable, 16>);
        impl FrameAllocator for PoolAlloc<'_> {
            fn allocate_frame(&mut self) -> Option<u64> {
                let handle = self.0.alloc(PageTable::new()).ok()?;
                self.0.get_mut(handle).map(|t| core::ptr::from_mut(t) as u64)
            }
        }
        let mut allocator = PoolAlloc(frame_pool);
        let base = staging.0.as_ptr() as u64;
        for page in 0..2u64 {
            paging::map_4k(
                pml4,
                &mut allocator,
                base + page * PAGE_SIZE,
                base + page * PAGE_SIZE,
                true,
                false,
            )
            .expect("identity view of staging must map");
        }
    }

    // STORY-P1-03-02 AC A3 (review D5): sealing removes the writable
    // identity-view alias of every non-writable image page, leaves the
    // writable pages' aliases alone, and unsealing restores them.
    #[test]
    fn sealing_removes_the_writable_alias_of_non_writable_pages_only() {
        let bytes = AlignedPages([0x5A; 8192]);
        let mut staging = AlignedPages([0; 8192]);
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 16> = Pool::new();
        let sections = rx_then_rw_sections();
        let space = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        )
        .expect("valid sections should map");

        let mut kernel_pml4 = PageTable::new();
        let mut kernel_pool: Pool<PageTable, 16> = Pool::new();
        kernel_tree_over(&staging, &mut kernel_pml4, &mut kernel_pool);

        let rx_frame = space.translate(IMAGE_BASE).unwrap().phys;
        let rw_frame = space.translate(IMAGE_BASE + PAGE_SIZE).unwrap().phys;

        space.seal_kernel_alias(&mut kernel_pml4).expect("sealing over a covering tree succeeds");
        let sealed = paging::translate(&kernel_pml4, rx_frame).unwrap();
        assert!(!sealed.writable && !sealed.executable, "the RX page's alias must be RO-NX");
        let untouched = paging::translate(&kernel_pml4, rw_frame).unwrap();
        assert!(untouched.writable, "a writable page's alias stays writable");

        space.unseal_kernel_alias(&mut kernel_pml4).expect("unsealing succeeds");
        assert!(paging::translate(&kernel_pml4, rx_frame).unwrap().writable);
    }

    // Sealing against a kernel tree that doesn't cover the staged frames
    // fails closed — never a silent partial seal.
    #[test]
    fn sealing_without_a_covering_kernel_tree_fails_closed() {
        let bytes = AlignedPages([0; 8192]);
        let mut staging = AlignedPages([0; 8192]);
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 16> = Pool::new();
        let sections = rx_then_rw_sections();
        let space = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        )
        .unwrap();
        let mut empty_kernel_tree = PageTable::new();
        assert_eq!(
            space.seal_kernel_alias(&mut empty_kernel_tree),
            Err(AddressSpaceError::NotMapped)
        );
    }

    // STORY-P1-03-02 AC A2 (`PD-13`, review D8): teardown revokes the image
    // mappings, wipes the staged frames, advances the generation — and
    // leaves an attached shared directory linked, so the torn tree stays
    // loadable for the stale-mapping probe.
    #[test]
    fn teardown_revokes_wipes_and_advances_while_keeping_shared_directories() {
        let bytes = AlignedPages([0xEE; 8192]);
        let mut staging = AlignedPages([0; 8192]);
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 16> = Pool::new();
        let sections = rx_then_rw_sections();
        let mut space = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        )
        .expect("valid sections should map");
        // A stand-in shared kernel directory at a fixed fake address —
        // teardown must leave the link itself intact.
        space.attach_shared_pd(0, 0x7000).expect("linking a shared directory succeeds");
        assert_eq!(space.translate(IMAGE_BASE).map(|p| p.writable), Some(false));

        let mut generation = TeardownGeneration::new();
        assert_eq!(generation.value(), 0);
        space.teardown(&mut staging.0, &mut generation).unwrap();

        assert_eq!(generation.value(), 1, "the generation advances exactly once per teardown");
        assert!(staging.0.iter().all(|&b| b == 0), "no residue of the dead task's frames");
        assert_eq!(paging::translate(&pml4, IMAGE_BASE), None, "image mappings are revoked");
        assert_eq!(
            paging::translate(&pml4, IMAGE_BASE + PAGE_SIZE),
            None,
            "every image page is revoked, not just the first"
        );
        assert_eq!(
            paging::directory_addr(&pml4, 0),
            Some(0x7000),
            "the shared kernel directory survives teardown — the torn tree stays loadable"
        );
    }

    #[test]
    fn teardown_generation_exhaustion_retires_the_arena_instead_of_reusing_identity() {
        let mut generation = TeardownGeneration(u64::MAX);
        assert_eq!(generation.advance(), Err(TeardownError::GenerationExhausted));
        assert_eq!(generation.value(), u64::MAX);
    }
}
