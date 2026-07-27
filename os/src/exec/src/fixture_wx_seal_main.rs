//! `STORY-P1-03-02` Part A's Tier 0 QEMU fixture binary (`TEST-P1-03-02-A`
//! clauses A1–A4): W^X proven adversarially in both directions with the
//! enforcement bits on, the boot-time RWX identity map retired for a real
//! W^X kernel tree, kernel mappings *shared* at page-directory granularity,
//! the loader's writable aliases sealed, and generation-safe teardown
//! proven by a stale-mapping probe task.
//!
//! A separate `no_std`/`no_main` binary in `exec`, following
//! `address-space-switch-fixture`'s precedent (and for the same crate-cycle
//! reason — see `hal_x86_64::boot`'s doc comment).
//!
//! **Sequence.** Bring-up on the boot map (`init_faults_only`, then
//! `paging::enable_nx_and_wp` — without `CR0.WP`/`EFER.NXE` every claim
//! below is vacuous, review D4); build the shared kernel directories from
//! the linker's own section-boundary symbols; link them into a supervisor
//! tree and load it — the moment the boot-time all-RWX map stops being the
//! system's address space (review D3). Build one task space (an RX page and
//! an RW page at `IMAGE_BASE`), link the same kernel directory into it,
//! seal the loader's staging aliases, audit every leaf of both trees for
//! W^X and every executable frame for writable aliases. Then three tasks,
//! each dispatched through the production `run_once_in_space` and each
//! contained by the unmodified fault policy: one *writes its own RX page*
//! (`CR0.WP` `#PF`), one *executes its own RW page* (NX `#PF`), and — after
//! unseal + generation-safe teardown wipes the space — one probes the stale
//! image address under the still-loadable torn tree (`#PF` on a revoked
//! mapping). The supervisor checks the wiped frames for residue and the
//! generation for its advance between the teardown and the probe.

#![no_std]
#![no_main]
#![allow(static_mut_refs, clippy::deref_addrof)]

use core::fmt::Write;
use exec::address_space::{AddressSpace, TeardownGeneration};
use exec::kernel_map::{self, KernelLayout};
use exec::pe::{Permissions, SectionDescriptor};
#[allow(unused_imports)]
// linked for its `global_asm!` side effect only, per its own doc comment
use hal_x86_64::boot as _;
use hal_x86_64::fault::FaultFrame;
use hal_x86_64::paging::{self, FrameAllocator, PageTable, PAGE_SIZE};
use hal_x86_64::qemu_exit::{exit_qemu, QemuExitCode};
use hal_x86_64::serial::SerialPort;
use kernel::context::{self, Context};
use kernel::dispatch;
use kernel::fault::{Disposition, FaultReport, FaultingContext};
use kernel::measure::write_result;
use kernel::mem::Pool;
use kernel::sched::{OverrunPolicy, Priority, Scheduler, TaskId, TaskState, WcetBudgetTicks};

// The linker script's section-boundary symbols (`targets/x86_64-tinyos.ld`)
// — the layout the W^X kernel tree is built from, so the permission map can
// never drift from what the linker actually produced.
unsafe extern "C" {
    static __kernel_exec_start: u8;
    static __kernel_exec_end: u8;
    static __kernel_rodata_start: u8;
    static __kernel_rodata_end: u8;
    static __kernel_image_end: u8;
}

const KERNEL_MAP_FRAMES: usize = 64;
const TASK_FRAMES: usize = 64;
const STACK_SIZE: usize = 8_192;
const TASKS: usize = 4;
const IMAGE_BASE: u64 = 0x1_4000_0000;
/// The architectural local-APIC MMIO page — mapped by the shared
/// directories even though this faults-only fixture never touches it, so
/// the directories under test are the same ones the integration fixture
/// runs its timer against.
const APIC_PAGE: u64 = 0xFEE0_0000;

const RX: Permissions = Permissions { read: true, write: false, execute: true };
const RW: Permissions = Permissions { read: true, write: true, execute: false };

#[repr(C, align(4096))]
struct AlignedPages([u8; 8192]);

static IMAGE_BYTES: AlignedPages = AlignedPages([0xAA; 8192]);
static mut STAGING: AlignedPages = AlignedPages([0; 8192]);
static mut TASK_PML4: PageTable = PageTable::new();
static mut TASK_FRAME_POOL: Pool<PageTable, TASK_FRAMES> = Pool::new();

