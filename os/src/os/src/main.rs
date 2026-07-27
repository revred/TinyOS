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
//!    under its own `CR3`, in a **preemptive** dispatch loop that runs with
//!    `IF` clear and charges every tick to whoever was on the CPU.
//! 6. Contains whatever happens, and journals every step as spoors.
//!
//! **What `STORY-P1-04-03` changed, and why it was a Story.** Until it, this
//! binary installed no [`hal_x86_64::interrupts::TickHook`]. `FEAT-P1-04`
//! had two Verified Stories — timer-driven preemption and WCET budget
//! enforcement — and *neither of them ran here*. The image ticked, counted
//! the tick, signalled end-of-interrupt and did nothing with it: dispatch was
//! cooperative, no budget was ever charged, and a workload that never yielded
//! would have kept the CPU until the machine was reset. That was `LE-20`, and
//! it was allowed to sit for two Features.
//!
//! Three things make installing the hook here a different job from installing
//! it in a fixture, and all three are load-bearing rather than tidy:
//!
//! - **The dispatch loop must run with `IF` clear.** It holds `&mut Scheduler`
//!   while it selects and switches, and the hook reads the same scheduler from
//!   an interrupt; those cannot both be true at once. The rule
//!   [`kernel::preempt`] documents — interrupts are enabled only while a task
//!   runs, re-enabled by each task's own saved `RFLAGS` across the switch into
//!   it — is what makes this sound, and [`boot`] reads the flag back on every
//!   round rather than asserting it in a comment.
//! - **This binary dispatches through [`dispatch::run_once_in_space`]**, which
//!   installs the selected task's `CR3` and does **not** restore the caller's
//!   on the way back. A preempting or enforcing tick returns control *inside*
//!   that function, so without the reinstatement [`boot`] now performs after
//!   every round, the supervisor would go on to select the next task with a
//!   task's address space live. The `FEAT-P1-04` fixtures could not have
//!   caught this: they use `run_once`, which touches no `CR3` at all.
//! - **The workload's overrun declaration had to become a decision.** See
//!   [`WORKLOAD_OVERRUN_POLICY`].
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
use hal_x86_64::interrupts;
use hal_x86_64::paging::{self, FrameAllocator, PageTable};
use hal_x86_64::qemu_exit::{exit_qemu, QemuExitCode};
use hal_x86_64::rflags;
use hal_x86_64::serial::SerialPort;
use kernel::context::{self, Context};
use kernel::dispatch;
use kernel::fault::{Disposition, FaultReport, FaultingContext};
use kernel::mem::Pool;
use kernel::preempt::{self, TickOutcome};
use kernel::sched::{
    OverrunPolicy, Priority, Scheduler, TaskEntry, TaskId, TaskState, WcetBudgetTicks, PRIORITY_MIN,
};
use kernel::spoor::{Action, Actor, Category, Outcome, Spoor};
use kernel::spoor_journal::SpoorJournal;
use kernel::wcet::{self, OverrunDisposition, TickAccounting};

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

/// The workload's static scheduling priority.
const WORKLOAD_PRIORITY: u8 = 8;

/// The workload's declared WCET budget, in ticks (`STORY-P1-04-03`).
///
/// The embedded workload's legitimate execution is a handful of
/// instructions — two indirect calls and a store — so this is roughly four
/// orders of magnitude more time than it can possibly need. It is chosen to
/// be *generously* above anything the workload legitimately does and still
/// small enough that a workload which never yields is caught in bounded
/// time, rather than tuned to any run. A budget the workload could plausibly
/// approach would make an enforcement a scheduling artefact instead of a
/// statement about the workload.
const WORKLOAD_BUDGET_TICKS: u32 = 8;

/// The floor the workload degrades to on overrun.
const WORKLOAD_DEGRADE_FLOOR: u8 = PRIORITY_MIN;

