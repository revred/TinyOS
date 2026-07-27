//! The shared, W^X-correct kernel page directories every address space
//! references (`STORY-P1-03-02`, acceptance criteria A3/A4).
//!
//! `STORY-P1-03-01`'s fixture identity-mapped an all-RWX kernel replica
//! into each task tree separately — an explicit, documented stand-in. This
//! module is the replacement: one page directory for kernel low memory
//! (kernel text RX, rodata RO-NX, everything else RW-NX, from the
//! [`KernelLayout`] the linker's own section-boundary symbols describe) and
//! one for the local-APIC MMIO page, each built **once** and then linked
//! into the supervisor's and every task's tree by
//! [`hal_x86_64::paging::install_shared_pd`] — shared, not duplicated, at
//! page-directory granularity (see `install_shared_pd`'s doc comment for
//! why the sharing unit can be no coarser, review D6).
//!
//! Pure page-table construction — no control registers touched here — so
//! the permission map is host-testable against a synthetic layout: the same
//! split `hal_x86_64::paging`'s own module doc draws.

use hal_x86_64::paging::{self, FrameAllocator, PageTable, PagingError, GIB, PAGE_SIZE};
use kernel::mem::Pool;

/// The kernel image's own extent and internal permission boundaries, as
/// physical/identity addresses. In production these come from the linker
/// script's `__kernel_exec_start`/`__kernel_exec_end`/
/// `__kernel_rodata_start`/`__kernel_rodata_end`/`__kernel_image_end`
/// symbols (`targets/x86_64-tinyos.ld`), so the permission map can never
/// drift from the layout the linker actually produced; host tests supply a
/// synthetic one.
#[derive(Debug, Clone, Copy)]
pub struct KernelLayout {
    /// Start of the executable range (`.boot` + `.text`), page-aligned by
    /// the linker script.
    pub exec_start: u64,
    /// One past the last executable byte (not necessarily page-aligned; the
    /// builder rounds up, which never spills into `.rodata` because the
    /// linker aligns `.rodata`'s start to a page).
    pub exec_end: u64,
    /// Start of `.rodata`, page-aligned by the linker script.
    pub rodata_start: u64,
    /// One past the last `.rodata` byte (rounded up the same way).
    pub rodata_end: u64,
    /// One past the last byte of the loaded image (`.bss` end) — everything
    /// from 0 up to this, rounded to the directory's 2MiB granularity, is
    /// mapped; everything the kernel was never linked to own is *not*.
    pub image_end: u64,
}

/// Errors [`build_shared_directories`] fails closed with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelMapError {
    /// A layout boundary that the linker script guarantees page-aligned
    /// wasn't — the layout didn't come from the linker symbols.
    Misaligned,
    /// The layout's ranges are not ordered `exec <= rodata <= image_end`.
    InvalidLayout,
    /// The frame pool ran out of page-table frames.
    FrameExhausted,
    /// The scratch tree hit an already-present entry — unreachable for a
    /// fresh build, kept exhaustive per `sched::TaskCreateError`'s precedent.
    AlreadyMapped,
}

impl From<PagingError> for KernelMapError {
    fn from(err: PagingError) -> Self {
        match err {
            PagingError::FrameExhausted => KernelMapError::FrameExhausted,
            PagingError::AlreadyMapped => KernelMapError::AlreadyMapped,
            PagingError::Misaligned | PagingError::NotMapped => KernelMapError::Misaligned,
        }
    }
}

/// The two shared directories, by the 1GiB region each serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedKernelDirectories {
    /// Serves `[0, 1GiB)`: the kernel image plus the low memory around it.
    pub low_pd: u64,
    /// Serves the 1GiB region containing `apic_page` — the local-APIC MMIO
    /// window (`0xFEE0_0000` architecturally), mapped RW-NX so the armed
    /// boot-path timer keeps working under every space.
    pub apic_pd: u64,
    /// The 1GiB-aligned base [`SharedKernelDirectories::apic_pd`] serves —
    /// what a caller passes to `install_shared_pd` alongside it.
    pub apic_base: u64,
}

fn align_up(value: u64, alignment: u64) -> u64 {
    value.div_ceil(alignment) * alignment
}

/// A [`Pool`]-backed allocator identical in shape to
/// `address_space::PoolFrameAllocator` — local rather than shared because
/// that one is deliberately private to its own module.
struct PoolFrames<'a, const FRAMES: usize> {
    pool: &'a mut Pool<PageTable, FRAMES>,
}

impl<const FRAMES: usize> FrameAllocator for PoolFrames<'_, FRAMES> {
    fn allocate_frame(&mut self) -> Option<u64> {
        let handle = self.pool.alloc(PageTable::new()).ok()?;
        let table = self.pool.get_mut(handle)?;
        Some(core::ptr::from_mut(table) as u64)
    }
}

