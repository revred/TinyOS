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
//!
//! **`STORY-P0-06-04`**: [`record_tick`] and [`reset_budget_window`] each
//! take a `&mut SpoorJournal<J>` parameter (the same Dependency Inversion
//! pattern this module's own `OverrunHandler` established, and
//! `crate::lock`'s `STORY-P0-06-03` already applied) and stamp a
//! [`crate::spoor::Spoor`] on the two *budget-boundary* events: an overrun
//! (`Category::Wcet`, `Action::Overrun`, `Outcome::Failed`, `TARGET` the
//! task's pool index, `COST` the total ticks consumed at the moment it
//! crossed budget) and a reset that actually clears nonzero consumption
//! (`Action::ResetBudget`, `Outcome::Ok`, `TARGET` the task's pool index,
//! `COST` the consumption cleared). A tick that stays within budget, or a
//! reset against an already-zero counter, stamps nothing — mirroring
//! `crate::lock`'s "audit trail of what changed", not a call-count log.

use crate::sched::{Scheduler, TaskId};
use crate::spoor::{Action, Actor, Category, Outcome, Spoor};
use crate::spoor_journal::SpoorJournal;

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
///
/// An overrun stamps a [`Spoor`] (`Category::Wcet`, `Action::Overrun`) into
/// `journal`, `TARGET` `task`'s pool index and `COST` the total ticks
/// consumed at the moment it crossed budget — see the module doc comment
/// (`STORY-P0-06-04`).
pub fn record_tick<const N: usize, const J: usize>(
    scheduler: &mut Scheduler<N>,
    journal: &mut SpoorJournal<J>,
    handler: &mut impl OverrunHandler,
    task: TaskId,
    ticks: u32,
) -> Result<(), WcetError> {
    let (_, budget) = scheduler.wcet_state(task).ok_or(WcetError::UnknownTask)?;
    let consumed = scheduler.add_ticks_consumed(task, ticks).ok_or(WcetError::UnknownTask)?;
    if consumed > budget.0 {
        handler.on_overrun(task);
        journal.append(Spoor::stamp(
            Category::Wcet,
            Actor::Kernel,
            Action::Overrun,
            Outcome::Failed,
            task.index() as u16,
            consumed,
        ));
        Err(WcetError::BudgetExceeded)
    } else {
        Ok(())
    }
}

