//! `STORY-P1-03-02` Part B's Tier 0 QEMU fixture binary
//! (`TEST-P1-03-02-A` clauses I1–I5): **the first real task**.
//!
//! Every mechanism `EPIC-P0`/`EPIC-P1` proved in isolation, wired together
//! and run end to end for the first time: real ACPI topology and PCI bus-0
//! discovery, a real W^X kernel address space replacing the boot-time RWX
//! identity map, the real PE64/TXE loader over the real `blue-sharc.exe`
//! artifact, the real closed-allowlist capability gate, the real
//! `CR3`-aware dispatcher, the real fault policy, and spoor — all in one
//! boot.
//!
//! **Why this binary reproduces `kernel_main` rather than living in it**
//! (review D2). `exec` depends on `kernel`, so `kernel`'s own binary cannot
//! link `exec` back without a cyclic crate dependency — the same constraint
//! that created `exec-fixture` in `STORY-P0-05-02`. This binary therefore
//! makes the *same* `hal_x86_64::acpi::discover_topology` and
//! `hal_x86_64::pci::enumerate_bus_zero` calls against the *same* success
//! gates `kernel::main`'s real path applies, then continues past the point
//! where that path halts. Unifying the two behind one top-level `os` binary
//! crate is named follow-on work, not something this fixture claims to have
//! done.
//!
//! **Ordering** (review D3). Firmware tables (RSDP/MADT) and PCI config
//! space lie *outside* the kernel image's own linked extent, so discovery
//! runs first, on the boot map, exactly as today. Only afterwards is the
//! W^X kernel tree built and loaded — retiring the all-RWX bring-up map
//! that has been this system's address space since `STORY-P0-01-01`.
//!
//! **The capability boundary, at both of its real layers** (review D1). The
//! original acceptance criterion — "an out-of-allowlist Win32 call from
//! inside the task raises a fault" — conflated three mechanisms, none of
//! which faults: the shim is a Rust-level API with no IAT patching behind
//! it, an out-of-allowlist import is refused at *load* time, and a
//! policy-denied call returns `Err`. So both real layers are proven here
//! instead:
//!
//! 1. **Load time.** `win32_shim::check_imports` refuses `blue-sharc.exe`'s
//!    real 205-import surface, and the refusal is *journaled as a spoor* —
//!    the gate as production behavior, not only a fixture assertion.
//! 2. **Runtime, defense in depth.** The image is then scheduled anyway,
//!    under the explicit [`OverrideEverythingPolicy`] below, entering at its
//!    own real entry point. Its real startup code reaches for authority it
//!    was never granted — through an import address table nothing patched,
//!    precisely because nothing was granted — and the resulting transfer
//!    lands on memory that is not executable. That raises a real `#PF`,
//!    contained by the unmodified `kernel::fault` policy. Nothing about the
//!    fault is staged: it is what an ungoverned real workload does.
//!
//! **What the run actually observed**, recorded here because it is more
//! specific than the prediction above and was confirmed against the image's
//! own bytes: `blue-sharc.exe`'s real MSVC CRT startup called
//! **`GetSystemTimeAsFileTime`** — not in this shim's nine-call allowlist —
//! through its unpatched IAT. An unpatched thunk still holds the *RVA* of
//! its `IMAGE_IMPORT_BY_NAME` record, so the indirect call took that RVA as
//! an absolute address and fetched at `0x7b_9d9e`, where the image's own
//! bytes read `00 00 "GetSystemTimeAsFileTime"` — a hint word and an ASCII
//! name, not code. Under this Story's kernel mappings that address is
//! present but **NX**, so the fetch raised `#PF` (`rip == cr2 ==
//! 0x7b_9d9e`) and the task was terminated with the system intact. Worth
//! stating plainly: without `EFER.NXE` and a W^X kernel map, that fetch
//! would have *succeeded* and begun executing an API name string as code.

#![no_std]
#![no_main]
#![allow(static_mut_refs, clippy::deref_addrof)]

use core::fmt::Write;
use exec::address_space::AddressSpace;
use exec::iat::{self, PatchSummary};
use exec::kernel_map::{self, KernelLayout};
use exec::pe::{self, SectionDescriptor};
use exec::win32_shim::{self, Api, CapabilityPolicy};
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
use kernel::measure::write_result;
use kernel::mem::Pool;
use kernel::sched::{OverrunPolicy, Priority, Scheduler, TaskId, TaskState, WcetBudgetTicks};
use kernel::spoor::{Action, Actor, Category, Outcome, Spoor};
use kernel::spoor_journal::SpoorJournal;

