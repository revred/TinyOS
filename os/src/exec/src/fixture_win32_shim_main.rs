//! `TEST-P0-05-03-A`'s Tier 0 QEMU fixture binary.
//!
//! A separate `no_std`/`no_main` binary, following `exec-fixture`'s own
//! precedent (see `exec/Cargo.toml`'s `[[bin]]` comments and
//! `hal_x86_64::boot`'s doc comment for why). Exercises
//! `exec::win32_shim`'s capability-gated calls end to end under real target
//! CPU paging semantics: an allowlisted call with a policy denial, an
//! allowlisted call whose buffer runs into unmapped memory, an allowlisted
//! call whose buffer points squarely at the kernel's own reserved region,
//! and a well-formed in-allowlist, in-policy, in-bounds call succeeding —
//! the same sequence `win32_shim.rs`'s own host unit tests already exercise
//! on the dev toolchain, run here a second time under real target
//! hardware's page-table walk.

#![no_std]
#![no_main]

use exec::address_space::AddressSpace;
use exec::pe::{Permissions, SectionDescriptor};
#[allow(unused_imports)]
// linked for its `global_asm!` side effect only, per its own doc comment
use hal_x86_64::boot as _;
use hal_x86_64::paging::{PageTable, PAGE_SIZE};
use hal_x86_64::qemu_exit::{exit_qemu, QemuExitCode};
use kernel::capacities::EXEC_FRAME_POOL_CAPACITY as FRAMES;
use kernel::mem::Pool;

use exec::win32_shim::{self, AllowAllPolicy, Api, Buffer, CapabilityPolicy, ShimError};

const RW: Permissions = Permissions { read: true, write: true, execute: false };
const IMAGE_BASE: u64 = 0x1_4000_0000;

#[repr(C, align(4096))]
struct AlignedPages([u8; 8192]);

static IMAGE_BYTES: AlignedPages = AlignedPages([0xAA; 8192]);
static mut STAGING: AlignedPages = AlignedPages([0; 8192]);
static mut PML4: PageTable = PageTable::new();
static mut FRAME_POOL: Pool<PageTable, FRAMES> = Pool::new();

struct DenyPolicy(Api);
impl CapabilityPolicy for DenyPolicy {
    fn is_granted(&self, api: Api) -> bool {
        api != self.0
    }
}

fn one_rw_section() -> [SectionDescriptor; 1] {
    [SectionDescriptor {
        virtual_address: 0,
        virtual_size: PAGE_SIZE as u32,
        file_offset: 0,
        file_size: PAGE_SIZE as u32,
        permissions: RW,
    }]
}

/// Runs the fixture, returning whether every checked property held.
///
/// `&raw mut` + deref mirrors `exec-fixture`'s own precedent for the
/// identical `static_mut_refs`/`deref_addrof` lint concern.
#[allow(static_mut_refs, clippy::deref_addrof)]
fn run() -> bool {
    let sections = one_rw_section();

    // A policy denial is rejected, not silently degraded to a no-op
    // (STORY-P0-05-03 AC3).
    // SAFETY: this fixture is the only code running (single-CPU boot
    // path), and each `&mut` borrow below is dropped (the `AddressSpace`
    // using it goes out of scope) before the next one is taken.
    let policy_denied_rejected = unsafe {
        let space = match AddressSpace::create(
            &mut *&raw mut PML4,
            &mut *&raw mut FRAME_POOL,
            &sections,
            IMAGE_BASE,
            &IMAGE_BYTES.0,
            &mut *&raw mut STAGING.0,
        ) {
            Ok(space) => space,
            Err(_) => return false,
        };
        win32_shim::write_file(
            &DenyPolicy(Api::WriteFile),
            &space,
            Buffer { virt_addr: IMAGE_BASE, len: 16 },
        ) == Err(ShimError::PolicyDenied)
    };
    if !policy_denied_rejected {
        return false;
    }

    // A buffer running past the single mapped page into unmapped memory
    // fails closed before any access (STORY-P0-05-03 AC4).
    // SAFETY: see above.
    let out_of_bounds_rejected = unsafe {
        let space = match AddressSpace::create(
            &mut *&raw mut PML4,
            &mut *&raw mut FRAME_POOL,
            &sections,
            IMAGE_BASE,
            &IMAGE_BYTES.0,
            &mut *&raw mut STAGING.0,
        ) {
            Ok(space) => space,
            Err(_) => return false,
        };
        win32_shim::write_file(
            &AllowAllPolicy,
            &space,
            Buffer { virt_addr: IMAGE_BASE, len: PAGE_SIZE + 1 },
        ) == Err(ShimError::OutOfBounds)
    };
    if !out_of_bounds_rejected {
        return false;
    }

    // A buffer pointing at the kernel's own reserved region (never part of
    // this process's mapped sections) is rejected the same way.
    // SAFETY: see above.
    let kernel_region_buffer_rejected = unsafe {
        let space = match AddressSpace::create(
            &mut *&raw mut PML4,
            &mut *&raw mut FRAME_POOL,
            &sections,
            IMAGE_BASE,
            &IMAGE_BYTES.0,
            &mut *&raw mut STAGING.0,
        ) {
            Ok(space) => space,
            Err(_) => return false,
        };
        win32_shim::write_file(&AllowAllPolicy, &space, Buffer { virt_addr: 0, len: 16 })
            == Err(ShimError::OutOfBounds)
    };
    if !kernel_region_buffer_rejected {
        return false;
    }

    // A well-formed, in-allowlist, in-policy call with valid arguments
    // succeeds (STORY-P0-05-03 AC4).
    // SAFETY: see above.
    let well_formed_call_succeeds = unsafe {
        let space = match AddressSpace::create(
            &mut *&raw mut PML4,
            &mut *&raw mut FRAME_POOL,
            &sections,
            IMAGE_BASE,
            &IMAGE_BYTES.0,
            &mut *&raw mut STAGING.0,
        ) {
            Ok(space) => space,
            Err(_) => return false,
        };
        win32_shim::write_file(&AllowAllPolicy, &space, Buffer { virt_addr: IMAGE_BASE, len: 64 })
            == Ok(64)
    };
    if !well_formed_call_succeeds {
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
