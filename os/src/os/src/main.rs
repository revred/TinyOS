//! The TinyOS system image (`STORY-P1-03-03`).
//!
//! **Why this crate exists.** Until now the shipping binary was `kernel`,
//! and `kernel`'s real boot path discovered ACPI topology, enumerated PCI
//! bus 0, and halted — it had never created a task. Not by oversight: `exec`
//! (the loader, the address spaces, the Win32 shim) depends on `kernel`, so
//! `kernel`'s own binary can never depend back on `exec` without a cyclic
//! crate dependency. Every integration this project has built therefore had
//! to live in a fixture binary inside `exec`, which is exactly why
//! `REPORT-2026-07-28-01` had to record that "the shipping `kernel` binary
//! still discovers topology and halts" as its largest open gap.
//!
//! This crate is the resolution named there: a top-level binary that depends
//! on *both*, so the real boot path can do the real thing. `kernel` remains
//! a library plus its own fixture binary; nothing about it changed to make
//! this possible.
//!
//! **What the real boot path now does**, in order:
//!
//! 1. Discovers ACPI topology and enumerates PCI bus 0 — unchanged calls,
//!    unchanged success gates, still on the boot-time identity map, because
//!    firmware tables live outside the kernel image's own linked extent.
//! 2. Enables `CR0.WP`/`EFER.NXE` and builds the W^X kernel page directories
//!    from the linker's own section symbols, then loads them — retiring the
//!    all-RWX bring-up map that has been this system's address space since
//!    `STORY-P0-01-01`.
//! 3. Loads the embedded capability probe through the real PE64 pipeline
//!    into its own W^X address space, sharing (not copying) the kernel
//!    directories.
//! 4. Resolves the image's imports against the closed allowlist and the
//!    capability policy, patching the IAT: granted calls get real
//!    trampolines, everything else gets `iat::CAPABILITY_TRAP_VIRT`. Then
//!    seals the loader's writable aliases.
//! 5. Schedules it as a real task through `dispatch::run_once_in_space`,
//!    under its own `CR3`.
//! 6. Contains whatever happens, and journals every step as spoors.
//!
//! **Why the embedded workload is the probe and not `blue-sharc.exe`.** A
//! system image is the operating system, not the applications it runs.
//! `blue-sharc.exe` is 8.3MiB — embedding it (plus its staging arena) would
//! put ~17MiB of third-party application into a kernel image whose `G-DX-8`
//! ceiling is 8MiB, to prove something the `first-task` fixture already
//! proves against the real artifact. The probe is 16KiB, is a genuine PE32+
//! parsed by the same loader, and demonstrates the half `blue-sharc.exe`
//! *cannot*: a granted call that actually resolves, executes, and returns.
//! Loading a workload from storage rather than from `.rodata` is what a
//! filesystem Story will replace this with; until one exists, an embedded
//! image is the only honest option and the small one is the right choice.

#![no_std]
#![no_main]
#![deny(missing_docs)]
#![allow(static_mut_refs, clippy::deref_addrof)]

use core::fmt::Write;
use exec::address_space::AddressSpace;
use exec::iat::{self, PatchSummary};
use exec::kernel_map::{self, KernelLayout};
use exec::pe::{self, SectionDescriptor};
use exec::win32_shim::{Api, CapabilityPolicy};
#[allow(unused_imports)]
// linked for its `global_asm!` side effect only, per its own doc comment
use hal_x86_64::boot as _;
use hal_x86_64::fault::FaultFrame;
use hal_x86_64::paging::{self, FrameAllocator, PageTable};
use hal_x86_64::qemu_exit::{exit_qemu, QemuExitCode};
use hal_x86_64::serial::SerialPort;
use kernel::context::Context;
use kernel::dispatch;
use kernel::fault::{Disposition, FaultReport, FaultingContext};
use kernel::mem::Pool;
use kernel::sched::{Priority, Scheduler, TaskId, TaskState, WcetBudgetTicks};
use kernel::spoor::{Action, Actor, Category, Outcome, Spoor};
use kernel::spoor_journal::SpoorJournal;

