//! Timer-driven preemption (`STORY-P1-04-01`).
//!
//! Every dispatch this kernel performed before this Story was cooperative:
//! [`crate::dispatch::run_once`] switched into a task and waited for that
//! task to switch back. This module is the other half — the local-APIC
//! timer tick, armed since `STORY-P0-04-02` and consumed by nothing, finally
//! gets a consumer that can take a task off the CPU whether it cooperates or
//! not.
//!
//! **The split, and why it is drawn here.** [`tick_outcome`] is the whole
//! scheduling decision as a pure function of two priorities; it is total,
//! host-tested on every arm, and has no idea a CPU exists.
//! [`on_timer_tick`] is the one place that decision meets the hardware — the
//! `fxsave`, the register swap — and it is deliberately the only place, so
//! there is exactly one reviewed ordering rather than one per call site.
//! This is the same seam `dispatch::switch_plan`/`run_once_in_space` already
//! draws for the `CR3` decision.
//!
//! **The re-entrancy rule** (`STORY-P1-04-01` acceptance criterion 5). The
//! dispatcher holds `&mut Scheduler` while it selects and switches; this
//! module reads the same scheduler from an interrupt. Those cannot both be
//! true at once, so the rule is:
//!
//! > **Interrupts are enabled only while a task runs.**
//!
//! The dispatcher body runs with `IF` clear, and a task's own saved `RFLAGS`
//! — `0x202` from [`crate::context::Context::new`], or whatever the ISR's
//! `pushfq` captured — re-enables interrupts across the switch into it and
//! clears them again across the switch back. Nothing has to remember to do
//! anything: the flag travels with the context. That is why this module
//! takes `*mut Scheduler` rather than `&mut Scheduler` and only ever forms a
//! short-lived shared reference from it, and why task code that touches the
//! scheduler itself must do so inside
//! `hal_x86_64::interrupts::without_interrupts`.
//!
//! **What is deliberately not here.** Driving `crate::wcet::record_tick` off
//! the real timer, and tripping a declared fault policy on overrun, is
//! `STORY-P1-04-02`; `LE-02` stays open until it lands. Equal-priority
//! rotation is not implemented and its absence is pinned by a test — see
//! [`tick_outcome`].

use crate::context::{switch, Context};
use crate::sched::{Priority, Scheduler, TaskId};

/// What a timer tick decided.
///
/// Returned by [`on_timer_tick`] for diagnosis and for a fixture to assert
/// on; the dispatch consequence has already happened by the time a caller
/// sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickOutcome {
    /// No task was running on this stack — the tick landed in the dispatcher
    /// or an idle context. Never switches: there is nothing to preempt, and
    /// the interrupted code may be mid-way through mutating the scheduler.
    NoRunningTask,
    /// A task is running and still outranks everything Ready. It keeps the
    /// CPU.
    Continue,
    /// A Ready task strictly outranks the running one; the running task was
    /// suspended in favour of it. Carries the task that won the decision.
    Preempt(TaskId),
}

/// The preemption decision, as a pure function of who is running and who is
/// the best Ready candidate.
///
/// `running` is `None` when the tick did not interrupt a task.
/// `best_ready` is whatever [`Scheduler::highest_priority_ready`] selects —
/// it must be *that* function's answer, never a parallel iteration that
/// could drift from it, because a decision about a task the dispatcher would
/// not then choose is worse than no decision at all. [`on_timer_tick`] is
/// what guarantees this.
///
/// **Equal priority does not preempt.** The comparison is strictly greater.
/// A tick-driven rotation between two equal-priority Ready tasks is a
/// scheduling policy `STORY-P1-04-01` has no requirement for, and adding one
/// silently would change dispatch behaviour nothing asked for. Its absence
/// is pinned by its own test, so a later Story that wants round-robin has to
/// change a test rather than discover the behaviour.
pub fn tick_outcome(
    running: Option<(TaskId, Priority)>,
    best_ready: Option<(TaskId, Priority)>,
) -> TickOutcome {
    let Some((_, running_priority)) = running else {
        return TickOutcome::NoRunningTask;
    };
    match best_ready {
        Some((candidate, candidate_priority)) if candidate_priority > running_priority => {
            TickOutcome::Preempt(candidate)
        }
        _ => TickOutcome::Continue,
    }
}

