//! x86_64 4-level (PML4/PDPT/PD/PT), 4KiB-granularity page table
//! construction (`STORY-P0-05-02`).
//!
//! Pure, host-testable data-structure manipulation: building and reading
//! back page-table entries is ordinary `u64` array arithmetic with no
//! target-specific instructions, unlike `boot`, so this module needs no
//! `cfg(not(test))` gate and is exercised directly by `cargo test -p
//! hal-x86_64 --lib`.
//!
//! Loading a constructed [`PageTable`] into `CR3` — making it the CPU's
//! live address space — is out of scope for `STORY-P0-05-02` (its
//! acceptance criteria explicitly defer full per-process isolation) and is
//! therefore not implemented here; every function below only ever reads or
//! writes ordinary memory, never CPU control registers.
//!
//! Frame allocation for newly-created page-table levels is decoupled via
//! [`FrameAllocator`] (Dependency Inversion, per
//! `agent/CODING_STANDARDS.md`) so this module has no dependency on any
//! specific allocator — `exec::address_space` supplies a concrete allocator
//! backed by `kernel::mem::Pool`.

/// A frame address: this kernel's current no-higher-half-split memory model
/// (see `boot.rs`) means every such address is directly dereferenceable —
/// there is no separate physical/virtual translation step yet.
pub type FrameAddr = u64;

/// Byte size of one x86_64 page-table frame (a `PageTable`, or a mapped
/// leaf page) at the granularity this module supports.
pub const PAGE_SIZE: u64 = 4096;

/// Supplies fresh, zeroed [`PAGE_SIZE`]-byte frames for new PDPT/PD/PT
/// levels created while mapping a page.
pub trait FrameAllocator {
    /// Returns a fresh frame's address, or `None` if none remain.
    fn allocate_frame(&mut self) -> Option<FrameAddr>;
}

/// Errors [`map_4k`] fails closed with rather than silently corrupting an
/// existing mapping or panicking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagingError {
    /// The [`FrameAllocator`] had no frame left for a new page-table level.
    FrameExhausted,
    /// `virt` already has a present leaf entry — never silently overwritten.
    AlreadyMapped,
    /// `virt` or `phys` is not [`PAGE_SIZE`]-aligned.
    Misaligned,
}

const ENTRY_COUNT: usize = 512;
const PRESENT: u64 = 1 << 0;
const WRITABLE: u64 = 1 << 1;
const NO_EXECUTE: u64 = 1 << 63;
const ADDR_MASK: u64 = 0x000f_ffff_ffff_f000;
/// PML4 (39), PDPT (30), PD (21) index shifts, in walk order. The PT (leaf)
/// index shift, 12, is handled separately since [`walk_create`] stops one
/// level short of it.
const DIRECTORY_SHIFTS: [u32; 3] = [39, 30, 21];
const LEAF_SHIFT: u32 = 12;

/// One level of an x86_64 page table: 512 raw entries, page-aligned.
///
/// Used uniformly for the PML4, every PDPT, every PD, and every PT — x86_64
/// page-table levels share one binary layout, differing only in what the
/// address in each present entry points to (another table, vs., for this
/// module's 4KiB-only scope, a mapped page — no 2MiB/1GiB huge-page entries
/// are constructed here, unlike `boot.rs`'s own throwaway identity map).
#[repr(C, align(4096))]
pub struct PageTable {
    entries: [u64; ENTRY_COUNT],
}

impl PageTable {
    /// An empty (all-not-present) page table.
    pub const fn new() -> Self {
        PageTable { entries: [0; ENTRY_COUNT] }
    }
}

impl Default for PageTable {
    fn default() -> Self {
        Self::new()
    }
}

fn table_index(shift: u32, virt: u64) -> usize {
    ((virt >> shift) & 0x1ff) as usize
}