unsafe extern "C" {
    static __kernel_exec_start: u8;
    static __kernel_exec_end: u8;
    static __kernel_rodata_start: u8;
    static __kernel_rodata_end: u8;
    static __kernel_image_end: u8;
}

/// The local-APIC timer reload value, unchanged from `kernel::main`'s own
/// real boot path.
const BOOT_TIMER_INITIAL_COUNT: u32 = 1_000_000;
/// The architectural local-APIC MMIO page, mapped by the shared kernel
/// directories so the armed timer keeps working under every address space.
const APIC_PAGE: u64 = 0xFEE0_0000;

const SECTIONS: usize = 8;
const IMPORTS: usize = 16;
const IMAGE_FRAMES: usize = 16;
const KERNEL_MAP_FRAMES: usize = 64;
const STACK_SIZE: usize = 16_384;
const TASKS: usize = 4;

/// The embedded capability probe (`xtask make-probe-pe`), four pages.
const PROBE_LEN: usize = 16_384;

#[repr(C, align(4096))]
struct AlignedImage([u8; PROBE_LEN]);
#[repr(C, align(4096))]
struct AlignedStaging([u8; PROBE_LEN]);

static PROBE_IMAGE: AlignedImage =
    AlignedImage(*include_bytes!("../../exec/fixtures/capability-probe.txe"));
static mut STAGING: AlignedStaging = AlignedStaging([0; PROBE_LEN]);
static mut IMAGE_PML4: PageTable = PageTable::new();
static mut IMAGE_FRAME_POOL: Pool<PageTable, IMAGE_FRAMES> = Pool::new();

static mut KERNEL_MAP_POOL: Pool<PageTable, KERNEL_MAP_FRAMES> = Pool::new();
static mut SUPERVISOR_PML4: PageTable = PageTable::new();
static mut SUPERVISOR_FRAMES: [PageTable; 4] = [const { PageTable::new() }; 4];
static mut SUPERVISOR_FRAMES_USED: usize = 0;

static mut SCHEDULER: Scheduler<TASKS> = Scheduler::new();
static mut SUPERVISOR_CTX: Context = Context::zeroed();
static mut ABANDONED_CTX: Context = Context::zeroed();
static mut TASK_CTX: [Context; TASKS] = [Context::zeroed(); TASKS];
static mut TASK_STACKS: [[u8; STACK_SIZE]; TASKS] = [[0; STACK_SIZE]; TASKS];

static mut CURRENT_TASK: Option<usize> = None;
static mut FAULT_ADDRESS: u64 = 0;
static mut FAULT_CAPTURED: bool = false;

/// The system's audit journal — spoor in the shipping image, not in a test
/// double (`kernel::capacities::SPOOR_JOURNAL_CAPACITY`).
static mut JOURNAL: SpoorJournal<{ kernel::capacities::SPOOR_JOURNAL_CAPACITY }> =
    SpoorJournal::new();

/// This image's capability policy: the probe is granted exactly the two
/// calls it imports, and nothing else.
///
/// Stated as a grant list rather than as "allow all", because the two are
/// indistinguishable for *this* image and completely different for the next
/// one. A policy that happens to be permissive enough today is how a
/// capability system quietly stops being one.
struct ProbePolicy;
impl CapabilityPolicy for ProbePolicy {
    fn is_granted(&self, api: Api) -> bool {
        matches!(api, Api::GetCurrentProcess | Api::ExitProcess)
    }
}

