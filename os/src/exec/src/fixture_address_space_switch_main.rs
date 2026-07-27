//! `STORY-P1-03-01`'s Tier 0 QEMU fixture binary.
//!
//! A separate `no_std`/`no_main` binary, following `exec-fixture`'s own
//! precedent (see `exec/Cargo.toml`'s `[[bin]]` comments and
//! `hal_x86_64::boot`'s doc comment for why): `exec` depends on `kernel`, so
//! a binary needing both lives here rather than making `kernel`'s own binary
//! depend back on `exec`.
//!
//! Proves the real mechanism `STORY-P1-03-01` adds — `CR3` actually switches
//! per task, and a task confined to its own address space cannot read
//! another task's private memory — with two genuinely distinct
//! `exec::AddressSpace` trees, not two sections inside one shared tree the
//! way `fixture_shared_memory_main.rs`'s owner/sharee pair is (that fixture
//! is deliberately about a *granted* mapping; this one is about two spaces
//! that share **nothing** except the low kernel region below).
//!
//! **Why each space also identity-maps the low 8 MiB.** `AddressSpace::create`
//! rejects any section below `KERNEL_RESERVED_REGION_END` (`0x4000_0000`) —
//! correctly, since a task's own image must never collide with kernel memory
//! — but that leaves each task's tree with *no* mapping for the code, stack,
//! IDT/GDT/TSS or statics a real `CR3` load would need to keep executing.
//! `AddressSpace::map_page` (already public, built for `STORY-P0-07-02`'s
//! shared-memory grants) has no such collision check, so this fixture uses it
//! directly to identity-map `0..0x0080_0000` (8 MiB — the same `G-DX-8`
//! ceiling this project already uses everywhere else as a deliberate,
//! non-arbitrary bound) as RWX into *both* trees before ever loading either
//! into `CR3`. This is a Tier 0 fixture's own bootstrap, not a claim that
//! duplicating the kernel's mapping into every task tree this way is the
//! production design — sharing the same top-level page-table entries across
//! every space (rather than each space owning its own copy) is the cheaper,
//! real-OS technique and is named as follow-on work this Story does not
//! itself need. Kernel mappings built this way are also all-RWX, matching
//! `boot.rs`'s own current identity map exactly — W^X-correct kernel mappings
//! are `STORY-P1-03-02`'s charge, not this one's.
//!
//! **The adversarial probe.** Task A's tree maps only its own private page,
//! at `TASK_A_PRIVATE_VIRT`; task B's maps only its own, at
//! `TASK_B_PRIVATE_VIRT`. Task A, running under its own real `CR3`, attempts
//! to read `TASK_B_PRIVATE_VIRT` — an address its own page tables have no
//! entry for at all — which raises a real `#PF`, captured and contained by
//! the same `kernel::fault` machinery `fixture_fault` already proves
//! (terminate the faulting task, keep the rest of the system running), not a
//! stand-in. Task B then runs afterward, proving isolation held.
//!
//! **Same-space skip (acceptance criterion 2)** is proven at the pure-logic
//! level by `hal_x86_64::paging::cr3_reload_needed`'s own host tests; this
//! fixture's contribution is confirming the real `CR3` register actually
//! reads back each task's own value after a real switch — the hardware half
//! the host tests cannot reach.

#![no_std]
#![no_main]
#![allow(static_mut_refs, clippy::deref_addrof)]

use core::fmt::Write;
use exec::address_space::AddressSpace;
use exec::pe::{Permissions, SectionDescriptor};
#[allow(unused_imports)]
// linked for its `global_asm!` side effect only, per its own doc comment
use hal_x86_64::boot as _;
use hal_x86_64::fault::FaultFrame;
use hal_x86_64::paging::{self, PageTable, PAGE_SIZE};
use hal_x86_64::qemu_exit::{exit_qemu, QemuExitCode};
use hal_x86_64::serial::SerialPort;
use kernel::context::{self, Context};
use kernel::fault::{Disposition, FaultReport, FaultingContext};
use kernel::measure::write_result;
use kernel::mem::Pool;
use kernel::sched::{Priority, Scheduler, TaskId, TaskState, WcetBudgetTicks};