/// What the workload declared should happen if it exceeds its budget
/// (`STORY-P1-04-03`, `TEST-P1-04-03-A` clause 5).
///
/// **This was `TripToSafeState` and that was wrong**, not merely
/// unconsidered. `TripToSafeState` means a contained, capability-mediated
/// application that does nothing worse than burn CPU may halt the entire
/// system — a *strictly more severe* consequence than this same system gives
/// this same task for a genuine CPU fault, where [`Disposition::of`] answers
/// [`Disposition::TerminateTask`] and reserves [`Disposition::HaltSystem`]
/// for the kernel's own context. It also hands the workload precisely the
/// denial of service that `PD-07`/`PD-08` temporal isolation and `BND-15`
/// exist to deny it: a busy loop would have been an application's route to
/// stopping the machine.
///
/// [`OverrunPolicy::Degrade`] to [`PRIORITY_MIN`] is the containment-consistent
/// answer. The offender keeps running — losing the CPU is a budget
/// consequence, not a death sentence — but at the bottom of the priority
/// space, where it can preempt nothing and starve nothing.
///
/// `TripToSafeState` stays the right declaration for a task whose *failure*
/// is a system-level event. This workload is not one, and that distinction is
/// the entire reason the policy is per-task rather than global.
const WORKLOAD_OVERRUN_POLICY: OverrunPolicy = match Priority::try_new(WORKLOAD_DEGRADE_FLOOR) {
    Ok(floor) => OverrunPolicy::Degrade(floor),
    // Unreachable: `PRIORITY_MIN` is in range by definition. Stated as a
    // fail-closed arm rather than an `unwrap` so this stays a `const`.
    Err(_) => OverrunPolicy::TripToSafeState,
};

/// `TEST-P1-04-03-A` clause 7's bound, fixed in that document before this
/// code existed: enforcement must land no later than the workload's
/// `WORKLOAD_BUDGET_TICKS + MAX_TICKS_TO_ENFORCE`-th attributed tick.
const MAX_TICKS_TO_ENFORCE: u32 = 1;

/// Bound on dispatch rounds (`TEST-P1-04-03-A` clause 7).
///
/// **This is not a scheduling policy.** It is a property of a boot path that
/// carries exactly one embedded workload and has no idle task to fall back
/// on: a workload that never terminates would otherwise leave this loop
/// spinning until the harness killed it, which reports nothing. Reaching the
/// bound is recorded and reported, never silently swallowed. A boot path that
/// gains a second task or an idle task should revisit it.
const MAX_DISPATCH_ROUNDS: u32 = 4;

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

/// The workload this image carries.
///
/// **The only thing the `fixture-os-runaway` feature changes is which four
/// pages are embedded here.** The hook, the dispatch loop, the declared
/// policy, the address-space handling and every assertion path below are the
/// same compiled code in both builds, and the two images are asserted (in
/// `xtask`'s host tests) to differ only in their `.text` section. That is
/// what makes a Tier 0 run against the runaway image evidence about the
/// binary that ships rather than about a scenario assembled to pass.
#[cfg(not(feature = "fixture-os-runaway"))]
static PROBE_IMAGE: AlignedImage =
    AlignedImage(*include_bytes!("../../exec/fixtures/capability-probe.txe"));
/// The same image with a two-byte self-jump for `.text`: software that will
/// not give up the CPU (`STORY-P1-04-03`, `TEST-P1-04-03-A` clause 7). It
/// imports the same two names and is admitted by the same capability gate —
/// it simply never calls them, never faults, and never yields.
#[cfg(feature = "fixture-os-runaway")]
static PROBE_IMAGE: AlignedImage =
    AlignedImage(*include_bytes!("../../exec/fixtures/runaway-probe.txe"));
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

