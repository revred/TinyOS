//! `TEST-P1-02-01-A`'s Tier 0 fault-injection fixture: raise a real `#UD`,
//! `#GP` and `#PF` from inside scheduled tasks and prove each one terminates
//! **that task only**, with the rest of the system still scheduling
//! afterwards.
//!
//! The whole Story reduces to clause 4, and clause 4 cannot be established by
//! any host test: it needs a real CPU raising a real exception against a real
//! IDT, on target-compiled code. So the three faults here are raised by
//! genuine instructions — `ud2`, a load of an out-of-range segment selector, and
//! a read from a canonical-but-unmapped address — never by synthesizing a frame and
//! calling the handler directly, which would test the plumbing while assuming
//! away the part that goes wrong.
//!
//! **How a terminated task is left behind.** With no TSS/IST (that is
//! `STORY-P1-02-02`), a same-privilege fault runs the handler on the faulting
//! task's own stack. The handler therefore does not return: it marks the task
//! `Finished` and switches to the supervisor context, abandoning the victim's
//! stack and its half-executed frame. That is sound precisely *because* the
//! task never runs again — the abandoned frame belongs to a context nothing
//! will ever resume — and it is why there is no register save/restore in the
//! entry stubs (`hal_x86_64::fault`'s own doc comment).
//!
//! Only reachable when the `fixture-fault` feature is enabled — never part of
//! a real boot image.

// Every `&mut *(&raw mut STATIC)` below is this workspace's established
// single-owner pattern for a `static mut` that one caller owns for a whole
// kernel run — `hal_x86_64::interrupts::init` carries the identical allow for
// the identical reason. Clippy's suggested simplification is `&mut STATIC`,
// which is exactly the `static_mut_refs` the raw-pointer form exists to avoid,
// so taking the suggestion would make the code worse rather than better.
#![allow(static_mut_refs, clippy::deref_addrof)]

use core::fmt::Write;
use hal_x86_64::fault::FaultFrame;
use hal_x86_64::qemu_exit::{exit_qemu, QemuExitCode};
use hal_x86_64::serial::SerialPort;
use kernel::context::{self, Context};
use kernel::fault::{Disposition, FaultReport, FaultingContext};
use kernel::measure::write_result;
use kernel::sched::{OverrunPolicy, Priority, Scheduler, TaskState, WcetBudgetTicks};
use kernel::spoor_journal::SpoorJournal;

/// Scheduler capacity: three victims plus one survivor.
const TASKS: usize = 4;

/// Per-task stack. Generous: a victim's stack also has to hold the fault
/// frame the CPU pushes and the handler's own frame, since no IST stack
/// exists yet.
const STACK_SIZE: usize = 8_192;

/// Journal capacity — two spoors per fault, three faults, with headroom.
const JOURNAL: usize = 16;

static mut SCHEDULER: Scheduler<TASKS> = Scheduler::new();
static mut JOURNAL_STORE: SpoorJournal<JOURNAL> = SpoorJournal::new();

/// The supervisor context every terminated task's handler escapes back to.
static mut SUPERVISOR_CTX: Context = Context::zeroed();
/// Where the fault handler saves the dying task's registers. Written once per
/// fault and never read: `context::switch` requires somewhere to save, and a
/// context nothing will ever resume is the honest destination.
static mut ABANDONED_CTX: Context = Context::zeroed();
static mut TASK_CTX: [Context; TASKS] = [Context::zeroed(); TASKS];
static mut TASK_STACKS: [[u8; STACK_SIZE]; TASKS] = [[0; STACK_SIZE]; TASKS];

/// Which task the supervisor last switched into, or `None` when the kernel
/// itself is running. This is the *only* input the disposition policy reads
/// (`kernel::fault::Disposition::of`), and the fixture is what makes it true.
static mut CURRENT_TASK: Option<usize> = None;

/// How many faults were captured, and how many were contained to their task.
static mut CAPTURED: usize = 0;
static mut CONTAINED: usize = 0;

/// Incremented by the survivor task, to prove the scheduler still runs work
/// after three separate faults.
static mut SURVIVOR_RUNS: u64 = 0;

