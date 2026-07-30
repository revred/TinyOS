//! `TEST-P0-05-04-A`'s Tier 0 QEMU fixture binary.
//!
//! Loads the real `blue-sharc.exe` build artifact — repacked once, offline,
//! via `xtask pack-txe` (`STORY-P0-08-01`) into `fixtures/blue-sharc.txe`,
//! a byte-for-byte-equivalent PE32+ image with every section's on-disk
//! layout page-aligned and its `.bss` tail physically zero-written — and
//! proves the real Phase 0 loader pipeline against it: `pe::parse` parses
//! its real header/section/import tables, `AddressSpace::create` maps its
//! real six sections with their real permissions, and
//! `win32_shim::check_imports` correctly rejects its complete 220-import
//! surface (205 named plus 15 ordinal; only 9 APIs are allowlisted) — proving the
//! load-time security gate works, not merely compiles. `win32_shim`'s
//! `HeapAlloc` resolves and is directly callable, satisfying this Story's
//! own redefined checkpoint. See `STORY-P0-05-04.md`'s rewritten
//! acceptance criteria for why this fixture does not attempt to jump into
//! the image's own entry point: that would require a CR3 switch, an IDT,
//! and dozens more Win32/CRT shim calls this kernel doesn't have yet.

#![no_std]
#![no_main]

use exec::address_space::AddressSpace;
use exec::pe::{self, ImportSymbol, SectionDescriptor};
use exec::win32_shim::{self, Api, CapabilityPolicy};
#[allow(unused_imports)]
// linked for its `global_asm!` side effect only, per its own doc comment
use hal_x86_64::boot as _;
use hal_x86_64::paging::PageTable;
use hal_x86_64::qemu_exit::{exit_qemu, QemuExitCode};
use kernel::mem::Pool;

/// Comfortably above `blue-sharc.txe`'s real 6 sections.
const SECTIONS: usize = 8;
/// Comfortably above `blue-sharc.exe`'s 205 named plus 15 ordinal imports.
const IMPORTS: usize = 256;
/// Page-table frame pool: this image's real sections span under 8.3MiB of
/// virtual address space, needing at most a handful of PT/PD/PDPT levels —
/// sized generously since the cost is negligible next to the image itself.
/// Fixture-local, deliberately not `kernel::capacities::EXEC_FRAME_POOL_CAPACITY`
/// (sized for the two much smaller existing fixtures; bumping it for this
/// one fixture's very different scale would be a speculative capacity
/// change everywhere else, not a fixture-scoped one).
const FRAMES: usize = 32;
/// The exact byte length `xtask pack-txe`'s output for `blue-sharc.exe`
/// produced — every section already page-aligned and `.bss`-flattened, so
/// this is also exactly the number of bytes `AddressSpace::create` needs to
/// stage (`sum` over sections of `virtual_size` rounded up to a page).
const STAGING_LEN: usize = 8_265_728;

#[repr(C, align(4096))]
struct AlignedImage([u8; 8_269_824]);
#[repr(C, align(4096))]
struct AlignedStaging([u8; STAGING_LEN]);

static IMAGE_BYTES: AlignedImage = AlignedImage(*include_bytes!("../fixtures/blue-sharc.txe"));
static mut STAGING: AlignedStaging = AlignedStaging([0; STAGING_LEN]);
static mut PML4: PageTable = PageTable::new();
static mut FRAME_POOL: Pool<PageTable, FRAMES> = Pool::new();

struct AllowNothingElsePolicy;
impl CapabilityPolicy for AllowNothingElsePolicy {
    fn is_granted(&self, api: Api) -> bool {
        api == Api::HeapAlloc
    }
}

/// Runs the fixture, returning whether every checked property held.
///
/// `&raw mut` + deref mirrors `exec-fixture`'s own precedent for the
/// identical `static_mut_refs`/`deref_addrof` lint concern.
#[allow(static_mut_refs, clippy::deref_addrof)]
fn run() -> bool {
    // The real image parses: real header fields, real section table, real
    // import table — not a synthetic hand-built fixture.
    let descriptor = match pe::parse::<SECTIONS, IMPORTS>(&IMAGE_BYTES.0) {
        Ok(d) => d,
        Err(_) => return false,
    };
    if descriptor.image_base() != 0x1_4000_0000 {
        return false;
    }
    // The real entry point RVA `objdump`-equivalent inspection found.
    if descriptor.entry_point_rva() != 0x71fe00 {
        return false;
    }
    let named =
        descriptor.imports().filter(|entry| matches!(entry.symbol, ImportSymbol::Name(_))).count();
    let ordinal = descriptor
        .imports()
        .filter(|entry| matches!(entry.symbol, ImportSymbol::Ordinal(_)))
        .count();
    if named != 205 || ordinal != 15 {
        return false;
    }

    let mut sections = [SectionDescriptor {
        virtual_address: 0,
        virtual_size: 0,
        file_offset: 0,
        file_size: 0,
        permissions: exec::pe::Permissions { read: false, write: false, execute: false },
    }; SECTIONS];
    let mut section_count = 0usize;
    for section in descriptor.sections() {
        sections[section_count] = *section;
        section_count += 1;
    }
    if section_count != 6 {
        return false;
    }

    // Real sections map with their real permissions.
    // SAFETY: this fixture is the only code running (single-CPU boot
    // path), and the one `&mut` borrow of each static below is dropped (the
    // `AddressSpace` using it goes out of scope) before this function
    // returns.
    let mapped_correctly = unsafe {
        let space = match AddressSpace::create(
            &mut *&raw mut PML4,
            &mut *&raw mut FRAME_POOL,
            &sections[..section_count],
            descriptor.image_base(),
            &IMAGE_BYTES.0,
            &mut *&raw mut STAGING.0,
        ) {
            Ok(space) => space,
            Err(_) => return false,
        };
        let entry_page = space.translate(descriptor.entry_virtual_address());
        matches!(entry_page, Some(p) if !p.writable && p.executable)
    };
    if !mapped_correctly {
        return false;
    }

    // The real import table — 205 named plus 15 ordinal imports, only 9 APIs
    // of which this Phase 0 shim allowlists — is correctly rejected at load time. This is
    // the security gate working as designed (`G-PC-2`/`G-PC-3`), not a
    // fixture failure: an image needing capabilities beyond what's granted
    // must never be silently allowed to run.
    let real_image_correctly_rejected = win32_shim::check_imports(descriptor.imports())
        == Err(win32_shim::ShimError::NotAllowlisted);
    if !real_image_correctly_rejected {
        return false;
    }

    // `HeapAlloc` — the specific capability this Story's checkpoint
    // exercises — resolves against the real allowlist and is directly
    // callable through the capability-mediated path when granted.
    if win32_shim::resolve(b"KERNEL32.dll", b"HeapAlloc") != Some(Api::HeapAlloc) {
        return false;
    }
    if win32_shim::heap_alloc(&AllowNothingElsePolicy, 64) != Ok(64) {
        return false;
    }

    true
}

#[no_mangle]
extern "C" fn kernel_main(_start_info_paddr: u64) -> ! {
    if run() {
        exit_qemu(QemuExitCode::Success)
    } else {
        exit_qemu(QemuExitCode::Failure)
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    hal_x86_64::qemu_exit::panic_report(info)
}
