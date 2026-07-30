//! Import Address Table resolution (`STORY-P1-03-03`).
//!
//! `STORY-P1-03-02` scheduled a real image for the first time and contained
//! what it did — but `REPORT-2026-07-28-01` recorded *how* it was contained,
//! and the honest reading of that capture is uncomfortable: nothing had
//! patched the image's IAT, so every import thunk still held the **RVA of
//! its own `IMAGE_IMPORT_BY_NAME` record**, and the CRT's first indirect
//! call jumped to that RVA taken as an absolute address. It landed on a
//! non-executable page and faulted, which is the right outcome — but it was
//! the right outcome *by arithmetic accident*. An RVA is an attacker-visible
//! number: an image laid out so that some thunk's RVA collides with a mapped
//! executable page would have transferred control there instead, and nothing
//! in the system was deciding otherwise.
//!
//! This module makes the outcome a decision. Every IAT slot is written,
//! exactly once, at load time, before the image's first instruction runs:
//!
//! - an import the allowlist resolves **and** the capability policy grants
//!   gets the address of its [`trampolines`] entry — a real, callable
//!   `extern "win64"` function, so the call actually works and returns;
//! - every other import — not allowlisted, or allowlisted but denied — gets
//!   [`CAPABILITY_TRAP_VIRT`], an address deliberately left unmapped in
//!   every task's address space.
//!
//! So a denied call faults at one known address, with `CR2` naming the trap
//! rather than an arbitrary RVA, and the fault is diagnosable as "this task
//! called something it was not granted" rather than as "this task jumped
//! somewhere". Same containment, now for a stated reason. This is the
//! `PD-04`/`G-PC-3` shape the Win32 shim always described — "loading a
//! program" and "granting it ambient authority" kept structurally separate —
//! applied at the one place the image can actually reach.
//!
//! **Why the patch is applied through the kernel's own view.** The IAT lives
//! in a section the image maps read-only (`.rdata` in every MSVC layout), and
//! it must *stay* read-only to the task — a task that can rewrite its own IAT
//! can grant itself capabilities. Patching therefore writes through the
//! loader's identity view of the staged frames, which is why
//! [`patch_imports`] must run **before**
//! [`crate::address_space::AddressSpace::seal_kernel_alias`] closes that view
//! (see that method's own doc comment). Ordering is enforced by the caller,
//! and getting it wrong fails closed rather than silently: after sealing, the
//! write faults under `CR0.WP`.

use crate::address_space::AddressSpace;
use crate::pe::ImportEntry;
use crate::win32_shim::{self, Api, CapabilityPolicy};

/// The address every denied or unresolvable import is pointed at:
/// deliberately unmapped in every address space, so a call through such a
/// slot faults immediately with `CR2` equal to this constant.
///
/// Chosen to be canonical, page-aligned, far above any image base this
/// loader accepts, and visually unmistakable in a fault capture — a reader
/// seeing `cr2=0xdead_0000` in a report is looking at a refused capability,
/// not at a wild jump.
pub const CAPABILITY_TRAP_VIRT: u64 = 0xdead_0000;

/// What [`patch_imports`] did, reported back so a caller can audit and
/// journal the decision rather than infer it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PatchSummary {
    /// Imports resolved to a real trampoline (allowlisted **and** granted).
    pub granted: usize,
    /// Imports pointed at [`CAPABILITY_TRAP_VIRT`] because the allowlist
    /// does not contain them.
    pub not_allowlisted: usize,
    /// Imports pointed at [`CAPABILITY_TRAP_VIRT`] because the policy
    /// denied an otherwise-allowlisted call.
    pub denied: usize,
}

impl PatchSummary {
    /// Every import this loader pointed at the trap rather than at code.
    pub const fn trapped(&self) -> usize {
        self.not_allowlisted + self.denied
    }

    /// Total slots written — every import in the table, with no third
    /// outcome: an unwritten slot is the failure mode this module exists to
    /// eliminate.
    pub const fn total(&self) -> usize {
        self.granted + self.trapped()
    }
}

/// Errors [`patch_imports`] fails closed with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IatError {
    /// An import's `iat_slot_rva` does not land inside a page this image
    /// actually mapped — a malformed or hostile import table pointing its
    /// own IAT outside the image. Nothing is patched past this point.
    SlotOutOfBounds,
    /// An IAT slot straddles a page boundary. Rejected rather than split
    /// across two frames: a partially-written function pointer is worse
    /// than a refused load.
    SlotStraddlesPage,
}