/// Builds the two shared kernel directories out of `pool` (which must
/// outlive and never move under every tree that links them — the same
/// stability discipline `address_space`'s own doc comment states).
///
/// The identity map it constructs covers `[0, align_up(image_end, 2MiB))`
/// at 4KiB granularity with W^X-correct permissions: pages inside
/// `[exec_start, exec_end)` RX, pages inside `[rodata_start, rodata_end)`
/// RO-NX, everything else RW-NX — there is no combination that yields a
/// writable *and* executable page, by construction, and the fixture's walk
/// audit re-verifies that rather than trusting this sentence.
///
/// Built through a scratch PML4 and [`paging::map_4k`] (the already-tested
/// path that owns the NX/W bit encoding), then the two directories are
/// extracted via [`paging::directory_addr`]; the scratch PML4/PDPT frames
/// stay allocated in `pool` — two frames, documented cost, not a leak the
/// caller can't see.
pub fn build_shared_directories<const FRAMES: usize>(
    pool: &mut Pool<PageTable, FRAMES>,
    layout: KernelLayout,
    apic_page: u64,
) -> Result<SharedKernelDirectories, KernelMapError> {
    for boundary in [layout.exec_start, layout.rodata_start] {
        if !boundary.is_multiple_of(PAGE_SIZE) {
            return Err(KernelMapError::Misaligned);
        }
    }
    if !apic_page.is_multiple_of(PAGE_SIZE) {
        return Err(KernelMapError::Misaligned);
    }
    let exec_end = align_up(layout.exec_end, PAGE_SIZE);
    let rodata_end = align_up(layout.rodata_end, PAGE_SIZE);
    if layout.exec_start > layout.exec_end
        || exec_end > layout.rodata_start
        || layout.rodata_start > layout.rodata_end
        || rodata_end > layout.image_end
    {
        return Err(KernelMapError::InvalidLayout);
    }

    let mut scratch = PageTable::new();
    let low_end = align_up(layout.image_end, 2 * 1024 * 1024);
    {
        let mut allocator = PoolFrames { pool };
        let mut page = 0u64;
        while page < low_end {
            let executable = page >= layout.exec_start && page < exec_end;
            let read_only = page >= layout.rodata_start && page < rodata_end;
            let writable = !executable && !read_only;
            paging::map_4k(&mut scratch, &mut allocator, page, page, writable, executable)?;
            page += PAGE_SIZE;
        }
        paging::map_4k(&mut scratch, &mut allocator, apic_page, apic_page, true, false)?;
    }

    let low_pd = paging::directory_addr(&scratch, 0).ok_or(KernelMapError::InvalidLayout)?;
    let apic_base = apic_page - (apic_page % GIB);
    let apic_pd =
        paging::directory_addr(&scratch, apic_page).ok_or(KernelMapError::InvalidLayout)?;
    Ok(SharedKernelDirectories { low_pd, apic_pd, apic_base })
}

#[cfg(test)]
mod tests {
    use super::*;

    const APIC_PAGE: u64 = 0xFEE0_0000;

    fn small_layout() -> KernelLayout {
        KernelLayout {
            exec_start: 0x1000,
            exec_end: 0x2800, // deliberately not page-aligned
            rodata_start: 0x3000,
            rodata_end: 0x3400, // deliberately not page-aligned
            image_end: 0x6000,
        }
    }

    // Heap-pinned storage throughout these helpers: page-table frames are
    // referenced by physical (here: virtual-identity) address from other
    // tables, so the storage must never move after construction — and a
    // 64-frame pool by value would also overflow the test thread's stack.
    fn build() -> (std::boxed::Box<Pool<PageTable, 64>>, SharedKernelDirectories) {
        let mut pool = std::boxed::Box::new(Pool::<PageTable, 64>::new());
        let dirs = build_shared_directories(&mut *pool, small_layout(), APIC_PAGE)
            .expect("a well-formed layout must build");
        (pool, dirs)
    }

