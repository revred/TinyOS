//! `TEST-P1-02-02-A`'s Tier 0 escalating-fault fixture: destroy the kernel
//! stack for real, and prove the `#DF` that follows lands on the IST stack and
//! reports, instead of triple-faulting the machine.
//!
//! **The escalation is genuine, not simulated.** A scheduled task points `RSP`
//! at a canonical-but-unmapped address and pushes. That raises `#PF` — and the
//! CPU cannot deliver `#PF` either, because delivering it means pushing a
//! six-quadword interrupt frame onto the same broken `RSP`. A fault during
//! fault delivery is, by definition, the double fault. Without an IST there is
//! nowhere to push *that* frame either, and the chain ends in a triple fault:
//! QEMU resets and never reaches the `isa-debug-exit` port, so the harness sees
//! no verdict at all. That is the exact behavior this fixture was run against
//! before the IST existed, and it is recorded in `REPORT-2026-07-27-06` rather
//! than assumed.
//!
//! **What "it worked" has to mean here.** A handler that merely produced output
//! would look identical whether or not the IST did anything — the stack might
//! simply have happened to still work. So the pass condition is stronger: the
//! handler checks that its *own* stack pointer lies inside the reserved `#DF`
//! stack (`hal_x86_64::tss::double_fault_stack_contains`), and that the frame's
//! saved `RSP` is the destroyed address it was told to expect. Those two facts
//! together say the fault really came from the broken stack and the handler
//! really is running somewhere else.
//!
//! **Terminal by design.** There is no containment arm here and no return. A
//! double fault means `STORY-P1-02-01`'s primary fault path failed while the
//! CPU was delivering a fault; the honest action is to attribute it, audit it,
//! say so, and stop. See `kernel::fault::audit_double_fault` for why no
//! disposition type exists.
//!
//! Only reachable when the `fixture-double-fault` feature is enabled — never
//! part of a real boot image.

// See `fixture_fault`'s identical allow: the `&mut *(&raw mut STATIC)` form is
// this workspace's single-owner `static mut` pattern, and clippy's suggested
// simplification is the `static_mut_refs` it exists to avoid.
#![allow(static_mut_refs, clippy::deref_addrof)]

use core::fmt::Write;
use hal_x86_64::fault::FaultFrame;
use hal_x86_64::qemu_exit::{exit_qemu, QemuExitCode};
use hal_x86_64::serial::SerialPort;
use kernel::fault::{audit_double_fault, FaultingContext};
use kernel::measure::write_result;
use kernel::sched::{OverrunPolicy, Priority, Scheduler, WcetBudgetTicks};
use kernel::spoor_journal::SpoorJournal;

/// One task: the stack destroyer. Nothing survives a double fault, so there is
/// no survivor to schedule afterwards — that property belongs to
/// `STORY-P1-02-01`'s fixture, and claiming it here would be claiming
/// containment this Story explicitly does not provide.
const TASKS: usize = 1;

/// Per-task stack, matching `fixture_fault`'s. Never actually used to hold a
/// fault frame here: the whole point is that `RSP` no longer points at it by
/// the time anything faults.
const STACK_SIZE: usize = 8_192;

/// Journal capacity — two spoors for the one double fault, with headroom.
const JOURNAL: usize = 8;

/// The address the victim installs as its stack pointer.
///
/// The same canonical-but-unmapped 512 GiB `fixture_fault`'s `#PF` victim
/// reads, and for the same reason: `hal_x86_64::boot` maps the first GiB and
/// the fourth, so nothing is mapped here, while the address stays a legal
/// 48-bit canonical one. A **non**-canonical stack pointer would raise `#GP`
/// (or `#SS`) on the push rather than `#PF`, which is a different escalation
/// and would leave the page-fault-during-delivery path untested.
const DESTROYED_STACK: u64 = 0x0000_0080_0000_0000;

static mut SCHEDULER: Scheduler<TASKS> = Scheduler::new();
static mut JOURNAL_STORE: SpoorJournal<JOURNAL> = SpoorJournal::new();

static mut SUPERVISOR_CTX: kernel::context::Context = kernel::context::Context::zeroed();
static mut TASK_CTX: [kernel::context::Context; TASKS] =
    [kernel::context::Context::zeroed(); TASKS];
static mut TASK_STACKS: [[u8; STACK_SIZE]; TASKS] = [[0; STACK_SIZE]; TASKS];

/// Which task the supervisor switched into, or `None` for kernel context —
/// carried for **attribution only**, exactly as
/// `kernel::fault::audit_double_fault` documents. It does not and cannot change
/// what happens next.
static mut CURRENT_TASK: Option<usize> = None;