/// Resolves every import in `imports` and writes each one's IAT slot in
/// `space`, returning what was decided.
///
/// Every slot is written exactly once — there is no path that leaves one
/// holding its original RVA, which is the whole point (see the module doc).
///
/// # Safety
/// `space`'s staged frames must currently be writable through the identity
/// view this function writes with — i.e. this must be called after
/// `AddressSpace::create` and **before** `seal_kernel_alias`. The frames
/// must also be live and unaliased for the duration of the call.
pub unsafe fn patch_imports<'a, const FRAMES: usize>(
    space: &AddressSpace<'_, FRAMES>,
    image_base: u64,
    imports: impl Iterator<Item = &'a ImportEntry>,
    policy: &impl CapabilityPolicy,
) -> Result<PatchSummary, IatError> {
    let mut summary = PatchSummary::default();
    for import in imports {
        let resolved = match import.symbol {
            crate::pe::ImportSymbol::Name(name) => {
                win32_shim::resolve(import.dll_name.as_bytes(), name.as_bytes())
            }
            crate::pe::ImportSymbol::Ordinal(_) => None,
        };
        let value = match resolved {
            Some(api) if policy.is_granted(api) => {
                summary.granted += 1;
                trampolines::address_of(api)
            }
            Some(_) => {
                summary.denied += 1;
                CAPABILITY_TRAP_VIRT
            }
            None => {
                summary.not_allowlisted += 1;
                CAPABILITY_TRAP_VIRT
            }
        };
        // SAFETY: forwarded from this function's own contract.
        unsafe { write_slot(space, image_base, import.iat_slot_rva, value)? };
    }
    Ok(summary)
}

/// Writes one 8-byte IAT cell through the loader's identity view of the
/// frame backing it.
///
/// # Safety
/// Per [`patch_imports`]'s contract.
unsafe fn write_slot<const FRAMES: usize>(
    space: &AddressSpace<'_, FRAMES>,
    image_base: u64,
    slot_rva: u32,
    value: u64,
) -> Result<(), IatError> {
    const PAGE_SIZE: u64 = hal_x86_64::paging::PAGE_SIZE;
    let virt = image_base.checked_add(u64::from(slot_rva)).ok_or(IatError::SlotOutOfBounds)?;
    let offset_in_page = virt % PAGE_SIZE;
    if offset_in_page + 8 > PAGE_SIZE {
        return Err(IatError::SlotStraddlesPage);
    }
    let page = space.translate(virt - offset_in_page).ok_or(IatError::SlotOutOfBounds)?;
    // SAFETY: `page.phys` is a frame this image's own mapping names, and
    // this kernel's no-higher-half-split model makes that physical address
    // directly addressable (`hal_x86_64::paging::FrameAddr`'s doc comment).
    // The write is 8 bytes wholly inside the frame, per the straddle check
    // above; the identity view is writable per this function's contract.
    unsafe {
        core::ptr::write_unaligned((page.phys + offset_in_page) as *mut u64, value);
    }
    Ok(())
}

/// The callable `extern "win64"` entry points a granted import's IAT slot
/// is pointed at.
///
/// **Why `extern "win64"`.** These are called by code MSVC compiled, which
/// passes arguments in `RCX`/`RDX`/`R8`/`R9` with a 32-byte shadow store —
/// not the System V convention the rest of this kernel is built with. Using
/// the kernel's default ABI here would mean reading arguments out of the
/// wrong registers, which is not a compile error and not a fault: it is
/// silently wrong data reaching a capability-mediated call. The ABI is
/// therefore pinned at the boundary, which is exactly where the two worlds
/// meet.
///
/// **What these do and do not do.** Each one is a real function that a
/// loaded image can call and return from — that much is the point of this
/// Story. Several return the shape a real Windows call returns while the
/// subsystem behind them does not exist yet (no heap, no filesystem, no
/// console driver), exactly as `win32_shim`'s own `write_file`/`heap_alloc`
/// already document for their Rust-level counterparts. Where a real call
/// would allocate or transfer bytes, these report failure the Windows way
/// (`NULL`, `FALSE`) rather than inventing a pointer a caller would then
/// dereference — a fake success is a wild write in the caller's address
/// space, which is worse than an honest failure.
pub mod trampolines {
    use super::Api;

    /// `GetCurrentProcess` — returns the same `(HANDLE)-1` pseudo-handle
    /// real Windows returns. Fully faithful: this call has no subsystem
    /// behind it on any OS.
    pub extern "win64" fn get_current_process() -> u64 {
        u64::MAX
    }

