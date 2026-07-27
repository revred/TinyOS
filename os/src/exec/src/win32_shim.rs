//! Capability-scoped Win32 API compatibility shim (`STORY-P0-05-03`).
//!
//! A small, explicitly-enumerated Windows API surface — sized to exactly
//! what `blue-sharc.exe` imports (process/thread basics, heap allocation,
//! file access, console I/O), per `G-PC-2` — never a general
//! `kernel32`/`ntdll` reimplementation. Every emulated call is mediated by
//! the same capability model that governs every other caller (`G-PC-3`):
//! this module is where "loading a program" and "granting it ambient
//! kernel authority" are kept structurally separate.
//!
//! **`aci` migration note.** The real capability/policy engine (`aci`,
//! Phase 5 per `docs/mvp-delivery-strategy.md`'s crate map) does not exist
//! in this workspace yet, and `STORY-P0-05-03` explicitly sanctions not
//! blocking this Phase 0 Story on a Phase 5 crate landing first. This
//! module therefore defines the minimal capability-check shape it needs
//! standalone — the [`CapabilityPolicy`] trait — rather than depending on
//! `aci` directly. Every function that needs a policy decision takes
//! `&impl CapabilityPolicy`, never a concrete policy type (Dependency
//! Inversion, `agent/CODING_STANDARDS.md` §D), so the migration path once
//! `aci` lands is additive: a new `CapabilityPolicy` implementor backed by
//! `aci`'s real policy engine, wired in at the call site, with no change to
//! [`resolve`], [`check_imports`], or any call implementation below.
//!
//! **Fault-handling note.** This kernel has no IDT/exception-handling
//! subsystem yet (`STORY-P0-05-02`'s handover names this as a concrete,
//! still-open prerequisite). A buffer argument to an allowlisted call is
//! therefore validated *proactively* against the calling process's own
//! [`AddressSpace`] — walked page by page via [`AddressSpace::translate`]
//! — before any byte of it is read or written, rather than relying on a
//! `#PF` caught after the fact: there is no fault handler to catch one.
//! This mirrors `pe::parse`'s and `address_space::validate_sections`'s own
//! validate-first, mutate-second discipline.

use hal_x86_64::paging::PAGE_SIZE;

use crate::address_space::AddressSpace;
use crate::pe::ImportEntry;

/// The closed, explicitly-enumerated set of Win32 API calls this shim
/// supports (`STORY-P0-05-03` acceptance criterion 1) — sized to
/// `blue-sharc.exe`'s needs (`G-PC-2`): process/thread basics, heap
/// allocation, file access, console I/O. Adding support for a new call is
/// an additive change to this enum and [`resolve`], never a fallback
/// "guess what this import wants" path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Api {
    /// `KERNEL32.dll!GetCurrentProcess`.
    GetCurrentProcess,
    /// `KERNEL32.dll!ExitProcess`.
    ExitProcess,
    /// `KERNEL32.dll!HeapAlloc`.
    HeapAlloc,
    /// `KERNEL32.dll!HeapFree`.
    HeapFree,
    /// `KERNEL32.dll!GetStdHandle`.
    GetStdHandle,
    /// `KERNEL32.dll!WriteFile` (used for console output in this Feature's
    /// scope — `blue-sharc.exe`'s console I/O goes through the same
    /// handle-based call real Windows console output uses).
    WriteFile,
    /// `KERNEL32.dll!ReadFile`.
    ReadFile,
    /// `KERNEL32.dll!CreateFileA`.
    CreateFileA,
    /// `KERNEL32.dll!CloseHandle`.
    CloseHandle,
}