static mut KERNEL_MAP_POOL: Pool<PageTable, KERNEL_MAP_FRAMES> = Pool::new();
static mut SUPERVISOR_PML4: PageTable = PageTable::new();
static mut SUPERVISOR_FRAMES: [PageTable; 4] = [const { PageTable::new() }; 4];
static mut SUPERVISOR_FRAMES_USED: usize = 0;

static mut SCHEDULER: Scheduler<TASKS> = Scheduler::new();
static mut SUPERVISOR_CTX: Context = Context::zeroed();
static mut ABANDONED_CTX: Context = Context::zeroed();
static mut TASK_CTX: [Context; TASKS] = [Context::zeroed(); TASKS];
static mut TASK_STACKS: [[u8; STACK_SIZE]; TASKS] = [[0; STACK_SIZE]; TASKS];

/// Which scheduler slot is currently running, or `None` for the supervisor
/// — `fixture_fault`'s convention, the only input the disposition reads.
static mut CURRENT_TASK: Option<usize> = None;
/// The `#PF` vector observed for each task slot, or 0 if it never faulted.
static mut FAULT_VECTORS: [u64; TASKS] = [0; TASKS];

/// Allocator over the supervisor's own few statically-reserved page-table
/// frames (its PDPTs) — the supervisor tree shares everything else.
struct SupervisorFrames;
impl FrameAllocator for SupervisorFrames {
    fn allocate_frame(&mut self) -> Option<u64> {
        // SAFETY: single-CPU fixture; only this allocator touches these.
        unsafe {
            let used = SUPERVISOR_FRAMES_USED;
            if used >= SUPERVISOR_FRAMES.len() {
                return None;
            }
            SUPERVISOR_FRAMES_USED = used + 1;
            Some(&raw mut SUPERVISOR_FRAMES[used] as u64)
        }
    }
}

/// One RX page then one RW page at `IMAGE_BASE` — the smallest section set
/// with both W^X directions to violate.
fn sections() -> [SectionDescriptor; 2] {
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

/// Task: writes to its own RX page — the write-to-executable-memory
/// direction of `BND-05`, a real `#PF` only because `CR0.WP` is on.
extern "C" fn task_write_rx_entry() -> ! {
    // SAFETY: the point — this mapped-RX address is written deliberately.
    unsafe {
        core::ptr::write_volatile(IMAGE_BASE as *mut u8, 0x42);
    }
    unreachable!("a write to an RX page must fault under CR0.WP")
}

/// Task: probes the torn-down space's stale image address — reads memory
/// whose mapping teardown revoked.
extern "C" fn task_stale_probe_entry() -> ! {
    // SAFETY: the point — this address's mapping was revoked by teardown.
    unsafe {
        let value = core::ptr::read_volatile(IMAGE_BASE as *const u8);
        core::hint::black_box(value);
    }
    unreachable!("a probe of a revoked mapping must fault")
}

/// This fixture's fault entry point — `address-space-switch-fixture`'s
/// containment shape, unchanged policy.
///
/// # Safety
/// Called only by the fault stubs, with `frame` pointing at a
/// fully-initialized [`FaultFrame`] on the faulting stack.
#[no_mangle]
extern "C" fn tinyos_fault_entry(frame: *const FaultFrame) -> ! {
    // SAFETY: the stubs pass a valid `FaultFrame` pointer, live for this call.
    let frame = unsafe { *frame };
    let mut serial = unsafe { SerialPort::init() };

    // SAFETY: single-CPU fixture; only this handler and `run` touch these
    // statics, never concurrently.
    let disposition = unsafe {
        let current = CURRENT_TASK;
        let scheduler = &mut *(&raw mut SCHEDULER);
        let context = match current.and_then(|slot| task_id(scheduler, slot)) {
            Some(task) => FaultingContext::Task(task),
            None => FaultingContext::Kernel,
        };
        if let Some(slot) = current {
            FAULT_VECTORS[slot] = frame.vector;
        }
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
            let _ = writeln!(
                serial,
                "wx-seal contained task {} vector={} rip={:#x} cr2={:#x}",
                task.index(),
                frame.vector,
                frame.rip,
                frame.faulting_address().unwrap_or(0)
            );
            // SAFETY: the victim's registers land in a context nothing
            // resumes; `SUPERVISOR_CTX` is suspended inside
            // `run_once_in_space`'s own switch call site.
            unsafe { context::switch(&raw mut ABANDONED_CTX, &raw mut SUPERVISOR_CTX) };
            unreachable!("a terminated task is never switched back into")
        }
        Disposition::HaltSystem => {
            let _ = writeln!(
                serial,
                "wx-seal kernel-context fault vector={} rip={:#x} cr2={:#x}: halting",
                frame.vector,
                frame.rip,
                frame.faulting_address().unwrap_or(0)
            );
            let _ = write_result(&mut serial, "wx-seal", false);
            exit_qemu(QemuExitCode::Failure)
        }
    }
}