/// A victim that executes `ud2` — the architecturally-guaranteed invalid
/// opcode, vector 6.
extern "C" fn victim_invalid_opcode() -> ! {
    // SAFETY: `ud2` has no operands and no memory effect; raising `#UD` is the
    // entire purpose of this task, and the handler never returns here.
    unsafe { core::arch::asm!("ud2", options(nomem, nostack)) };
    unreachable!("ud2 always faults")
}

/// A segment selector far past the end of any GDT this kernel installs.
/// Loading it is an architecturally guaranteed `#GP` carrying that selector as
/// its error code.
///
/// Index 511 (`511 << 3`), and the distance is the point. This constant was
/// originally `0x18` — index 3, chosen *because* `hal_x86_64::boot`'s GDT held
/// exactly three descriptors. `STORY-P1-02-02` then added a fourth entry (the
/// TSS descriptor, which sits at exactly index 3), which would have turned this
/// victim from an out-of-range load into a TSS-selector load: still a `#GP`,
/// but for a different reason, silently. A selector chosen by counting the
/// descriptors that happen to exist today is a fixture that breaks the next
/// time the GDT grows; one chosen far past any plausible table is not.
///
/// The first attempt at this victim used a `wrmsr` to a reserved MSR, which
/// **did not fault under QEMU/TCG** — the write was silently accepted and the
/// task fell through to its own `unreachable!()`, exiting as a panic with no
/// fault captured at all. Recorded here because it is the more useful fact:
/// "architecturally must fault" and "faults under this emulator" are not the
/// same claim, and a Tier 0 fixture can only ever check the second.
const OUT_OF_RANGE_SELECTOR: u16 = 511 << 3;

/// A victim that loads an out-of-range segment selector — `#GP`, vector 13,
/// with a **non-zero** error code naming the offending selector.
extern "C" fn victim_general_protection() -> ! {
    // SAFETY: `OUT_OF_RANGE_SELECTOR` is past the GDT limit, so the load
    // raises `#GP` *before* `DS` is modified — the segment register is left
    // exactly as the kernel set it, and the handler never returns here.
    unsafe {
        core::arch::asm!(
            "mov ds, ax",
            in("ax") OUT_OF_RANGE_SELECTOR,
            options(nomem, nostack)
        )
    };
    unreachable!("loading a selector past the GDT limit always faults")
}

/// A canonical address that `hal_x86_64::boot` never maps: the boot page
/// tables cover the first GiB and the fourth (for the local APIC), so 512 GiB
/// is present in no table while remaining a legal 48-bit canonical address —
/// a **non**-canonical address would raise `#GP`, not `#PF`, and would test
/// the wrong vector.
const UNMAPPED_BUT_CANONICAL: u64 = 0x0000_0080_0000_0000;

/// A victim that reads unmapped memory — `#PF`, vector 14, with `CR2` holding
/// [`UNMAPPED_BUT_CANONICAL`].
extern "C" fn victim_page_fault() -> ! {
    // SAFETY: this read is *expected* to fault — that is the test. It cannot
    // corrupt anything, because no mapping exists at this address for it to
    // reach. The handler never returns here.
    unsafe {
        let value = core::ptr::read_volatile(UNMAPPED_BUT_CANONICAL as *const u64);
        core::hint::black_box(value);
    }
    unreachable!("a load from an unmapped page always faults")
}

/// The survivor: increments a counter and yields back, proving the scheduler
/// still dispatches real work after three faults.
extern "C" fn survivor() -> ! {
    loop {
        // SAFETY: single-CPU fixture; only this task writes `SURVIVOR_RUNS`,
        // and slot 3 is the only context it is ever switched into.
        unsafe {
            SURVIVOR_RUNS += 1;
            context::switch(&raw mut TASK_CTX[3], &raw mut SUPERVISOR_CTX);
        }
    }
}

