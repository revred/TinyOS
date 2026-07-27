//! `TEST-P1-04-01-A` clauses 4 and 5: a task that never yields is taken off
//! the CPU by the timer, and the SSE state it was using survives the
//! experience (`STORY-P1-04-01`, closing `LE-14`).
//!
//! **What makes this fixture evidence rather than decoration.**
//!
//! 1. The low-priority task's body contains **no `switch`, no `hlt`, and no
//!    scheduler call** — it is a plain counting loop, inspectable below. If
//!    it stops running, something took it off the CPU.
//! 2. The event it cannot cooperate with is created *by the tick hook
//!    itself*: the high-priority task starts `Blocked` and is made `Ready`
//!    from interrupt context after a fixed tick count. Nothing on the low
//!    task's own path could have observed or triggered it.
//! 3. The high task, while it runs, deliberately writes a **different**
//!    64-bit pattern into `XMM0`. The low task re-reads `XMM0` on *every*
//!    loop iteration — including the first one after it is resumed — and
//!    records the first value that is not its own, and at which iteration.
//!    With the `fxsave`/`fxrstor` pair removed from
//!    `hal_x86_64::interrupts`' timer ISR stub, that read returns a foreign
//!    value. The falsification was run deliberately; see
//!    `REPORT-2026-07-28-03`.
//!
//! **The `XMM0` reads are raw register reads with no operand declaration**,
//! which is the only way to observe a specific register's ambient value from
//! Rust. Nothing else in either task touches floating point, so no compiler
//! decision competes for the register — but that is a property of this
//! fixture's own code, stated here rather than assumed, and it is why both
//! tasks' bodies are kept deliberately trivial.
//!
//! **Who ends the fixture.** Not the low task: the tick hook marks it
//! `Finished` and switches away once its evidence is recorded, so the low
//! task is removed from the CPU by an interrupt on the way out exactly as it
//! was on the way in. Its loop still carries an iteration bound as
//! defence-in-depth (`agent/CODING_STANDARDS.md`'s fail-safe discipline, and
//! the same reason `fixture_idt_apic_timer` carries one) so a broken
//! preemption path fails as a reported `ok=false` rather than as a harness
//! timeout, which is a much less useful signal.
//!
//! Only reachable when the `fixture-preempt` feature is enabled — never part
//! of a real boot image.

#![allow(static_mut_refs, clippy::deref_addrof)]

use core::fmt::Write;
use hal_x86_64::interrupts;
use hal_x86_64::serial::SerialPort;
use kernel::context::{self, Context};
use kernel::dispatch;
use kernel::measure::write_result;
use kernel::preempt::{self, TickOutcome};
use kernel::sched::{OverrunPolicy, Priority, Scheduler, TaskId, TaskState, WcetBudgetTicks};

/// Two tasks: the busy-looping victim and its preemptor.
const TASKS: usize = 2;
const LOW_SLOT: usize = 0;
const HIGH_SLOT: usize = 1;

/// Per-task stack. The ISR runs on the interrupted task's own stack (no IST
/// for this vector), so a task's stack must also hold the interrupt frame,
/// fifteen pushed registers and the hook's own frames.
const STACK_SIZE: usize = 8_192;

/// Local-APIC timer reload, matching `fixture_idt_apic_timer`'s own
/// empirically-chosen value: fast enough that several ticks land well inside
/// `xtask`'s boot budget, slow enough that QEMU's interrupt-delivery
/// emulation does not starve the tasks of real CPU time to advance in.
const INITIAL_COUNT: u32 = 500_000;

/// Which tick makes the high-priority task `Ready`. Late enough that the low
/// task has demonstrably been running for a while first.
const ARM_AT_TICK: u32 = 3;

/// `TEST-P1-04-01-A` clause 4's bound, fixed **in the specification before
/// this fixture existed**: at most this many ticks between the high task
/// becoming Ready and the high task first running.
///
/// Two rather than one because the tick that arms it is the same tick whose
/// decision is taken, and one further tick of slack absorbs the dispatcher
/// round in between. A bound read out of a capture afterwards would not be
/// a bound.
const MAX_TICKS_TO_PREEMPT: u32 = 2;