/// Required `#DF` entry — never expected to be reached here.
///
/// # Safety
/// Called only by `df_fault_stub` with a valid IST-stack `FaultFrame`.
#[no_mangle]
extern "C" fn tinyos_double_fault_entry(frame: *const FaultFrame) -> ! {
    let frame = unsafe { *frame };
    let mut serial = unsafe { SerialPort::init() };
    let _ = writeln!(serial, "wx-seal unexpected #DF rip={:#x} — halting", frame.rip);
    let _ = write_result(&mut serial, "wx-seal", false);
    exit_qemu(QemuExitCode::Failure)
}

fn task_id(scheduler: &Scheduler<TASKS>, slot: usize) -> Option<TaskId> {
    scheduler.iter_tasks().map(|(task, _)| task).find(|task| task.index() == slot)
}

/// Dispatches one round through the production `run_once_in_space`,
/// maintaining the `CURRENT_TASK` attribution the fault policy reads.
fn dispatch_round() -> Option<TaskId> {
    // SAFETY: single-CPU fixture; contexts/scheduler satisfy the dispatch
    // contract (each selected task's context slot is freshly initialized
    // and not otherwise in use).
    unsafe {
        let scheduler = &mut *(&raw mut SCHEDULER);
        let next = scheduler.highest_priority_ready()?;
        CURRENT_TASK = Some(next.index());
        let ran =
            dispatch::run_once_in_space(scheduler, &raw mut SUPERVISOR_CTX, &raw mut TASK_CTX);
        CURRENT_TASK = None;
        ran
    }
}

/// The audit over one tree: every present leaf must not be simultaneously
/// writable and executable, and every executable frame's identity-view
/// alias in the supervisor tree must be non-writable (review D5). Returns
/// `(leaves, executable_leaves, violations)`.
fn audit_tree(tree: &PageTable, kernel_view: &PageTable) -> (usize, usize, usize) {
    let mut leaves = 0usize;
    let mut executable = 0usize;
    let mut violations = 0usize;
    paging::for_each_leaf(tree, &mut |_virt, page| {
        leaves += 1;
        if page.writable && page.executable {
            violations += 1;
        }
        if page.executable {
            executable += 1;
            match paging::translate(kernel_view, page.phys) {
                // An executable frame with a writable identity alias is the
                // alias hole sealing exists to close.
                Some(alias) if alias.writable => violations += 1,
                _ => {}
            }
        }
    });
    (leaves, executable, violations)
}

