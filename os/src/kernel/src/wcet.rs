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

use crate::sched::{Priority, Scheduler, TaskId, TaskState};
use crate::spoor::{Action, Actor, Category, Outcome, Spoor};
use crate::spoor_journal::SpoorJournal;

/// What a task declared should happen if it exceeds its budget.
///
/// Re-exported rather than defined here so that the module dependency stays
/// one-way — `wcet` reads `sched`, never the reverse — while the type still
/// has the `kernel::wcet::OverrunPolicy` path `TEST-P1-04-02-A` names. The
/// declaration belongs to a task; the *consequence* belongs to this module.
pub use crate::sched::OverrunPolicy;

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

/// Which task a timer tick is charged to (`STORY-P1-04-02` acceptance
/// criterion 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickAttribution {
    /// Charge this tick to `task` — it was on the CPU when the timer fired.
    Task(TaskId),
    /// Charge this tick to **nobody**. The tick landed in the dispatcher or
    /// an idle context, and kernel time belongs to no task's budget.
    Nobody,
}

/// The attribution rule: a tick is charged to exactly the task that was
/// running, and to no one otherwise.
///
/// **Why this is a named function rather than an `if` at the call site.**
/// The tempting wrong implementation is not a complicated one — it is
/// charging the tick to whichever task ran *most recently*, which produces
/// budgets that are wrong in one consistent direction, and a budget that is
/// consistently wrong is worse than one that is obviously broken because it
/// produces plausible numbers nobody questions. The defence is structural:
/// this rule takes exactly one input, so there is no "last task" in scope for
/// it to fall back on, and [`account_tick`] is its only caller.
pub const fn attribute_tick(running: Option<TaskId>) -> TickAttribution {
    match running {
        Some(task) => TickAttribution::Task(task),
        None => TickAttribution::Nobody,
    }
}

/// What the kernel does about a detected overrun — the consequence half of
/// [`OverrunPolicy`].
///
/// Separate from the policy because a policy is a *declaration* and a
/// disposition is an *action*, and because two of the three arms oblige the
/// caller to do something this module cannot: see [`account_tick`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverrunDisposition {
    /// The task's budget window was reset and it was returned to
    /// [`TaskState::Ready`]; **the caller must re-initialize its
    /// [`crate::context::Context`] to its entry point before it is selected
    /// again.** This module does not own task contexts or stacks (see
    /// `sched`'s own note that it has no dependency on `context`), so the
    /// half of a restart that rewinds the instruction pointer necessarily
    /// happens where those live.
    RestartTask,
    /// The task's priority was lowered to this floor, its budget window was
    /// reset, and it stays [`TaskState::Ready`]. Nothing further is required
    /// of the caller.
    DegradeTo(Priority),
    /// The task was set [`TaskState::Finished`]; **the caller must enter its
    /// declared safe state.** What a safe state *is* — a fail-closed stop, a
    /// limp-home mode, a watchdog reset — is a deployment question this
    /// kernel does not answer. At Tier 0 it is a reported fail-closed exit.
    TripToSafeState,
}

/// The decision table: what a declared [`OverrunPolicy`] means the kernel
/// must do (`TEST-P1-04-02-A` clause 2).
///
/// **This deliberately reads exactly one input**, and the input is the policy
/// the task declared *in advance* — never how far over budget it went, never
/// how many times it has overrun, never which task it is. That is the same
/// discipline `crate::fault::Disposition::of` holds to for the same reason:
/// every other quantity available at the moment of detection is downstream of
/// the offending task's own execution, so consulting one would let a task
/// influence its own consequence by misbehaving harder.
///
/// **And it is deliberately not `crate::fault::Disposition`.** An overrun has
/// no frame, no vector and no hardware event; it is a scheduler-detected
/// budget condition with a genuine choice of outcomes. Routing it through
/// that function would mean giving it a second input and ending the one
/// invariant it exists to hold. `crate::fault` is not modified by this Story
/// — exactly as `crate::dispatch` was not modified by `STORY-P1-04-01`.
pub const fn disposition_for(policy: OverrunPolicy) -> OverrunDisposition {
    match policy {
        OverrunPolicy::Restart => OverrunDisposition::RestartTask,
        OverrunPolicy::Degrade(floor) => OverrunDisposition::DegradeTo(floor),
        OverrunPolicy::TripToSafeState => OverrunDisposition::TripToSafeState,
    }
}