const FRAMES: usize = 64;
const STACK_SIZE: usize = 8_192;
const TASKS: usize = 2;

const RW: Permissions = Permissions { read: true, write: true, execute: false };
const RWX: Permissions = Permissions { read: true, write: true, execute: true };

/// Each task's own private page, at a *different* virtual address in its
/// own tree — the point of the adversarial probe below is that task B's
/// address is genuinely absent from task A's own page tables, not merely
/// backed by a different frame at the address task A also uses.
const TASK_A_PRIVATE_VIRT: u64 = 0x1_4000_0000;
const TASK_B_PRIVATE_VIRT: u64 = 0x1_5000_0000;

/// How much low memory each tree identity-maps as RWX so a real `CR3` load
/// into it does not immediately fault — see this module's own doc comment.
const KERNEL_REPLICA_BYTES: u64 = 0x0080_0000;

#[repr(C, align(4096))]
struct AlignedPage([u8; 4096]);

static TASK_A_BYTES: AlignedPage = AlignedPage([0xAA; 4096]);
static mut TASK_A_STAGING: AlignedPage = AlignedPage([0; 4096]);
static mut TASK_A_PML4: PageTable = PageTable::new();
static mut TASK_A_FRAME_POOL: Pool<PageTable, FRAMES> = Pool::new();

static TASK_B_BYTES: AlignedPage = AlignedPage([0xBB; 4096]);
static mut TASK_B_STAGING: AlignedPage = AlignedPage([0; 4096]);
static mut TASK_B_PML4: PageTable = PageTable::new();
static mut TASK_B_FRAME_POOL: Pool<PageTable, FRAMES> = Pool::new();

static mut SCHEDULER: Scheduler<TASKS> = Scheduler::new();
static mut SUPERVISOR_CTX: Context = Context::zeroed();
static mut ABANDONED_CTX: Context = Context::zeroed();
static mut TASK_CTX: [Context; TASKS] = [Context::zeroed(); TASKS];
static mut TASK_STACKS: [[u8; STACK_SIZE]; TASKS] = [[0; STACK_SIZE]; TASKS];

/// Which scheduler slot is currently running, or `None` for the supervisor —
/// the same convention `fixture_fault`'s `CURRENT_TASK` establishes, and the
/// only input the disposition policy reads.
static mut CURRENT_TASK: Option<usize> = None;
static mut TASK_B_RUNS: u64 = 0;
static mut FAULT_CAPTURED: bool = false;

/// One RW page, at offset 0 from whatever `image_base` `AddressSpace::create`
/// is called with — the real address lives in `image_base`, not here.
fn private_section() -> [SectionDescriptor; 1] {
    [SectionDescriptor {
        virtual_address: 0,
        virtual_size: PAGE_SIZE as u32,
        file_offset: 0,
        file_size: PAGE_SIZE as u32,
        permissions: RW,
    }]
}

/// Identity-maps `0..KERNEL_REPLICA_BYTES` into `space` as RWX — see this
/// module's own doc comment for why a task's own tree needs this before a
/// real `CR3` load into it is safe.
fn map_kernel_replica<const N: usize>(space: &mut AddressSpace<'_, N>) -> bool {
    let mut virt = 0u64;
    while virt < KERNEL_REPLICA_BYTES {
        if space.map_page(virt, virt, RWX).is_err() {
            return false;
        }
        virt += PAGE_SIZE;
    }
    true
}