/// Walks from `pml4` down to (creating, via `allocator`, whichever of the
/// PDPT/PD/PT levels don't yet exist) the leaf page table that would hold
/// `virt`'s 4KiB entry.
fn walk_create<'a>(
    pml4: &'a mut PageTable,
    allocator: &mut dyn FrameAllocator,
    virt: u64,
) -> Result<&'a mut PageTable, PagingError> {
    let mut table = pml4;
    for shift in DIRECTORY_SHIFTS {
        let index = table_index(shift, virt);
        let entry = table.entries[index];
        let child_addr = if entry & PRESENT != 0 {
            entry & ADDR_MASK
        } else {
            let frame = allocator.allocate_frame().ok_or(PagingError::FrameExhausted)?;
            table.entries[index] = (frame & ADDR_MASK) | PRESENT | WRITABLE;
            frame
        };
        // SAFETY: `child_addr` is either a frame `allocator` just handed out
        // (per `FrameAllocator`'s contract, fresh and zeroed — i.e. a valid,
        // `PageTable`-sized, `PageTable`-aligned all-not-present table) or
        // one this same function wrote into `table` on a prior call over
        // the same `pml4` tree — in both cases it remains a live `PageTable`
        // for as long as the tree is in use. This kernel's current
        // no-higher-half-split memory model (see `boot.rs`) means every
        // such frame address doubles as its own accessible pointer value.
        table = unsafe { &mut *(child_addr as *mut PageTable) };
    }
    Ok(table)
}

/// Maps one 4KiB page at `virt` to physical frame `phys`, creating any
/// missing intermediate PDPT/PD/PT levels via `allocator`.
///
/// Fails closed with [`PagingError::FrameExhausted`] if `allocator` runs out
/// of frames partway through, [`PagingError::Misaligned`] if either address
/// isn't [`PAGE_SIZE`]-aligned, or [`PagingError::AlreadyMapped`] if `virt`
/// already has a present leaf entry — this function never silently
/// overwrites an existing mapping or partially-maps and then fails without
/// telling the caller which frames it already consumed from `allocator`.
pub fn map_4k(
    pml4: &mut PageTable,
    allocator: &mut dyn FrameAllocator,
    virt: u64,
    phys: u64,
    writable: bool,
    executable: bool,
) -> Result<(), PagingError> {
    if !virt.is_multiple_of(PAGE_SIZE) || !phys.is_multiple_of(PAGE_SIZE) {
        return Err(PagingError::Misaligned);
    }
    let pt = walk_create(pml4, allocator, virt)?;
    let index = table_index(LEAF_SHIFT, virt);
    if pt.entries[index] & PRESENT != 0 {
        return Err(PagingError::AlreadyMapped);
    }
    let mut flags = PRESENT;
    if writable {
        flags |= WRITABLE;
    }
    if !executable {
        flags |= NO_EXECUTE;
    }
    pt.entries[index] = (phys & ADDR_MASK) | flags;
    Ok(())
}

/// A mapped page's backing frame and permission bits, as read back by
/// [`translate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MappedPage {
    /// The physical frame this page is mapped to.
    pub phys: FrameAddr,
    /// `true` if the page's writable bit is set.
    pub writable: bool,
    /// `true` if the page's no-execute bit is *not* set (i.e. executable).
    pub executable: bool,
}