struct SupervisorFrames;
impl FrameAllocator for SupervisorFrames {
    fn allocate_frame(&mut self) -> Option<u64> {
        // SAFETY: single-CPU boot path; only this allocator touches these.
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

fn journal(spoor: Spoor) {
    // SAFETY: single-CPU boot path; the journal is touched only here and in
    // the never-returning fault handler, never concurrently.
    unsafe {
        (*(&raw mut JOURNAL)).append(spoor);
    }
}

fn task_id(scheduler: &Scheduler<TASKS>, slot: usize) -> Option<TaskId> {
    scheduler.iter_tasks().map(|(task, _)| task).find(|task| task.index() == slot)
}

/// The system's fault entry point: `kernel::fault`'s policy, unmodified,
/// applied to whichever context was running.
///
/// # Safety
/// Called only by the fault stubs, with `frame` pointing at a
/// fully-initialized [`FaultFrame`] on the faulting stack.
#[no_mangle]
extern "C" fn tinyos_fault_entry(frame: *const FaultFrame) -> ! {
    // SAFETY: the stubs pass a valid `FaultFrame` pointer, live for this call.
    let frame = unsafe { *frame };
    // SAFETY: this handler never returns, so re-initializing COM1 cannot
    // race any other user of it on this single-CPU path.
    let mut serial = unsafe { SerialPort::init() };

    // SAFETY: single-CPU path; only this handler and `boot` touch these
    // statics, never concurrently.
    let disposition = unsafe {
        FAULT_CAPTURED = true;
        FAULT_ADDRESS = frame.faulting_address().unwrap_or(0);
        let scheduler = &mut *(&raw mut SCHEDULER);
        let context = match CURRENT_TASK.and_then(|slot| task_id(scheduler, slot)) {
            Some(task) => FaultingContext::Task(task),
            None => FaultingContext::Kernel,
        };
        let report = FaultReport { vector: frame.vector, context };
        let disposition = Disposition::of(&report);
        for spoor in kernel::fault::audit(&report, disposition) {
            (*(&raw mut JOURNAL)).append(spoor);
        }
        disposition
    };

    let refused_capability = frame.faulting_address() == Some(iat::CAPABILITY_TRAP_VIRT);
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
                "tinyos: task {} terminated — vector={} rip={:#x} cr2={:#x}{}",
                task.index(),
                frame.vector,
                frame.rip,
                frame.faulting_address().unwrap_or(0),
                if refused_capability { " (refused capability)" } else { "" }
            );
            // SAFETY: the victim's registers land in a context nothing
            // resumes; `SUPERVISOR_CTX` is suspended inside the dispatcher's
            // own switch call site.
            unsafe {
                kernel::context::switch(&raw mut ABANDONED_CTX, &raw mut SUPERVISOR_CTX);
            }
            unreachable!("a terminated task is never switched back into")
        }
        Disposition::HaltSystem => {
            let _ = writeln!(
                serial,
                "tinyos: kernel-context fault vector={} rip={:#x} cr2={:#x} — halting",
                frame.vector,
                frame.rip,
                frame.faulting_address().unwrap_or(0)
            );
            exit_qemu(QemuExitCode::Failure)
        }
    }
}

/// The system's `#DF` entry point — terminal by nature, reporting before it
/// stops (`STORY-P1-02-02`).
///
/// # Safety
/// Called only by `df_fault_stub`, with `frame` pointing at a
/// fully-initialized [`FaultFrame`] on the IST stack.
#[no_mangle]
extern "C" fn tinyos_double_fault_entry(frame: *const FaultFrame) -> ! {
    // SAFETY: the stub passes a valid `FaultFrame` pointer on the IST stack.
    let frame = unsafe { *frame };
    // SAFETY: never returns; see `tinyos_fault_entry`.
    let mut serial = unsafe { SerialPort::init() };
    let _ = writeln!(
        serial,
        "tinyos double fault #DF rip={:#x} — the fault path itself failed, halting",
        frame.rip
    );
    let _ = kernel::fault::audit_double_fault(kernel::fault::FaultingContext::Kernel);
    exit_qemu(QemuExitCode::Failure)
}