    /// `ExitProcess` — a task calling this has finished. There is no
    /// process-teardown path reachable from task context yet (returning
    /// into the scheduler needs the task's own saved context, which the
    /// task cannot safely reach), so this deliberately faults into the same
    /// containment path any other terminal task event uses, by calling
    /// through the trap address. Terminating cleanly-on-request is
    /// follow-on work; pretending to exit by returning to a caller that no
    /// longer expects control would be worse.
    pub extern "win64" fn exit_process(_code: u32) -> ! {
        // SAFETY: deliberately transferring to the unmapped trap address —
        // the documented, contained way this shim ends a task today.
        unsafe {
            let trap: extern "win64" fn() -> ! =
                core::mem::transmute(super::CAPABILITY_TRAP_VIRT as usize);
            trap()
        }
    }

    /// `HeapAlloc` — returns `NULL`. This kernel has no heap wired to a
    /// loaded process (`win32_shim::heap_alloc`'s own note), and `NULL` is
    /// precisely how a real `HeapAlloc` reports that it could not satisfy a
    /// request, so a well-written caller's own error path runs.
    pub extern "win64" fn heap_alloc(_heap: u64, _flags: u32, _bytes: u64) -> u64 {
        0
    }

    /// `HeapFree` — freeing nothing succeeds, since `heap_alloc` never
    /// handed out anything to free. Returns `TRUE`.
    pub extern "win64" fn heap_free(_heap: u64, _flags: u32, _mem: u64) -> u32 {
        1
    }

    /// `GetStdHandle` — no console/file subsystem exists, so every standard
    /// handle is `INVALID_HANDLE_VALUE`.
    pub extern "win64" fn get_std_handle(_which: u32) -> u64 {
        u64::MAX
    }

    /// `WriteFile` — returns `FALSE` with nothing written. No console
    /// driver exists (`win32_shim::write_file`'s own note); reporting bytes
    /// written that went nowhere would be a lie a caller acts on.
    pub extern "win64" fn write_file(
        _handle: u64,
        _buffer: u64,
        _count: u32,
        _written: u64,
        _overlapped: u64,
    ) -> u32 {
        0
    }

    /// `ReadFile` — returns `FALSE`, for the same reason as [`write_file`].
    pub extern "win64" fn read_file(
        _handle: u64,
        _buffer: u64,
        _count: u32,
        _read: u64,
        _overlapped: u64,
    ) -> u32 {
        0
    }

    /// `CreateFileA` — returns `INVALID_HANDLE_VALUE`; there is no
    /// filesystem.
    pub extern "win64" fn create_file_a(
        _name: u64,
        _access: u32,
        _share: u32,
        _security: u64,
        _disposition: u32,
        _flags: u32,
        _template: u64,
    ) -> u64 {
        u64::MAX
    }

    /// `CloseHandle` — closing a handle nothing ever opened succeeds.
    pub extern "win64" fn close_handle(_handle: u64) -> u32 {
        1
    }

