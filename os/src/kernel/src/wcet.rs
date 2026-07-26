//! WCET budget enforcement (`STORY-P0-02-04`).
//!
//! Every RT task declares a worst-case execution time (WCET) budget as
//! part of its task definition (`sched::Tcb::wcet_budget`,
//! `agent/CODING_STANDARDS.md`'s Real-time discipline section);
//! [`record_tick`] is the scheduler-side detection half of enforcing it —
//! checking a task's cumulative consumed ticks against its budget
//! synchronously, as each tick is attributed, rather than retroactively
//! after the fact (`STORY-P0-02-04` acceptance criterion 1).
//!
//! **Scope note**, mirroring `crate::lock`'s own: this kernel has neither
//! a periodic timer-tick source that calls [`record_tick`] on its own (no
//! ready-queue/priority-based dispatch loop exists yet — see
//! `crate::lock`'s doc comment for the identical gap `STORY-P0-02-03`
//! already surfaced) nor the documented watchdog/failsafe system
//! (`README.md` Non-Negotiable #5) to hand a detected overrun off to —
//! both are concrete, still-open prerequisites, not silently assumed to
//! exist. [`OverrunHandler`] is this Story's own minimal standalone trait
//! standing in for that not-yet-built watchdog, the same Dependency
//! Inversion pattern `exec::win32_shim::CapabilityPolicy` already
//! established for the not-yet-built `aci`: every function that needs to
//! react to an overrun takes `&mut impl OverrunHandler`, never a concrete
//! watchdog type, so wiring in the real one later is additive.

use crate::sched::{Scheduler, TaskId};

/// Errors [`record_tick`] fails closed with, per
/// `agent/CODING_STANDARDS.md`'s "no stringly-typed errors" rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WcetError {
    /// `task`'s cumulative consumed ticks now exceed its declared WCET
    /// budget. `on_overrun` was already called before this is returned.
    BudgetExceeded,
    /// `task` doesn't identify a currently live task.
    UnknownTask,
}

/// Reacts to a detected WCET overrun — the standalone stand-in for the
/// not-yet-built watchdog/failsafe system this Story's acceptance
/// criterion 2 calls for (`README.md` Non-Negotiable #5). See this
/// module's own doc comment for the migration path once a real one exists.
pub trait OverrunHandler {
    /// Called exactly once, synchronously, the moment [`record_tick`]
    /// detects that `task` has exceeded its budget — never a silent
    /// log-and-continue.
    fn on_overrun(&mut self, task: TaskId);
}

/// Attributes `ticks` more of execution to `task` and checks the result
/// against its declared WCET budget, calling `handler.on_overrun` and
/// returning [`WcetError::BudgetExceeded`] the moment (not retroactively
/// after the fact) the running total exceeds it.
///
/// Fails closed with [`WcetError::UnknownTask`] (no side effect, `handler`
/// never called) if `task` doesn't identify a currently live task.
pub fn record_tick<const N: usize>(
    scheduler: &mut Scheduler<N>,
    handler: &mut impl OverrunHandler,
    task: TaskId,
    ticks: u32,
) -> Result<(), WcetError> {
    let (_, budget) = scheduler.wcet_state(task).ok_or(WcetError::UnknownTask)?;
    let consumed = scheduler.add_ticks_consumed(task, ticks).ok_or(WcetError::UnknownTask)?;
    if consumed > budget.0 {
        handler.on_overrun(task);
        Err(WcetError::BudgetExceeded)
    } else {
        Ok(())
    }
}