/// The victim: replaces its own stack pointer with an unmapped address and
/// pushes.
///
/// `options(nostack)` is not a claim that this touches no stack — it is a claim
/// that the *compiler* need not assume a valid red zone or stack around this
/// block, which is exactly right here, since the block's whole purpose is to
/// leave `RSP` invalid. Nothing after the `asm!` ever executes.
extern "C" fn victim_stack_destroyer() -> ! {
    // SAFETY: this deliberately destroys the current stack pointer and then
    // faults. It cannot corrupt anything, because the address it writes to is
    // mapped in no page table — the write never reaches memory. Control never
    // returns from here by any path: the escalation reaches `#DF`, whose
    // handler exits QEMU.
    unsafe {
        core::arch::asm!(
            "mov rsp, {destroyed}",
            "push rax",
            destroyed = in(reg) DESTROYED_STACK,
            options(nostack)
        )
    };
    unreachable!("pushing onto an unmapped stack always faults")
}

/// The kernel-side **double-fault** entry point the `hal_x86_64::fault` `#DF`
/// stub calls.
///
/// Reached with `RSP` already switched to the IST stack by hardware, before any
/// frame was pushed — that switch is what this whole Story buys, and the checks
/// below are what prove it happened rather than assuming it.
///
/// # Safety
/// Called only by `df_fault_stub`, with `frame` pointing at the [`FaultFrame`]
/// it just built on the IST stack. Runs with `IF` clear (interrupt gate).
#[no_mangle]
extern "C" fn tinyos_double_fault_entry(frame: *const FaultFrame) -> ! {
    // SAFETY: the stub passes a pointer to a fully-initialized `FaultFrame` on
    // the IST stack, live for this call.
    let frame = unsafe { *frame };

    // A local whose address is this handler's own stack. If the IST switch did
    // not happen, this lies wherever the broken stack was — and the fixture
    // fails rather than reporting a success it did not earn.
    let stack_probe = 0u64;
    let handler_stack_pointer = &stack_probe as *const u64 as u64;

    // SAFETY: `SerialPort::init` reprograms COM1's divisor and line control,
    // which is idempotent; single-CPU fixture with interrupts disabled.
    let mut serial = unsafe { SerialPort::init() };

    let (ist_bottom, ist_top) = hal_x86_64::tss::double_fault_stack_range();
    let on_ist_stack = hal_x86_64::tss::double_fault_stack_contains(handler_stack_pointer);
    // SAFETY: read back after `gdt::install`, single-CPU, no concurrent writer.
    let installed_top = unsafe { hal_x86_64::gdt::installed_double_fault_stack_top() };

    let _ = writeln!(
        serial,
        "fixture-double-fault captured #DF vector={} error_code={:#x} rip={:#x} faulting_rsp={:#x}",
        frame.vector, frame.error_code, frame.rip, frame.rsp
    );
    let _ = writeln!(
        serial,
        "fixture-double-fault handler_rsp={handler_stack_pointer:#x} ist_stack={ist_bottom:#x}..{ist_top:#x} on_ist_stack={on_ist_stack}"
    );

    // SAFETY: single-CPU fixture; this handler is the only code touching these
    // statics, and nothing else runs while it does.
    let (spoors, context) = unsafe {
        let context = match CURRENT_TASK.and_then(|slot| task_id(&*(&raw const SCHEDULER), slot)) {
            Some(task) => FaultingContext::Task(task),
            None => FaultingContext::Kernel,
        };
        let journal = &mut *(&raw mut JOURNAL_STORE);
        for spoor in audit_double_fault(context) {
            journal.append(spoor);
        }
        let spoors = journal
            .iter()
            .filter(|spoor| spoor.category() == kernel::spoor::Category::Fault)
            .count();
        (spoors, context)
    };

    let attributed = matches!(context, FaultingContext::Task(_));
    let mut ok = true;
    // The vector really is the double fault, not something that merely reached
    // this symbol.
    ok &= frame.vector == kernel::fault::DOUBLE_FAULT_VECTOR;
    // `#DF`'s hardware error code is architecturally always zero.
    ok &= frame.error_code == 0;
    // The escalation started on the stack this fixture destroyed, not somewhere
    // incidental. The saved `RSP` is the value at the faulting instruction,
    // before its push — i.e. the address the victim installed.
    ok &= frame.rsp == DESTROYED_STACK;
    // The load-bearing check: hardware switched stacks before pushing.
    ok &= on_ist_stack;
    ok &= installed_top == ist_top;
    ok &= spoors == 2;
    ok &= attributed;

    let _ = writeln!(
        serial,
        "fixture-double-fault double_fault_spoors={spoors} attributed_to_task={attributed} terminal=true"
    );
    let _ = write_result(&mut serial, "double-fault", ok);
    if ok {
        exit_qemu(QemuExitCode::Success)
    } else {
        exit_qemu(QemuExitCode::Failure)
    }
}