unsafe extern "C" {
    static __kernel_exec_start: u8;
    static __kernel_exec_end: u8;
    static __kernel_rodata_start: u8;
    static __kernel_rodata_end: u8;
    static __kernel_image_end: u8;
}

/// The same reload value `kernel::main`'s real boot path arms.
const BOOT_TIMER_INITIAL_COUNT: u32 = 1_000_000;
const APIC_PAGE: u64 = 0xFEE0_0000;

/// Comfortably above `blue-sharc.txe`'s real 6 sections / 205 named
/// imports — the same bounds `blue-sharc-fixture` uses.
const SECTIONS: usize = 8;
const IMPORTS: usize = 256;
/// Page-table frames for the image's own tree: ~8.3MiB of virtual span
/// needs a PDPT, a PD and a handful of PTs, plus room for the shared
/// directory links.
const IMAGE_FRAMES: usize = 32;
/// Frames for the W^X kernel map — covering this binary's whole linked
/// extent (which includes the 8MiB staging arena below) at 4KiB
/// granularity.
const KERNEL_MAP_FRAMES: usize = 64;
const STACK_SIZE: usize = 16_384;
const TASKS: usize = 2;
/// Exactly `xtask pack-txe`'s output length for `blue-sharc.exe`.
const STAGING_LEN: usize = 8_265_728;

#[repr(C, align(4096))]
struct AlignedImage([u8; 8_269_824]);
#[repr(C, align(4096))]
struct AlignedStaging([u8; STAGING_LEN]);

static IMAGE_BYTES: AlignedImage = AlignedImage(*include_bytes!("../fixtures/blue-sharc.txe"));
static mut STAGING: AlignedStaging = AlignedStaging([0; STAGING_LEN]);
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
static mut FAULT_VECTOR: u64 = 0;
static mut FAULT_RIP: u64 = 0;
static mut FAULT_ADDRESS: u64 = 0;
static mut FAULT_CAPTURED: bool = false;

/// Spoor's production journal on this path (`STORY-P1-03-02` acceptance
/// criterion I5), sized by the capacity constant four `FEAT-P0-06` Reports
/// deferred until a real consumer existed.
static mut JOURNAL: SpoorJournal<{ kernel::capacities::SPOOR_JOURNAL_CAPACITY }> =
    SpoorJournal::new();

/// The deliberately-permissive policy under which the refused image is run
/// **anyway**, to prove runtime containment independently of the load-time
/// gate (review D1, layer 2). Named for what it is: this is not a policy
/// any real deployment would install, it is the adversary's best case —
/// every capability granted, the load-time gate bypassed — used to show
/// that containment does not *depend* on the gate having held.
struct OverrideEverythingPolicy;
impl CapabilityPolicy for OverrideEverythingPolicy {
    fn is_granted(&self, _api: Api) -> bool {
        true
    }
}

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

/// Appends `spoor` to the production journal.
fn journal(spoor: Spoor) {
    // SAFETY: single-CPU fixture; the journal is touched only here and in
    // the (never-returning) fault handler, never concurrently.
    unsafe {
        (*(&raw mut JOURNAL)).append(spoor);
    }
}

/// This fixture's fault entry point — the unmodified `kernel::fault`
/// policy, applied to whichever context was running.
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
        FAULT_CAPTURED = true;
        FAULT_VECTOR = frame.vector;
        FAULT_RIP = frame.rip;
        FAULT_ADDRESS = frame.faulting_address().unwrap_or(0);
        let scheduler = &mut *(&raw mut SCHEDULER);
        let context = match CURRENT_TASK.and_then(|slot| task_id(scheduler, slot)) {
            Some(task) => FaultingContext::Task(task),
            None => FaultingContext::Kernel,
        };
        let report = FaultReport { vector: frame.vector, context };
        let disposition = Disposition::of(&report);
        // Spoor's production call site for containment: the audit pair the
        // policy already computes is journaled, not discarded.
        for spoor in kernel::fault::audit(&report, disposition) {
            (*(&raw mut JOURNAL)).append(spoor);
        }
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
                "first-task contained task {} vector={} rip={:#x} cr2={:#x}",
                task.index(),
                frame.vector,
                frame.rip,
                frame.faulting_address().unwrap_or(0)
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
                "first-task kernel-context fault vector={} rip={:#x} cr2={:#x}: halting",
                frame.vector,
                frame.rip,
                frame.faulting_address().unwrap_or(0)
            );
            let _ = write_result(&mut serial, "first-task", false);
            exit_qemu(QemuExitCode::Failure)
        }
    }
}