/// Errors this module fails closed with, per `agent/CODING_STANDARDS.md`'s
/// "no stringly-typed errors" rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShimError {
    /// An imported `(DLL, symbol)` pair isn't in [`resolve`]'s allowlist —
    /// per `STORY-P0-05-03` acceptance criterion 2, this is a load-time
    /// rejection ([`check_imports`]), never a runtime stub that fails when
    /// called.
    NotAllowlisted,
    /// An allowlisted call the capability policy denies for this process
    /// (`STORY-P0-05-03` acceptance criterion 3) — the documented
    /// Windows-API-shaped error path, never a kernel panic or a silent
    /// no-op that pretends to succeed.
    PolicyDenied,
    /// A buffer argument's `[addr, addr + len)` range is not fully mapped
    /// (or not mapped with the access this call needs) in the calling
    /// process's own address space — rejected before any access is
    /// attempted (`STORY-P0-05-03` acceptance criterion 4).
    OutOfBounds,
}

/// Resolves one imported `(DLL, symbol)` pair against the closed
/// allowlist. The single designated dispatch point permitted to know
/// about every [`Api`] variant — `agent/CODING_STANDARDS.md`'s Open/Closed
/// exemption for an unavoidable central dispatch point, kept as thin as
/// possible, deferring all per-call behavior to [`CapabilityPolicy`] and
/// the call implementations below rather than growing this match with
/// call logic.
pub fn resolve(dll_name: &[u8], symbol_name: &[u8]) -> Option<Api> {
    if !dll_name.eq_ignore_ascii_case(b"KERNEL32.dll") {
        return None;
    }
    match symbol_name {
        b"GetCurrentProcess" => Some(Api::GetCurrentProcess),
        b"ExitProcess" => Some(Api::ExitProcess),
        b"HeapAlloc" => Some(Api::HeapAlloc),
        b"HeapFree" => Some(Api::HeapFree),
        b"GetStdHandle" => Some(Api::GetStdHandle),
        b"WriteFile" => Some(Api::WriteFile),
        b"ReadFile" => Some(Api::ReadFile),
        b"CreateFileA" => Some(Api::CreateFileA),
        b"CloseHandle" => Some(Api::CloseHandle),
        _ => None,
    }
}

/// Validates every import `STORY-P0-05-01`'s parser found against the
/// allowlist before any code from the loaded image runs.
///
/// Fails closed with the first non-allowlisted import found, in
/// declaration order — matching `pe::parse`'s and
/// `address_space::validate_sections`'s own "first violation, no partial
/// state" precedent. A caller that gets `Ok(())` back has an image whose
/// entire import table is satisfiable by this shim.
pub fn check_imports<'a>(imports: impl Iterator<Item = &'a ImportEntry>) -> Result<(), ShimError> {
    for import in imports {
        if resolve(import.dll_name.as_bytes(), import.symbol_name.as_bytes()).is_none() {
            return Err(ShimError::NotAllowlisted);
        }
    }
    Ok(())
}

/// The minimal capability-check shape this Story needs standalone, since
/// the real `aci` policy engine doesn't exist in this workspace yet — see
/// this module's own doc comment for the migration path once it lands.
pub trait CapabilityPolicy {
    /// Whether this process is currently granted the capability `api`
    /// requires.
    fn is_granted(&self, api: Api) -> bool;
}

/// A policy that grants every allowlisted call — the default standalone
/// behavior until a real per-process, per-scope policy exists.
pub struct AllowAllPolicy;

impl CapabilityPolicy for AllowAllPolicy {
    fn is_granted(&self, _api: Api) -> bool {
        true
    }
}

/// A caller-supplied buffer argument (a virtual address range in the
/// calling process's own address space) to an allowlisted call.
#[derive(Debug, Clone, Copy)]
pub struct Buffer {
    /// The buffer's starting virtual address, in the calling process's own
    /// address space.
    pub virt_addr: u64,
    /// The buffer's length in bytes.
    pub len: u64,
}