    /// The address to write into a granted import's IAT slot.
    ///
    /// The single designated dispatch point that knows every [`Api`]
    /// variant, kept as thin as `win32_shim::resolve` is and for the same
    /// documented Open/Closed exemption: adding a call is an additive arm
    /// here and a new trampoline above, never a fallback that guesses.
    pub fn address_of(api: Api) -> u64 {
        let pointer = match api {
            Api::GetCurrentProcess => get_current_process as *const (),
            Api::ExitProcess => exit_process as *const (),
            Api::HeapAlloc => heap_alloc as *const (),
            Api::HeapFree => heap_free as *const (),
            Api::GetStdHandle => get_std_handle as *const (),
            Api::WriteFile => write_file as *const (),
            Api::ReadFile => read_file as *const (),
            Api::CreateFileA => create_file_a as *const (),
            Api::CloseHandle => close_handle as *const (),
        };
        pointer as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pe::{FixedBytes, ImportSymbol, Permissions, SectionDescriptor};
    use hal_x86_64::paging::{PageTable, PAGE_SIZE};
    use kernel::mem::Pool;

    fn import(dll: &[u8], symbol: &[u8], iat_slot_rva: u32) -> ImportEntry {
        let mut dll_bytes = [0u8; crate::pe::MAX_DLL_NAME_LEN];
        dll_bytes[..dll.len()].copy_from_slice(dll);
        let mut symbol_bytes = [0u8; crate::pe::MAX_SYMBOL_NAME_LEN];
        symbol_bytes[..symbol.len()].copy_from_slice(symbol);
        ImportEntry {
            dll_name: FixedBytes::for_test(dll_bytes, dll.len()),
            symbol: ImportSymbol::Name(FixedBytes::for_test(symbol_bytes, symbol.len())),
            iat_slot_rva,
        }
    }

    #[repr(C, align(4096))]
    struct AlignedPages([u8; 8192]);

    const IMAGE_BASE: u64 = 0x1_4000_0000;
    const RW: Permissions = Permissions { read: true, write: true, execute: false };

    struct DenyPolicy(Api);
    impl CapabilityPolicy for DenyPolicy {
        fn is_granted(&self, api: Api) -> bool {
            api != self.0
        }
    }

    fn two_rw_sections() -> [SectionDescriptor; 1] {
        [SectionDescriptor {
            virtual_address: 0,
            virtual_size: 2 * PAGE_SIZE as u32,
            file_offset: 0,
            file_size: 2 * PAGE_SIZE as u32,
            permissions: RW,
        }]
    }

    /// Reads back what the patcher wrote, through the same identity view.
    fn slot_value<const FRAMES: usize>(space: &AddressSpace<'_, FRAMES>, slot_rva: u32) -> u64 {
        let virt = IMAGE_BASE + u64::from(slot_rva);
        let offset = virt % PAGE_SIZE;
        let page = space.translate(virt - offset).expect("slot page mapped");
        // SAFETY: reading back a frame this test just had written.
        unsafe { core::ptr::read_unaligned((page.phys + offset) as *const u64) }
    }

    // STORY-P1-03-03 AC1: a granted import gets a real, callable address;
    // a non-allowlisted one gets the trap; a denied one gets the trap. No
    // slot keeps its original value.
    #[test]
    fn every_slot_is_written_and_only_granted_imports_get_real_addresses() {
        let bytes = AlignedPages([0xFF; 8192]);
        let mut staging = AlignedPages([0; 8192]);
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 16> = Pool::new();
        let sections = two_rw_sections();
        let space = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        )
        .expect("valid sections should map");

        let mut ordinal = import(b"KERNEL32.dll", b"ignored", 0x118);
        ordinal.symbol = ImportSymbol::Ordinal(7);
        let imports = [
            import(b"KERNEL32.dll", b"GetCurrentProcess", 0x100),
            import(b"KERNEL32.dll", b"CreateRemoteThread", 0x108),
            import(b"KERNEL32.dll", b"WriteFile", 0x110),
            ordinal,
        ];
        // SAFETY: nothing sealed this space's identity view; the staged
        // frames are live and unaliased for this call.
        let summary = unsafe {
            patch_imports(&space, IMAGE_BASE, imports.iter(), &DenyPolicy(Api::WriteFile))
        }
        .expect("in-bounds slots should patch");

        assert_eq!(summary.granted, 1);
        assert_eq!(summary.not_allowlisted, 2);
        assert_eq!(summary.denied, 1);
        assert_eq!(summary.total(), 4, "every named and ordinal slot is accounted for");

        assert_eq!(
            slot_value(&space, 0x100),
            trampolines::address_of(Api::GetCurrentProcess),
            "a granted import must point at its real trampoline"
        );
        assert_eq!(
            slot_value(&space, 0x108),
            CAPABILITY_TRAP_VIRT,
            "a non-allowlisted import must point at the trap"
        );
        assert_eq!(
            slot_value(&space, 0x110),
            CAPABILITY_TRAP_VIRT,
            "an allowlisted but policy-denied import must point at the trap too"
        );
        assert_eq!(
            slot_value(&space, 0x118),
            CAPABILITY_TRAP_VIRT,
            "an unsupported ordinal import must also be overwritten with the trap"
        );
    }

    // The defect this Story exists to fix, stated as a test: after
    // patching, no slot still holds the RVA-shaped value it started with.
    #[test]
    fn no_slot_retains_its_unpatched_rva() {
        let bytes = AlignedPages([0; 8192]);
        let mut staging = AlignedPages([0; 8192]);
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 16> = Pool::new();
        let sections = two_rw_sections();
        let space = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        )
        .unwrap();

        // Seed each slot with a plausible unpatched thunk value (an RVA),
        // exactly as a real unloaded image's IAT holds.
        for (index, rva) in [0x7b_9d9eu64, 0x7b_9db8].iter().enumerate() {
            let virt = IMAGE_BASE + 0x200 + (index as u64 * 8);
            let offset = virt % PAGE_SIZE;
            let page = space.translate(virt - offset).unwrap();
            // SAFETY: writable identity view of a just-mapped frame.
            unsafe { core::ptr::write_unaligned((page.phys + offset) as *mut u64, *rva) };
        }

