//! `TEST-P0-05-02-A`'s Tier 0 QEMU fixture binary.
//!
//! A separate `no_std`/`no_main` binary (see `exec/Cargo.toml`'s `[[bin]]`
//! comment and `hal_x86_64::boot`'s doc comment for why this can't simply
//! be another `kernel` fixture feature). Exercises the real
//! `exec::address_space::AddressSpace::create` end to end under QEMU/target
//! CPU semantics: mapping two sections with distinct permissions, reading
//! their permission bits back, rejecting an overlapping and a
//! kernel-region-colliding section set without mapping anything, and
//! repeated create/drop cycles not exhausting the frame pool — the same
//! sequence `address_space.rs`'s own host unit tests already exercise on
//! the dev toolchain, run here a second time under real target hardware.

#![no_std]
#![no_main]

use exec::address_space::{AddressSpace, AddressSpaceError};
use exec::pe::{Permissions, SectionDescriptor};
#[allow(unused_imports)]
// linked for its `global_asm!` side effect only, per its own doc comment
use hal_x86_64::boot as _;
use hal_x86_64::paging::{PageTable, PAGE_SIZE};
use hal_x86_64::qemu_exit::{exit_qemu, QemuExitCode};
use kernel::capacities::EXEC_FRAME_POOL_CAPACITY as FRAMES;
use kernel::mem::Pool;

const RX: Permissions = Permissions { read: true, write: false, execute: true };
const RW: Permissions = Permissions { read: true, write: true, execute: false };
const IMAGE_BASE: u64 = 0x1_4000_0000;

#[repr(C, align(4096))]
struct AlignedPages([u8; 8192]);

static IMAGE_BYTES: AlignedPages = AlignedPages([0xAA; 8192]);
static mut STAGING: AlignedPages = AlignedPages([0; 8192]);
static mut PML4: PageTable = PageTable::new();
static mut FRAME_POOL: Pool<PageTable, FRAMES> = Pool::new();

fn two_sections() -> [SectionDescriptor; 2] {
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

/// Runs the fixture, returning whether every checked property held.
///
/// `&raw mut` + deref (rather than a direct `&mut STATIC`) avoids the
/// `static_mut_refs` lint's "shared reference to a mutable static" concern
/// (there is exactly one live `&mut` borrow of each static at a time,
/// scoped to each `AddressSpace::create` call below); clippy's
/// `deref_addrof` doesn't know that distinction and flags the idiom as a
/// no-op, so it's silenced narrowly here, mirroring
/// `context.rs`'s own host-test precedent for the identical pattern.
#[allow(static_mut_refs, clippy::deref_addrof)]
fn run() -> bool {
    // SAFETY: this fixture is the only code running (single-CPU boot path,
    // nothing else touches these statics), and each `&mut` borrow below is
    // dropped (the `AddressSpace` using it goes out of scope) before the
    // next one is taken.
    let sections = two_sections();
    let mapped_correctly = unsafe {
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
        let code = space.translate(IMAGE_BASE);
        let data = space.translate(IMAGE_BASE + PAGE_SIZE);
        matches!(code, Some(p) if !p.writable && p.executable)
            && matches!(data, Some(p) if p.writable && !p.executable)
        // `space` drops here, reclaiming PML4/FRAME_POOL per
        // STORY-P0-05-02 acceptance criterion 3.
    };
    if !mapped_correctly {
        return false;
    }

    // Overlapping sections: rejected, nothing mapped.
    let overlapping = [
        SectionDescriptor {
            virtual_address: 0,
            virtual_size: (2 * PAGE_SIZE) as u32,
            file_offset: 0,
            file_size: (2 * PAGE_SIZE) as u32,
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
    // SAFETY: see above — single-CPU boot fixture, sequential use.
    let overlap_rejected = unsafe {
        matches!(
            AddressSpace::create(
                &mut *&raw mut PML4,
                &mut *&raw mut FRAME_POOL,
                &overlapping,
                IMAGE_BASE,
                &IMAGE_BYTES.0,
                &mut *&raw mut STAGING.0,
            )
            .err(),
            Some(AddressSpaceError::SectionOverlap)
        )
    };
    if !overlap_rejected {
        return false;
    }

    // A section colliding with the kernel's own identity-mapped region:
    // rejected, never silently mapped over kernel memory.
    let colliding = [SectionDescriptor {
        virtual_address: 0,
        virtual_size: PAGE_SIZE as u32,
        file_offset: 0,
        file_size: PAGE_SIZE as u32,
        permissions: RX,
    }];
    // SAFETY: see above.
    let collision_rejected = unsafe {
        matches!(
            AddressSpace::create(
                &mut *&raw mut PML4,
                &mut *&raw mut FRAME_POOL,
                &colliding,
                0,
                &IMAGE_BYTES.0,
                &mut *&raw mut STAGING.0,
            )
            .err(),
            Some(AddressSpaceError::KernelRegionCollision)
        )
    };
    if !collision_rejected {
        return false;
    }

    // Repeated create/drop cycles never exhaust the frame pool
    // (STORY-P0-05-02 acceptance criterion 3).
    for _ in 0..(FRAMES * 2) {
        // SAFETY: see above.
        let ok = unsafe {
            AddressSpace::create(
                &mut *&raw mut PML4,
                &mut *&raw mut FRAME_POOL,
                &sections,
                IMAGE_BASE,
                &IMAGE_BYTES.0,
                &mut *&raw mut STAGING.0,
            )
            .is_ok()
        };
        if !ok {
            return false;
        }
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