/// The kernel-side fault entry point the `hal_x86_64::fault` stubs call.
///
/// `#[no_mangle] extern "C"` and never returning, exactly like `kernel_main`:
/// the HAL declares the symbol and the binary defines it, so `hal-x86_64` needs
/// no dependency on `kernel`'s policy (Dependency Inversion — the same seam
/// `kernel_main` itself uses).
///
/// # Safety
/// Called only by the fault stubs, with `frame` pointing at the [`FaultFrame`]
/// they just built on the faulting stack. Runs with `IF` clear (the IDT gates
/// are interrupt gates), so it cannot be re-entered by an interrupt.
#[no_mangle]
extern "C" fn tinyos_fault_entry(frame: *const FaultFrame) -> ! {
    // SAFETY: the stubs pass a pointer to a fully-initialized `FaultFrame` on
    // the current stack, live for this call.
    let frame = unsafe { *frame };

    // SAFETY: single-CPU fixture with interrupts disabled inside the handler;
    // this is the only code that touches these statics while a fault is being
    // serviced.
    let current = unsafe { CURRENT_TASK };

    // SAFETY: `SerialPort::init` reprograms COM1's divisor and line control,
    // which is idempotent — re-initializing here rather than sharing a
    // `&mut SerialPort` across a context switch avoids handing the handler a
    // reference whose owning frame it is about to abandon.
    let mut serial = unsafe { SerialPort::init() };

    let kind = frame.kind();
    let mnemonic = match kind {
        Some(vector) => vector.mnemonic(),
        // An unwired vector reaching this entry point means the IDT and
        // `hal_x86_64::fault` disagree; decoding it as one of the three would
        // produce a confident, wrong report (`TEST-P1-02-01-A` clause 2).
        None => "unwired-vector",
    };
    let _ = writeln!(
        serial,
        "fixture-fault captured {mnemonic} vector={} error_code={:#x} rip={:#x} cr2={}",
        frame.vector,
        frame.error_code,
        frame.rip,
        // `0` rather than a stale `CR2`: the register is meaningless for a
        // non-`#PF`, and printing it anyway would report an address from an
        // unrelated earlier fault with total confidence.
        frame.faulting_address().unwrap_or(0)
    );

    // SAFETY: as above — the scheduler and journal are owned solely by this
    // fixture, and no other context runs while this handler does.
    let disposition = unsafe {
        CAPTURED += 1;
        let scheduler = &mut *(&raw mut SCHEDULER);
        let context = match current.and_then(|slot| task_id(scheduler, slot)) {
            Some(task) => FaultingContext::Task(task),
            None => FaultingContext::Kernel,
        };
        let report = FaultReport { vector: frame.vector, context };
        let disposition = Disposition::of(&report);
        let journal = &mut *(&raw mut JOURNAL_STORE);
        for spoor in kernel::fault::audit(&report, disposition) {
            journal.append(spoor);
        }
        disposition
    };

    match disposition {
        Disposition::TerminateTask(task) => {
            // SAFETY: as above.
            unsafe {
                let scheduler = &mut *(&raw mut SCHEDULER);
                scheduler.set_state(task, TaskState::Finished);
                CONTAINED += 1;
                CURRENT_TASK = None;
            }
            let _ = writeln!(serial, "fixture-fault terminated task {}", task.index());
            // SAFETY: the victim's registers are saved into a context nothing
            // will ever resume, and `SUPERVISOR_CTX` is suspended at its own
            // `switch` call site inside `run` — `switch`'s documented
            // contract. This call does not return.
            unsafe { context::switch(&raw mut ABANDONED_CTX, &raw mut SUPERVISOR_CTX) };
            unreachable!("a terminated task is never switched back into")
        }
        Disposition::HaltSystem => {
            let _ = writeln!(serial, "fixture-fault kernel-context fault: halting");
            let _ = write_result(&mut serial, "fault", false);
            exit_qemu(QemuExitCode::Failure)
        }
    }
}

/// Resolves a pool slot back to the live [`TaskId`] the scheduler handed out.
///
/// Via `iter_tasks` rather than by fabricating an id: `TaskId` has no public
/// constructor precisely so a fault path cannot invent one for a slot that was
/// never created (`sched::TaskId`'s own doc comment), and this fixture is not
/// the place to make an exception.
fn task_id(scheduler: &Scheduler<TASKS>, slot: usize) -> Option<kernel::sched::TaskId> {
    scheduler.iter_tasks().map(|(task, _)| task).find(|task| task.index() == slot)
}