/// Starts a fresh WCET budget window for `task` (e.g. at the start of each
/// new scheduling period), resetting its consumed-ticks counter to 0.
/// `None` (no side effect) if `task` doesn't identify a currently live
/// task.
pub fn reset_budget_window<const N: usize>(
    scheduler: &mut Scheduler<N>,
    task: TaskId,
) -> Option<()> {
    scheduler.reset_ticks_consumed(task)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sched::{Priority, WcetBudgetTicks};
    use std::vec::Vec;

    #[allow(clippy::empty_loop)]
    extern "C" fn dummy_entry() -> ! {
        loop {}
    }

    fn priority(value: u8) -> Priority {
        Priority::try_new(value).expect("value is in range")
    }

    #[derive(Default)]
    struct RecordingHandler {
        overruns: Vec<TaskId>,
    }
    impl OverrunHandler for RecordingHandler {
        fn on_overrun(&mut self, task: TaskId) {
            self.overruns.push(task);
        }
    }

    // STORY-P0-02-04 AC1/AC3: a tick that stays within budget is not an
    // overrun, and the handler is never called for it.
    #[test]
    fn a_tick_within_budget_does_not_overrun() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let task = sched.create_task(priority(1), WcetBudgetTicks(1000), dummy_entry).unwrap();
        let mut handler = RecordingHandler::default();

        assert_eq!(record_tick(&mut sched, &mut handler, task, 400), Ok(()));
        assert!(handler.overruns.is_empty());
    }

    // STORY-P0-02-04 AC1: detection happens the instant cumulative
    // consumption crosses the budget — not retroactively, and not only on
    // a single oversized tick: three in-budget-looking ticks (400 each,
    // budget 1000) only trip on the third, the one that actually crosses.
    #[test]
    fn overrun_is_detected_on_the_exact_tick_that_crosses_the_budget() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let task = sched.create_task(priority(1), WcetBudgetTicks(1000), dummy_entry).unwrap();
        let mut handler = RecordingHandler::default();

        assert_eq!(record_tick(&mut sched, &mut handler, task, 400), Ok(()));
        assert_eq!(record_tick(&mut sched, &mut handler, task, 400), Ok(()));
        assert!(handler.overruns.is_empty(), "800/1000 consumed should not yet overrun");

        assert_eq!(
            record_tick(&mut sched, &mut handler, task, 400),
            Err(WcetError::BudgetExceeded)
        );
        assert_eq!(handler.overruns, std::vec![task], "1200/1000 should overrun exactly once");
    }

    // A single tick larger than the whole budget overruns immediately.
    #[test]
    fn a_single_oversized_tick_overruns_immediately() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let task = sched.create_task(priority(1), WcetBudgetTicks(100), dummy_entry).unwrap();
        let mut handler = RecordingHandler::default();

        assert_eq!(
            record_tick(&mut sched, &mut handler, task, 500),
            Err(WcetError::BudgetExceeded)
        );
        assert_eq!(handler.overruns, std::vec![task]);
    }

    // STORY-P0-02-04 AC1: consumption exactly equal to the budget is not
    // yet an overrun (only *exceeding* it is).
    #[test]
    fn consumption_exactly_at_the_budget_is_not_an_overrun() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let task = sched.create_task(priority(1), WcetBudgetTicks(1000), dummy_entry).unwrap();
        let mut handler = RecordingHandler::default();

        assert_eq!(record_tick(&mut sched, &mut handler, task, 1000), Ok(()));
        assert!(handler.overruns.is_empty());
    }

    // Resetting the budget window after an overrun lets the task run
    // fresh, rather than remaining permanently flagged.
    #[test]
    fn resetting_the_budget_window_clears_prior_consumption() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let task = sched.create_task(priority(1), WcetBudgetTicks(100), dummy_entry).unwrap();
        let mut handler = RecordingHandler::default();

        record_tick(&mut sched, &mut handler, task, 500).unwrap_err();
        assert_eq!(reset_budget_window(&mut sched, task), Some(()));
        assert_eq!(record_tick(&mut sched, &mut handler, task, 50), Ok(()));
    }

    // An unknown task fails closed without calling the handler.
    #[test]
    fn record_tick_against_an_unknown_task_fails_closed() {
        let mut sched: Scheduler<1> = Scheduler::new();
        let task = sched.create_task(priority(1), WcetBudgetTicks(100), dummy_entry).unwrap();
        sched.free_task_for_test(task);
        let mut handler = RecordingHandler::default();

        assert_eq!(record_tick(&mut sched, &mut handler, task, 10), Err(WcetError::UnknownTask));
        assert!(handler.overruns.is_empty());
    }
}