/// Validates that every page `buffer` spans is mapped in `space` (and, if
/// `require_write`, mapped writable), failing closed with
/// [`ShimError::OutOfBounds`] *before* any byte of the buffer is read or
/// written. See this module's doc comment for why this proactive check —
/// not a fault caught after the fact — is this kernel's only enforcement
/// mechanism today.
fn validate_buffer<const FRAMES: usize>(
    space: &AddressSpace<'_, FRAMES>,
    buffer: Buffer,
    require_write: bool,
) -> Result<(), ShimError> {
    if buffer.len == 0 {
        return Ok(());
    }
    let end = buffer.virt_addr.checked_add(buffer.len).ok_or(ShimError::OutOfBounds)?;
    let mut page = buffer.virt_addr - (buffer.virt_addr % PAGE_SIZE);
    while page < end {
        match space.translate(page) {
            Some(mapped) if mapped.writable || !require_write => {}
            _ => return Err(ShimError::OutOfBounds),
        }
        page += PAGE_SIZE;
    }
    Ok(())
}

/// `KERNEL32.dll!WriteFile`, scoped to this Feature's actual need (console
/// output for `blue-sharc.exe`'s exercised subset, not general file I/O —
/// `G-PC-2`).
///
/// Validates the policy grant and the source buffer's bounds before
/// touching anything. This kernel has no console/serial driver yet (a
/// separate, unimplemented subsystem — flagged here, not silently assumed
/// complete), so a well-formed, in-policy, in-bounds call succeeds and
/// reports every byte written without actually transmitting them anywhere;
/// wiring a real console driver in is future work this Story's acceptance
/// criteria don't require.
pub fn write_file<const FRAMES: usize>(
    policy: &impl CapabilityPolicy,
    space: &AddressSpace<'_, FRAMES>,
    buffer: Buffer,
) -> Result<u64, ShimError> {
    if !policy.is_granted(Api::WriteFile) {
        return Err(ShimError::PolicyDenied);
    }
    validate_buffer(space, buffer, false)?;
    Ok(buffer.len)
}

/// `KERNEL32.dll!ReadFile`, scoped the same way as [`write_file`]. The
/// destination buffer must be mapped *writable* — the shim is about to
/// write into it — so an attempt to read into a read-only or unmapped
/// range fails closed rather than corrupting memory the caller doesn't
/// own write access to.
pub fn read_file<const FRAMES: usize>(
    policy: &impl CapabilityPolicy,
    space: &AddressSpace<'_, FRAMES>,
    buffer: Buffer,
) -> Result<u64, ShimError> {
    if !policy.is_granted(Api::ReadFile) {
        return Err(ShimError::PolicyDenied);
    }
    validate_buffer(space, buffer, true)?;
    Ok(0)
}

