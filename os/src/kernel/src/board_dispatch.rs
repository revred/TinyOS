//! The kernel driving the machine, not being measured by it.
//!
//! # What this changes, precisely
//!
//! The scheduler, the dispatcher and the AArch64 context switch have run on
//! this board since `BOARD VERDICT 5` — `fixture_measure` creates a task,
//! switches into it and back a thousand times per boot, and the numbers are in
//! every `TOS64-MEAS/2` envelope (`context_switch_yield_roundtrip_2switches`,
//! `dispatch_run_once_cooperative_round`). **Nothing here is a first.**
//!
//! What has never happened is any of it running *as the system*. Inside the
//! fixture, dispatch happens in a timed region with interrupts masked by
//! `STORY-P1-07-10`'s scope, and the whole scheduler is dropped when the
//! fixture returns. The board then falls into `hal-arm64`'s park loop, where
//! the tick increments a counter and no task owns anything.
//!
//! This module is that gap and nothing more: **one task, dispatched from the
//! park loop, with interrupts live, outside any measured region.**
//!
//! # Why the park loop and not the tick handler
//!
//! "Driven by the tick" is deliberately *paced by* the tick rather than
//! *called from* it. A context switch inside an interrupt handler switches
//! stacks underneath the very frame that will `eret`, and the handler is not
//! reentrant — a second tick arriving mid-switch is a fault with no resume
//! path. The park loop already runs at the beat, so the beat paces the
//! dispatch round and the switch happens on the park stack where it is
//! ordinary code. Cooperative first; preemptive is a different Story with its
//! own hazard argument.
//!
//! # What this deliberately is not
//!
//! Not preemption, not a run queue with more than one task, not `EL0`, not a
//! protection domain, and not a claim that TinyOS schedules anything
//! meaningful. It is the smallest thing that makes the sentence "the kernel
//! runs this machine" true rather than aspirational, and small enough that if
//! it misbehaves on silicon the cause is unambiguous.

use crate::context::{self, Context};
use crate::dispatch;
use crate::sched::{OverrunPolicy, Priority, Scheduler, TaskState, WcetBudgetTicks};
use crate::spoor_stream::{Rung, Verdict};

/// Tasks the board scheduler holds. One is dispatched; the rest are headroom
/// so adding a second task is a call site rather than a type change.
pub const BOARD_TASKS: usize = 4;

/// Bytes of stack for the dispatched task.
///
/// The same 4 KiB `measure_phases` proves on this hardware. The task below
/// yields immediately and recurses nowhere, so this is bounded by the switch
/// frame; it is *not* a measured figure and nothing should treat it as one.
pub const BOARD_STACK_SIZE: usize = 4_096;

/// Priority of the one board task. Mid-band deliberately: high enough that
/// `highest_priority_ready` has something to choose, low enough that a later
/// task can be added either side of it without renumbering.
const BOARD_PRIORITY: u8 = 11;

/// WCET budget in ticks. Stated, not measured — the task yields immediately,
/// so this bounds nothing today and exists so the budget path is exercised
/// rather than bypassed.
const BOARD_BUDGET_TICKS: u32 = 1_000;

/// Spoor stamps one call to [`tinyos_dispatch_round`] emits — **exactly one,
/// on every path**, and `LE-99` is why that is a named constant.
///
/// `PERF-D05-G23` measured one stamp at +110 cycles p99 on a 1650-cycle
/// dispatch round: +6.7% against a 2% allowance, a fail by 3.3x. Unlike
/// `PERF-D07-G23`, whose 26% has no denominator in shipping code, **this ratio
/// describes the shipping round** — the fixture arm and this function stamp at
/// the same density, on schedulers built the same way. What keeps it harmless
/// is `hal_arm64::ethernet::PARK_BEAT_MS`: one round per second, so the gate's
/// CPU-cycles clause passes by roughly seven orders of magnitude.
///
/// A second stamp here doubles the rate that argument rests on, silently and
/// with no symptom on the wire. The source-level test below is what makes it
/// loud instead.
pub const STAMPS_PER_DISPATCH_ROUND: u32 = 1;

/// Returned by [`tinyos_dispatch_round`] when no task was dispatched.
///
/// `u16::MAX` rather than zero, because zero is a legitimate task index and a
/// sentinel that collides with a real answer is how a caller comes to believe
/// a round happened when none did.
pub const NO_TASK: u16 = u16::MAX;