/// Starts a fresh WCET budget window for `task` (e.g. at the start of each
/// new scheduling period), resetting its consumed-ticks counter to 0.
/// `None` (no side effect) if `task` doesn't identify a currently live
/// task.
///
/// A reset that actually clears nonzero consumption stamps a [`Spoor`]
/// (`Category::Wcet`, `Action::ResetBudget`, `Outcome::Ok`) into `journal`,
/// `TARGET` `task`'s pool index and `COST` the consumption cleared; a reset
/// against an already-zero counter stamps nothing (`STORY-P0-06-04`).
pub fn reset_budget_window<const N: usize, const J: usize>(
    scheduler: &mut Scheduler<N>,
    journal: &mut SpoorJournal<J>,
    task: TaskId,
) -> Option<()> {
    let (consumed, _) = scheduler.wcet_state(task)?;
    scheduler.reset_ticks_consumed(task)?;
    if consumed > 0 {
        journal.append(Spoor::stamp(
            Category::Wcet,
            Actor::Kernel,
            Action::ResetBudget,
            Outcome::Ok,
            task.index() as u16,
            consumed,
        ));
    }
    Some(())
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
    // overrun, and the handler is never called for it. STORY-P0-06-04: nor
    // does it stamp anything — only a budget-crossing tick does.
    #[test]
    fn a_tick_within_budget_does_not_overrun() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let mut journal: SpoorJournal<4> = SpoorJournal::new();
        let task = sched.create_task(priority(1), WcetBudgetTicks(1000), dummy_entry).unwrap();
        let mut handler = RecordingHandler::default();

        assert_eq!(record_tick(&mut sched, &mut journal, &mut handler, task, 400), Ok(()));
        assert!(handler.overruns.is_empty());
        assert!(journal.is_empty());
    }

    // STORY-P0-02-04 AC1: detection happens the instant cumulative
    // consumption crosses the budget — not retroactively, and not only on
    // a single oversized tick: three in-budget-looking ticks (400 each,
    // budget 1000) only trip on the third, the one that actually crosses.
    // STORY-P0-06-04: exactly that third tick stamps a spoor, naming the
    // total (1200) that crossed budget.
    #[test]
    fn overrun_is_detected_on_the_exact_tick_that_crosses_the_budget() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let mut journal: SpoorJournal<4> = SpoorJournal::new();
        let task = sched.create_task(priority(1), WcetBudgetTicks(1000), dummy_entry).unwrap();
        let mut handler = RecordingHandler::default();

        assert_eq!(record_tick(&mut sched, &mut journal, &mut handler, task, 400), Ok(()));
        assert_eq!(record_tick(&mut sched, &mut journal, &mut handler, task, 400), Ok(()));
        assert!(handler.overruns.is_empty(), "800/1000 consumed should not yet overrun");
        assert!(journal.is_empty());

        assert_eq!(
            record_tick(&mut sched, &mut journal, &mut handler, task, 400),
            Err(WcetError::BudgetExceeded)
        );
        assert_eq!(handler.overruns, std::vec![task], "1200/1000 should overrun exactly once");

        let spoors: std::vec::Vec<Spoor> = journal.iter().collect();
        assert_eq!(spoors.len(), 1);
        assert_eq!(spoors[0].category(), Category::Wcet);
        assert_eq!(spoors[0].action(), Action::Overrun);
        assert_eq!(spoors[0].outcome(), Outcome::Failed);
        assert_eq!(spoors[0].target(), task.index() as u16);
        assert_eq!(spoors[0].cost(), 1200);
    }

    // A single tick larger than the whole budget overruns immediately, and
    // stamps a spoor naming the (over-budget) consumed total.
    #[test]
    fn a_single_oversized_tick_overruns_immediately() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let mut journal: SpoorJournal<4> = SpoorJournal::new();
        let task = sched.create_task(priority(1), WcetBudgetTicks(100), dummy_entry).unwrap();
        let mut handler = RecordingHandler::default();

        assert_eq!(
            record_tick(&mut sched, &mut journal, &mut handler, task, 500),
            Err(WcetError::BudgetExceeded)
        );
        assert_eq!(handler.overruns, std::vec![task]);

        let spoors: std::vec::Vec<Spoor> = journal.iter().collect();
        assert_eq!(spoors.len(), 1);
        assert_eq!(spoors[0].action(), Action::Overrun);
        assert_eq!(spoors[0].target(), task.index() as u16);
        assert_eq!(spoors[0].cost(), 500);
    }

    // STORY-P0-02-04 AC1: consumption exactly equal to the budget is not
    // yet an overrun (only *exceeding* it is). STORY-P0-06-04: nor does it
    // stamp anything.
    #[test]
    fn consumption_exactly_at_the_budget_is_not_an_overrun() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let mut journal: SpoorJournal<4> = SpoorJournal::new();
        let task = sched.create_task(priority(1), WcetBudgetTicks(1000), dummy_entry).unwrap();
        let mut handler = RecordingHandler::default();

        assert_eq!(record_tick(&mut sched, &mut journal, &mut handler, task, 1000), Ok(()));
        assert!(handler.overruns.is_empty());
        assert!(journal.is_empty());
    }

    // Resetting the budget window after an overrun lets the task run
    // fresh, rather than remaining permanently flagged. STORY-P0-06-04: the
    // reset stamps a spoor naming the consumption it cleared (500, the
    // overrunning tick), and the subsequent within-budget tick against the
    // fresh window stamps nothing.
    #[test]
    fn resetting_the_budget_window_clears_prior_consumption() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let mut journal: SpoorJournal<4> = SpoorJournal::new();
        let task = sched.create_task(priority(1), WcetBudgetTicks(100), dummy_entry).unwrap();
        let mut handler = RecordingHandler::default();

        record_tick(&mut sched, &mut journal, &mut handler, task, 500).unwrap_err();
        assert_eq!(reset_budget_window(&mut sched, &mut journal, task), Some(()));
        assert_eq!(record_tick(&mut sched, &mut journal, &mut handler, task, 50), Ok(()));

        let spoors: std::vec::Vec<Spoor> = journal.iter().collect();
        assert_eq!(spoors.len(), 2);
        assert_eq!(spoors[0].action(), Action::Overrun);
        assert_eq!(spoors[1].category(), Category::Wcet);
        assert_eq!(spoors[1].action(), Action::ResetBudget);
        assert_eq!(spoors[1].outcome(), Outcome::Ok);
        assert_eq!(spoors[1].target(), task.index() as u16);
        assert_eq!(spoors[1].cost(), 500);
    }

    // A reset against a counter that's already at 0 (never ticked, or
    // already reset) has nothing to clear — it still succeeds, but stamps
    // nothing, matching the "audit trail of what changed" discipline.
    #[test]
    fn resetting_an_already_zero_counter_stamps_nothing() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let mut journal: SpoorJournal<4> = SpoorJournal::new();
        let task = sched.create_task(priority(1), WcetBudgetTicks(100), dummy_entry).unwrap();

        assert_eq!(reset_budget_window(&mut sched, &mut journal, task), Some(()));
        assert!(journal.is_empty());
    }

    // An unknown task fails closed without calling the handler or stamping
    // anything.
    #[test]
    fn record_tick_against_an_unknown_task_fails_closed() {
        let mut sched: Scheduler<1> = Scheduler::new();
        let mut journal: SpoorJournal<4> = SpoorJournal::new();
        let task = sched.create_task(priority(1), WcetBudgetTicks(100), dummy_entry).unwrap();
        sched.free_task_for_test(task);
        let mut handler = RecordingHandler::default();

        assert_eq!(
            record_tick(&mut sched, &mut journal, &mut handler, task, 10),
            Err(WcetError::UnknownTask)
        );
        assert!(handler.overruns.is_empty());
        assert!(journal.is_empty());
    }

    // Resetting an unknown task fails closed and stamps nothing either.
    #[test]
    fn reset_budget_window_against_an_unknown_task_fails_closed() {
        let mut sched: Scheduler<1> = Scheduler::new();
        let mut journal: SpoorJournal<4> = SpoorJournal::new();
        let task = sched.create_task(priority(1), WcetBudgetTicks(100), dummy_entry).unwrap();
        sched.free_task_for_test(task);

        assert_eq!(reset_budget_window(&mut sched, &mut journal, task), None);
        assert!(journal.is_empty());
    }
}