/// Iteration ceiling on the low task's busy loop — defence in depth only;
/// a passing run never approaches it.
const LOW_LOOP_CEILING: u64 = 4_000_000_000;

/// Bound on dispatcher rounds, so a scheduling defect ends the run instead
/// of spinning until the harness kills it.
const MAX_ROUNDS: u32 = 64;

/// The low task's own `XMM0` contents: a bit in every byte lane, so a
/// truncated or byte-swapped restore is caught rather than coincidentally
/// matching.
const LOW_XMM_PATTERN: u64 = 0x0123_4567_89ab_cdef;
/// What the high task writes over it — the adversarial half of clause 5.
const HIGH_XMM_PATTERN: u64 = 0xfedc_ba98_7654_3210;

static mut SCHEDULER: Scheduler<TASKS> = Scheduler::new();
static mut DISPATCHER_CTX: Context = Context::zeroed();
/// Where the tick hook saves the registers of a task it is retiring. Written
/// once and never read: `context::switch` needs a destination, and a context
/// nothing will ever resume is the honest one (the same pattern
/// `fixture_fault`'s `ABANDONED_CTX` established).
static mut ABANDONED_CTX: Context = Context::zeroed();
static mut TASK_CTX: [Context; TASKS] = [Context::zeroed(); TASKS];
static mut TASK_STACKS: [[u8; STACK_SIZE]; TASKS] = [[0; STACK_SIZE]; TASKS];

/// Which task the dispatcher last switched into, or `None` while the
/// dispatcher itself is running. The tick hook reads nothing else to decide
/// whether it is allowed to touch the scheduler at all.
static mut CURRENT_TASK: Option<usize> = None;
static mut LOW_TASK: Option<TaskId> = None;
static mut HIGH_TASK: Option<TaskId> = None;

static mut HIGH_ARMED: bool = false;
static mut HIGH_READY_TICK: u32 = 0;
static mut HIGH_FIRST_RAN_TICK: u32 = 0;
static mut HIGH_RAN: bool = false;

static mut LOW_ITERATIONS: u64 = 0;
static mut LOW_ITERATIONS_AT_RESUME: u64 = 0;
static mut LOW_DONE: bool = false;
static mut LOW_EXHAUSTED: bool = false;
/// The first `XMM0` value the low task ever read back that was not its own,
/// or `0` if it never saw one. This is the whole of `LE-14`'s evidence.
static mut LOW_XMM_SEEN: u64 = 0;
static mut LOW_XMM_CORRUPTED: bool = false;
/// Which iteration first saw a foreign value — `1` would mean the low task's
/// own compiled code, not an interrupt, was responsible.
static mut LOW_XMM_CORRUPTED_AT: u64 = 0;

static mut PREEMPTIONS: u32 = 0;
static mut RETIRED_BY_TICK: bool = false;

/// Reads `XMM0`'s current contents.
///
/// A bare register read with no operand declaration — the only way to
/// observe a specific register's ambient value from Rust. See this module's
/// doc comment for why that is sound *here* specifically.
#[inline(always)]
fn read_xmm0() -> u64 {
    let value: u64;
    // SAFETY: `movq r64, xmm` reads one register and writes another; it has
    // no memory effect and no control-flow effect.
    unsafe {
        core::arch::asm!(
            "movq {out}, xmm0",
            out = out(reg) value,
            options(nostack, nomem, preserves_flags),
        );
    }
    value
}

/// Writes `value` into `XMM0`.
#[inline(always)]
fn write_xmm0(value: u64) {
    // SAFETY: writes only `XMM0`, which is declared clobbered.
    unsafe {
        core::arch::asm!(
            "movq xmm0, {value}",
            value = in(reg) value,
            out("xmm0") _,
            options(nostack, nomem, preserves_flags),
        );
    }
}

/// Resolves a pool slot back to the live [`TaskId`] the scheduler issued.
///
/// Via `iter_tasks` rather than by fabricating one: `TaskId` has no public
/// constructor precisely so an interrupt path cannot invent an id for a slot
/// that was never created.
fn task_id(scheduler: &Scheduler<TASKS>, slot: usize) -> Option<TaskId> {
    scheduler.iter_tasks().map(|(task, _)| task).find(|task| task.index() == slot)
}