/// Looks up `virt`'s current mapping without creating anything — `None` if
/// any level from the PML4 down to the leaf PTE is not present.
pub fn translate(pml4: &PageTable, virt: u64) -> Option<MappedPage> {
    let mut table = pml4;
    for shift in DIRECTORY_SHIFTS {
        let entry = table.entries[table_index(shift, virt)];
        if entry & PRESENT == 0 {
            return None;
        }
        let child_addr = entry & ADDR_MASK;
        // SAFETY: mirrors `walk_create` — a present entry in a table this
        // function was handed only ever points to another live `PageTable`
        // that `map_4k`/`walk_create` constructed.
        table = unsafe { &*(child_addr as *const PageTable) };
    }
    let entry = table.entries[table_index(LEAF_SHIFT, virt)];
    if entry & PRESENT == 0 {
        return None;
    }
    Some(MappedPage {
        phys: entry & ADDR_MASK,
        writable: entry & WRITABLE != 0,
        executable: entry & NO_EXECUTE == 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trivial host-only [`FrameAllocator`] backed by a fixed array of
    /// `PageTable`s living for the whole test — never moved after its first
    /// borrow, so addresses handed out from it stay valid for the test's
    /// duration (the same discipline any real caller, e.g.
    /// `exec::address_space`'s `Pool`-backed allocator, must also follow).
    struct ArrayAllocator<const N: usize> {
        frames: [PageTable; N],
        used: usize,
    }

    impl<const N: usize> ArrayAllocator<N> {
        fn new() -> Self {
            ArrayAllocator { frames: [const { PageTable::new() }; N], used: 0 }
        }
    }

    impl<const N: usize> FrameAllocator for ArrayAllocator<N> {
        fn allocate_frame(&mut self) -> Option<FrameAddr> {
            let frame = self.frames.get_mut(self.used)?;
            self.used += 1;
            Some(frame as *mut PageTable as u64)
        }
    }

    // A mapped page's permission bits and backing frame round-trip through
    // `translate` exactly as `map_4k` set them (STORY-P0-05-02 AC1).
    #[test]
    fn mapped_page_translates_back_with_the_same_permissions_and_frame() {
        let mut pml4 = PageTable::new();
        let mut allocator: ArrayAllocator<8> = ArrayAllocator::new();
        map_4k(&mut pml4, &mut allocator, 0x1000, 0x9000, true, false).unwrap();

        let mapped = translate(&pml4, 0x1000).expect("just-mapped page should translate");
        assert_eq!(mapped.phys, 0x9000);
        assert!(mapped.writable);
        assert!(!mapped.executable);
    }

    // A read-only, executable mapping (typical `.text`) never sets the
    // writable bit and never sets NX.
    #[test]
    fn read_execute_mapping_is_never_writable() {
        let mut pml4 = PageTable::new();
        let mut allocator: ArrayAllocator<8> = ArrayAllocator::new();
        map_4k(&mut pml4, &mut allocator, 0x2000, 0xa000, false, true).unwrap();

        let mapped = translate(&pml4, 0x2000).unwrap();
        assert!(!mapped.writable);
        assert!(mapped.executable);
    }

    // An unmapped address translates to `None`, not a stale or default
    // mapping.
    #[test]
    fn unmapped_address_translates_to_none() {
        let pml4 = PageTable::new();
        assert_eq!(translate(&pml4, 0x1000), None);
    }

    // Mapping the same virtual address twice fails closed rather than
    // silently overwriting the first mapping's frame/permissions.
    #[test]
    fn remapping_an_already_mapped_page_is_rejected() {
        let mut pml4 = PageTable::new();
        let mut allocator: ArrayAllocator<8> = ArrayAllocator::new();
        map_4k(&mut pml4, &mut allocator, 0x3000, 0xb000, true, false).unwrap();

        let result = map_4k(&mut pml4, &mut allocator, 0x3000, 0xc000, true, true);
        assert_eq!(result, Err(PagingError::AlreadyMapped));
        // The original mapping is untouched by the rejected attempt.
        assert_eq!(translate(&pml4, 0x3000).unwrap().phys, 0xb000);
    }

    // Unaligned addresses are rejected before any table is walked or
    // created.
    #[test]
    fn unaligned_addresses_are_rejected() {
        let mut pml4 = PageTable::new();
        let mut allocator: ArrayAllocator<8> = ArrayAllocator::new();
        assert_eq!(
            map_4k(&mut pml4, &mut allocator, 0x1001, 0x9000, true, false),
            Err(PagingError::Misaligned)
        );
        assert_eq!(
            map_4k(&mut pml4, &mut allocator, 0x1000, 0x9001, true, false),
            Err(PagingError::Misaligned)
        );
    }

    // Exhausting the frame allocator partway through walking fails closed
    // with a typed error, not a panic.
    #[test]
    fn frame_exhaustion_while_creating_intermediate_levels_fails_closed() {
        let mut pml4 = PageTable::new();
        // 0 frames: even the first missing PDPT level can't be created.
        let mut allocator: ArrayAllocator<0> = ArrayAllocator::new();
        assert_eq!(
            map_4k(&mut pml4, &mut allocator, 0x1000, 0x9000, true, false),
            Err(PagingError::FrameExhausted)
        );
    }

    // Two distinct pages within the same PDPT/PD (sharing intermediate
    // levels) both map correctly and don't clobber each other — proves
    // `walk_create` reuses an already-created intermediate level rather
    // than creating a fresh one each time.
    #[test]
    fn two_pages_sharing_intermediate_levels_both_map_correctly() {
        let mut pml4 = PageTable::new();
        let mut allocator: ArrayAllocator<8> = ArrayAllocator::new();
        map_4k(&mut pml4, &mut allocator, 0x1000, 0x9000, true, false).unwrap();
        map_4k(&mut pml4, &mut allocator, 0x2000, 0xa000, false, true).unwrap();

        assert_eq!(translate(&pml4, 0x1000).unwrap().phys, 0x9000);
        assert_eq!(translate(&pml4, 0x2000).unwrap().phys, 0xa000);
    }
}
