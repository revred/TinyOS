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
//! live address space — was out of scope for `STORY-P0-05-02` (its
//! acceptance criteria explicitly deferred full per-process isolation).
//! [`read_cr3`]/[`write_cr3`] add exactly that (`STORY-P1-03-01`), kept to
//! the bottom of this module and behind the same `not(target_os =
//! "windows")` gate `hal_x86_64::fault`'s assembly stubs use, since they are
//! the one pair of functions here that actually touch a CPU control
//! register rather than ordinary memory.
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
    /// [`unmap_4k`] was called against a `virt` with no present leaf entry
    /// (some level from the PML4 down to the leaf PTE is not present).
    NotMapped,
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

/// Clears `virt`'s leaf page-table entry, if present — the counterpart to
/// [`map_4k`] (`STORY-P0-07-02`, deterministic shared-memory grant
/// revocation). Never frees an intermediate PDPT/PD/PT level even if this
/// was its last present leaf entry: this module has no notion of a
/// `FrameAllocator`-side "free," and an empty-but-still-`PRESENT`
/// intermediate level is otherwise harmless, just unreclaimed — matching
/// this module's own "read/write ordinary memory, no CPU control
/// registers" scope, never more.
///
/// Fails closed with [`PagingError::Misaligned`] if `virt` isn't
/// [`PAGE_SIZE`]-aligned, or [`PagingError::NotMapped`] if any level from
/// the PML4 down to the leaf PTE is not present — never silently a no-op.
pub fn unmap_4k(pml4: &mut PageTable, virt: u64) -> Result<(), PagingError> {
    if !virt.is_multiple_of(PAGE_SIZE) {
        return Err(PagingError::Misaligned);
    }
    let mut table = pml4;
    for shift in DIRECTORY_SHIFTS {
        let entry = table.entries[table_index(shift, virt)];
        if entry & PRESENT == 0 {
            return Err(PagingError::NotMapped);
        }
        let child_addr = entry & ADDR_MASK;
        // SAFETY: mirrors `walk_create`/`translate` — a present entry in a
        // table this function was handed only ever points to another live
        // `PageTable` that `map_4k`/`walk_create` constructed.
        table = unsafe { &mut *(child_addr as *mut PageTable) };
    }
    let index = table_index(LEAF_SHIFT, virt);
    if table.entries[index] & PRESENT == 0 {
        return Err(PagingError::NotMapped);
    }
    table.entries[index] = 0;
    Ok(())
}

/// Rewrites the permission bits of `virt`'s already-present leaf entry,
/// leaving its frame address untouched (`STORY-P1-03-02`): the primitive
/// behind *sealing* — after a loader has copied an image's bytes through a
/// writable alias, that alias is re-protected read-only so no executable
/// frame keeps a writable view anywhere (review D5's alias hole).
///
/// Fails closed with [`PagingError::Misaligned`] if `virt` isn't
/// [`PAGE_SIZE`]-aligned, or [`PagingError::NotMapped`] if any level down to
/// the leaf is not present — re-protecting nothing is never a silent no-op.
pub fn protect_4k(
    pml4: &mut PageTable,
    virt: u64,
    writable: bool,
    executable: bool,
) -> Result<(), PagingError> {
    if !virt.is_multiple_of(PAGE_SIZE) {
        return Err(PagingError::Misaligned);
    }
    let mut table = pml4;
    for shift in DIRECTORY_SHIFTS {
        let entry = table.entries[table_index(shift, virt)];
        if entry & PRESENT == 0 {
            return Err(PagingError::NotMapped);
        }
        let child_addr = entry & ADDR_MASK;
        // SAFETY: mirrors `walk_create`/`unmap_4k` — a present entry only
        // ever points to another live `PageTable` this module constructed.
        table = unsafe { &mut *(child_addr as *mut PageTable) };
    }
    let index = table_index(LEAF_SHIFT, virt);
    let entry = table.entries[index];
    if entry & PRESENT == 0 {
        return Err(PagingError::NotMapped);
    }
    let mut flags = PRESENT;
    if writable {
        flags |= WRITABLE;
    }
    if !executable {
        flags |= NO_EXECUTE;
    }
    table.entries[index] = (entry & ADDR_MASK) | flags;
    Ok(())
}

/// Bytes one PDPT entry (and therefore one shared page directory) spans.
pub const GIB: u64 = 1 << 30;