/// The busy-looping victim: **no `switch`, no `hlt`, no scheduler call**.
///
/// It seeds `XMM0` once, then re-reads it every iteration. Everything else
/// in the body is integer work, which is what leaves `XMM0` uncontended by
/// anything but a context switch.
extern "C" fn low_task() -> ! {
    write_xmm0(LOW_XMM_PATTERN);
    loop {
        let live = read_xmm0();
        // SAFETY: single-CPU fixture; only this task writes these statics,
        // and the tick hook only reads them.
        unsafe {
            LOW_ITERATIONS += 1;
            if live != LOW_XMM_PATTERN && !LOW_XMM_CORRUPTED {
                LOW_XMM_CORRUPTED = true;
                LOW_XMM_SEEN = live;
                LOW_XMM_CORRUPTED_AT = LOW_ITERATIONS;
            }
            if HIGH_RAN && LOW_ITERATIONS_AT_RESUME == 0 {
                // The high task has been and gone, so this iteration is the
                // first one after a real preemption round-trip: this is the
                // moment clause 5 is about.
                LOW_ITERATIONS_AT_RESUME = LOW_ITERATIONS;
                LOW_DONE = true;
            }
            if LOW_ITERATIONS >= LOW_LOOP_CEILING {
                // Defence in depth: preemption never happened. Report it as
                // a failed run rather than as a harness timeout.
                LOW_EXHAUSTED = true;
                LOW_DONE = true;
                context::switch(&raw mut TASK_CTX[LOW_SLOT], &raw mut DISPATCHER_CTX);
            }
        }
    }
}

/// The preemptor: records when it first ran, clobbers `XMM0`, and retires
/// itself so the victim can be resumed and read its register back.
extern "C" fn high_task() -> ! {
    // SAFETY: single-CPU fixture; only this task writes these statics.
    unsafe {
        HIGH_FIRST_RAN_TICK = interrupts::tick_count();
    }
    // The adversarial half of clause 5: if extended state is not saved and
    // restored, this is the value the low task reads back.
    write_xmm0(HIGH_XMM_PATTERN);
    // SAFETY: as above.
    unsafe {
        HIGH_RAN = true;
    }

    // Retire, so the victim (which this task outranks) can run again. A task
    // mutating the scheduler must do so with interrupts masked — otherwise
    // it races the very tick hook that reads the scheduler. This is
    // `TEST-P1-04-01-A` clause 3's Tier 0 half.
    //
    // SAFETY: the closure is bounded (one pool write) and takes no lock;
    // `SCHEDULER` is not concurrently borrowed, since the dispatcher runs
    // only while this task does not.
    unsafe {
        interrupts::without_interrupts(|| {
            if let Some(task) = HIGH_TASK {
                (*(&raw mut SCHEDULER)).set_state(task, TaskState::Finished);
            }
        });
        context::switch(&raw mut TASK_CTX[HIGH_SLOT], &raw mut DISPATCHER_CTX);
    }
    unreachable!("a Finished task is never selected again")
}