    fn tree_over(
        dirs: &SharedKernelDirectories,
    ) -> (std::boxed::Box<PageTable>, std::boxed::Box<[PageTable; 2]>) {
        // A caller's own PML4 linking both shared directories, exactly as
        // the supervisor and every task space do.
        let mut pml4 = std::boxed::Box::new(PageTable::new());
        let mut frames = std::boxed::Box::new([PageTable::new(), PageTable::new()]);
        struct Two<'a>(&'a mut [PageTable; 2], usize);
        impl FrameAllocator for Two<'_> {
            fn allocate_frame(&mut self) -> Option<u64> {
                let frame = self.0.get_mut(self.1)?;
                self.1 += 1;
                Some(core::ptr::from_mut(frame) as u64)
            }
        }
        let mut allocator = Two(&mut frames, 0);
        paging::install_shared_pd(&mut pml4, &mut allocator, 0, dirs.low_pd).unwrap();
        paging::install_shared_pd(&mut pml4, &mut allocator, dirs.apic_base, dirs.apic_pd).unwrap();
        (pml4, frames)
    }

    // STORY-P1-03-02 AC A3: the constructed map is W^X-correct by region —
    // exec pages RX, rodata RO-NX, data RW-NX, checked through a linking
    // tree via `translate` rather than trusted from the builder.
    #[test]
    fn the_kernel_map_is_wx_correct_by_region() {
        let (_pool, dirs) = build();
        let (pml4, _frames) = tree_over(&dirs);

        let exec = paging::translate(&pml4, 0x1000).expect("exec page mapped");
        assert!(exec.executable && !exec.writable, "kernel text must be RX");
        // 0x2800 rounds up to 0x3000 — the page containing the text tail
        // stays executable, and .rodata's aligned start is untouched by it.
        let exec_tail = paging::translate(&pml4, 0x2000).expect("exec tail page mapped");
        assert!(exec_tail.executable && !exec_tail.writable);

        let rodata = paging::translate(&pml4, 0x3000).expect("rodata page mapped");
        assert!(!rodata.executable && !rodata.writable, "rodata must be RO-NX");

        let data = paging::translate(&pml4, 0x5000).expect("data page mapped");
        assert!(!data.executable && data.writable, "data must be RW-NX");

        let low = paging::translate(&pml4, 0).expect("the low page below the image is mapped");
        assert!(!low.executable && low.writable);

        let apic = paging::translate(&pml4, APIC_PAGE).expect("APIC MMIO page mapped");
        assert!(!apic.executable && apic.writable);

        // And the audit predicate over the whole linked tree: nothing is
        // simultaneously writable and executable.
        let mut wx = 0usize;
        paging::for_each_leaf(&pml4, &mut |_, page| {
            if page.writable && page.executable {
                wx += 1;
            }
        });
        assert_eq!(wx, 0);
    }

    // STORY-P1-03-02 AC A4: two trees linking the directories share *one*
    // directory each — the same physical address read back through both.
    #[test]
    fn two_trees_link_the_same_physical_directories() {
        let (_pool, dirs) = build();
        let (tree_a, _fa) = tree_over(&dirs);
        let (tree_b, _fb) = tree_over(&dirs);
        assert_eq!(paging::directory_addr(&tree_a, 0), Some(dirs.low_pd));
        assert_eq!(paging::directory_addr(&tree_b, 0), Some(dirs.low_pd));
        assert_eq!(paging::directory_addr(&tree_a, APIC_PAGE), Some(dirs.apic_pd));
        assert_eq!(paging::directory_addr(&tree_b, APIC_PAGE), Some(dirs.apic_pd));
    }

    // The identity map stops at the rounded image end: memory the kernel
    // was never linked to own is not mapped at all.
    #[test]
    fn the_map_covers_only_the_kernel_images_own_extent() {
        let (_pool, dirs) = build();
        let (pml4, _frames) = tree_over(&dirs);
        // image_end 0x6000 rounds up to one 2MiB directory entry.
        assert!(paging::translate(&pml4, 0x1F_F000).is_some(), "inside the rounded extent");
        assert_eq!(paging::translate(&pml4, 0x20_0000), None, "past the rounded extent");
    }

    // A layout whose boundaries can't have come from the linker symbols
    // fails closed before touching the pool's frames beyond the scratch.
    #[test]
    fn malformed_layouts_fail_closed() {
        let mut pool: Pool<PageTable, 64> = Pool::new();
        let mut misaligned = small_layout();
        misaligned.rodata_start = 0x3001;
        assert_eq!(
            build_shared_directories(&mut pool, misaligned, APIC_PAGE),
            Err(KernelMapError::Misaligned)
        );

        let mut pool: Pool<PageTable, 64> = Pool::new();
        let mut inverted = small_layout();
        inverted.image_end = 0x2000; // ends before rodata does
        assert_eq!(
            build_shared_directories(&mut pool, inverted, APIC_PAGE),
            Err(KernelMapError::InvalidLayout)
        );
    }

    // Pool exhaustion partway through construction is a typed error, not a
    // panic — the caller sized the pool, the builder reports the miss.
    #[test]
    fn frame_exhaustion_fails_closed() {
        let mut pool: Pool<PageTable, 2> = Pool::new();
        assert_eq!(
            build_shared_directories(&mut pool, small_layout(), APIC_PAGE),
            Err(KernelMapError::FrameExhausted)
        );
    }
}