/// Which task the dispatch loop last switched into, or `None` while the
/// supervisor itself is running.
///
/// Written **only** by the dispatch loop, and only with interrupts disabled.
/// The tick hook reads it *first*, before touching the scheduler at all —
/// which is simultaneously `wcet::attribute_tick`'s `Nobody` arm and the
/// precondition that makes it sound for the hook to form a `&mut Scheduler`
/// on the other branch. It has existed since `STORY-P1-03-03` for the fault
/// path's benefit; `STORY-P1-04-03` made it load-bearing for soundness too.
static mut CURRENT_TASK: Option<usize> = None;
/// Each task's entry point, kept so the tick hook can do the caller's half of
/// an `OverrunDisposition::RestartTask` — rewinding the instruction pointer
/// needs a stack and an entry point, and `kernel::wcet` owns neither.
static mut TASK_ENTRY: [Option<TaskEntry>; TASKS] = [None; TASKS];
static mut FAULT_ADDRESS: u64 = 0;
static mut FAULT_CAPTURED: bool = false;

/// The hook's own count of the ticks it attributed, per slot, kept entirely
/// independently of the scheduler's books so the two can be compared
/// (`TEST-P1-04-03-A` clause 8).
static mut TICKS_ATTRIBUTED: [u32; TASKS] = [0; TASKS];
/// Ticks that landed while no task was on the CPU. Nonzero is the falsifiable
/// form of "the hook is installed and running on the real boot path": a build
/// that never calls `set_tick_hook` reports 0 because nothing counts.
static mut TICKS_UNATTRIBUTED: u32 = 0;
/// A tick attributed to a task the scheduler does not know. Must stay 0.
static mut TICKS_UNKNOWN: u32 = 0;
static mut ENFORCEMENTS: u32 = 0;
/// The offender's attributed-tick count when enforcement first fired.
static mut TICKS_AT_FIRST_ENFORCE: u32 = 0;
static mut FIRST_ENFORCE_TICK: u32 = 0;
/// The attributed-tick count at the previous enforcement, so the spacing
/// between enforcements can be checked. See [`ENFORCEMENT_SPACING_OK`].
static mut LAST_ENFORCE_ATTRIBUTED: u32 = 0;
/// Every enforcement must be a full `budget + 1` attributed ticks after the
/// last. This is the only externally visible consequence of the kernel having
/// reset the budget window, and `TEST-P1-04-02-A`'s falsification proved it is
/// the one assertion a fixture's own machinery cannot fake: a task whose
/// window is never reset is over budget on every subsequent tick, so
/// enforcements pile up one per tick instead of one per window.
static mut ENFORCEMENT_SPACING_OK: bool = true;
/// The workload's live priority read back out of the scheduler immediately
/// after the first enforcement — the declared consequence, observed rather
/// than assumed. Nothing else in this binary writes a priority.
static mut PRIORITY_AFTER_FIRST_ENFORCE: Option<u8> = None;
/// Set if any enforcement ever named a task that was not the running one.
static mut ENFORCED_WRONG_TASK: bool = false;
/// Set if any enforcement produced a disposition other than the one the
/// declared policy maps to.
static mut WRONG_DISPOSITION: bool = false;
static mut PREEMPTIONS: u32 = 0;
/// Cleared if the dispatch loop ever observed `IF` set on entry to a round.
static mut DISPATCHER_IF_CLEAR: bool = true;
/// `CR3` read back after the last dispatch round returned and the supervisor
/// tree was reinstalled (`TEST-P1-04-03-A` clause 4).
static mut CR3_AFTER_ROUND: u64 = 0;

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