/// The primary (`#UD`/`#GP`/`#PF`) entry point, required because
/// `interrupts::init_faults_only` wires those three vectors in every build.
///
/// Reaching it here is a **fixture failure**, and a specific one worth naming:
/// it means the `#PF` this fixture provokes was delivered successfully, so the
/// escalation to `#DF` never happened and nothing about the IST was exercised.
/// A fixture that quietly passed in that case would be testing the primary path
/// while claiming to test the double-fault path.
///
/// # Safety
/// Called only by the fault stubs, per their own contract.
#[no_mangle]
extern "C" fn tinyos_fault_entry(frame: *const FaultFrame) -> ! {
    // SAFETY: as above — the stubs pass a live, fully-initialized frame.
    let frame = unsafe { *frame };
    // SAFETY: single-CPU fixture, no concurrent COM1 user, never returns.
    let mut serial = unsafe { SerialPort::init() };
    let _ = writeln!(
        serial,
        "fixture-double-fault primary fault vector={} reached the containment path: the escalation to #DF did not happen",
        frame.vector
    );
    let _ = write_result(&mut serial, "double-fault", false);
    exit_qemu(QemuExitCode::Failure)
}

/// Resolves a pool slot back to the live `TaskId` the scheduler handed out —
/// via `iter_tasks` rather than by fabricating one, for the reason
/// `sched::TaskId` gives: a fault path must not be able to invent an id for a
/// slot that was never created.
fn task_id(scheduler: &Scheduler<TASKS>, slot: usize) -> Option<kernel::sched::TaskId> {
    scheduler.iter_tasks().map(|(task, _)| task).find(|task| task.index() == slot)
}

/// Runs the fixture. Returns only on a setup failure — a successful run ends
/// inside [`tinyos_double_fault_entry`], which exits QEMU directly.
pub fn run() -> bool {
    // SAFETY: this fixture is the only code running; `init` is called once,
    // before any other `SerialPort` method.
    let mut serial = unsafe { SerialPort::init() };

    // SAFETY: called once, before any fault can occur, on the QEMU `q35`
    // hardware this module's constants assume. Faults only — an APIC timer tick
    // arriving mid-fixture would switch contexts under the very escalation
    // being tested. This is also what installs the GDT and TSS.
    unsafe { hal_x86_64::interrupts::init_faults_only() };

    // SAFETY: read back after `init_faults_only`; single-CPU, no other writer.
    let installed_top = unsafe { hal_x86_64::gdt::installed_double_fault_stack_top() };
    let (bottom, top) = hal_x86_64::tss::double_fault_stack_range();
    let _ = writeln!(
        serial,
        "fixture-double-fault ist stack {bottom:#x}..{top:#x} installed_top={installed_top:#x}"
    );
    if installed_top != top {
        let _ = write_result(&mut serial, "double-fault", false);
        return false;
    }

    // SAFETY: single-CPU fixture; slot 0 is this task's own context and stack,
    // and nothing else is running.
    unsafe {
        let scheduler = &mut *(&raw mut SCHEDULER);
        let Ok(priority) = Priority::try_new(8) else {
            return false;
        };
        let Ok(task) = scheduler.create_task(
            priority,
            WcetBudgetTicks(1_000),
            OverrunPolicy::TripToSafeState,
            victim_stack_destroyer,
        ) else {
            return false;
        };
        if task.index() != 0 {
            return false;
        }
        let stack =
            core::slice::from_raw_parts_mut((&raw mut TASK_STACKS[0]).cast::<u8>(), STACK_SIZE);
        let Ok(context) = kernel::context::Context::new(stack, victim_stack_destroyer) else {
            return false;
        };
        TASK_CTX[0] = context;
        CURRENT_TASK = Some(0);

        let _ =
            writeln!(serial, "fixture-double-fault switching into the stack destroyer (slot 0)");
        kernel::context::switch(&raw mut SUPERVISOR_CTX, &raw mut TASK_CTX[0]);
    }

    // Only reachable if the victim somehow returned without faulting, which
    // would mean the whole premise of this fixture is wrong.
    let _ = writeln!(serial, "fixture-double-fault victim returned without faulting");
    let _ = write_result(&mut serial, "double-fault", false);
    false
}