/// Task A: writes and reads back its own private page (sanity), then
/// deliberately reads task B's private address — unmapped in its own tree —
/// which faults for real and is contained.
extern "C" fn task_a_entry() -> ! {
    // SAFETY: `TASK_A_PRIVATE_VIRT` is mapped RW in task A's own tree, which
    // is the live `CR3` while this entry point runs.
    unsafe {
        core::ptr::write_volatile(TASK_A_PRIVATE_VIRT as *mut u8, 0x42);
        let _ = core::ptr::read_volatile(TASK_A_PRIVATE_VIRT as *const u8);
        // Deliberately unmapped in task A's own tree — the adversarial
        // probe this Story's acceptance criterion 1 asks for.
        let value = core::ptr::read_volatile(TASK_B_PRIVATE_VIRT as *const u8);
        core::hint::black_box(value);
    }
    unreachable!("reading an address absent from this task's own page tables always faults")
}

/// Task B: increments a counter and yields back, proving the scheduler
/// still runs it after task A's contained fault.
extern "C" fn task_b_entry() -> ! {
    loop {
        // SAFETY: single-CPU fixture; only this task writes `TASK_B_RUNS`,
        // and slot 1 is the only context it is ever switched into.
        unsafe {
            TASK_B_RUNS += 1;
            context::switch(&raw mut TASK_CTX[1], &raw mut SUPERVISOR_CTX);
        }
    }
}

/// This fixture's own fault entry point — installed via `hal_x86_64::fault`'s
/// stubs exactly like `kernel::fixture_fault`'s, adapted for two real address
/// spaces: the handler itself runs out of the low kernel replica every tree
/// maps identically, so it needs no `CR3` switch of its own.
///
/// # Safety
/// Called only by the fault stubs, with `frame` pointing at a
/// fully-initialized [`FaultFrame`] on the faulting stack.
#[no_mangle]
extern "C" fn tinyos_fault_entry(frame: *const FaultFrame) -> ! {
    // SAFETY: the stubs pass a pointer to a fully-initialized `FaultFrame` on
    // the current stack, live for this call.
    let frame = unsafe { *frame };
    let mut serial = unsafe { SerialPort::init() };

    // SAFETY: single-CPU fixture with interrupts disabled inside the
    // handler; only this handler and `run` touch these statics, never
    // concurrently.
    let disposition = unsafe {
        FAULT_CAPTURED = true;
        let current = CURRENT_TASK;
        let scheduler = &mut *(&raw mut SCHEDULER);
        let context = match current.and_then(|slot| task_id(scheduler, slot)) {
            Some(task) => FaultingContext::Task(task),
            None => FaultingContext::Kernel,
        };
        let report = FaultReport { vector: frame.vector, context };
        let disposition = Disposition::of(&report);
        let _ = kernel::fault::audit(&report, disposition);
        disposition
    };

    match disposition {
        Disposition::TerminateTask(task) => {
            // SAFETY: as above.
            unsafe {
                let scheduler = &mut *(&raw mut SCHEDULER);
                scheduler.set_state(task, TaskState::Finished);
                CURRENT_TASK = None;
            }
            let _ = writeln!(serial, "address-space-switch terminated task {}", task.index());
            // SAFETY: the victim's registers are saved into a context
            // nothing will ever resume, and `SUPERVISOR_CTX` is suspended at
            // its own `switch`/`switch_address_space` call site inside `run`.
            unsafe { context::switch(&raw mut ABANDONED_CTX, &raw mut SUPERVISOR_CTX) };
            unreachable!("a terminated task is never switched back into")
        }
        Disposition::HaltSystem => {
            let _ = writeln!(serial, "address-space-switch kernel-context fault: halting");
            let _ = write_result(&mut serial, "address-space-switch", false);
            exit_qemu(QemuExitCode::Failure)
        }
    }
}

fn task_id(scheduler: &Scheduler<TASKS>, slot: usize) -> Option<TaskId> {
    scheduler.iter_tasks().map(|(task, _)| task).find(|task| task.index() == slot)
}