fn run() -> bool {
    let mut serial = unsafe { SerialPort::init() };

    // SAFETY: called once, before any fault can occur.
    unsafe { hal_x86_64::interrupts::init_faults_only() };
    // SAFETY: still on the boot map (no NX bits anywhere in it), and every
    // mapping the kernel writes through hereafter is genuinely writable in
    // the W^X trees built below.
    unsafe { paging::enable_nx_and_wp() };
    let _ = writeln!(serial, "wx-seal: NXE+WP enabled");

    let layout = KernelLayout {
        exec_start: (&raw const __kernel_exec_start) as u64,
        exec_end: (&raw const __kernel_exec_end) as u64,
        rodata_start: (&raw const __kernel_rodata_start) as u64,
        rodata_end: (&raw const __kernel_rodata_end) as u64,
        image_end: (&raw const __kernel_image_end) as u64,
    };
    let _ = writeln!(
        serial,
        "wx-seal: layout exec={:#x}..{:#x} rodata={:#x}..{:#x} image_end={:#x}",
        layout.exec_start,
        layout.exec_end,
        layout.rodata_start,
        layout.rodata_end,
        layout.image_end
    );

    // SAFETY: single-CPU fixture; the pool static is borrowed once here and
    // never moves (the shared directories reference its frames by address).
    let dirs = unsafe {
        match kernel_map::build_shared_directories(
            &mut *(&raw mut KERNEL_MAP_POOL),
            layout,
            APIC_PAGE,
        ) {
            Ok(dirs) => dirs,
            Err(err) => {
                let _ = writeln!(serial, "wx-seal: kernel map build failed: {err:?}");
                return false;
            }
        }
    };

    // The supervisor's own tree: nothing but the shared directories — and
    // loading it is the moment the boot RWX identity map is retired.
    // SAFETY: single-CPU fixture; the supervisor PML4/frames statics never
    // move; the tree maps the executing code/stack/IDT (the whole kernel
    // image extent) before it is loaded.
    unsafe {
        let supervisor = &mut *(&raw mut SUPERVISOR_PML4);
        if paging::install_shared_pd(supervisor, &mut SupervisorFrames, 0, dirs.low_pd).is_err() {
            return false;
        }
        if paging::install_shared_pd(
            supervisor,
            &mut SupervisorFrames,
            dirs.apic_base,
            dirs.apic_pd,
        )
        .is_err()
        {
            return false;
        }
        paging::write_cr3(&raw const SUPERVISOR_PML4 as u64);
    }
    let _ = writeln!(serial, "wx-seal: boot RWX map retired, supervisor W^X tree live");

    // The task space: two pages of its own plus the same shared kernel
    // directory every other tree links.
    // SAFETY: single-CPU fixture; each static is borrowed once; the space
    // (and the borrows it holds) lives to the end of `run`.
    let (space, cr3_task) = unsafe {
        let task_sections = sections();
        let mut space = match AddressSpace::create(
            &mut *(&raw mut TASK_PML4),
            &mut *(&raw mut TASK_FRAME_POOL),
            &task_sections,
            IMAGE_BASE,
            &IMAGE_BYTES.0,
            &mut *(&raw mut STAGING.0),
        ) {
            Ok(space) => space,
            Err(_) => return false,
        };
        if space.attach_shared_pd(0, dirs.low_pd).is_err()
            || space.attach_shared_pd(dirs.apic_base, dirs.apic_pd).is_err()
        {
            return false;
        }
        if space.seal_kernel_alias(&mut *(&raw mut SUPERVISOR_PML4)).is_err() {
            return false;
        }
        let cr3 = space.cr3();
        (space, cr3)
    };
    let _ = writeln!(serial, "wx-seal: task space built and sealed, cr3={cr3_task:#x}");

    let mut ok = true;

    // AC A4 — shared, not duplicated: both trees name the same physical
    // directory for the kernel's own 1GiB region.
    // SAFETY: read-only walks of the two static trees.
    unsafe {
        let supervisor_pd = paging::directory_addr(&*(&raw const SUPERVISOR_PML4), 0);
        let task_pd = paging::directory_addr(&*(&raw const TASK_PML4), 0);
        ok &= supervisor_pd == Some(dirs.low_pd);
        ok &= task_pd == Some(dirs.low_pd);
        let _ = writeln!(
            serial,
            "wx-seal: shared low PD {:#x} (supervisor={supervisor_pd:?} task={task_pd:?})",
            dirs.low_pd
        );
    }

    // AC A3 — the walk audit over both live trees, alias clause included.
    // SAFETY: read-only walks of the two static trees.
    let (sup_leaves, sup_exec, sup_violations) =
        unsafe { audit_tree(&*(&raw const SUPERVISOR_PML4), &*(&raw const SUPERVISOR_PML4)) };
    let (task_leaves, task_exec, task_violations) =
        unsafe { audit_tree(&*(&raw const TASK_PML4), &*(&raw const SUPERVISOR_PML4)) };
    ok &= sup_violations == 0 && task_violations == 0;
    ok &= sup_exec > 0 && task_exec > sup_exec; // the task tree adds its RX page
    let _ = writeln!(
        serial,
        "wx-seal: audit supervisor leaves={sup_leaves} exec={sup_exec} violations={sup_violations}; \
         task leaves={task_leaves} exec={task_exec} violations={task_violations}"
    );

    // The three adversarial tasks. Priorities order the rounds: write-RX
    // first, execute-RW second, stale-probe created only after teardown.
    // SAFETY: single-CPU fixture; each slot's stack/context serves exactly
    // one task for the whole run.
    let (task_write, task_exec_rw) = unsafe {
        let scheduler = &mut *(&raw mut SCHEDULER);
        let Ok(high) = Priority::try_new(10) else { return false };
        let Ok(low) = Priority::try_new(5) else { return false };
        // The execute-RW task's entry *is* the RW page's own address — the
        // first instruction fetch is the violation.
        let rw_page_entry: extern "C" fn() -> ! =
            core::mem::transmute((IMAGE_BASE + PAGE_SIZE) as usize);

        let Ok(task_write) = scheduler.create_task(
            high,
            WcetBudgetTicks(1_000),
            OverrunPolicy::TripToSafeState,
            task_write_rx_entry,
        ) else {
            return false;
        };
        let Ok(task_exec_rw) = scheduler.create_task(
            low,
            WcetBudgetTicks(1_000),
            OverrunPolicy::TripToSafeState,
            rw_page_entry,
        ) else {
            return false;
        };
        if scheduler.set_address_space(task_write, cr3_task).is_none()
            || scheduler.set_address_space(task_exec_rw, cr3_task).is_none()
        {
            return false;
        }

        let stack_0 =
            core::slice::from_raw_parts_mut((&raw mut TASK_STACKS[0]).cast::<u8>(), STACK_SIZE);
        let Ok(ctx_0) = Context::new(stack_0, task_write_rx_entry) else { return false };
        TASK_CTX[0] = ctx_0;
        let stack_1 =
            core::slice::from_raw_parts_mut((&raw mut TASK_STACKS[1]).cast::<u8>(), STACK_SIZE);
        let Ok(ctx_1) = Context::new(stack_1, rw_page_entry) else { return false };
        TASK_CTX[1] = ctx_1;
        (task_write, task_exec_rw)
    };

    // Round 1: the write-to-RX task. Round 2: the execute-RW task.
    let first = dispatch_round();
    ok &= first == Some(task_write);
    let second = dispatch_round();
    ok &= second == Some(task_exec_rw);
    // SAFETY: read after both rounds returned via the handler's escape.
    unsafe {
        ok &= SCHEDULER.state_of(task_write) == Some(TaskState::Finished);
        ok &= SCHEDULER.state_of(task_exec_rw) == Some(TaskState::Finished);
        ok &= FAULT_VECTORS[0] == 14 && FAULT_VECTORS[1] == 14;
        let _ = writeln!(
            serial,
            "wx-seal: write-to-RX vector={} execute-RW vector={} (14 = #PF)",
            FAULT_VECTORS[0], FAULT_VECTORS[1]
        );
    }

    // AC A2 — unseal, tear down, then prove it: residue, generation, and a
    // stale probe under the still-loadable torn tree.
    let mut generation = TeardownGeneration::new();
    // SAFETY: the supervisor tree static is the kernel view the seal was
    // applied to; staging is quiescent (both image tasks are Finished).
    unsafe {
        if space.unseal_kernel_alias(&mut *(&raw mut SUPERVISOR_PML4)).is_err() {
            return false;
        }
        let had_residue = STAGING.0.iter().any(|&b| b != 0);
        ok &= had_residue; // the dead task's bytes were really there...
        space.teardown(&mut *(&raw mut STAGING.0), &mut generation);
        ok &= STAGING.0.iter().all(|&b| b == 0); // ...and now they are not.
        ok &= generation.value() == 1;
    }
    let _ = writeln!(
        serial,
        "wx-seal: teardown complete, generation={} staging wiped",
        generation.value()
    );

    // SAFETY: as for the earlier task creation; slot 2's stack/context are
    // fresh.
    let task_probe = unsafe {
        let scheduler = &mut *(&raw mut SCHEDULER);
        let Ok(priority) = Priority::try_new(8) else { return false };
        let Ok(task_probe) = scheduler.create_task(
            priority,
            WcetBudgetTicks(1_000),
            OverrunPolicy::TripToSafeState,
            task_stale_probe_entry,
        ) else {
            return false;
        };
        if scheduler.set_address_space(task_probe, cr3_task).is_none() {
            return false;
        }
        let stack_2 =
            core::slice::from_raw_parts_mut((&raw mut TASK_STACKS[2]).cast::<u8>(), STACK_SIZE);
        let Ok(ctx_2) = Context::new(stack_2, task_stale_probe_entry) else { return false };
        TASK_CTX[2] = ctx_2;
        task_probe
    };

    let third = dispatch_round();
    ok &= third == Some(task_probe);
    // SAFETY: read after the round returned via the handler's escape.
    unsafe {
        ok &= SCHEDULER.state_of(task_probe) == Some(TaskState::Finished);
        ok &= FAULT_VECTORS[2] == 14;
        let _ = writeln!(serial, "wx-seal: stale probe vector={} (14 = #PF)", FAULT_VECTORS[2]);
    }

    let _ = write_result(&mut serial, "wx-seal", ok);
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