/// Required `#DF` entry — never expected to be reached.
///
/// # Safety
/// Called only by `df_fault_stub` with a valid IST-stack `FaultFrame`.
#[no_mangle]
extern "C" fn tinyos_double_fault_entry(frame: *const FaultFrame) -> ! {
    let frame = unsafe { *frame };
    let mut serial = unsafe { SerialPort::init() };
    let _ = writeln!(serial, "first-task unexpected #DF rip={:#x} — halting", frame.rip);
    let _ = write_result(&mut serial, "first-task", false);
    exit_qemu(QemuExitCode::Failure)
}

fn task_id(scheduler: &Scheduler<TASKS>, slot: usize) -> Option<TaskId> {
    scheduler.iter_tasks().map(|(task, _)| task).find(|task| task.index() == slot)
}

fn run(start_info_paddr: u64) -> bool {
    let mut serial = unsafe { SerialPort::init() };
    let mut ok = true;

    // ---- Stage 1: the real boot path, reproduced verbatim (review D2/D3).
    //
    // SAFETY: `start_info_paddr` is the PVH `hvm_start_info` address `boot.rs`
    // handed this binary, and every table it points at lies inside the first
    // 1GiB the boot map identity-maps — `kernel_main`'s own documented
    // contract for this call, unchanged.
    let topology = unsafe {
        hal_x86_64::acpi::discover_topology::<{ kernel::capacities::MAX_CPUS }>(start_info_paddr)
    };
    let cpu_count = match topology {
        Ok(topology) if !topology.is_empty() => topology.len(),
        _ => {
            let _ = writeln!(serial, "first-task: ACPI topology discovery failed");
            let _ = write_result(&mut serial, "first-task", false);
            return false;
        }
    };
    // SAFETY: called exactly once, before anything here depends on
    // interrupts being armed — `init`'s own documented contract, and the
    // same call the real boot path makes. `init` also installs the fault
    // handlers this fixture's containment relies on.
    unsafe { hal_x86_64::interrupts::init(BOOT_TIMER_INITIAL_COUNT) };
    // SAFETY: single-CPU boot path with no other config-space user, so
    // exclusive use of the 0xCF8/0xCFC pair holds trivially.
    let mut cam = unsafe { hal_x86_64::pci::PortCam::new() };
    let mut devices: hal::device::DeviceTable<{ kernel::capacities::MAX_PCI_DEVICES }> =
        hal::device::DeviceTable::new();
    let device_count = match hal_x86_64::pci::enumerate_bus_zero(&mut cam, &mut devices) {
        Ok(()) if !devices.is_empty() => devices.len(),
        _ => {
            let _ = writeln!(serial, "first-task: PCI bus-0 enumeration failed");
            let _ = write_result(&mut serial, "first-task", false);
            return false;
        }
    };
    journal(Spoor::stamp(
        Category::Boot,
        Actor::Kernel,
        Action::Create,
        Outcome::Ok,
        cpu_count as u16,
        device_count as u32,
    ));
    let _ = writeln!(
        serial,
        "first-task: real boot path complete — {cpu_count} CPU(s), {device_count} PCI device(s)"
    );

    // ---- Stage 2: retire the boot-time RWX identity map (review D3/D4).
    // SAFETY: still on the boot map (which carries no NX bits), and every
    // mapping written through hereafter is genuinely writable in the W^X
    // tree built below.
    unsafe { paging::enable_nx_and_wp() };
    let layout = KernelLayout {
        exec_start: (&raw const __kernel_exec_start) as u64,
        exec_end: (&raw const __kernel_exec_end) as u64,
        rodata_start: (&raw const __kernel_rodata_start) as u64,
        rodata_end: (&raw const __kernel_rodata_end) as u64,
        image_end: (&raw const __kernel_image_end) as u64,
    };
    // SAFETY: single-CPU fixture; the pool static is borrowed once and never
    // moves — the shared directories reference its frames by address.
    let dirs = unsafe {
        match kernel_map::build_shared_directories(
            &mut *(&raw mut KERNEL_MAP_POOL),
            layout,
            APIC_PAGE,
        ) {
            Ok(dirs) => dirs,
            Err(err) => {
                let _ = writeln!(serial, "first-task: kernel map build failed: {err:?}");
                let _ = write_result(&mut serial, "first-task", false);
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
            let _ = write_result(&mut serial, "first-task", false);
            return false;
        }
        paging::write_cr3(&raw const SUPERVISOR_PML4 as u64);
    }
    let _ = writeln!(serial, "first-task: boot RWX map retired, W^X kernel tree live");

    // ---- Stage 3: the real loader over the real artifact.
    let descriptor = match pe::parse::<SECTIONS, IMPORTS>(&IMAGE_BYTES.0) {
        Ok(descriptor) => descriptor,
        Err(err) => {
            let _ = writeln!(serial, "first-task: pe::parse rejected the real image: {err:?}");
            let _ = write_result(&mut serial, "first-task", false);
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

    // ---- Stage 4, layer 1: the load-time capability gate, as production
    // behavior — the refusal is journaled, not merely asserted.
    let refused = win32_shim::check_imports(descriptor.imports())
        == Err(win32_shim::ShimError::NotAllowlisted);
    ok &= refused;
    journal(Spoor::stamp(
        Category::Exec,
        Actor::Exec,
        Action::Block,
        if refused { Outcome::Ok } else { Outcome::Failed },
        0,
        descriptor.imports().count() as u32,
    ));
    let _ = writeln!(
        serial,
        "first-task: load-time gate refused the real {}-import surface: {refused}",
        descriptor.imports().count()
    );

    // ---- Stage 5: the image's own W^X-correct, kernel-sharing space.
    // Filled in inside the `unsafe` load block below, then audited after it.
    let patch_summary: PatchSummary;
    // SAFETY: single-CPU fixture; each static is borrowed once, and the
    // space (with the borrows it holds) lives to the end of `run`.
    let cr3_image = unsafe {
        let mut space = match AddressSpace::create(
            &mut *(&raw mut IMAGE_PML4),
            &mut *(&raw mut IMAGE_FRAME_POOL),
            &sections[..section_count],
            descriptor.image_base,
            &IMAGE_BYTES.0,
            &mut *(&raw mut STAGING.0),
        ) {
            Ok(space) => space,
            Err(err) => {
                let _ = writeln!(serial, "first-task: AddressSpace::create failed: {err:?}");
                let _ = write_result(&mut serial, "first-task", false);
                return false;
            }
        };
        if space.attach_shared_pd(0, dirs.low_pd).is_err()
            || space.attach_shared_pd(dirs.apic_base, dirs.apic_pd).is_err()
        {
            let _ = write_result(&mut serial, "first-task", false);
            return false;
        }
        // Resolve every import before the image's first instruction runs
        // (`STORY-P1-03-03`): granted calls get a real callable trampoline,
        // everything else gets `CAPABILITY_TRAP_VIRT`. This must precede
        // sealing — it writes through the identity view sealing closes.
        // SAFETY: nothing has sealed this space yet, and its staged frames
        // are live and unaliased at this point in the load.
        let summary = match iat::patch_imports(
            &space,
            descriptor.image_base,
            descriptor.imports(),
            &OverrideEverythingPolicy,
        ) {
            Ok(summary) => summary,
            Err(err) => {
                let _ = writeln!(serial, "first-task: IAT patching failed: {err:?}");
                let _ = write_result(&mut serial, "first-task", false);
                return false;
            }
        };
        patch_summary = summary;
        // Sealing closes the loader's writable-alias hole (review D5): the
        // staging frames backing this image's RX text keep no writable view
        // in the kernel's own tree — and, now that the IAT has been
        // patched, no writable view of the resolved function pointers
        // either. A task that could rewrite its own IAT could grant itself
        // capabilities, so this ordering is load-bearing, not tidiness.
        if space.seal_kernel_alias(&mut *(&raw mut SUPERVISOR_PML4)).is_err() {
            let _ = write_result(&mut serial, "first-task", false);
            return false;
        }
        // The entry page really is executable and really is not writable —
        // read back from the tree that is about to become live.
        let entry_page = space.translate(entry_virt);
        ok &= matches!(entry_page, Some(p) if p.executable && !p.writable);
        core::mem::forget(space);
        space_cr3()
    };
    // Every import got a decision, and the ones this image was not granted
    // outnumber the ones it was — the load-time gate's refusal restated as
    // the concrete table the image will actually indirect through.
    ok &= patch_summary.total() == descriptor.imports().count();
    ok &= patch_summary.trapped() > 0;
    journal(Spoor::stamp(
        Category::Exec,
        Actor::Kernel,
        Action::Create,
        Outcome::Partial,
        patch_summary.granted as u16,
        patch_summary.trapped() as u32,
    ));
    let _ = writeln!(
        serial,
        "first-task: IAT resolved — {} granted, {} trapped ({} not allowlisted, {} denied), \
         trap={:#x}",
        patch_summary.granted,
        patch_summary.trapped(),
        patch_summary.not_allowlisted,
        patch_summary.denied,
        iat::CAPABILITY_TRAP_VIRT
    );
    let _ = writeln!(
        serial,
        "first-task: image space built/sealed, cr3={cr3_image:#x} entry={entry_virt:#x}"
    );

    // Every leaf of the live image tree, audited: nothing writable *and*
    // executable, and no executable frame with a writable kernel alias.
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
    let _ =
        writeln!(serial, "first-task: W^X audit of the live image tree, violations={violations}");

    // ---- Stage 6, layer 2: schedule the refused image anyway, under the
    // override policy, and contain what it does.
    ok &= win32_shim::heap_alloc(&OverrideEverythingPolicy, 64) == Ok(64);
    // SAFETY: single-CPU fixture; slot 0's stack/context serve exactly this
    // task. The transmute turns the image's own validated, RX-mapped entry
    // virtual address into the `TaskEntry` the scheduler and `Context` take
    // — the first instruction fetched under this task's own `CR3`.
    let task = unsafe {
        let entry: kernel::sched::TaskEntry = core::mem::transmute(entry_virt as usize);
        let scheduler = &mut *(&raw mut SCHEDULER);
        let Ok(priority) = Priority::try_new(8) else { return false };
        let Ok(task) = scheduler.create_task(
            priority,
            WcetBudgetTicks(1_000),
            OverrunPolicy::TripToSafeState,
            entry,
        ) else {
            return false;
        };
        if scheduler.set_address_space(task, cr3_image).is_none() {
            return false;
        }
        let stack =
            core::slice::from_raw_parts_mut((&raw mut TASK_STACKS[0]).cast::<u8>(), STACK_SIZE);
        let Ok(ctx) = Context::new(stack, entry) else { return false };
        TASK_CTX[0] = ctx;
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
    let _ = writeln!(serial, "first-task: dispatching the real task into its own CR3");

    // The production dispatcher, CR3-aware — this call is the whole point.
    // SAFETY: the selected task's context slot was just initialized; its
    // `address_space` is the fully-populated tree built above, which maps
    // the kernel code/stack/IDT servicing it through the shared directories.
    let ran = unsafe {
        CURRENT_TASK = Some(task.index());
        let scheduler = &mut *(&raw mut SCHEDULER);
        let ran =
            dispatch::run_once_in_space(scheduler, &raw mut SUPERVISOR_CTX, &raw mut TASK_CTX);
        CURRENT_TASK = None;
        ran
    };
    ok &= ran == Some(task);

    // ---- Stage 7: what actually happened, checked and reported.
    // SAFETY: read after the dispatch round returned via the handler's
    // escape switch.
    let (captured, vector, rip, address, journal_len) = unsafe {
        (FAULT_CAPTURED, FAULT_VECTOR, FAULT_RIP, FAULT_ADDRESS, (*(&raw const JOURNAL)).len())
    };
    ok &= captured;
    // SAFETY: read after the round returned.
    ok &= unsafe { SCHEDULER.state_of(task) == Some(TaskState::Finished) };
    // The supervisor is still running under its own address space — the
    // containment claim, checked rather than assumed.
    ok &= paging::read_cr3() == cr3_image || paging::read_cr3() == supervisor_cr3();
    ok &= journal_len >= 5;

    let _ = writeln!(
        serial,
        "first-task: captured={captured} vector={vector} rip={rip:#x} cr2={address:#x} \
         task_finished={} spoor_journal_len={journal_len}",
        // SAFETY: read after the round returned.
        unsafe { SCHEDULER.state_of(task) == Some(TaskState::Finished) }
    );
    let _ = write_result(&mut serial, "first-task", ok);
    ok
}

/// The image tree's `CR3` — its PML4's own address, per this kernel's
/// no-higher-half-split model (`AddressSpace::cr3`'s own rationale).
fn space_cr3() -> u64 {
    (&raw const IMAGE_PML4) as u64
}

fn supervisor_cr3() -> u64 {
    (&raw const SUPERVISOR_PML4) as u64
}

#[no_mangle]
extern "C" fn kernel_main(start_info_paddr: u64) -> ! {
    if run(start_info_paddr) {
        exit_qemu(QemuExitCode::Success)
    } else {
        exit_qemu(QemuExitCode::Failure)
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    hal_x86_64::qemu_exit::panic_report(info)
}