/// The timer-tick consumer this fixture registers with
/// `hal_x86_64::interrupts::set_tick_hook`.
///
/// Runs in interrupt context on the interrupted task's own stack with `IF`
/// clear. Bounded and allocation-free throughout.
extern "C" fn on_tick() {
    // The dispatcher is the one context that legitimately holds a
    // `&mut Scheduler`, and it runs with `IF` clear — but a tick already in
    // flight when it cleared the flag can still land here. Checking this
    // *first*, before touching the scheduler at all, is what makes that
    // harmless.
    //
    // SAFETY: single-CPU fixture; `CURRENT_TASK` is written only by the
    // dispatcher, with interrupts disabled.
    let Some(slot) = (unsafe { CURRENT_TASK }) else {
        return;
    };

    let tick = interrupts::tick_count();

    // SAFETY: a task is running, so the dispatcher holds no borrow; this is
    // the only code touching the scheduler for the duration.
    unsafe {
        let scheduler = &mut *(&raw mut SCHEDULER);

        // The event the busy-looping victim cannot cooperate with. Raised
        // from interrupt context deliberately: there is no point on the low
        // task's own path where it could have been observed or triggered.
        if !HIGH_ARMED && tick >= ARM_AT_TICK {
            if let Some(high) = HIGH_TASK {
                scheduler.set_state(high, TaskState::Ready);
                HIGH_ARMED = true;
                HIGH_READY_TICK = tick;
            }
        }

        // The victim's evidence is complete: retire it from interrupt
        // context too, so it leaves the CPU the same way it was suspended.
        if LOW_DONE && slot == LOW_SLOT {
            if let Some(low) = LOW_TASK {
                scheduler.set_state(low, TaskState::Finished);
                RETIRED_BY_TICK = true;
                CURRENT_TASK = None;
                context::switch(&raw mut ABANDONED_CTX, &raw mut DISPATCHER_CTX);
                unreachable!("a retired task is never switched back into")
            }
        }
    }

    // SAFETY: `slot` is the task this interrupt is executing on, so
    // `TASK_CTX[slot]` is its own; `DISPATCHER_CTX` is
    // suspended at the dispatcher's own `run_once` call site.
    let outcome = unsafe {
        let running = task_id(&*(&raw const SCHEDULER), slot);
        preempt::on_timer_tick(
            &raw mut SCHEDULER,
            running,
            &raw mut TASK_CTX[slot],
            &raw mut DISPATCHER_CTX,
        )
    };
    if matches!(outcome, TickOutcome::Preempt(_)) {
        // SAFETY: reached only after the preempted task is resumed, still on
        // this single-CPU path.
        unsafe {
            PREEMPTIONS += 1;
        }
    }
}

/// Creates one task in `slot` at `priority`, initializing its context.
///
/// # Safety
/// `slot` must be the next unused scheduler slot, and its stack must not be
/// in use by any other context.
unsafe fn create(slot: usize, priority: u8, entry: extern "C" fn() -> !) -> Option<TaskId> {
    // SAFETY: per this function's own contract.
    unsafe {
        let scheduler = &mut *(&raw mut SCHEDULER);
        let priority = Priority::try_new(priority).ok()?;
        let task = scheduler
            .create_task(
                priority,
                WcetBudgetTicks(1_000_000),
                OverrunPolicy::TripToSafeState,
                entry,
            )
            .ok()?;
        if task.index() != slot {
            return None;
        }
        let stack =
            core::slice::from_raw_parts_mut((&raw mut TASK_STACKS[slot]).cast::<u8>(), STACK_SIZE);
        TASK_CTX[slot] = Context::new(stack, entry).ok()?;
        Some(task)
    }
}