        let imports = [
            import(b"KERNEL32.dll", b"GetSystemTimeAsFileTime", 0x200),
            import(b"KERNEL32.dll", b"CloseHandle", 0x208),
        ];
        // SAFETY: as above — unsealed, live, unaliased.
        unsafe { patch_imports(&space, IMAGE_BASE, imports.iter(), &win32_shim::AllowAllPolicy) }
            .unwrap();

        assert_eq!(slot_value(&space, 0x200), CAPABILITY_TRAP_VIRT);
        assert_ne!(slot_value(&space, 0x200), 0x7b_9d9e, "the RVA must not survive");
        assert_eq!(slot_value(&space, 0x208), trampolines::address_of(Api::CloseHandle));
        assert_ne!(slot_value(&space, 0x208), 0x7b_9db8);
    }

    // A slot pointing outside anything the image mapped fails closed.
    #[test]
    fn a_slot_outside_the_mapped_image_fails_closed() {
        let bytes = AlignedPages([0; 8192]);
        let mut staging = AlignedPages([0; 8192]);
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 16> = Pool::new();
        let sections = two_rw_sections();
        let space = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        )
        .unwrap();

        let imports = [import(b"KERNEL32.dll", b"CloseHandle", 0x99_0000)];
        // SAFETY: as above.
        let result = unsafe {
            patch_imports(&space, IMAGE_BASE, imports.iter(), &win32_shim::AllowAllPolicy)
        };
        assert_eq!(result, Err(IatError::SlotOutOfBounds));
    }

    // A slot straddling a page boundary is refused rather than half-written.
    #[test]
    fn a_slot_straddling_a_page_boundary_fails_closed() {
        let bytes = AlignedPages([0; 8192]);
        let mut staging = AlignedPages([0; 8192]);
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 16> = Pool::new();
        let sections = two_rw_sections();
        let space = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        )
        .unwrap();

        // Four bytes before the end of the first page: the 8-byte cell runs
        // into the next frame.
        let straddling = (PAGE_SIZE - 4) as u32;
        let imports = [import(b"KERNEL32.dll", b"CloseHandle", straddling)];
        // SAFETY: as above.
        let result = unsafe {
            patch_imports(&space, IMAGE_BASE, imports.iter(), &win32_shim::AllowAllPolicy)
        };
        assert_eq!(result, Err(IatError::SlotStraddlesPage));
    }

    // Every allowlisted API has a distinct, non-null trampoline — a
    // duplicated or null entry would silently route one capability's calls
    // into another's implementation.
    #[test]
    fn every_api_has_a_distinct_non_null_trampoline() {
        let apis = [
            Api::GetCurrentProcess,
            Api::ExitProcess,
            Api::HeapAlloc,
            Api::HeapFree,
            Api::GetStdHandle,
            Api::WriteFile,
            Api::ReadFile,
            Api::CreateFileA,
            Api::CloseHandle,
        ];
        let mut addresses = std::vec::Vec::new();
        for api in apis {
            let address = trampolines::address_of(api);
            assert_ne!(address, 0, "{api:?} has a null trampoline");
            assert_ne!(address, CAPABILITY_TRAP_VIRT, "{api:?} resolves to the trap");
            addresses.push(address);
        }
        addresses.sort_unstable();
        let before = addresses.len();
        addresses.dedup();
        assert_eq!(addresses.len(), before, "two APIs share one trampoline address");
    }

    // A granted trampoline is genuinely callable and returns the value real
    // Windows returns — the claim "an allowlisted call resolves and
    // returns" checked directly rather than only through a fixture.
    #[test]
    fn a_granted_trampoline_is_callable_through_its_patched_slot() {
        let bytes = AlignedPages([0; 8192]);
        let mut staging = AlignedPages([0; 8192]);
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 16> = Pool::new();
        let sections = two_rw_sections();
        let space = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        )
        .unwrap();
        let imports = [import(b"KERNEL32.dll", b"GetCurrentProcess", 0x300)];
        // SAFETY: as above.
        unsafe { patch_imports(&space, IMAGE_BASE, imports.iter(), &win32_shim::AllowAllPolicy) }
            .unwrap();

        // Call exactly the way the image's own code would: read the
        // function pointer out of the patched slot and indirect through it.
        let slot = slot_value(&space, 0x300);
        // SAFETY: `slot` is an address this module itself just wrote, and
        // it is `get_current_process`'s own `extern "win64"` signature.
        let result = unsafe {
            let f: extern "win64" fn() -> u64 = core::mem::transmute(slot as usize);
            f()
        };
        assert_eq!(result, u64::MAX, "GetCurrentProcess returns the (HANDLE)-1 pseudo-handle");
    }
}