/// Enters the system's declared safe state after a `TripToSafeState`
/// overrun, reporting before it stops.
///
/// A safe state that returned to the dispatcher would not be a safe state, so
/// this does not return. At Tier 0 the declared safe state is a reported,
/// fail-closed stop — the same thing [`Disposition::HaltSystem`] already does
/// on the fault path, and what a *real* safe state is (a limp-home mode, a
/// watchdog reset) is a deployment question this kernel does not answer.
///
/// Unreachable under this image's own declaration, which is
/// [`OverrunPolicy::Degrade`]. It exists because a hook that handles only the
/// arm its current workload happens to declare is a hook that breaks the first
/// time the declaration changes.
fn enter_safe_state(task: TaskId, ticks: u32, tick: u32) -> ! {
    // SAFETY: this function never returns, so re-initializing COM1 here
    // cannot race any other user of it on this single-CPU path.
    let mut serial = unsafe { SerialPort::init() };
    // SAFETY: a task is running, so the dispatch loop holds no borrow, and
    // the hook's own accounting borrow was dropped before this call.
    let finished = unsafe { (*(&raw const SCHEDULER)).state_of(task) == Some(TaskState::Finished) };
    let _ = writeln!(
        serial,
        "tinyos: task {} exceeded its declared WCET budget — attributed_ticks={ticks} \
         budget={WORKLOAD_BUDGET_TICKS} tick={tick} task_finished={finished}; entering the \
         declared safe state (fail-closed stop)",
        task.index()
    );
    exit_qemu(QemuExitCode::Failure)
}