/// This binary's `#DF` entry point — required because `init_faults_only`
/// installs an IST-bearing gate for vector 8 unconditionally
/// (`STORY-P1-02-02`), so every binary linking `hal_x86_64::fault` needs one.
/// Never expected to be reached: a passing run here never destroys a task's
/// stack, unlike `fixture_double_fault`'s own deliberate escalation.
///
/// # Safety
/// Called only by `df_fault_stub`, with `frame` pointing at a
/// fully-initialized [`FaultFrame`] on the IST stack.
#[no_mangle]
extern "C" fn tinyos_double_fault_entry(frame: *const FaultFrame) -> ! {
    let frame = unsafe { *frame };
    let mut serial = unsafe { SerialPort::init() };
    let _ = writeln!(
        serial,
        "address-space-switch unexpected #DF vector={} rip={:#x} — halting",
        frame.vector, frame.rip
    );
    let _ = write_result(&mut serial, "address-space-switch", false);
    exit_qemu(QemuExitCode::Failure)
}

/// Runs the fixture: build two real, disjoint address spaces, switch a real
/// `CR3` into each, and prove the isolation and the switch mechanism both
/// hold under real hardware paging.
fn run() -> bool {
    let mut serial = unsafe { SerialPort::init() };

    // SAFETY: called once, before any fault can occur; installs the fault
    // handling this fixture's own `tinyos_fault_entry` above relies on.
    unsafe { hal_x86_64::interrupts::init_faults_only() };

    // SAFETY: each `&mut` borrow below is dropped before this function
    // returns; the two trees never alias any frame (separate `PML4`/
    // `frame_pool` statics), and each is built before its `CR3` is ever
    // loaded.
    let (cr3_a, cr3_b) = unsafe {
        let sections_a = private_section();
        let mut space_a = match AddressSpace::create(
            &mut *&raw mut TASK_A_PML4,
            &mut *&raw mut TASK_A_FRAME_POOL,
            &sections_a,
            TASK_A_PRIVATE_VIRT,
            &TASK_A_BYTES.0,
            &mut *&raw mut TASK_A_STAGING.0,
        ) {
            Ok(space) => space,
            Err(_) => return false,
        };
        if !map_kernel_replica(&mut space_a) {
            return false;
        }
        let _ = writeln!(serial, "address-space-switch: space A built, cr3={:#x}", space_a.cr3());

        let sections_b = private_section();
        let mut space_b = match AddressSpace::create(
            &mut *&raw mut TASK_B_PML4,
            &mut *&raw mut TASK_B_FRAME_POOL,
            &sections_b,
            TASK_B_PRIVATE_VIRT,
            &TASK_B_BYTES.0,
            &mut *&raw mut TASK_B_STAGING.0,
        ) {
            Ok(space) => space,
            Err(_) => return false,
        };
        if !map_kernel_replica(&mut space_b) {
            return false;
        }
        let _ = writeln!(serial, "address-space-switch: space B built, cr3={:#x}", space_b.cr3());

        let cr3_a = space_a.cr3();
        let cr3_b = space_b.cr3();
        // `AddressSpace::drop` tears its tree down (resets `pml4`/
        // `frame_pool`) — correct for its own Story's teardown contract, but
        // fatal here: this fixture's whole point is loading these trees into
        // a real `CR3` *after* this scope ends, so they must outlive
        // `space_a`/`space_b` themselves. Generation-safe teardown is
        // `STORY-P1-03-02`'s charge; this Story deliberately leaks rather
        // than reaching for a mechanism that doesn't exist yet.
        core::mem::forget(space_a);
        core::mem::forget(space_b);
        (cr3_a, cr3_b)
    };

    if cr3_a == cr3_b {
        let _ = writeln!(serial, "address-space-switch: the two trees must be genuinely distinct");
        return false;
    }
    let _ = writeln!(serial, "address-space-switch: distinct cr3 confirmed, creating tasks");

    // SAFETY: single-CPU fixture; each slot's stack/context is used by
    // exactly one task for the whole run.
    let (task_a, task_b) = unsafe {
        let scheduler = &mut *(&raw mut SCHEDULER);
        let priority = match Priority::try_new(8) {
            Ok(p) => p,
            Err(_) => return false,
        };
        let Ok(task_a) = scheduler.create_task(priority, WcetBudgetTicks(1_000), task_a_entry)
        else {
            return false;
        };
        let Ok(task_b) = scheduler.create_task(priority, WcetBudgetTicks(1_000), task_b_entry)
        else {
            return false;
        };
        if scheduler.set_address_space(task_a, cr3_a).is_none()
            || scheduler.set_address_space(task_b, cr3_b).is_none()
        {
            return false;
        }

        let stack_a =
            core::slice::from_raw_parts_mut((&raw mut TASK_STACKS[0]).cast::<u8>(), STACK_SIZE);
        let Ok(ctx_a) = Context::new(stack_a, task_a_entry) else { return false };
        TASK_CTX[0] = ctx_a;

        let stack_b =
            core::slice::from_raw_parts_mut((&raw mut TASK_STACKS[1]).cast::<u8>(), STACK_SIZE);
        let Ok(ctx_b) = Context::new(stack_b, task_b_entry) else { return false };
        TASK_CTX[1] = ctx_b;

        (task_a, task_b)
    };

    let _ = writeln!(serial, "address-space-switch: tasks created");
    let _ = writeln!(serial, "address-space-switch: about to switch into task A, cr3_a={cr3_a:#x}");
    // SAFETY: `TASK_CTX[0]` was just initialized above; `SUPERVISOR_CTX` is
    // this call's own slot, resumed only by task A's fault escape switch.
    unsafe {
        CURRENT_TASK = Some(0);
        context::switch_address_space(&raw mut SUPERVISOR_CTX, &raw mut TASK_CTX[0], cr3_a);
    }
    let _ = writeln!(serial, "address-space-switch: back from task A");
    // Control returns here only via `tinyos_fault_entry`'s escape switch,
    // after task A's real #PF was captured and contained.

    // SAFETY: read after the switch above returned.
    let observed_cr3_after_a = paging::read_cr3();

    let mut ok = true;
    unsafe {
        ok &= FAULT_CAPTURED;
        ok &= SCHEDULER.state_of(task_a) == Some(TaskState::Finished);
    }
    ok &= observed_cr3_after_a == cr3_a;

    // Switch into task B — a genuinely different address space, so this is a
    // real reload, not a same-space skip.
    // SAFETY: `TASK_CTX[1]` was initialized above; `SUPERVISOR_CTX` is
    // suspended at its own call site until task B yields back.
    unsafe {
        CURRENT_TASK = Some(1);
        for _ in 0..3 {
            context::switch_address_space(&raw mut SUPERVISOR_CTX, &raw mut TASK_CTX[1], cr3_b);
        }
        CURRENT_TASK = None;
    }
    let observed_cr3_after_b = paging::read_cr3();
    ok &= observed_cr3_after_b == cr3_b;
    ok &= observed_cr3_after_a != observed_cr3_after_b;

    // SAFETY: read after every switch above has returned.
    unsafe {
        ok &= TASK_B_RUNS == 3;
        ok &= SCHEDULER.state_of(task_b) == Some(TaskState::Ready);
    }

    let _ = writeln!(
        serial,
        "address-space-switch cr3_a={cr3_a:#x} cr3_b={cr3_b:#x} fault_captured={} \
         task_a_finished={} task_b_runs={} observed_cr3_after_a={observed_cr3_after_a:#x} \
         observed_cr3_after_b={observed_cr3_after_b:#x}",
        unsafe { FAULT_CAPTURED },
        unsafe { SCHEDULER.state_of(task_a) == Some(TaskState::Finished) },
        unsafe { TASK_B_RUNS },
    );

    let _ = write_result(&mut serial, "address-space-switch", ok);
    ok
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