/// Links an existing page directory at physical address `pd` into `pml4`'s
/// tree so that it serves the whole 1GiB region starting at `virt_base`
/// (`STORY-P1-03-02` acceptance criterion A4).
///
/// This is how kernel mappings are *shared* rather than duplicated: one
/// W^X-correct directory, built once, referenced from every space's own
/// PDPT. Sharing sits at PD granularity deliberately — sharing a PML4 entry
/// or a whole PDPT would share everything else under it too, and the image
/// base (`0x1_4000_0000`) lives under the same PML4 slot as kernel low
/// memory (review D6), so a coarser unit would leak one task's image into
/// every other space.
///
/// Fails closed with [`PagingError::Misaligned`] if `virt_base` isn't
/// 1GiB-aligned or `pd` isn't [`PAGE_SIZE`]-aligned,
/// [`PagingError::AlreadyMapped`] if the PDPT slot is already present, and
/// [`PagingError::FrameExhausted`] if a missing PDPT can't be created.
pub fn install_shared_pd(
    pml4: &mut PageTable,
    allocator: &mut dyn FrameAllocator,
    virt_base: u64,
    pd: FrameAddr,
) -> Result<(), PagingError> {
    if !virt_base.is_multiple_of(GIB) || !pd.is_multiple_of(PAGE_SIZE) {
        return Err(PagingError::Misaligned);
    }
    let pml4_index = table_index(DIRECTORY_SHIFTS[0], virt_base);
    let entry = pml4.entries[pml4_index];
    let pdpt_addr = if entry & PRESENT != 0 {
        entry & ADDR_MASK
    } else {
        let frame = allocator.allocate_frame().ok_or(PagingError::FrameExhausted)?;
        pml4.entries[pml4_index] = (frame & ADDR_MASK) | PRESENT | WRITABLE;
        frame
    };
    // SAFETY: mirrors `walk_create` — the entry either came from `allocator`
    // (fresh, zeroed table) or was written by this module over the same tree.
    let pdpt = unsafe { &mut *(pdpt_addr as *mut PageTable) };
    let pdpt_index = table_index(DIRECTORY_SHIFTS[1], virt_base);
    if pdpt.entries[pdpt_index] & PRESENT != 0 {
        return Err(PagingError::AlreadyMapped);
    }
    pdpt.entries[pdpt_index] = (pd & ADDR_MASK) | PRESENT | WRITABLE;
    Ok(())
}

/// The physical address of the page *directory* serving `virt`'s 1GiB
/// region in `pml4`'s tree, or `None` if the walk ends before one — the
/// read-back half of [`install_shared_pd`], so a fixture can *prove* two
/// trees share one directory rather than each holding a same-shaped copy.
pub fn directory_addr(pml4: &PageTable, virt: u64) -> Option<FrameAddr> {
    let entry = pml4.entries[table_index(DIRECTORY_SHIFTS[0], virt)];
    if entry & PRESENT == 0 {
        return None;
    }
    // SAFETY: mirrors `translate` — a present entry points to a live table.
    let pdpt = unsafe { &*((entry & ADDR_MASK) as *const PageTable) };
    let pdpt_entry = pdpt.entries[table_index(DIRECTORY_SHIFTS[1], virt)];
    if pdpt_entry & PRESENT == 0 {
        return None;
    }
    Some(pdpt_entry & ADDR_MASK)
}

/// x86_64 page-size bit (bit 7): a PDPT/PD entry with it set maps a huge
/// page directly rather than pointing at a further table.
const PAGE_SIZE_BIT: u64 = 1 << 7;