/// `KERNEL32.dll!HeapAlloc`, scoped the same way as [`write_file`]/
/// [`read_file`] (`STORY-P0-05-04`).
///
/// This kernel has no real heap allocator wired to a loaded process yet — a
/// separate, unimplemented subsystem, flagged here rather than silently
/// assumed complete, mirroring `write_file`'s own "no real driver backing
/// it yet" precedent. A well-formed, in-policy call proves the capability-
/// mediation path this Story's checkpoint exercises (an allowlisted call
/// resolves and is only permitted when the policy grants it) by reporting
/// `size` back as a stand-in allocation handle, the same "report success,
/// no real backing yet" shape `write_file` already established — it does
/// not yet hand back real, usable memory.
pub fn heap_alloc(policy: &impl CapabilityPolicy, size: u64) -> Result<u64, ShimError> {
    if !policy.is_granted(Api::HeapAlloc) {
        return Err(ShimError::PolicyDenied);
    }
    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pe::{FixedBytes, Permissions, SectionDescriptor};
    use hal_x86_64::paging::PageTable;
    use kernel::mem::Pool;

    fn import(dll: &[u8], symbol: &[u8]) -> ImportEntry {
        let mut dll_bytes = [0u8; crate::pe::MAX_DLL_NAME_LEN];
        dll_bytes[..dll.len()].copy_from_slice(dll);
        let mut symbol_bytes = [0u8; crate::pe::MAX_SYMBOL_NAME_LEN];
        symbol_bytes[..symbol.len()].copy_from_slice(symbol);
        ImportEntry {
            dll_name: FixedBytes::for_test(dll_bytes, dll.len()),
            symbol_name: FixedBytes::for_test(symbol_bytes, symbol.len()),
        }
    }

    // STORY-P0-05-03 AC1: every allowlisted (DLL, symbol) pair resolves.
    #[test]
    fn resolve_recognizes_every_allowlisted_call() {
        assert_eq!(resolve(b"KERNEL32.dll", b"HeapAlloc"), Some(Api::HeapAlloc));
        assert_eq!(resolve(b"KERNEL32.dll", b"WriteFile"), Some(Api::WriteFile));
        assert_eq!(resolve(b"KERNEL32.dll", b"CloseHandle"), Some(Api::CloseHandle));
    }

    // STORY-P0-05-03 AC1/AC2: a call outside the allowlist is rejected,
    // never guessed at.
    #[test]
    fn resolve_rejects_a_call_outside_the_allowlist() {
        assert_eq!(resolve(b"KERNEL32.dll", b"CreateRemoteThread"), None);
        assert_eq!(resolve(b"WS2_32.dll", b"WSAStartup"), None);
    }

    // STORY-P0-05-03 AC2 / TEST-P0-05-03-A: an import not on the allowlist
    // fails the whole image at load time, before any code runs.
    #[test]
    fn check_imports_rejects_a_non_allowlisted_import() {
        let imports =
            [import(b"KERNEL32.dll", b"HeapAlloc"), import(b"KERNEL32.dll", b"CreateRemoteThread")];
        assert_eq!(check_imports(imports.iter()), Err(ShimError::NotAllowlisted));
    }

    #[test]
    fn check_imports_accepts_an_all_allowlisted_import_table() {
        let imports = [
            import(b"KERNEL32.dll", b"GetStdHandle"),
            import(b"KERNEL32.dll", b"WriteFile"),
            import(b"KERNEL32.dll", b"ExitProcess"),
        ];
        assert_eq!(check_imports(imports.iter()), Ok(()));
    }

    struct DenyPolicy(Api);
    impl CapabilityPolicy for DenyPolicy {
        fn is_granted(&self, api: Api) -> bool {
            api != self.0
        }
    }

    #[repr(C, align(4096))]
    struct AlignedPages([u8; 8192]);

    fn staging() -> AlignedPages {
        AlignedPages([0; 8192])
    }

    const RW: Permissions = Permissions { read: true, write: true, execute: false };
    const IMAGE_BASE: u64 = 0x1_4000_0000;

    fn one_rw_section() -> [SectionDescriptor; 1] {
        [SectionDescriptor {
            virtual_address: 0,
            virtual_size: PAGE_SIZE as u32,
            file_offset: 0,
            file_size: PAGE_SIZE as u32,
            permissions: RW,
        }]
    }

    // STORY-P0-05-03 AC3: a policy-denied allowlisted call is rejected,
    // not silently degraded to a no-op.
    #[test]
    fn write_file_is_rejected_when_the_policy_denies_it() {
        let bytes = AlignedPages([0; 8192]);
        let mut staging = staging();
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 4> = Pool::new();
        let sections = one_rw_section();
        let space = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        )
        .unwrap();

        let result = write_file(
            &DenyPolicy(Api::WriteFile),
            &space,
            Buffer { virt_addr: IMAGE_BASE, len: 16 },
        );
        assert_eq!(result, Err(ShimError::PolicyDenied));
    }

    // STORY-P0-05-03 AC4 / TEST-P0-05-03-A: a buffer reaching outside the
    // calling process's own mapped memory is rejected before any access —
    // here, a buffer that starts inside the mapped section but runs past
    // its single mapped page into unmapped memory.
    #[test]
    fn write_file_rejects_a_buffer_running_past_the_mapped_section() {
        let bytes = AlignedPages([0; 8192]);
        let mut staging = staging();
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 4> = Pool::new();
        let sections = one_rw_section();
        let space = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        )
        .unwrap();

        let result = write_file(
            &AllowAllPolicy,
            &space,
            Buffer { virt_addr: IMAGE_BASE, len: PAGE_SIZE + 1 },
        );
        assert_eq!(result, Err(ShimError::OutOfBounds));
    }

    // A buffer entirely outside any mapped range (e.g. pointing at another
    // process's / the kernel's own memory) is rejected the same way.
    #[test]
    fn write_file_rejects_a_buffer_pointing_at_unmapped_memory() {
        let bytes = AlignedPages([0; 8192]);
        let mut staging = staging();
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 4> = Pool::new();
        let sections = one_rw_section();
        let space = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        )
        .unwrap();

        // Address 0 is inside the kernel's own reserved region — never
        // part of any process's mapped section.
        let result = write_file(&AllowAllPolicy, &space, Buffer { virt_addr: 0, len: 16 });
        assert_eq!(result, Err(ShimError::OutOfBounds));
    }

    // `ReadFile`'s destination buffer must be mapped *writable* — a
    // read-only destination fails closed rather than being silently
    // "read into" nowhere.
    #[test]
    fn read_file_rejects_a_read_only_destination_buffer() {
        const RX: Permissions = Permissions { read: true, write: false, execute: true };
        let bytes = AlignedPages([0; 8192]);
        let mut staging = staging();
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 4> = Pool::new();
        let sections = [SectionDescriptor {
            virtual_address: 0,
            virtual_size: PAGE_SIZE as u32,
            file_offset: 0,
            file_size: PAGE_SIZE as u32,
            permissions: RX,
        }];
        let space = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        )
        .unwrap();

        let result = read_file(&AllowAllPolicy, &space, Buffer { virt_addr: IMAGE_BASE, len: 16 });
        assert_eq!(result, Err(ShimError::OutOfBounds));
    }

    // STORY-P0-05-03 AC4: a well-formed, in-allowlist, in-policy call with
    // valid arguments succeeds.
    #[test]
    fn write_file_succeeds_for_a_well_formed_in_bounds_buffer() {
        let bytes = AlignedPages([0; 8192]);
        let mut staging = staging();
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 4> = Pool::new();
        let sections = one_rw_section();
        let space = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        )
        .unwrap();

        let result = write_file(&AllowAllPolicy, &space, Buffer { virt_addr: IMAGE_BASE, len: 64 });
        assert_eq!(result, Ok(64));
    }

    #[test]
    fn zero_length_buffer_is_always_in_bounds() {
        let bytes = AlignedPages([0; 8192]);
        let mut staging = staging();
        let mut pml4 = PageTable::new();
        let mut frame_pool: Pool<PageTable, 4> = Pool::new();
        let sections = one_rw_section();
        let space = AddressSpace::create(
            &mut pml4,
            &mut frame_pool,
            &sections,
            IMAGE_BASE,
            &bytes.0,
            &mut staging.0,
        )
        .unwrap();

        let result = write_file(&AllowAllPolicy, &space, Buffer { virt_addr: 0, len: 0 });
        assert_eq!(result, Ok(0));
    }

    // STORY-P0-05-04 AC1: a well-formed, in-policy HeapAlloc call succeeds.
    #[test]
    fn heap_alloc_succeeds_when_granted() {
        assert_eq!(heap_alloc(&AllowAllPolicy, 64), Ok(64));
    }

    // STORY-P0-05-04: a policy-denied HeapAlloc call is rejected, not
    // silently degraded to a no-op.
    #[test]
    fn heap_alloc_is_rejected_when_the_policy_denies_it() {
        assert_eq!(heap_alloc(&DenyPolicy(Api::HeapAlloc), 64), Err(ShimError::PolicyDenied));
    }
}