/// Runs the fixture.
pub fn run() -> bool {
    // SAFETY: this fixture is the only code running; `init` is called once,
    // before any other `SerialPort` method.
    let mut serial = unsafe { SerialPort::init() };
    let mut ok = true;

    // SAFETY: single-CPU fixture, each slot used exactly once.
    unsafe {
        let Some(low) = create(LOW_SLOT, 5, low_task) else {
            let _ = writeln!(serial, "fixture-preempt: low task creation failed");
            return false;
        };
        let Some(high) = create(HIGH_SLOT, 25, high_task) else {
            let _ = writeln!(serial, "fixture-preempt: high task creation failed");
            return false;
        };
        LOW_TASK = Some(low);
        HIGH_TASK = Some(high);
        // The high task must not be selectable until the tick hook makes it
        // so — otherwise it would simply be picked first and nothing would
        // ever be preempted.
        (*(&raw mut SCHEDULER)).set_state(high, TaskState::Blocked);
    }

    // SAFETY: registered before interrupts are armed, so no tick can arrive
    // between arming and installation. The hook's own contract (bounded,
    // allocation-free, leaves the interrupt frame intact) is met by
    // `on_tick` above.
    unsafe { interrupts::set_tick_hook(on_tick) };

    // SAFETY: called exactly once, before anything here depends on
    // interrupts being armed — `init`'s own documented contract. It ends
    // with `sti`.
    unsafe { interrupts::init(INITIAL_COUNT) };

    // From here the dispatcher runs with `IF` clear and never re-enables it:
    // a task's own saved `RFLAGS` is what turns interrupts back on across
    // the switch into it, and turns them off again across the switch back.
    // That, and not a convention, is what stops the tick hook ever observing
    // a scheduler this loop is mid-mutation of.
    //
    // SAFETY: every subsequent re-enable happens via a context switch's own
    // `popfq`, so interrupts are not lost.
    let _ = unsafe { interrupts::disable_interrupts() };

    let mut rounds: u32 = 0;
    loop {
        // SAFETY: interrupts are masked, so this is the only code touching
        // the scheduler; `TASK_CTX` slots are each owned by one task.
        let ran = unsafe {
            let scheduler = &mut *(&raw mut SCHEDULER);
            let Some(next) = scheduler.highest_priority_ready() else {
                break;
            };
            CURRENT_TASK = Some(next.index());
            let ran = dispatch::run_once(scheduler, &raw mut DISPATCHER_CTX, &raw mut TASK_CTX);
            CURRENT_TASK = None;
            // The tick hook's decision is taken against
            // `highest_priority_ready`; if `run_once` ever selected something
            // else, every decision would be about the wrong task.
            ok &= ran == Some(next);
            ran
        };
        if ran.is_none() {
            break;
        }
        rounds += 1;
        if rounds > MAX_ROUNDS {
            let _ = writeln!(serial, "fixture-preempt: dispatcher exceeded {MAX_ROUNDS} rounds");
            ok = false;
            break;
        }
    }

    // SAFETY: read after every switch has returned and with interrupts
    // masked; nothing else can be running.
    let (
        preemptions,
        high_ran,
        ready_tick,
        first_ran_tick,
        iterations,
        iterations_at_resume,
        exhausted,
        corrupted,
        seen,
        corrupted_at,
        retired,
    ) = unsafe {
        (
            PREEMPTIONS,
            HIGH_RAN,
            HIGH_READY_TICK,
            HIGH_FIRST_RAN_TICK,
            LOW_ITERATIONS,
            LOW_ITERATIONS_AT_RESUME,
            LOW_EXHAUSTED,
            LOW_XMM_CORRUPTED,
            LOW_XMM_SEEN,
            LOW_XMM_CORRUPTED_AT,
            RETIRED_BY_TICK,
        )
    };
    let ticks_to_preempt = first_ran_tick.saturating_sub(ready_tick);

    // Clause 4: the victim really was running, it really was preempted, and
    // the preemption happened within the bound this test fixed in advance.
    ok &= iterations > 0;
    ok &= !exhausted;
    ok &= preemptions >= 1;
    ok &= high_ran;
    ok &= ticks_to_preempt <= MAX_TICKS_TO_PREEMPT;
    // The victim was resumed *and kept running* afterwards — otherwise
    // "preempted" would be indistinguishable from "killed".
    ok &= iterations_at_resume > 0 && iterations >= iterations_at_resume;
    ok &= retired;

    // Clause 5: across a preemption in which another task wrote `XMM0`, the
    // victim never once observed a value that was not its own.
    ok &= !corrupted;

    let _ = writeln!(
        serial,
        "fixture-preempt: preemptions={preemptions} high_ready_tick={ready_tick} \
         high_first_ran_tick={first_ran_tick} ticks_to_preempt={ticks_to_preempt} \
         (bound {MAX_TICKS_TO_PREEMPT})"
    );
    let _ = writeln!(
        serial,
        "fixture-preempt: low_iterations={iterations} resumed_at={iterations_at_resume} \
         exhausted={exhausted} retired_by_tick={retired}"
    );
    let _ = writeln!(
        serial,
        "fixture-preempt: xmm0 pattern={LOW_XMM_PATTERN:#x} clobber={HIGH_XMM_PATTERN:#x} \
         corrupted={corrupted} first_foreign_value={seen:#x} at_iteration={corrupted_at}"
    );

    let _ = write_result(&mut serial, "preempt", ok);
    ok
}