/// Walks every present leaf mapping in `pml4`'s tree, calling `visit` with
/// each mapped virtual address and its [`MappedPage`] — the audit primitive
/// behind `STORY-P1-03-02` acceptance criterion A3 ("no leaf mapping is
/// simultaneously writable and executable, verified by a walk, not by
/// convention"). A PDPT/PD entry with the page-size bit set (a huge page —
/// e.g. `boot.rs`'s bring-up identity map, if this is ever pointed at it) is
/// reported as a single leaf at its own granularity rather than misread as
/// a further table.
pub fn for_each_leaf(pml4: &PageTable, visit: &mut dyn FnMut(u64, MappedPage)) {
    for (i, &pml4_entry) in pml4.entries.iter().enumerate() {
        if pml4_entry & PRESENT == 0 {
            continue;
        }
        // SAFETY: mirrors `translate` — present entries point to live tables.
        let pdpt = unsafe { &*((pml4_entry & ADDR_MASK) as *const PageTable) };
        for (j, &pdpt_entry) in pdpt.entries.iter().enumerate() {
            if pdpt_entry & PRESENT == 0 {
                continue;
            }
            let base_1g = ((i as u64) << DIRECTORY_SHIFTS[0]) | ((j as u64) << DIRECTORY_SHIFTS[1]);
            if pdpt_entry & PAGE_SIZE_BIT != 0 {
                visit(base_1g, entry_to_page(pdpt_entry));
                continue;
            }
            // SAFETY: as above.
            let pd = unsafe { &*((pdpt_entry & ADDR_MASK) as *const PageTable) };
            for (k, &pd_entry) in pd.entries.iter().enumerate() {
                if pd_entry & PRESENT == 0 {
                    continue;
                }
                let base_2m = base_1g | ((k as u64) << DIRECTORY_SHIFTS[2]);
                if pd_entry & PAGE_SIZE_BIT != 0 {
                    visit(base_2m, entry_to_page(pd_entry));
                    continue;
                }
                // SAFETY: as above.
                let pt = unsafe { &*((pd_entry & ADDR_MASK) as *const PageTable) };
                for (l, &pt_entry) in pt.entries.iter().enumerate() {
                    if pt_entry & PRESENT == 0 {
                        continue;
                    }
                    visit(base_2m | ((l as u64) << LEAF_SHIFT), entry_to_page(pt_entry));
                }
            }
        }
    }
}