/// The system's timer-tick consumer (`STORY-P1-04-03`): charges the tick to
/// whoever was on the CPU, applies whatever consequence that task declared if
/// it crossed its budget, and then takes the preemption decision.
///
/// Runs in interrupt context, on the interrupted task's own stack, with `IF`
/// clear. Bounded and allocation-free throughout, per
/// `agent/CODING_STANDARDS.md`'s RT rules: two `O(TASKS)` walks of a
/// fixed-capacity pool, a handful of counter updates, and on the enforcement
/// and preemption arms one register swap.
///
/// **The first thing it does is check [`CURRENT_TASK`], before touching the
/// scheduler at all.** The dispatch loop legitimately holds a `&mut Scheduler`
/// and runs with `IF` clear, but a tick already in flight when it cleared the
/// flag can still land here. That single check is what makes that harmless,
/// and it is simultaneously `wcet::attribute_tick`'s `Nobody` arm — kernel
/// time belongs to no task's budget. One rule, two jobs.
extern "C" fn tinyos_tick_entry() {
    // SAFETY: single-CPU path; `CURRENT_TASK` is written only by the dispatch
    // loop, with interrupts disabled.
    let Some(slot) = (unsafe { CURRENT_TASK }) else {
        // SAFETY: as above.
        unsafe { TICKS_UNATTRIBUTED += 1 };
        return;
    };

    let tick = interrupts::tick_count();

    // SAFETY: a task is running, so the dispatch loop holds no borrow; this is
    // the only code touching the scheduler for the duration. The `&mut` is
    // formed and dropped inside this block, before any switch is taken.
    let enforced = unsafe {
        let scheduler = &mut *(&raw mut SCHEDULER);
        let journal = &mut *(&raw mut JOURNAL);
        let running = task_id(scheduler, slot);

        match wcet::account_tick(scheduler, journal, running) {
            // Unreachable: `slot` is `Some`, so `running` is `Some` for any
            // live task. Counted rather than ignored.
            TickAccounting::Unattributed => {
                TICKS_UNATTRIBUTED += 1;
                None
            }
            TickAccounting::UnknownTask => {
                TICKS_UNKNOWN += 1;
                None
            }
            TickAccounting::WithinBudget(task) => {
                TICKS_ATTRIBUTED[task.index()] += 1;
                None
            }
            TickAccounting::Enforced { task, disposition } => {
                TICKS_ATTRIBUTED[task.index()] += 1;
                ENFORCEMENTS += 1;
                if task.index() != slot {
                    ENFORCED_WRONG_TASK = true;
                }
                // Checked against the *declaration*, not against whatever came
                // back — otherwise this would assert only that the enumeration
                // round-trips.
                let as_declared = matches!(
                    (WORKLOAD_OVERRUN_POLICY, disposition),
                    (OverrunPolicy::Restart, OverrunDisposition::RestartTask)
                        | (OverrunPolicy::TripToSafeState, OverrunDisposition::TripToSafeState)
                ) || matches!(
                    (WORKLOAD_OVERRUN_POLICY, disposition),
                    (OverrunPolicy::Degrade(declared), OverrunDisposition::DegradeTo(applied))
                        if declared == applied
                );
                if !as_declared {
                    WRONG_DISPOSITION = true;
                }
                let attributed = TICKS_ATTRIBUTED[task.index()];
                if attributed - LAST_ENFORCE_ATTRIBUTED != WORKLOAD_BUDGET_TICKS + 1 {
                    ENFORCEMENT_SPACING_OK = false;
                }
                LAST_ENFORCE_ATTRIBUTED = attributed;
                if ENFORCEMENTS == 1 {
                    TICKS_AT_FIRST_ENFORCE = attributed;
                    FIRST_ENFORCE_TICK = tick;
                    // Read back *after* the disposition was applied: this is
                    // the declared consequence observed, not predicted.
                    PRIORITY_AFTER_FIRST_ENFORCE =
                        scheduler.live_priority_of(task).map(|priority| priority.value());
                }
                Some((task, disposition))
            }
        }
    };

    if let Some((task, disposition)) = enforced {
        // SAFETY: `slot` is the task this interrupt is executing on, so
        // `TASK_CTX[slot]` is its own; `SUPERVISOR_CTX` is suspended at the
        // dispatch loop's own `run_once_in_space` call site. The scheduler
        // borrow above has been dropped.
        unsafe {
            match disposition {
                // The caller's half of a restart: rewind the instruction
                // pointer. `wcet::account_tick` has already reset the budget
                // window and returned the task to `Ready`; only this part
                // needs a stack and an entry point. Building a fresh `Context`
                // over a stack that at this moment still holds the suspended
                // interrupt frame is sound because nothing ever resumes that
                // frame — the switch below abandons it, and the task is next
                // entered through the fresh context, whose `RSP` is the top of
                // the same stack.
                OverrunDisposition::RestartTask => {
                    let stack = core::slice::from_raw_parts_mut(
                        (&raw mut TASK_STACKS[slot]).cast::<u8>(),
                        STACK_SIZE,
                    );
                    match TASK_ENTRY[slot].and_then(|entry| Context::new(stack, entry).ok()) {
                        Some(fresh) => TASK_CTX[slot] = fresh,
                        // Fail closed: a task whose context could not be
                        // rewound must not be selected again with a stale one.
                        None => {
                            (*(&raw mut SCHEDULER)).set_state(task, TaskState::Finished);
                        }
                    }
                    CURRENT_TASK = None;
                    context::switch(&raw mut ABANDONED_CTX, &raw mut SUPERVISOR_CTX);
                    unreachable!("a rewound or retired task is never switched back into")
                }
                // The priority is already lowered and the window already
                // reset; nothing further is required of the caller. Leaving
                // the CPU here is what makes the consequence take effect
                // immediately — the very next selection is taken under the
                // new priority, which is what a degrade is *for*.
                OverrunDisposition::DegradeTo(_) => {
                    CURRENT_TASK = None;
                    context::switch(&raw mut TASK_CTX[slot], &raw mut SUPERVISOR_CTX);
                }
                OverrunDisposition::TripToSafeState => {
                    enter_safe_state(task, TICKS_AT_FIRST_ENFORCE, tick)
                }
            }
        }
    }

    // The preemption decision (`STORY-P1-04-01`). Enforcement and preemption
    // share this hook and neither may perturb the other.
    //
    // SAFETY: as above; the scheduler borrow is dropped, and `CURRENT_TASK` is
    // re-read because an enforcement arm above may have cleared it.
    let outcome = unsafe {
        let Some(slot) = CURRENT_TASK else { return };
        let running = task_id(&*(&raw const SCHEDULER), slot);
        preempt::on_timer_tick(
            &raw mut SCHEDULER,
            running,
            &raw mut TASK_CTX[slot],
            &raw mut SUPERVISOR_CTX,
        )
    };
    if matches!(outcome, TickOutcome::Preempt(_)) {
        // SAFETY: reached only after the preempted task is resumed.
        unsafe { PREEMPTIONS += 1 };
    }
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
    // `STORY-P1-04-03`, closing `LE-20`: the shipping image gets a tick
    // consumer. Registered **before** `init` arms the timer, so no tick can
    // be delivered into a window where the hook is not yet installed.
    //
    // SAFETY: `tinyos_tick_entry` is bounded, allocation-free, does not
    // re-enter `hal_x86_64::interrupts`, and leaves the interrupt frame the
    // ISR stub is standing on intact on every path that returns — see its own
    // doc comment for the re-entrancy argument.
    unsafe { interrupts::set_tick_hook(tinyos_tick_entry) };
    // SAFETY: called exactly once, before anything here depends on
    // interrupts being armed — `init`'s own documented contract.
    unsafe { interrupts::init(BOOT_TIMER_INITIAL_COUNT) };
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
        let entry: TaskEntry = core::mem::transmute(entry_virt as usize);
        let scheduler = &mut *(&raw mut SCHEDULER);
        let Ok(priority) = Priority::try_new(WORKLOAD_PRIORITY) else { return false };
        let Ok(task) = scheduler.create_task(
            priority,
            WcetBudgetTicks(WORKLOAD_BUDGET_TICKS),
            WORKLOAD_OVERRUN_POLICY,
            entry,
        ) else {
            return false;
        };
        if scheduler.set_address_space(task, cr3_image).is_none() {
            return false;
        }
        let stack =
            core::slice::from_raw_parts_mut((&raw mut TASK_STACKS[0]).cast::<u8>(), STACK_SIZE);
        let Ok(context) = Context::new(stack, entry) else { return false };
        TASK_CTX[0] = context;
        // Kept so the tick hook can rewind this task to its entry point if a
        // `RestartTask` disposition is ever taken against it.
        TASK_ENTRY[task.index()] = Some(entry);
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

    // From here the supervisor runs with `IF` clear and never re-enables it:
    // each task's own saved `RFLAGS` is what turns interrupts back on across
    // the switch into it, and turns them off again across the switch back.
    // That, and not a convention, is what stops the tick hook ever observing a
    // scheduler this loop is mid-mutation of — see `kernel::preempt`'s module
    // doc for the whole argument.
    //
    // SAFETY: every subsequent re-enable happens via a context switch's own
    // `popfq`, so no interrupt is lost.
    let _ = unsafe { interrupts::disable_interrupts() };

    let mut rounds: u32 = 0;
    let mut rounds_exhausted = false;
    let mut last_ran = None;
    loop {
        // SAFETY: interrupts are masked, so this is the only code touching the
        // scheduler; each `TASK_CTX` slot is owned by exactly one task, and the
        // selected task's address space maps the kernel code/stack/IDT
        // servicing it through the shared directories.
        let ran = unsafe {
            let scheduler = &mut *(&raw mut SCHEDULER);
            // The same selection `run_once_in_space` is about to make, read
            // ahead of it purely so `CURRENT_TASK` can be set before the switch.
            let Some(next) = scheduler.highest_priority_ready() else {
                break;
            };
            // Clause 3, read rather than asserted: `disable_interrupts` returns
            // the `RFLAGS` that were live before it, and is a no-op when they
            // were already masked — so this observes the actual flag on the
            // actual path for the cost of one `pushfq` per round.
            let saved = interrupts::disable_interrupts();
            if rflags::interrupts_enabled(saved) {
                DISPATCHER_IF_CLEAR = false;
            }
            CURRENT_TASK = Some(next.index());
            let ran =
                dispatch::run_once_in_space(scheduler, &raw mut SUPERVISOR_CTX, &raw mut TASK_CTX);
            CURRENT_TASK = None;
            // Clause 4, and the one part of the fixture pattern that does not
            // transfer. `run_once_in_space` installed the task's `CR3` and does
            // not restore this one's on the way back — and control returns here
            // from a preempting or enforcing tick just as readily as from a
            // cooperative yield. Without this, the next selection would be made
            // with a task's address space live.
            paging::write_cr3(&raw const SUPERVISOR_PML4 as u64);
            CR3_AFTER_ROUND = paging::read_cr3();
            ran
        };
        if ran.is_none() {
            break;
        }
        last_ran = ran;
        rounds += 1;
        if rounds >= MAX_DISPATCH_ROUNDS {
            // Recorded and reported, never silently swallowed: for a workload
            // that never terminates this is the correct outcome, and for one
            // that should have terminated it is the diagnosis.
            rounds_exhausted = true;
            break;
        }
    }
    ok &= last_ran == Some(task);

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

    // SAFETY: read after the round returned via the handler's escape switch.
    let (captured, trap_address, finished, journal_len) = unsafe {
        (
            FAULT_CAPTURED,
            FAULT_ADDRESS,
            SCHEDULER.state_of(task) == Some(TaskState::Finished),
            (*(&raw const JOURNAL)).len(),
        )
    };

    // ---- Stage 6: what the scheduler did about it (`STORY-P1-04-03`).
    //
    // SAFETY: read with interrupts masked and every switch returned; nothing
    // else can be executing on this single-CPU path.
    let (
        attributed,
        unattributed,
        unknown,
        enforcements,
        ticks_at_first,
        first_enforce_tick,
        spacing_ok,
        priority_after,
        wrong_task,
        wrong_disposition,
        preemptions,
        if_clear,
        cr3_after,
    ) = unsafe {
        (
            TICKS_ATTRIBUTED,
            TICKS_UNATTRIBUTED,
            TICKS_UNKNOWN,
            ENFORCEMENTS,
            TICKS_AT_FIRST_ENFORCE,
            FIRST_ENFORCE_TICK,
            ENFORCEMENT_SPACING_OK,
            PRIORITY_AFTER_FIRST_ENFORCE,
            ENFORCED_WRONG_TASK,
            WRONG_DISPOSITION,
            PREEMPTIONS,
            DISPATCHER_IF_CLEAR,
            CR3_AFTER_ROUND,
        )
    };

    // Clause 8: the hook's own count and the scheduler's books agree, task by
    // task, once the budget-window resets the kernel itself performed are
    // accounted for. One formula covers both workloads: with no enforcement it
    // reduces to plain equality, and with enforcement it is the same statement
    // the spacing check makes, read off the scheduler instead of the hook.
    //
    // SAFETY: as above.
    let books_agree = unsafe {
        let scheduler = &mut *(&raw mut SCHEDULER);
        let consumed = scheduler.wcet_state(task).map_or(u32::MAX, |(consumed, _)| consumed);
        consumed + enforcements * (WORKLOAD_BUDGET_TICKS + 1) == attributed[task.index()]
    };

    // Clause 1: the hook is installed on the real boot path, read back from
    // the register it was stored in rather than inferred from how many ticks
    // this boot happened to be slow enough to service. A build that never
    // called `set_tick_hook` reports `false` here, deterministically, on any
    // host — which is what makes this an assertion rather than a race.
    let hook_installed = interrupts::tick_hook_installed();
    // How many ticks the hook actually serviced. Reported in both arms and
    // asserted only where it is guaranteed (the runaway arm, below): a boot
    // fast enough to finish inside one tick period legitimately services
    // none, and gating on that would be gating on the host's speed.
    let ticks_serviced = unattributed + attributed.iter().sum::<u32>();

    ok &= hook_installed;
    ok &= books_agree;
    ok &= unknown == 0;
    ok &= !wrong_task;
    ok &= !wrong_disposition;
    // Clause 3 and clause 4.
    ok &= if_clear;
    ok &= cr3_after == (&raw const SUPERVISOR_PML4) as u64;

    let _ = writeln!(
        serial,
        "tinyos: scheduler hook_installed={hook_installed} if_clear={if_clear} \
         cr3_after_round={cr3_after:#x} rounds={rounds} rounds_exhausted={rounds_exhausted} \
         preemptions={preemptions} ticks_serviced={ticks_serviced}"
    );
    let _ = writeln!(
        serial,
        "tinyos: budget ticks_attributed={attributed:?} unattributed={unattributed} \
         unknown={unknown} books_agree={books_agree} budget={WORKLOAD_BUDGET_TICKS} \
         policy=degrade_to_{WORKLOAD_DEGRADE_FLOOR}"
    );
    let _ = writeln!(
        serial,
        "tinyos: enforcements={enforcements} ticks_at_first_enforce={ticks_at_first} \
         (bound {}) first_enforce_tick={first_enforce_tick} spacing_ok={spacing_ok} \
         priority_after_enforce={priority_after:?} wrong_task={wrong_task} \
         wrong_disposition={wrong_disposition}",
        WORKLOAD_BUDGET_TICKS + MAX_TICKS_TO_ENFORCE
    );
    let _ = writeln!(
        serial,
        "tinyos: workload returned {reported:#x} (GetCurrentProcess ok={call_succeeded}), \
         exited via trap {trap_address:#x}, task_finished={finished}, spoors={journal_len}"
    );

    // The two arms diverge only in what the *workload* is expected to have
    // done — never in the mechanism under test, which is identical code.
    #[cfg(not(feature = "fixture-os-runaway"))]
    {
        // `ExitProcess` has no process-teardown path yet, so it routes into
        // the capability trap — the documented, contained way this shim ends a
        // task today (`iat::trampolines::exit_process`). The task is therefore
        // expected to finish via containment at exactly that address.
        ok &= call_succeeded;
        ok &= captured && finished && trap_address == iat::CAPABILITY_TRAP_VIRT;
        ok &= journal_len >= 5;
        // Clause 6: this workload executes a handful of instructions, so it
        // must not come anywhere near its budget. Asserted deliberately rather
        // than left unstated — a shipping run in which the shipping workload
        // tripped its own declared budget would be a defect, not a curiosity.
        ok &= enforcements == 0;
        ok &= !rounds_exhausted;
    }
    #[cfg(feature = "fixture-os-runaway")]
    {
        // Clause 7. The workload never calls, never faults and never yields,
        // so none of the shipping run's outcomes apply — what must hold is
        // that the shipping image's own hook caught it and applied the
        // declared consequence.
        ok &= enforcements >= 1;
        // A workload that runs until the loop bound necessarily services many
        // ticks, so here the count is guaranteed and is asserted.
        ok &= ticks_serviced >= 1;
        // Detection on the exact tick that could first have crossed the
        // budget: no earlier (the budget was not honoured) and no later than
        // the bound `TEST-P1-04-03-A` fixed before this code existed.
        ok &= ticks_at_first > WORKLOAD_BUDGET_TICKS;
        ok &= ticks_at_first <= WORKLOAD_BUDGET_TICKS + MAX_TICKS_TO_ENFORCE;
        // The declared consequence, read back out of the scheduler.
        ok &= priority_after == Some(WORKLOAD_DEGRADE_FLOOR);
        // And the kernel really did reset the budget window each time.
        ok &= spacing_ok;
        // A workload that never terminates leaves through the loop's own
        // bound. That is the correct outcome here, and its absence would mean
        // the runaway image stopped running for some reason of its own.
        ok &= rounds_exhausted;
        ok &= !captured;
    }

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
fn panic(info: &core::panic::PanicInfo) -> ! {
    hal_x86_64::qemu_exit::panic_report(info)
}
