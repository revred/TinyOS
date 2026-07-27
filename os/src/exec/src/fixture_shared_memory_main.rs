//! `TEST-P0-07-02-A`'s Tier 0 QEMU fixture binary.
//!
//! A separate `no_std`/`no_main` binary, following `exec-fixture`'s own
//! precedent (see `exec/Cargo.toml`'s `[[bin]]` comments and
//! `hal_x86_64::boot`'s doc comment for why). Exercises
//! `exec::shared_memory::grant`/`revoke` end to end under real target CPU
//! paging semantics: a well-formed grant maps the sharee to the owner's own
//! backing frame with the requested (not broader) permissions, and
//! revoking it deterministically unmaps the sharee's page — the same
//! sequence `shared_memory.rs`'s own host unit tests already exercise on
//! the dev toolchain, run here a second time under real target hardware's
//! page-table walk.

#![no_std]
#![no_main]

use exec::address_space::AddressSpace;
use exec::pe::{Permissions, SectionDescriptor};
use exec::shared_memory::{self, GrantRegistry, GrantRequest, SharedMemoryError};
#[allow(unused_imports)]
// linked for its `global_asm!` side effect only, per its own doc comment
use hal_x86_64::boot as _;
use hal_x86_64::paging::{PageTable, PAGE_SIZE};
use hal_x86_64::qemu_exit::{exit_qemu, QemuExitCode};
use kernel::mem::Pool;
use kernel::sched::{Priority, Scheduler, TaskId, WcetBudgetTicks};

const FRAMES: usize = 8;
const RW: Permissions = Permissions { read: true, write: true, execute: false };
const RO: Permissions = Permissions { read: true, write: false, execute: false };
const IMAGE_BASE: u64 = 0x1_4000_0000;
const SHAREE_VIRT: u64 = 0x1_5000_0000;

#[repr(C, align(4096))]
struct AlignedPages([u8; 8192]);

static OWNER_BYTES: AlignedPages = AlignedPages([0xAA; 8192]);
static mut OWNER_STAGING: AlignedPages = AlignedPages([0; 8192]);
static mut OWNER_PML4: PageTable = PageTable::new();
static mut OWNER_FRAME_POOL: Pool<PageTable, FRAMES> = Pool::new();
static SHAREE_BYTES: AlignedPages = AlignedPages([0; 8192]);
static mut SHAREE_STAGING: AlignedPages = AlignedPages([0; 8192]);
static mut SHAREE_PML4: PageTable = PageTable::new();
static mut SHAREE_FRAME_POOL: Pool<PageTable, FRAMES> = Pool::new();
static mut GRANT_REGISTRY: GrantRegistry<4> = GrantRegistry::new();

fn owner_rw_section() -> [SectionDescriptor; 1] {
    [SectionDescriptor {
        virtual_address: 0,
        virtual_size: PAGE_SIZE as u32,
        file_offset: 0,
        file_size: PAGE_SIZE as u32,
        permissions: RW,
    }]
}

fn dummy_tasks() -> (TaskId, TaskId) {
    #[allow(clippy::empty_loop)]
    extern "C" fn dummy_entry() -> ! {
        loop {}
    }
    let mut sched: Scheduler<4> = Scheduler::new();
    let priority = Priority::try_new(1).expect("1 is in range");
    let owner = sched.create_task(priority, WcetBudgetTicks(1000), dummy_entry).unwrap();
    let sharee = sched.create_task(priority, WcetBudgetTicks(1000), dummy_entry).unwrap();
    (owner, sharee)
}

/// Runs the fixture, returning whether every checked property held.
///
/// `&raw mut` + deref mirrors `exec-fixture`'s own precedent for the
/// identical `static_mut_refs`/`deref_addrof` lint concern.
#[allow(static_mut_refs, clippy::deref_addrof)]
fn run() -> bool {
    let (owner, sharee_task) = dummy_tasks();
    let sections = owner_rw_section();

    // SAFETY: this fixture is the only code running (single-CPU boot
    // path), and each `&mut` borrow below is dropped (the `AddressSpace`
    // using it goes out of scope) before this function returns.
    unsafe {
        let owner_space = match AddressSpace::create(
            &mut *&raw mut OWNER_PML4,
            &mut *&raw mut OWNER_FRAME_POOL,
            &sections,
            IMAGE_BASE,
            &OWNER_BYTES.0,
            &mut *&raw mut OWNER_STAGING.0,
        ) {
            Ok(space) => space,
            Err(_) => return false,
        };
        let mut sharee_space = match AddressSpace::create(
            &mut *&raw mut SHAREE_PML4,
            &mut *&raw mut SHAREE_FRAME_POOL,
            &[],
            IMAGE_BASE,
            &SHAREE_BYTES.0,
            &mut *&raw mut SHAREE_STAGING.0,
        ) {
            Ok(space) => space,
            Err(_) => return false,
        };

        // A well-formed grant maps the sharee to the owner's own backing
        // frame, with the requested (not broader) permissions.
        let live_grant = match shared_memory::grant(
            owner,
            &owner_space,
            &mut sharee_space,
            GrantRequest {
                owner_virt: IMAGE_BASE,
                sharee_virt: SHAREE_VIRT,
                pages: 1,
                sharee_permissions: RO,
            },
            &mut *&raw mut GRANT_REGISTRY,
        ) {
            Ok(g) => g,
            Err(_) => return false,
        };
        let owner_page = match owner_space.translate(IMAGE_BASE) {
            Some(p) => p,
            None => return false,
        };
        let sharee_page = match sharee_space.translate(SHAREE_VIRT) {
            Some(p) => p,
            None => return false,
        };
        if sharee_page.phys != owner_page.phys || sharee_page.writable {
            return false;
        }

        // A non-owner's revoke attempt is rejected, and the mapping
        // survives it.
        if shared_memory::revoke(
            sharee_task,
            &live_grant,
            &mut sharee_space,
            &mut *&raw mut GRANT_REGISTRY,
        ) != Err(SharedMemoryError::NotOwner)
        {
            return false;
        }
        if sharee_space.translate(SHAREE_VIRT).is_none() {
            return false;
        }

        // The owner's own revoke deterministically unmaps it.
        if shared_memory::revoke(
            owner,
            &live_grant,
            &mut sharee_space,
            &mut *&raw mut GRANT_REGISTRY,
        )
        .is_err()
        {
            return false;
        }
        if sharee_space.translate(SHAREE_VIRT).is_some() {
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
fn panic(_info: &core::panic::PanicInfo) -> ! {
    exit_qemu(QemuExitCode::Failure)
}