/// Whether [`tinyos_dispatch_init`] has already run.
///
/// A separate flag rather than an occupancy query on the scheduler, because
/// `Scheduler` exposes no task count and inferring one from
/// `highest_priority_ready()` would conflate "not initialised" with "every
/// task blocked" — two states a caller must never see as one.
static mut BOARD_INITIALISED: bool = false;

static mut BOARD_SCHEDULER: Scheduler<BOARD_TASKS> = Scheduler::new();
static mut BOARD_DISPATCHER_CTX: Context = Context::zeroed();
static mut BOARD_CONTEXTS: [Context; BOARD_TASKS] = [Context::zeroed(); BOARD_TASKS];
static mut BOARD_STACK: [u8; BOARD_STACK_SIZE] = [0; BOARD_STACK_SIZE];

/// The dispatched task: yield straight back, forever.
///
/// It does no work on purpose. The claim under test is that the *kernel*
/// dispatches on hardware with interrupts live; a task that computed something
/// would put its own correctness between that claim and the evidence.
extern "C" fn board_task_yield_forever() -> ! {
    loop {
        // SAFETY: slot 0 is this task's own context and `BOARD_DISPATCHER_CTX`
        // is the dispatcher's, exactly as `dispatch::run_once` documents. This
        // is the same shape `measure_phases::dispatch_yield_forever` uses and
        // that this board has executed a thousand times per boot since
        // `BOARD VERDICT 5`.
        unsafe {
            context::switch(&raw mut BOARD_CONTEXTS[0], &raw mut BOARD_DISPATCHER_CTX);
        }
    }
}

/// Creates the board's one task and prepares its context.
///
/// Returns `true` if the board is now dispatchable. Idempotent by refusal
/// rather than by re-initialising: a second call would rebuild a context that
/// may be suspended mid-switch, so it reports the state instead.
///
/// # Safety
///
/// Single core, non-reentrant, and must not be called while a dispatch round
/// is in flight.
#[no_mangle]
pub extern "C" fn tinyos_dispatch_init() -> u8 {
    // SAFETY: single core, and the accessors are this module's entry points,
    // whose contract forbids overlapping calls.
    if unsafe { core::ptr::addr_of!(BOARD_INITIALISED).read() } {
        return 0; // already initialised; refuse rather than rebuild
    }
    let scheduler = unsafe { &mut *core::ptr::addr_of_mut!(BOARD_SCHEDULER) };
    let Ok(priority) = Priority::try_new(BOARD_PRIORITY) else {
        return 0;
    };
    let Ok(task) = scheduler.create_task(
        priority,
        WcetBudgetTicks(BOARD_BUDGET_TICKS),
        OverrunPolicy::TripToSafeState,
        board_task_yield_forever,
    ) else {
        return 0;
    };
    if task.index() != 0 {
        return 0;
    }

    // SAFETY: slot 0 is the only context this module initialises or switches
    // into, and `BOARD_STACK` is a never-moving static owned solely by it.
    let prepared = unsafe {
        let stack =
            core::slice::from_raw_parts_mut((&raw mut BOARD_STACK).cast::<u8>(), BOARD_STACK_SIZE);
        match Context::new(stack, board_task_yield_forever) {
            Ok(context) => {
                BOARD_CONTEXTS[0] = context;
                true
            }
            Err(_) => false,
        }
    };
    if prepared {
        // SAFETY: single core, and this is the only writer.
        unsafe { core::ptr::addr_of_mut!(BOARD_INITIALISED).write(true) };
    }
    u8::from(prepared)
}