/// What one timer tick did to the budget books.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickAccounting {
    /// The tick was charged to nobody — it landed in the dispatcher or an
    /// idle context. No task's consumption moved.
    Unattributed,
    /// The tick was charged to `task`, which is still inside its budget.
    WithinBudget(TaskId),
    /// The tick was charged to `task`, which crossed its budget. The
    /// disposition has already been applied to the scheduler and audited;
    /// see [`OverrunDisposition`] for what the *caller* still owes.
    Enforced {
        /// The task that overran.
        task: TaskId,
        /// The consequence it declared, now applied.
        disposition: OverrunDisposition,
    },
    /// `running` named a task that is not live. Fails closed: nothing
    /// charged, nothing stamped, no state changed.
    UnknownTask,
}

/// Services one timer tick's worth of budget accounting: attributes the tick,
/// charges it, and — if it crossed a budget — applies the consequence the
/// task declared at creation and audits it (`STORY-P1-04-02`).
///
/// This is the function `STORY-P0-02-04` said did not exist. That Story built
/// [`record_tick`] and its module doc has said plainly for two Epics that the
/// kernel had "neither a periodic timer-tick source that calls `record_tick`
/// on its own … nor the documented watchdog/failsafe system to hand a
/// detected overrun off to". `STORY-P1-04-01` built the first;
/// this is the second, and the [`OverrunHandler`] trait that stood in for it
/// is now driven by a real consequence rather than by a test double.
///
/// **There is no ignore branch.** Every arm of [`OverrunDisposition`] changes
/// the task's state or its priority and stamps a spoor. A fourth arm added to
/// [`OverrunPolicy`] later cannot fall through silently — it will not compile
/// until [`disposition_for`] and this function's own `match` both name it.
///
/// **Interrupt-context discipline.** This takes `&mut Scheduler`, and calling
/// it from a timer ISR is sound *only* under the discipline
/// `STORY-P1-04-01` established: interrupts are enabled only while a task
/// runs, so the dispatcher's `&mut` and this call can never coexist. Called
/// from anywhere else, that argument does not carry.
pub fn account_tick<const N: usize, const J: usize>(
    scheduler: &mut Scheduler<N>,
    journal: &mut SpoorJournal<J>,
    running: Option<TaskId>,
) -> TickAccounting {
    let TickAttribution::Task(task) = attribute_tick(running) else {
        return TickAccounting::Unattributed;
    };
    let Some(policy) = scheduler.overrun_policy_of(task) else {
        return TickAccounting::UnknownTask;
    };

    // Exactly one tick, from the one place a tick is counted. `record_tick`
    // itself is untouched by this Story (clause 8): its detection semantics
    // are the no-regression guard for every pre-existing caller.
    let mut detected = DetectedOverrun::default();
    match record_tick(scheduler, journal, &mut detected, task, 1) {
        Ok(()) => TickAccounting::WithinBudget(task),
        Err(WcetError::UnknownTask) => TickAccounting::UnknownTask,
        // The task enforced against is the one `record_tick`'s own handler
        // named, not the one this function started with — so detection and
        // enforcement are literally the same value rather than two that
        // happen to agree. `on_overrun` fires before `record_tick` returns
        // `BudgetExceeded`, so this is never `None` on this arm; `task` is
        // the fail-closed fallback rather than an `unwrap`.
        Err(WcetError::BudgetExceeded) => {
            let offender = detected.task.unwrap_or(task);
            let disposition = disposition_for(policy);
            apply(scheduler, journal, offender, disposition);
            TickAccounting::Enforced { task: offender, disposition }
        }
    }
}