/// Services one timer tick from interrupt context: decides, and — only if
/// the decision is [`TickOutcome::Preempt`] — saves the running task's
/// extended state and suspends it into `dispatcher_ctx`.
///
/// The running task is left in [`crate::sched::TaskState::Running`]; nothing
/// here mutates the scheduler. [`crate::dispatch::run_once`] already returns
/// a still-`Running` task to `Ready` the moment its switch call returns,
/// which is exactly where control lands, so the state transition happens on
/// the dispatcher's side where a `&mut Scheduler` legitimately exists.
///
/// **Interrupt-context discipline** (`agent/CODING_STANDARDS.md`'s RT
/// rules): no allocation, no I/O, and bounded work — two `O(N)` walks of a
/// fixed-capacity task pool, one comparison, and on the preempt arm one
/// register swap. Nothing here can block.
///
/// **Extended state is not this function's job**, deliberately. `FXSAVE`/
/// `FXRSTOR` happen in `hal_x86_64::interrupts`' ISR stub, around the whole
/// handler call. Doing it here instead was this Story's first implementation
/// and was wrong: a tick that *decides not to preempt* still runs compiled
/// handler code on the interrupted task's stack, and can clobber `XMM0` just
/// as thoroughly. See that module's own note for the Tier 0 capture that
/// caught it.
///
/// **On the return value.** This function returns when the preempted task is
/// *resumed*, which may be many ticks later; the `Preempt` it returns
/// describes the decision it took on the way out, not the state of the world
/// on the way back. It is diagnostic.
///
/// # Safety
/// - `scheduler` must point at a live [`Scheduler`] that no `&mut` is
///   currently outstanding against. On this kernel that is guaranteed
///   structurally rather than by convention: interrupts are enabled only
///   while a task runs, and the dispatcher's `&mut` exists only while they
///   are disabled — see this module's own doc comment.
/// - `running` must name the task whose stack this interrupt is executing
///   on, and `running_ctx` must be that same task's own context slot.
///   Passing another task's slot would save this task's registers over that
///   one's.
/// - `dispatcher_ctx` must be the dispatcher's suspended context, per
///   [`switch`]'s own contract.
pub unsafe fn on_timer_tick<const N: usize>(
    scheduler: *mut Scheduler<N>,
    running: Option<TaskId>,
    running_ctx: *mut Context,
    dispatcher_ctx: *mut Context,
) -> TickOutcome {
    // SAFETY: a shared borrow, formed and dropped entirely within this
    // block, against a scheduler no `&mut` is outstanding for per this
    // function's own contract.
    let (running_now, best_ready) = unsafe {
        let scheduler = &*scheduler;
        let running_now =
            running.and_then(|task| scheduler.live_priority_of(task).map(|p| (task, p)));
        // Deliberately `highest_priority_ready` itself, so this decision can
        // never be about a task the dispatcher would not select.
        let best_ready = scheduler
            .highest_priority_ready()
            .and_then(|task| scheduler.live_priority_of(task).map(|p| (task, p)));
        (running_now, best_ready)
    };

    let outcome = tick_outcome(running_now, best_ready);
    if matches!(outcome, TickOutcome::Preempt(_)) {
        // SAFETY: forwarded from this function's own contract. The running
        // task's x87/SSE state is already safe on its own stack — the ISR
        // stub saved it before any of this ran — so the switch needs only
        // the integer half.
        unsafe { switch(running_ctx, dispatcher_ctx) };
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sched::{OverrunPolicy, TaskState, WcetBudgetTicks};

    #[allow(clippy::empty_loop)]
    extern "C" fn dummy_entry() -> ! {
        loop {}
    }

    fn priority(value: u8) -> Priority {
        Priority::try_new(value).expect("value is in range")
    }

    fn scheduler_with_two() -> (Scheduler<4>, TaskId, TaskId) {
        let mut sched: Scheduler<4> = Scheduler::new();
        let low = sched
            .create_task(
                priority(5),
                WcetBudgetTicks(1_000),
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();
        let high = sched
            .create_task(
                priority(25),
                WcetBudgetTicks(1_000),
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();
        (sched, low, high)
    }

    // `TEST-P1-04-01-A` clause 1, row 1: a tick that did not interrupt a task
    // never switches — it may have landed in the dispatcher, which is the one
    // context that legitimately holds a `&mut Scheduler`.
    #[test]
    fn a_tick_with_no_running_task_never_preempts() {
        let (_sched, _low, high) = scheduler_with_two();
        assert_eq!(tick_outcome(None, None), TickOutcome::NoRunningTask);
        assert_eq!(
            tick_outcome(None, Some((high, priority(31)))),
            TickOutcome::NoRunningTask,
            "even a maximum-priority candidate must not preempt the dispatcher"
        );
    }

    // Clause 1, row 2: a running task with nothing Ready keeps the CPU.
    #[test]
    fn a_running_task_with_no_ready_candidate_continues() {
        let (_sched, low, _high) = scheduler_with_two();
        assert_eq!(tick_outcome(Some((low, priority(5))), None), TickOutcome::Continue);
    }

    // Clause 1, row 3: strictly higher preempts, and names the winner.
    #[test]
    fn a_strictly_higher_priority_ready_task_preempts() {
        let (_sched, low, high) = scheduler_with_two();
        assert_eq!(
            tick_outcome(Some((low, priority(5))), Some((high, priority(25)))),
            TickOutcome::Preempt(high)
        );
        // The narrowest possible margin still preempts — an implementation
        // using `>=` on the wrong side, or an off-by-one, fails here.
        assert_eq!(
            tick_outcome(Some((low, priority(5))), Some((high, priority(6)))),
            TickOutcome::Preempt(high)
        );
    }

    // Clause 1, row 4 and the boundary that matters: equal priority does
    // **not** preempt. This pins the deliberate absence of tick-driven
    // round-robin, so a later Story that wants it has to change a test.
    #[test]
    fn an_equal_priority_ready_task_does_not_preempt() {
        let (_sched, low, high) = scheduler_with_two();
        assert_eq!(
            tick_outcome(Some((low, priority(15))), Some((high, priority(15)))),
            TickOutcome::Continue
        );
    }

    #[test]
    fn a_lower_priority_ready_task_does_not_preempt() {
        let (_sched, low, high) = scheduler_with_two();
        assert_eq!(
            tick_outcome(Some((high, priority(25))), Some((low, priority(5)))),
            TickOutcome::Continue
        );
    }

    // The candidate the decision is taken about must be the one the
    // dispatcher would actually select. Asserted against a real `Scheduler`
    // rather than against hand-built tuples, because the failure this guards
    // is precisely a *drift* between two selections — a pure-function test
    // over invented inputs could never see it.
    #[test]
    fn the_candidate_is_the_task_the_dispatcher_itself_would_select() {
        let (mut sched, low, high) = scheduler_with_two();
        let medium = sched
            .create_task(
                priority(15),
                WcetBudgetTicks(1_000),
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();

        // `low` is running, so it is not Ready and cannot be its own
        // candidate; `high` outranks `medium`.
        sched.set_state(low, TaskState::Running).unwrap();
        assert_eq!(sched.highest_priority_ready(), Some(high));

        let running = (low, sched.live_priority_of(low).unwrap());
        let best = sched
            .highest_priority_ready()
            .map(|task| (task, sched.live_priority_of(task).unwrap()))
            .unwrap();
        assert_eq!(tick_outcome(Some(running), Some(best)), TickOutcome::Preempt(high));

        // Block the winner and the decision must move to `medium`, not stay
        // stale on `high`.
        sched.set_state(high, TaskState::Blocked).unwrap();
        let best = sched
            .highest_priority_ready()
            .map(|task| (task, sched.live_priority_of(task).unwrap()))
            .unwrap();
        assert_eq!(tick_outcome(Some(running), Some(best)), TickOutcome::Preempt(medium));
    }

    // The inversion scenario's decision, in bookkeeping form: a boosted
    // holder must stop being preemptible by the medium task that outranked
    // it a moment earlier. `fixture_priority_inversion` proves this
    // behaviorally under real ticks; this pins the decision itself.
    #[test]
    fn a_boosted_holder_is_no_longer_preemptible_by_the_medium_task() {
        let (mut sched, low, _high) = scheduler_with_two();
        let medium = sched
            .create_task(
                priority(15),
                WcetBudgetTicks(1_000),
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();

        // Before the boost: medium outranks the holder and would preempt it.
        let before = (low, sched.live_priority_of(low).unwrap());
        let candidate = (medium, sched.live_priority_of(medium).unwrap());
        assert_eq!(tick_outcome(Some(before), Some(candidate)), TickOutcome::Preempt(medium));

        // After inheritance boosts the holder to the waiter's priority, it
        // does not.
        sched.inherit_priority(low, priority(25)).unwrap();
        let after = (low, sched.live_priority_of(low).unwrap());
        assert_eq!(tick_outcome(Some(after), Some(candidate)), TickOutcome::Continue);
    }

    // `live_priority_of` is the read `on_timer_tick` depends on, from a
    // shared reference. An unknown task must fail closed rather than
    // fabricating a priority the decision would then act on.
    #[test]
    fn live_priority_of_reports_the_boosted_value_and_fails_closed_when_unknown() {
        let (mut sched, low, _high) = scheduler_with_two();
        assert_eq!(sched.live_priority_of(low), Some(priority(5)));
        sched.inherit_priority(low, priority(25)).unwrap();
        assert_eq!(sched.live_priority_of(low), Some(priority(25)));

        let mut empty: Scheduler<1> = Scheduler::new();
        let gone = empty
            .create_task(
                priority(3),
                WcetBudgetTicks(10),
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();
        empty.free_task_for_test(gone);
        assert_eq!(empty.live_priority_of(gone), None);
        // And a tick naming a task that no longer exists decides nothing.
        assert_eq!(tick_outcome(None, None), TickOutcome::NoRunningTask);
    }
}