/// Creates one task, switches into it, and returns once the fault handler has
/// escaped back here.
///
/// # Safety
/// Single-CPU fixture; `slot` must be the next unused scheduler slot and its
/// stack must not be in use by any other context.
unsafe fn run_victim(
    serial: &mut SerialPort,
    slot: usize,
    label: &str,
    entry: extern "C" fn() -> !,
) -> bool {
    // SAFETY: per this function's own contract.
    unsafe {
        let scheduler = &mut *(&raw mut SCHEDULER);
        let Ok(priority) = Priority::try_new(8) else {
            return false;
        };
        let Ok(task) = scheduler.create_task(
            priority,
            WcetBudgetTicks(1_000),
            OverrunPolicy::TripToSafeState,
            entry,
        ) else {
            return false;
        };
        if task.index() != slot {
            return false;
        }
        let stack =
            core::slice::from_raw_parts_mut((&raw mut TASK_STACKS[slot]).cast::<u8>(), STACK_SIZE);
        let Ok(context) = Context::new(stack, entry) else {
            return false;
        };
        TASK_CTX[slot] = context;

        CURRENT_TASK = Some(slot);
        let _ = writeln!(serial, "fixture-fault switching into {label} (slot {slot})");
        context::switch(&raw mut SUPERVISOR_CTX, &raw mut TASK_CTX[slot]);
        // Control returns here only via the fault handler's escape switch.

        let terminated = scheduler.state_of(task) == Some(TaskState::Finished);
        let released = (&raw const CURRENT_TASK).read().is_none();
        let _ = writeln!(
            serial,
            "fixture-fault back in supervisor after {label}: finished={terminated} current_cleared={released}"
        );
        terminated && released
    }
}

/// Runs the fixture: three faults, three contained terminations, then proof
/// that the scheduler still dispatches real work.
pub fn run() -> bool {
    // SAFETY: this fixture is the only code running; `init` is called once,
    // before any other `SerialPort` method.
    let mut serial = unsafe { SerialPort::init() };

    // SAFETY: called once, before any fault can occur, on the QEMU `q35`
    // hardware this module's constants assume. Faults only — no APIC timer is
    // armed, because a tick arriving mid-fixture would switch contexts under
    // the very test that is checking which context faulted.
    unsafe { hal_x86_64::interrupts::init_faults_only() };

    let mut ok = true;
    // SAFETY: single-CPU fixture, one victim at a time, each in its own slot.
    unsafe {
        ok &= run_victim(&mut serial, 0, "#UD victim", victim_invalid_opcode);
        ok &= run_victim(&mut serial, 1, "#GP victim", victim_general_protection);
        ok &= run_victim(&mut serial, 2, "#PF victim", victim_page_fault);
    }

    // The point of the whole Story: after three faults, the scheduler still
    // runs something that did not fault.
    // SAFETY: as above; slot 3 is the survivor's own context and stack.
    unsafe {
        let scheduler = &mut *(&raw mut SCHEDULER);
        let Ok(priority) = Priority::try_new(9) else {
            return false;
        };
        let Ok(task) = scheduler.create_task(
            priority,
            WcetBudgetTicks(1_000),
            OverrunPolicy::TripToSafeState,
            survivor,
        ) else {
            return false;
        };
        let stack =
            core::slice::from_raw_parts_mut((&raw mut TASK_STACKS[3]).cast::<u8>(), STACK_SIZE);
        let Ok(context) = Context::new(stack, survivor) else {
            return false;
        };
        TASK_CTX[3] = context;
        CURRENT_TASK = Some(3);
        for _ in 0..3 {
            context::switch(&raw mut SUPERVISOR_CTX, &raw mut TASK_CTX[3]);
        }
        CURRENT_TASK = None;
        ok &= SURVIVOR_RUNS == 3;
        ok &= scheduler.state_of(task) == Some(TaskState::Ready);
        let runs = (&raw const SURVIVOR_RUNS).read();
        let _ = writeln!(
            serial,
            "fixture-fault survivor ran {runs} times after three contained faults"
        );
    }

    // SAFETY: as above — read after every switch has returned.
    unsafe {
        ok &= CAPTURED == 3;
        ok &= CONTAINED == 3;
        let journal = &mut *(&raw mut JOURNAL_STORE);
        // Two spoors per fault: the capture and the disposition.
        let fault_spoors = journal
            .iter()
            .filter(|spoor| spoor.category() == kernel::spoor::Category::Fault)
            .count();
        ok &= fault_spoors == 6;
        let captured = (&raw const CAPTURED).read();
        let contained = (&raw const CONTAINED).read();
        let _ = writeln!(
            serial,
            "fixture-fault captured={captured} contained={contained} fault_spoors={fault_spoors}"
        );
    }

    let _ = write_result(&mut serial, "fault", ok);
    ok
}