/// Discovers hardware, brings up W^X memory protection, loads and schedules
/// the embedded workload, and reports. Returns whether every stage held.
fn boot(start_info_paddr: u64) -> bool {
    // SAFETY: single-CPU boot path with no concurrent COM1 user yet.
    let mut serial = unsafe { SerialPort::init() };
    let mut ok = true;

    // ---- Stage 1: hardware discovery, exactly as before.
    //
    // SAFETY: `start_info_paddr` is the PVH `hvm_start_info` address
    // `boot.rs` handed this binary, and every table it points at lies inside
    // the first 1GiB the bring-up map identity-maps.
    let topology = unsafe {
        hal_x86_64::acpi::discover_topology::<{ kernel::capacities::MAX_CPUS }>(start_info_paddr)
    };
    let Ok(topology) = topology else {
        let _ = writeln!(serial, "tinyos: ACPI topology discovery failed");
        return false;
    };
    if topology.is_empty() {
        let _ = writeln!(serial, "tinyos: ACPI reported no CPUs");
        return false;
    }
    // SAFETY: called exactly once, before anything here depends on
    // interrupts being armed — `init`'s own documented contract.
    unsafe { hal_x86_64::interrupts::init(BOOT_TIMER_INITIAL_COUNT) };
    // SAFETY: single-CPU boot path with no other config-space user, so
    // exclusive use of the 0xCF8/0xCFC pair holds trivially.
    let mut cam = unsafe { hal_x86_64::pci::PortCam::new() };
    let mut devices: hal::device::DeviceTable<{ kernel::capacities::MAX_PCI_DEVICES }> =
        hal::device::DeviceTable::new();
    if hal_x86_64::pci::enumerate_bus_zero(&mut cam, &mut devices).is_err() || devices.is_empty() {
        let _ = writeln!(serial, "tinyos: PCI bus-0 enumeration failed");
        return false;
    }
    journal(Spoor::stamp(
        Category::Boot,
        Actor::Kernel,
        Action::Create,
        Outcome::Ok,
        topology.len() as u16,
        devices.len() as u32,
    ));
    let _ = writeln!(serial, "tinyos: {} CPU(s), {} PCI device(s)", topology.len(), devices.len());

    // ---- Stage 2: retire the all-RWX bring-up map.
    // SAFETY: still on the bring-up map (which carries no NX bits), and
    // every mapping written through hereafter is genuinely writable in the
    // W^X tree built below.
    unsafe { paging::enable_nx_and_wp() };
    let layout = KernelLayout {
        exec_start: (&raw const __kernel_exec_start) as u64,
        exec_end: (&raw const __kernel_exec_end) as u64,
        rodata_start: (&raw const __kernel_rodata_start) as u64,
        rodata_end: (&raw const __kernel_rodata_end) as u64,
        image_end: (&raw const __kernel_image_end) as u64,
    };
    // SAFETY: the pool static is borrowed once here and never moves — the
    // shared directories reference its frames by address.
    let dirs = unsafe {
        match kernel_map::build_shared_directories(
            &mut *(&raw mut KERNEL_MAP_POOL),
            layout,
            APIC_PAGE,
        ) {
            Ok(dirs) => dirs,
            Err(err) => {
                let _ = writeln!(serial, "tinyos: kernel map build failed: {err:?}");
                return false;
            }
        }
    };
    // SAFETY: the supervisor tree maps this binary's whole linked extent
    // (code, stacks, IDT/GDT/TSS, statics) plus the APIC MMIO page before it
    // is loaded — `write_cr3`'s contract.
    unsafe {
        let supervisor = &mut *(&raw mut SUPERVISOR_PML4);
        if paging::install_shared_pd(supervisor, &mut SupervisorFrames, 0, dirs.low_pd).is_err()
            || paging::install_shared_pd(
                supervisor,
                &mut SupervisorFrames,
                dirs.apic_base,
                dirs.apic_pd,
            )
            .is_err()
        {
            let _ = writeln!(serial, "tinyos: supervisor tree construction failed");
            return false;
        }
        paging::write_cr3(&raw const SUPERVISOR_PML4 as u64);
    }
    let _ = writeln!(serial, "tinyos: W^X memory protection active");

    // ---- Stage 3: load the embedded workload.
    let descriptor = match pe::parse::<SECTIONS, IMPORTS>(&PROBE_IMAGE.0) {
        Ok(descriptor) => descriptor,
        Err(err) => {
            let _ = writeln!(serial, "tinyos: image rejected by the loader: {err:?}");
            return false;
        }
    };
    let entry_virt = descriptor.image_base + u64::from(descriptor.entry_point_rva);
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
    // The writable section is where the workload reports its result. Derived
    // from the image's own section table rather than hardcoded, so a change
    // to the image's layout cannot silently make this read the wrong page.
    let Some(result_virt) = sections[..section_count]
        .iter()
        .find(|section| section.permissions.write && !section.permissions.execute)
        .map(|section| descriptor.image_base + u64::from(section.virtual_address))
    else {
        let _ = writeln!(serial, "tinyos: image declares no writable data section");
        return false;
    };

    // The load-time capability gate. This image is expected to pass it —
    // unlike `blue-sharc.exe`, whose 205-import surface the same gate
    // refuses (see `exec`'s `first-task` fixture).
    let admitted = exec::win32_shim::check_imports(descriptor.imports()).is_ok();
    ok &= admitted;
    journal(Spoor::stamp(
        Category::Exec,
        Actor::Exec,
        Action::Block,
        if admitted { Outcome::Skipped } else { Outcome::Failed },
        0,
        descriptor.imports().count() as u32,
    ));

    let patch: PatchSummary;
    // SAFETY: each static is borrowed once; the space (and the borrows it
    // holds) lives to the end of this function, and nothing has sealed the
    // identity view at the point `patch_imports` writes through it.
    let cr3_image = unsafe {
        let mut space = match AddressSpace::create(
            &mut *(&raw mut IMAGE_PML4),
            &mut *(&raw mut IMAGE_FRAME_POOL),
            &sections[..section_count],
            descriptor.image_base,
            &PROBE_IMAGE.0,
            &mut *(&raw mut STAGING.0),
        ) {
            Ok(space) => space,
            Err(err) => {
                let _ = writeln!(serial, "tinyos: address space creation failed: {err:?}");
                return false;
            }
        };
        if space.attach_shared_pd(0, dirs.low_pd).is_err()
            || space.attach_shared_pd(dirs.apic_base, dirs.apic_pd).is_err()
        {
            let _ = writeln!(serial, "tinyos: kernel directory attach failed");
            return false;
        }
        patch = match iat::patch_imports(
            &space,
            descriptor.image_base,
            descriptor.imports(),
            &ProbePolicy,
        ) {
            Ok(summary) => summary,
            Err(err) => {
                let _ = writeln!(serial, "tinyos: IAT resolution failed: {err:?}");
                return false;
            }
        };
        // Sealing must follow patching: it closes the very identity view the
        // patch writes through, and it is what stops the task rewriting its
        // own IAT to grant itself capabilities.
        if space.seal_kernel_alias(&mut *(&raw mut SUPERVISOR_PML4)).is_err() {
            let _ = writeln!(serial, "tinyos: sealing failed");
            return false;
        }
        ok &=
            matches!(space.translate(entry_virt), Some(page) if page.executable && !page.writable);
        core::mem::forget(space);
        (&raw const IMAGE_PML4) as u64
    };
    ok &= patch.total() == descriptor.imports().count();
    ok &= patch.granted == 2 && patch.trapped() == 0;
    let _ = writeln!(
        serial,
        "tinyos: loaded image — {} import(s), {} granted, {} trapped, cr3={cr3_image:#x}",
        descriptor.imports().count(),
        patch.granted,
        patch.trapped()
    );

    // The live tree must contain no writable-and-executable mapping, and no
    // executable frame with a writable alias — audited, not assumed.
    // SAFETY: read-only walks of the two static trees.
    let violations = unsafe {
        let mut violations = 0usize;
        let kernel_view = &*(&raw const SUPERVISOR_PML4);
        paging::for_each_leaf(&*(&raw const IMAGE_PML4), &mut |_virt, page| {
            if page.writable && page.executable {
                violations += 1;
            }
            if page.executable {
                if let Some(alias) = paging::translate(kernel_view, page.phys) {
                    if alias.writable {
                        violations += 1;
                    }
                }
            }
        });
        violations
    };
    ok &= violations == 0;

    // ---- Stage 4: schedule it.
    // SAFETY: slot 0's stack and context serve exactly this task. The
    // transmute turns the image's own validated, RX-mapped entry virtual
    // address into the `TaskEntry` the scheduler takes — the first
    // instruction fetched under this task's own `CR3`.
    let task = unsafe {
        let entry: kernel::sched::TaskEntry = core::mem::transmute(entry_virt as usize);
        let scheduler = &mut *(&raw mut SCHEDULER);
        let Ok(priority) = Priority::try_new(8) else { return false };
        let Ok(task) = scheduler.create_task(priority, WcetBudgetTicks(1_000), entry) else {
            return false;
        };
        if scheduler.set_address_space(task, cr3_image).is_none() {
            return false;
        }
        let stack =
            core::slice::from_raw_parts_mut((&raw mut TASK_STACKS[0]).cast::<u8>(), STACK_SIZE);
        let Ok(context) = Context::new(stack, entry) else { return false };
        TASK_CTX[0] = context;
        task
    };
    journal(Spoor::stamp(
        Category::Dispatch,
        Actor::Kernel,
        Action::Select,
        Outcome::Chose,
        task.index() as u16,
        0,
    ));

    // SAFETY: the selected task's context slot was just initialized, and its
    // address space maps the kernel code/stack/IDT servicing it through the
    // shared directories.
    let ran = unsafe {
        CURRENT_TASK = Some(task.index());
        let scheduler = &mut *(&raw mut SCHEDULER);
        let ran =
            dispatch::run_once_in_space(scheduler, &raw mut SUPERVISOR_CTX, &raw mut TASK_CTX);
        CURRENT_TASK = None;
        ran
    };
    ok &= ran == Some(task);

    // ---- Stage 5: what the workload actually did.
    //
    // The probe calls `GetCurrentProcess` through its patched IAT, stores
    // the returned pseudo-handle into its own writable page, then calls
    // `ExitProcess`. Reading that page back is the evidence the *granted*
    // call really executed and returned correctly — not merely that the
    // task ran and then stopped.
    // SAFETY: read after the dispatch round returned; the writable page's
    // kernel alias is left writable by sealing (only non-writable pages are
    // sealed), so it is mapped and readable here.
    let reported = unsafe {
        match paging::translate(&*(&raw const IMAGE_PML4), result_virt) {
            Some(page) => core::ptr::read_volatile(page.phys as *const u64),
            None => 0,
        }
    };
    // `GetCurrentProcess` returns the `(HANDLE)-1` pseudo-handle.
    let call_succeeded = reported == u64::MAX;
    ok &= call_succeeded;

    // SAFETY: read after the round returned via the handler's escape switch.
    let (captured, trap_address, finished, journal_len) = unsafe {
        (
            FAULT_CAPTURED,
            FAULT_ADDRESS,
            SCHEDULER.state_of(task) == Some(TaskState::Finished),
            (*(&raw const JOURNAL)).len(),
        )
    };
    // `ExitProcess` has no process-teardown path yet, so it routes into the
    // capability trap — the documented, contained way this shim ends a task
    // today (`iat::trampolines::exit_process`). The task is therefore
    // expected to finish via containment at exactly that address.
    ok &= captured && finished && trap_address == iat::CAPABILITY_TRAP_VIRT;
    ok &= journal_len >= 5;

    let _ = writeln!(
        serial,
        "tinyos: workload returned {reported:#x} (GetCurrentProcess ok={call_succeeded}), \
         exited via trap {trap_address:#x}, task_finished={finished}, spoors={journal_len}"
    );
    let _ = writeln!(serial, "tinyos: boot complete, ok={ok}");
    ok
}

/// Entry point reached from `hal_x86_64::boot`'s long-mode transition.
///
/// `start_info_paddr` is the physical address of the PVH `hvm_start_info`
/// struct, handed here in `RDI`.
#[no_mangle]
extern "C" fn kernel_main(start_info_paddr: u64) -> ! {
    if boot(start_info_paddr) {
        exit_qemu(QemuExitCode::Success)
    } else {
        exit_qemu(QemuExitCode::Failure)
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    exit_qemu(QemuExitCode::Failure)
}