/// The [`OverrunHandler`] the enforcement path hands to [`record_tick`] —
/// the real consumer that trait's own doc comment has been waiting for since
/// `STORY-P0-02-04`.
///
/// It records rather than acts because `on_overrun` has no `&mut Scheduler`
/// to act *with*; the disposition is applied by [`apply`] immediately after
/// `record_tick` returns, on the same tick, with no path between the two.
#[derive(Default)]
struct DetectedOverrun {
    task: Option<TaskId>,
}

impl OverrunHandler for DetectedOverrun {
    fn on_overrun(&mut self, task: TaskId) {
        self.task = Some(task);
    }
}

/// Applies `disposition` to `task` and stamps the audit record for it.
///
/// The spoor's `COST` is the consumption total at the moment the budget was
/// crossed, read back from the scheduler *before* any arm resets it, so the
/// enforcement record and the `Action::Overrun` record it follows carry the
/// same number and can be paired. The degrade floor is not in the spoor
/// because it is not variable: an [`OverrunPolicy`] is immutable for a task's
/// whole life, so the floor is always recoverable from the declaration.
fn apply<const N: usize, const J: usize>(
    scheduler: &mut Scheduler<N>,
    journal: &mut SpoorJournal<J>,
    task: TaskId,
    disposition: OverrunDisposition,
) {
    let consumed = scheduler.wcet_state(task).map_or(0, |(consumed, _)| consumed);
    let action = match disposition {
        OverrunDisposition::RestartTask => Action::Restart,
        OverrunDisposition::DegradeTo(_) => Action::Degrade,
        OverrunDisposition::TripToSafeState => Action::Terminate,
    };
    let outcome = match disposition {
        // The task keeps running, in a diminished form: the budget was hit
        // and the ceiling enforced.
        OverrunDisposition::RestartTask | OverrunDisposition::DegradeTo(_) => Outcome::Capped,
        // The task does not keep running.
        OverrunDisposition::TripToSafeState => Outcome::Failed,
    };
    journal.append(Spoor::stamp(
        Category::Wcet,
        Actor::Kernel,
        action,
        outcome,
        task.index() as u16,
        consumed,
    ));

    match disposition {
        OverrunDisposition::RestartTask => {
            reset_budget_window(scheduler, journal, task);
            scheduler.set_state(task, TaskState::Ready);
        }
        OverrunDisposition::DegradeTo(floor) => {
            // Writes the task's *own* priority and nothing else
            // (`STORY-P1-04-04`, closing `LE-22`). If `task` holds a
            // `PriorityInheritingLock` and a waiter has boosted it, that boost
            // is a separate field and survives this call: the task keeps
            // running at the waiter's priority until it releases, and falls to
            // `floor` then. The enforcement decision is not lost and the
            // waiter is not starved — neither subsystem writes the quantity
            // the scheduler actually reads, which is derived from both.
            scheduler.set_base_priority(task, floor);
            reset_budget_window(scheduler, journal, task);
            scheduler.set_state(task, TaskState::Ready);
        }
        OverrunDisposition::TripToSafeState => {
            scheduler.set_state(task, TaskState::Finished);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sched::{TaskCreateError, WcetBudgetTicks};
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
        let task = sched
            .create_task(
                priority(1),
                WcetBudgetTicks(1000),
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();
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
        let task = sched
            .create_task(
                priority(1),
                WcetBudgetTicks(1000),
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();
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
        let task = sched
            .create_task(
                priority(1),
                WcetBudgetTicks(100),
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();
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
        let task = sched
            .create_task(
                priority(1),
                WcetBudgetTicks(1000),
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();
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
        let task = sched
            .create_task(
                priority(1),
                WcetBudgetTicks(100),
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();
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
        let task = sched
            .create_task(
                priority(1),
                WcetBudgetTicks(100),
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();

        assert_eq!(reset_budget_window(&mut sched, &mut journal, task), Some(()));
        assert!(journal.is_empty());
    }

    // An unknown task fails closed without calling the handler or stamping
    // anything.
    #[test]
    fn record_tick_against_an_unknown_task_fails_closed() {
        let mut sched: Scheduler<1> = Scheduler::new();
        let mut journal: SpoorJournal<4> = SpoorJournal::new();
        let task = sched
            .create_task(
                priority(1),
                WcetBudgetTicks(100),
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();
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
        let task = sched
            .create_task(
                priority(1),
                WcetBudgetTicks(100),
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();
        sched.free_task_for_test(task);

        assert_eq!(reset_budget_window(&mut sched, &mut journal, task), None);
        assert!(journal.is_empty());
    }

    // ---------------------------------------------------------------
    // STORY-P1-04-02 — enforcement. Everything above this line is the
    // bookkeeping half and is `TEST-P1-04-02-A` clause 8's no-regression
    // guard: not one of those tests was modified by this Story beyond the
    // creation-call signature every call site in the workspace took.
    // ---------------------------------------------------------------

    fn one_task(sched: &mut Scheduler<4>, p: u8, budget: u32, policy: OverrunPolicy) -> TaskId {
        sched.create_task(priority(p), WcetBudgetTicks(budget), policy, dummy_entry).unwrap()
    }

    fn one_task_ok(sched: &mut Scheduler<4>) -> bool {
        sched
            .create_task(
                priority(1),
                WcetBudgetTicks(100),
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .is_ok()
    }

    // Clause 1: the attribution rule, both arms. A tick that interrupted a
    // task is charged to that task; a tick that did not is charged to
    // nobody.
    #[test]
    fn a_tick_is_attributed_to_the_running_task_and_otherwise_to_nobody() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let task = one_task(&mut sched, 5, 100, OverrunPolicy::TripToSafeState);

        assert_eq!(attribute_tick(Some(task)), TickAttribution::Task(task));
        assert_eq!(attribute_tick(None), TickAttribution::Nobody);
    }

    // Clause 1, the failure this rule exists to prevent: ticks that land in
    // the dispatcher or an idle context must not be charged to whichever
    // task ran most recently. Asserted through the real accounting path
    // against a real `Scheduler`, because the bug being guarded against is
    // precisely a hidden "last task" that a pure-function test over invented
    // inputs could never see.
    #[test]
    fn unattributed_ticks_do_not_accumulate_against_the_task_that_ran_last() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<16> = SpoorJournal::new();
        let task = one_task(&mut sched, 5, 100, OverrunPolicy::TripToSafeState);

        for _ in 0..10 {
            assert_eq!(
                account_tick(&mut sched, &mut journal, Some(task)),
                TickAccounting::WithinBudget(task)
            );
        }
        assert_eq!(sched.wcet_state(task).map(|(consumed, _)| consumed), Some(10));

        // A thousand ticks in the dispatcher. If any of them found their way
        // onto the last task to run, this task would be ten times over its
        // budget of 100 and would have been terminated.
        for _ in 0..1_000 {
            assert_eq!(account_tick(&mut sched, &mut journal, None), TickAccounting::Unattributed);
        }
        assert_eq!(
            sched.wcet_state(task).map(|(consumed, _)| consumed),
            Some(10),
            "kernel time belongs to no task's budget"
        );
        assert_eq!(sched.state_of(task), Some(TaskState::Ready));
        assert!(journal.is_empty(), "a within-budget tick stamps nothing, attributed or not");
    }

    // Clause 2: every declared policy maps to exactly one disposition. The
    // `match` in `disposition_for` is exhaustive over `OverrunPolicy`, so a
    // fourth arm added later cannot fall through — it will not compile.
    #[test]
    fn the_policy_decision_table_is_total_and_pins_every_arm() {
        assert_eq!(disposition_for(OverrunPolicy::Restart), OverrunDisposition::RestartTask);
        assert_eq!(
            disposition_for(OverrunPolicy::Degrade(priority(3))),
            OverrunDisposition::DegradeTo(priority(3))
        );
        assert_eq!(
            disposition_for(OverrunPolicy::TripToSafeState),
            OverrunDisposition::TripToSafeState
        );
    }

    // Clause 2, second half, stated as a test rather than as a comment:
    // `kernel::fault::Disposition::of` still reads exactly one field and
    // still answers exactly as it did before this Story. If a later change
    // routed overruns through it, this is what would have to be edited.
    #[test]
    fn fault_disposition_is_unchanged_by_this_story() {
        use crate::fault::{Disposition, FaultReport, FaultingContext};
        let mut sched: Scheduler<4> = Scheduler::new();
        let task = one_task(&mut sched, 5, 100, OverrunPolicy::Restart);

        for vector in [6u64, 13, 14, 0, 255] {
            assert_eq!(
                Disposition::of(&FaultReport { vector, context: FaultingContext::Task(task) }),
                Disposition::TerminateTask(task),
                "the vector must not influence the decision"
            );
            assert_eq!(
                Disposition::of(&FaultReport { vector, context: FaultingContext::Kernel }),
                Disposition::HaltSystem
            );
        }
        // And an overrun does not become one of its arms: the overrun
        // decision table is a different function over a different input.
        assert_eq!(disposition_for(OverrunPolicy::Restart), OverrunDisposition::RestartTask);
    }

    // Clause 3: a task cannot hold a budget with no declared consequence —
    // there is no defaulted arm, and the declaration is immutable for the
    // task's whole life (there is no setter, only `overrun_policy_of`).
    #[test]
    fn every_task_carries_the_policy_it_declared_and_it_cannot_be_changed() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let restart = one_task(&mut sched, 5, 100, OverrunPolicy::Restart);
        let degrade = one_task(&mut sched, 20, 100, OverrunPolicy::Degrade(priority(2)));
        let trip = one_task(&mut sched, 5, 100, OverrunPolicy::TripToSafeState);

        assert_eq!(sched.overrun_policy_of(restart), Some(OverrunPolicy::Restart));
        assert_eq!(sched.overrun_policy_of(degrade), Some(OverrunPolicy::Degrade(priority(2))));
        assert_eq!(sched.overrun_policy_of(trip), Some(OverrunPolicy::TripToSafeState));

        sched.free_task_for_test(trip);
        assert_eq!(sched.overrun_policy_of(trip), None, "an unknown task declares nothing");
    }

    // Clause 3: a degrade floor *above* the task's own priority is rejected,
    // not clamped. An overrun that raised a task's priority would turn a
    // missed deadline into a route to a criticality level nobody granted.
    #[test]
    fn a_degrade_floor_above_the_declared_priority_is_rejected_not_clamped() {
        let mut sched: Scheduler<4> = Scheduler::new();
        assert_eq!(
            sched.create_task(
                priority(5),
                WcetBudgetTicks(100),
                OverrunPolicy::Degrade(priority(25)),
                dummy_entry
            ),
            Err(TaskCreateError::DegradeFloorAbovePriority)
        );
        // Rejected before a slot was claimed: all four are still available.
        for _ in 0..4 {
            assert!(one_task_ok(&mut sched));
        }

        // Equal is allowed — a floor at the task's own priority is a
        // declaration that a degrade changes nothing but the budget window,
        // which is a choice, not an error.
        let mut sched: Scheduler<4> = Scheduler::new();
        assert!(sched
            .create_task(
                priority(5),
                WcetBudgetTicks(100),
                OverrunPolicy::Degrade(priority(5)),
                dummy_entry
            )
            .is_ok());
    }

    // Clause 2 + 7, the `Restart` arm: the budget window is reset, the task
    // returns to Ready, and the decision is audited. The context rewind is
    // the caller's — signalled by the returned disposition, and proven in
    // `fixture_wcet_restart`.
    #[test]
    fn the_restart_arm_resets_the_window_requeues_the_task_and_audits() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<16> = SpoorJournal::new();
        let task = one_task(&mut sched, 5, 3, OverrunPolicy::Restart);
        sched.set_state(task, TaskState::Running).unwrap();

        for _ in 0..3 {
            assert_eq!(
                account_tick(&mut sched, &mut journal, Some(task)),
                TickAccounting::WithinBudget(task),
                "consumption exactly equal to the budget is not an overrun"
            );
        }
        assert_eq!(
            account_tick(&mut sched, &mut journal, Some(task)),
            TickAccounting::Enforced { task, disposition: OverrunDisposition::RestartTask }
        );

        assert_eq!(sched.wcet_state(task).map(|(consumed, _)| consumed), Some(0));
        assert_eq!(sched.state_of(task), Some(TaskState::Ready));
        assert_eq!(sched.priority_of(task), Some(priority(5)), "restart does not touch priority");

        let spoors: Vec<Spoor> = journal.iter().collect();
        assert_eq!(spoors.len(), 3, "overrun, then the arm taken, then the window reset");
        assert_eq!(spoors[0].action(), Action::Overrun);
        assert_eq!(spoors[0].cost(), 4);
        assert_eq!(spoors[1].category(), Category::Wcet);
        assert_eq!(spoors[1].who(), Actor::Kernel);
        assert_eq!(spoors[1].action(), Action::Restart);
        assert_eq!(spoors[1].outcome(), Outcome::Capped);
        assert_eq!(spoors[1].target(), task.index() as u16);
        assert_eq!(spoors[1].cost(), 4, "the enforcement record pairs with the overrun record");
        assert_eq!(spoors[2].action(), Action::ResetBudget);
    }

    // Clause 2 + 7, the `Degrade` arm: the priority actually drops to the
    // declared floor, the window resets, the task stays Ready. That the drop
    // *changes a scheduling decision* is `fixture_wcet_degrade`'s claim; here
    // the state change itself is pinned, against a real competitor.
    #[test]
    fn the_degrade_arm_lowers_priority_to_the_floor_and_keeps_the_task_ready() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<16> = SpoorJournal::new();
        let task = one_task(&mut sched, 25, 2, OverrunPolicy::Degrade(priority(5)));
        // A competitor between the offender's priority and its floor: after
        // the degrade it must start winning selections it lost before.
        let competitor = one_task(&mut sched, 15, 1_000, OverrunPolicy::TripToSafeState);
        assert_eq!(sched.highest_priority_ready(), Some(task), "25 outranks 15");

        account_tick(&mut sched, &mut journal, Some(task));
        account_tick(&mut sched, &mut journal, Some(task));
        assert_eq!(
            account_tick(&mut sched, &mut journal, Some(task)),
            TickAccounting::Enforced {
                task,
                disposition: OverrunDisposition::DegradeTo(priority(5))
            }
        );

        assert_eq!(sched.priority_of(task), Some(priority(5)));
        assert_eq!(sched.wcet_state(task).map(|(consumed, _)| consumed), Some(0));
        assert_eq!(sched.state_of(task), Some(TaskState::Ready));
        assert_eq!(
            sched.highest_priority_ready(),
            Some(competitor),
            "the degrade must change who the dispatcher would choose, not just a number"
        );

        let spoors: Vec<Spoor> = journal.iter().collect();
        assert_eq!(spoors[1].action(), Action::Degrade);
        assert_eq!(spoors[1].outcome(), Outcome::Capped);
        assert_eq!(spoors[1].target(), task.index() as u16);
    }

    // Clause 2 + 7, the `TripToSafeState` arm: the task is Finished — not
    // Ready, not merely flagged — and the caller is told to enter its safe
    // state. A Finished task is never selected again.
    #[test]
    fn the_trip_arm_finishes_the_task_and_it_is_never_selected_again() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<16> = SpoorJournal::new();
        let task = one_task(&mut sched, 25, 1, OverrunPolicy::TripToSafeState);

        account_tick(&mut sched, &mut journal, Some(task));
        assert_eq!(
            account_tick(&mut sched, &mut journal, Some(task)),
            TickAccounting::Enforced { task, disposition: OverrunDisposition::TripToSafeState }
        );

        assert_eq!(sched.state_of(task), Some(TaskState::Finished));
        assert_eq!(sched.highest_priority_ready(), None);

        let spoors: Vec<Spoor> = journal.iter().collect();
        assert_eq!(spoors.len(), 2, "overrun, then termination — no window reset for a dead task");
        assert_eq!(spoors[1].action(), Action::Terminate);
        assert_eq!(spoors[1].outcome(), Outcome::Failed);
        assert_eq!(spoors[1].target(), task.index() as u16);
    }

    // Clause 7, stated structurally: every arm changes the task's state or
    // its priority, and every arm stamps. There is no arm that observes an
    // overrun and does nothing. Driven over the whole enumeration, so an arm
    // added later fails to compile here rather than passing silently.
    #[test]
    fn no_disposition_arm_observes_an_overrun_and_does_nothing() {
        let policies = [
            OverrunPolicy::Restart,
            OverrunPolicy::Degrade(priority(1)),
            OverrunPolicy::TripToSafeState,
        ];
        for policy in policies {
            // Named exhaustively rather than with a wildcard, so this test is
            // one of the places a fourth arm must be handled.
            match policy {
                OverrunPolicy::Restart
                | OverrunPolicy::Degrade(_)
                | OverrunPolicy::TripToSafeState => {}
            }

            let mut sched: Scheduler<4> = Scheduler::new();
            let mut journal: SpoorJournal<16> = SpoorJournal::new();
            let task = one_task(&mut sched, 20, 1, policy);
            sched.set_state(task, TaskState::Running).unwrap();
            let before_state = sched.state_of(task);
            let before_priority = sched.priority_of(task);

            account_tick(&mut sched, &mut journal, Some(task));
            let result = account_tick(&mut sched, &mut journal, Some(task));

            assert!(matches!(result, TickAccounting::Enforced { .. }), "{policy:?} must enforce");
            let changed =
                sched.state_of(task) != before_state || sched.priority_of(task) != before_priority;
            assert!(changed, "{policy:?} left the task exactly as it was");
            assert!(!journal.is_empty(), "{policy:?} stamped nothing");
        }
    }

    // Fails closed on a tick attributed to a task that is no longer live:
    // nothing charged, nothing stamped, no state anywhere changed. The same
    // contract `record_tick` itself has always had, carried through the
    // enforcement path rather than re-decided by it.
    #[test]
    fn accounting_against_an_unknown_task_fails_closed() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<16> = SpoorJournal::new();
        let task = one_task(&mut sched, 5, 1, OverrunPolicy::TripToSafeState);
        sched.free_task_for_test(task);

        assert_eq!(account_tick(&mut sched, &mut journal, Some(task)), TickAccounting::UnknownTask);
        assert!(journal.is_empty());
    }

    // The degrade arm at its own floor is idempotent: a task that overruns
    // again after being degraded is degraded again to no further effect, and
    // keeps running at the floor. Pinned deliberately — escalation on
    // repeated overrun is a policy this Story has no requirement for, and
    // inventing one silently would be a scheduling change nobody asked for.
    #[test]
    fn a_repeated_overrun_at_the_floor_degrades_again_to_no_further_effect() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<32> = SpoorJournal::new();
        let task = one_task(&mut sched, 20, 1, OverrunPolicy::Degrade(priority(4)));

        account_tick(&mut sched, &mut journal, Some(task));
        account_tick(&mut sched, &mut journal, Some(task));
        assert_eq!(sched.priority_of(task), Some(priority(4)));

        account_tick(&mut sched, &mut journal, Some(task));
        let again = account_tick(&mut sched, &mut journal, Some(task));
        assert_eq!(
            again,
            TickAccounting::Enforced {
                task,
                disposition: OverrunDisposition::DegradeTo(priority(4))
            }
        );
        assert_eq!(sched.priority_of(task), Some(priority(4)), "already at the floor");
        assert_eq!(sched.state_of(task), Some(TaskState::Ready), "and still running");
    }
}