const fn entry_to_page(entry: u64) -> MappedPage {
    MappedPage {
        phys: entry & ADDR_MASK,
        writable: entry & WRITABLE != 0,
        executable: entry & NO_EXECUTE == 0,
    }
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

/// Whether switching to a task whose address space's PML4 lives at physical
/// address `next` needs a real `CR3` reload, given the value currently
/// loaded is `current` (`STORY-P1-03-01` acceptance criterion 2).
///
/// Pure comparison, deliberately split out from [`write_cr3`] so the decision
/// is host-testable independent of the two functions below that actually
/// touch the register — same split this module's own doc comment already
/// draws between ordinary memory manipulation and CPU control state.
pub const fn cr3_reload_needed(current: u64, next: u64) -> bool {
    current != next
}

/// Reads the CPU's current `CR3` value — the physical address of the active
/// PML4 (`STORY-P1-03-01`).
#[cfg(not(target_os = "windows"))]
pub fn read_cr3() -> u64 {
    let value: u64;
    // SAFETY: `mov reg, cr3` only reads a control register into a general
    // register; it has no memory effect and cannot fault.
    unsafe {
        core::arch::asm!("mov {}, cr3", out(reg) value, options(nomem, nostack, preserves_flags));
    }
    value
}

/// Loads `phys` into `CR3`, making the PML4 at that physical address the
/// CPU's live address space (`STORY-P1-03-01`).
///
/// # Safety
/// `phys` must be the physical, [`PAGE_SIZE`]-aligned address of a fully
/// populated PML4 that maps every page the CPU will need to fetch or touch
/// **immediately after this instruction retires**: the currently executing
/// code and its stack at minimum, and — if interrupts are enabled — the
/// IDT/GDT/TSS and their handlers too. Loading a PML4 missing any of those is
/// an immediate, unrecoverable fault with no handler able to run (there is no
/// stack or code left mapped to run it from). This function also flushes
/// every non-global TLB entry, per the architecture's own `CR3`-write
/// semantics.
#[cfg(not(target_os = "windows"))]
pub unsafe fn write_cr3(phys: u64) {
    // SAFETY: per this function's own contract.
    unsafe {
        core::arch::asm!("mov cr3, {}", in(reg) phys, options(nomem, nostack, preserves_flags));
    }
}

/// Enables the two CPU bits without which W^X is unenforceable at ring 0
/// (`STORY-P1-03-02` acceptance criterion A1, review D4):
///
/// - `EFER.NXE` (MSR `0xC000_0080`, bit 11): makes PTE bit 63 mean
///   *no-execute* instead of *reserved* — [`map_4k`] has set that bit on
///   every non-executable mapping since `STORY-P0-05-02`, but only this
///   flag makes the hardware honor it (and stops it being a latent
///   reserved-bit `#PF` on stricter implementations).
/// - `CR0.WP` (bit 16): makes supervisor-mode writes respect read-only
///   pages. Without it, every "read-only" kernel/task mapping is writable
///   at CPL 0 and a write-to-executable-memory test passes vacuously.
///
/// # Safety
/// Must be called before any page table containing NX bits is loaded on a
/// strict implementation, and after it every mapping the currently
/// executing code writes through must genuinely be writable — enabling
/// `CR0.WP` turns latent permission mistakes into immediate faults.
#[cfg(not(target_os = "windows"))]
pub unsafe fn enable_nx_and_wp() {
    // SAFETY: RDMSR/WRMSR on EFER and a CR0 read-modify-write have no
    // memory operands; the architectural effects are exactly this
    // function's documented contract.
    unsafe {
        core::arch::asm!(
            "mov ecx, 0xC0000080",
            "rdmsr",
            "or eax, 1 << 11",
            "wrmsr",
            "mov {tmp}, cr0",
            "or {tmp}, 1 << 16",
            "mov cr0, {tmp}",
            tmp = out(reg) _,
            out("eax") _,
            out("ecx") _,
            out("edx") _,
            options(nomem, nostack)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // STORY-P1-03-01 AC2: the reload decision is a pure equality check,
    // host-testable independent of the real `CR3` read/write below.
    #[test]
    fn cr3_reload_is_needed_only_when_the_target_differs_from_the_current_value() {
        assert!(!cr3_reload_needed(0x1000, 0x1000));
        assert!(cr3_reload_needed(0x1000, 0x2000));
        assert!(!cr3_reload_needed(0, 0));
        assert!(cr3_reload_needed(0, 0x1000));
    }

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

    // STORY-P0-07-02: unmapping a present page clears it deterministically
    // — translate sees nothing left behind.
    #[test]
    fn unmap_clears_a_present_mapping() {
        let mut pml4 = PageTable::new();
        let mut allocator: ArrayAllocator<8> = ArrayAllocator::new();
        map_4k(&mut pml4, &mut allocator, 0x1000, 0x9000, true, false).unwrap();

        assert_eq!(unmap_4k(&mut pml4, 0x1000), Ok(()));
        assert_eq!(translate(&pml4, 0x1000), None);
    }

    // Unmapping something never mapped fails closed rather than silently
    // succeeding as a no-op.
    #[test]
    fn unmap_of_an_unmapped_address_fails_closed() {
        let mut pml4 = PageTable::new();
        assert_eq!(unmap_4k(&mut pml4, 0x1000), Err(PagingError::NotMapped));
    }

    // An unaligned address is rejected before any table is walked.
    #[test]
    fn unmap_of_an_unaligned_address_is_rejected() {
        let mut pml4 = PageTable::new();
        assert_eq!(unmap_4k(&mut pml4, 0x1001), Err(PagingError::Misaligned));
    }

    // STORY-P1-03-02: `protect_4k` rewrites permissions in place, keeping
    // the frame — the sealing primitive's round trip.
    #[test]
    fn protect_rewrites_permissions_without_moving_the_frame() {
        let mut pml4 = PageTable::new();
        let mut allocator: ArrayAllocator<8> = ArrayAllocator::new();
        map_4k(&mut pml4, &mut allocator, 0x1000, 0x9000, true, false).unwrap();

        assert_eq!(protect_4k(&mut pml4, 0x1000, false, false), Ok(()));
        let sealed = translate(&pml4, 0x1000).unwrap();
        assert_eq!(sealed.phys, 0x9000, "the frame must survive re-protection");
        assert!(!sealed.writable);
        assert!(!sealed.executable);

        // And back again — teardown unseals before wiping (review D8).
        assert_eq!(protect_4k(&mut pml4, 0x1000, true, false), Ok(()));
        assert!(translate(&pml4, 0x1000).unwrap().writable);
    }

    // Re-protecting an unmapped or unaligned address fails closed rather
    // than silently sealing nothing.
    #[test]
    fn protect_of_an_unmapped_or_unaligned_address_fails_closed() {
        let mut pml4 = PageTable::new();
        assert_eq!(protect_4k(&mut pml4, 0x1000, false, false), Err(PagingError::NotMapped));
        assert_eq!(protect_4k(&mut pml4, 0x1001, false, false), Err(PagingError::Misaligned));
    }

    // STORY-P1-03-02 AC A4: a directory installed into two trees is *the
    // same* directory — a mapping added through one tree is visible through
    // the other, and both trees read back the identical directory address.
    #[test]
    fn a_shared_pd_is_one_directory_visible_through_every_tree() {
        let mut allocator: ArrayAllocator<8> = ArrayAllocator::new();
        let mut tree_a = PageTable::new();
        map_4k(&mut tree_a, &mut allocator, 0x1000, 0x9000, false, true).unwrap();
        let pd = directory_addr(&tree_a, 0x1000).expect("the walk just created this directory");

        let mut tree_b = PageTable::new();
        assert_eq!(install_shared_pd(&mut tree_b, &mut allocator, 0, pd), Ok(()));

        assert_eq!(directory_addr(&tree_b, 0x1000), Some(pd), "both trees name one directory");
        let through_b = translate(&tree_b, 0x1000).expect("shared mapping visible through B");
        assert_eq!(through_b.phys, 0x9000);
        // The proof of *sharing* rather than copying: a page mapped through
        // tree A after the install appears through tree B too.
        map_4k(&mut tree_a, &mut allocator, 0x2000, 0xa000, true, false).unwrap();
        assert_eq!(translate(&tree_b, 0x2000).map(|p| p.phys), Some(0xa000));
    }

    // `install_shared_pd` fails closed on misalignment and on a PDPT slot
    // that already serves the region — never silently re-points live
    // mappings.
    #[test]
    fn install_shared_pd_fails_closed_on_misalignment_and_occupied_slots() {
        let mut allocator: ArrayAllocator<8> = ArrayAllocator::new();
        let mut tree = PageTable::new();
        assert_eq!(
            install_shared_pd(&mut tree, &mut allocator, 0x1000, 0x9000),
            Err(PagingError::Misaligned),
            "virt_base must be 1GiB-aligned"
        );
        assert_eq!(
            install_shared_pd(&mut tree, &mut allocator, 0, 0x9001),
            Err(PagingError::Misaligned),
            "the directory address must be page-aligned"
        );
        assert_eq!(install_shared_pd(&mut tree, &mut allocator, 0, 0x9000), Ok(()));
        assert_eq!(
            install_shared_pd(&mut tree, &mut allocator, 0, 0xa000),
            Err(PagingError::AlreadyMapped)
        );
    }

    // STORY-P1-03-02 AC A3: the audit walk visits exactly the present
    // leaves with their real permission bits.
    #[test]
    fn for_each_leaf_visits_every_present_mapping_with_its_permissions() {
        let mut pml4 = PageTable::new();
        let mut allocator: ArrayAllocator<8> = ArrayAllocator::new();
        map_4k(&mut pml4, &mut allocator, 0x1000, 0x9000, false, true).unwrap();
        map_4k(&mut pml4, &mut allocator, 0x2000, 0xa000, true, false).unwrap();

        let mut seen = std::vec::Vec::new();
        for_each_leaf(&pml4, &mut |virt, page| seen.push((virt, page.writable, page.executable)));
        seen.sort_unstable();
        assert_eq!(seen, std::vec![(0x1000, false, true), (0x2000, true, false)]);
    }

    // The W^X audit predicate over a tree that *does* contain a W+X leaf
    // must find it — the audit is falsifiable, not decorative.
    #[test]
    fn the_audit_walk_finds_a_deliberately_wx_mapping() {
        let mut pml4 = PageTable::new();
        let mut allocator: ArrayAllocator<8> = ArrayAllocator::new();
        map_4k(&mut pml4, &mut allocator, 0x1000, 0x9000, true, true).unwrap();
        let mut wx = 0usize;
        for_each_leaf(&pml4, &mut |_, page| {
            if page.writable && page.executable {
                wx += 1;
            }
        });
        assert_eq!(wx, 1);
    }

    // Unmapping one page leaves a sibling page (sharing the same
    // intermediate levels) untouched.
    #[test]
    fn unmapping_one_page_leaves_a_sibling_page_mapped() {
        let mut pml4 = PageTable::new();
        let mut allocator: ArrayAllocator<8> = ArrayAllocator::new();
        map_4k(&mut pml4, &mut allocator, 0x1000, 0x9000, true, false).unwrap();
        map_4k(&mut pml4, &mut allocator, 0x2000, 0xa000, false, true).unwrap();

        assert_eq!(unmap_4k(&mut pml4, 0x1000), Ok(()));
        assert_eq!(translate(&pml4, 0x1000), None);
        assert_eq!(translate(&pml4, 0x2000).unwrap().phys, 0xa000);
    }
}