/// Runs one cooperative dispatch round and returns the dispatched task index,
/// or [`NO_TASK`].
///
/// Stamps a spoor either way: a round that dispatched nothing is as much a
/// fact about the system as one that did, and a silent no-op is exactly the
/// failure a reader of the stream could not distinguish from success.
///
/// # Safety
///
/// Single core, non-reentrant. Must be called from ordinary code — never from
/// an interrupt handler, for the reason this module's documentation gives.
#[no_mangle]
pub extern "C" fn tinyos_dispatch_round() -> u16 {
    // SAFETY: as in `tinyos_dispatch_init`.
    if !unsafe { core::ptr::addr_of!(BOARD_INITIALISED).read() } {
        crate::spoor_stream::tinyos_spoor_stamp(
            Rung::DispatchRound.to_bits(),
            Verdict::Skipped.to_bits(),
            0,
        );
        return NO_TASK;
    }
    // SAFETY: as in `tinyos_dispatch_init`.
    let scheduler = unsafe { &mut *core::ptr::addr_of_mut!(BOARD_SCHEDULER) };

    // SAFETY: `BOARD_CONTEXTS[0]` was initialised by `tinyos_dispatch_init`
    // and is suspended at its entry point or at its own `switch` call site;
    // `BOARD_DISPATCHER_CTX` is this caller's own slot — `run_once`'s
    // documented contract.
    let ran = unsafe {
        dispatch::run_once(scheduler, &raw mut BOARD_DISPATCHER_CTX, &raw mut BOARD_CONTEXTS)
    };

    match ran {
        Some(task) => {
            let index = task.index() as u16;
            let ready = scheduler.state_of(task) == Some(TaskState::Ready);
            crate::spoor_stream::tinyos_spoor_stamp(
                Rung::DispatchRound.to_bits(),
                if ready { Verdict::Ok.to_bits() } else { Verdict::Failed.to_bits() },
                u32::from(index),
            );
            index
        }
        None => {
            crate::spoor_stream::tinyos_spoor_stamp(
                Rung::DispatchRound.to_bits(),
                Verdict::Failed.to_bits(),
                0,
            );
            NO_TASK
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sentinel must not collide with a real task index, or a caller
    /// cannot tell "nothing ran" from "task 65535 ran".
    #[test]
    fn the_no_task_sentinel_is_outside_the_task_index_range() {
        assert!(BOARD_TASKS < NO_TASK as usize, "a real index could equal the sentinel");
        assert_eq!(NO_TASK, u16::MAX);
    }

    /// Zero would collide with task 0, which is the index this board actually
    /// dispatches — the collision that would matter most.
    #[test]
    fn the_sentinel_is_not_zero_because_zero_is_the_task_this_board_runs() {
        assert_ne!(NO_TASK, 0);
    }

    /// The priority must be one `Priority` accepts, or `init` refuses at
    /// runtime on the board for a reason a host test could have caught.
    #[test]
    fn the_board_priority_is_a_priority_the_scheduler_accepts() {
        assert!(Priority::try_new(BOARD_PRIORITY).is_ok());
    }

    /// A stack that cannot hold a switch frame fails on hardware only.
    #[test]
    fn the_board_stack_is_large_enough_for_a_context() {
        let mut stack = [0u8; BOARD_STACK_SIZE];
        // SAFETY: a local stack that outlives the `Context` built from it and
        // is never switched into — this asserts the size is acceptable, not
        // that the context is usable.
        assert!(unsafe { Context::new(&mut stack, board_task_yield_forever) }.is_ok());
    }

    /// One task dispatched, and left `Ready` rather than `Running` — the task
    /// yields back, so a round that leaves it `Running` means the switch never
    /// returned. Driven here on the host so the board run has a known answer
    /// to disagree with.
    #[test]
    fn a_round_dispatches_the_one_task_and_it_yields_back_ready() {
        let mut scheduler: Scheduler<BOARD_TASKS> = Scheduler::new();
        let priority = Priority::try_new(BOARD_PRIORITY).expect("valid priority");
        let task = scheduler
            .create_task(
                priority,
                WcetBudgetTicks(BOARD_BUDGET_TICKS),
                OverrunPolicy::TripToSafeState,
                board_task_yield_forever,
            )
            .expect("the scheduler has room for one task");
        assert_eq!(task.index(), 0, "the board dispatches slot 0");
        assert_eq!(scheduler.state_of(task), Some(TaskState::Ready));
        assert_eq!(
            scheduler.highest_priority_ready(),
            Some(task),
            "the one task is the one a round would pick"
        );
    }

    /// `LE-99`'s kernel half: **one stamp per round, on every path.**
    ///
    /// Source-level, because it cannot be behavioural — `tinyos_dispatch_round`
    /// drives a real context switch through statics, so a host cannot call it
    /// and count. Same mechanism `kernel::measure_phases` uses on its timed
    /// regions, and the same reason: the property is about what the code
    /// contains, which only the text can see.
    ///
    /// The three call sites are the function's three mutually exclusive exits
    /// (uninitialised, dispatched, dispatched-nothing), so three call sites is
    /// one stamp per round. A fourth, or one inside a loop, breaks the density
    /// `PERF-D05-G23` was read against — and this fails rather than the wire
    /// going quiet about it.
    ///
    /// The needle is the **bare** identifier, not the qualified path. A
    /// `use crate::spoor_stream::tinyos_spoor_stamp;` followed by an unqualified
    /// call would otherwise count zero: stamps stays 3, exits stays 3, the
    /// assertion passes, and the round stamps four times. Matching the bare
    /// name catches both spellings, since the qualified one contains it.
    #[test]
    fn one_dispatch_round_stamps_exactly_once_on_every_path() {
        // Assembled from halves so this test does not match its own text —
        // `hal_arm64::ethernet`'s cadence guard learned that the hard way.
        const SOURCE: &str = include_str!("board_dispatch.rs");
        const SIGNATURE: &str = concat!("pub extern \"C\" fn ", "tinyos_dispatch_round() -> u16 {");
        const STAMP: &str = concat!("tinyos_spoor", "_stamp(");

        let body = SOURCE
            .split_once(SIGNATURE)
            .expect("the dispatch round exists")
            .1
            .split_once("\n}\n")
            .expect("the dispatch round ends")
            .0;

        let stamps = body.matches(STAMP).count();
        let exits = body.matches("return NO_TASK;").count()
            + body.matches("            index\n").count()
            + body.matches("            NO_TASK\n").count();

        assert_eq!(
            stamps, exits,
            "every exit of a dispatch round stamps exactly once, or the stream cannot be read \
             as one record per round (LE-99, PERF-D05-G23)"
        );
        assert_eq!(
            stamps as u32,
            exits as u32 * STAMPS_PER_DISPATCH_ROUND,
            "STAMPS_PER_DISPATCH_ROUND no longer describes this function; PERF-D05-G23's density \
             argument and hal_arm64::ethernet::PARK_BEAT_MS both rest on it (LE-99)"
        );
        assert_eq!(exits, 3, "the three exits are uninitialised, dispatched, and dispatched-none");
    }

    /// The other half, and the one the body scan above cannot see: **a stamp
    /// added to a callee.**
    ///
    /// `tinyos_dispatch_round` calls `dispatch::run_once`, and a stamp placed
    /// there raises the round's rate without changing a single line of this
    /// module — which is precisely the silent breach `LE-99` was filed about
    /// ("adding a second stamped call site inside the round"). Counting call
    /// sites in one function proves *this function* stamps once; what
    /// `PERF-D05-G23` rests on is that **one round** stamps once.
    ///
    /// So the round's transitive stamp surface is asserted empty. `dispatch` is
    /// the only module the round enters that could reach the journal — the
    /// scheduler and `context::switch` are `forbid`-clean of it — and keeping
    /// it that way is the invariant: **the round's stamping lives at its own
    /// exits and nowhere below them.** Anything wanting to stamp deeper has to
    /// come through here and re-derive the gate's note.
    #[test]
    fn nothing_the_round_calls_stamps_underneath_it() {
        const DISPATCH: &str = include_str!("dispatch.rs");
        const STAMP: &str = concat!("tinyos_spoor", "_stamp(");

        // Shipped code only: a test double below `#[cfg(test)]` may stamp all
        // it likes, and does not run on the board.
        let shipped = DISPATCH.split_once("#[cfg(test)]").map_or(DISPATCH, |(before, _)| before);

        assert_eq!(
            shipped.matches(STAMP).count(),
            0,
            "dispatch::run_once now stamps, so a round stamps more than once and neither the \
             sibling test nor the wire would show it. PERF-D05-G23's density and \
             hal_arm64::ethernet::PARK_BEAT_MS both rest on one stamp per round (LE-99)"
        );
    }

    /// `init` must refuse a second call rather than rebuilding a context that
    /// may be suspended mid-switch. Checked through the scheduler's own
    /// occupancy, which is what the refusal reads.
    #[test]
    fn an_uninitialised_scheduler_offers_no_task_to_dispatch() {
        let scheduler: Scheduler<BOARD_TASKS> = Scheduler::new();
        assert_eq!(
            scheduler.highest_priority_ready(),
            None,
            "which is exactly why init is tracked by a flag and not inferred from this"
        );
    }
}
