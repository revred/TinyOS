//! Priority inheritance on lock contention (`STORY-P0-02-03`).
//!
//! [`PriorityInheritingLock`] is the classic priority-inversion mitigation
//! `README.md` Design Pillar 1 and Goal `G-RT-1` both name explicitly: when
//! a higher-priority task contends for a lock a lower-priority task
//! already holds, the holder is temporarily boosted to the waiter's
//! priority (via [`Scheduler::inherit_priority`]) so a *third*, medium-priority
//! task can't keep preempting the holder and starving the high-priority
//! waiter indefinitely. The boost is released — the holder's priority
//! restored to what it was before contention — the moment the lock is
//! unlocked, with no path that leaves a task permanently boosted
//! (`STORY-P0-02-03` acceptance criterion 2).
//!
//! **Scope note — the behavioral half, and when it closed.** This module's
//! host tests prove the *bookkeeping* of priority inheritance end to end (a
//! contended lock boosts the holder; an uncontended unlock restores it
//! exactly) by asserting on the resulting priority *values*. When
//! `STORY-P0-02-03` was written that was as far as its acceptance criteria
//! could be verified: this kernel had no priority-based dispatch loop —
//! `context::switch` was a raw two-context primitive invoked explicitly by
//! whoever called it — so the *behavioral* claim, that a real, running
//! medium-priority task is actually kept from starving the now-boosted
//! holder, had no way to be demonstrated. That gap was recorded here as a
//! concrete prerequisite rather than assumed away.
//!
//! `STORY-P0-02-05` closed half of it (`dispatch::run_once` selects and
//! actually runs a boosted holder ahead of an uninvolved higher-static-priority
//! task), and **`STORY-P1-04-01` closed the rest**: under genuine
//! timer-driven preemption, `kernel::fixture_priority_inversion` runs the
//! classic three-task scenario for real and asserts both the dispatch order
//! taken (`low → high → low → high`) and that the medium task — `Ready`
//! throughout, and demonstrably able to run afterwards — makes no progress
//! at all while the boosted holder finishes. See `TEST-P1-04-01-A` clause 6.
//!
//! **`STORY-P0-06-03`**: every boost and every restore also stamps a
//! [`crate::spoor::Spoor`] into a caller-supplied
//! [`crate::spoor_journal::SpoorJournal`] — `FEAT-P0-06`'s own exit
//! criterion that at least one real subsystem actually emit spoors through
//! its API, not just compile against it. The journal is a parameter, not a
//! field this type owns (the same Dependency Inversion `crate::wcet`'s
//! `OverrunHandler` already established): `PriorityInheritingLock` picks no
//! capacity `N` of its own, so wiring in a real, sized journal later is
//! additive, not a rewrite. Only the two priority-*mutating* events are
//! stamped — a contended `try_lock` that doesn't boost (contender already
//! outranked) and an `unlock` with nothing to restore are no-ops on
//! priority, so they stamp nothing, matching this journal's own "audit
//! trail of what changed" scope rather than every call attempt.

use crate::sched::{Priority, Scheduler, TaskId};
use crate::spoor::{Action, Actor, Category, Outcome, Spoor};
use crate::spoor_journal::SpoorJournal;

/// Errors [`PriorityInheritingLock::try_lock`]/`unlock` fail closed with,
/// per `agent/CODING_STANDARDS.md`'s "no stringly-typed errors" rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockError {
    /// The lock is already held by a different task (or, for a reentrant
    /// attempt, by the caller itself — this lock is not reentrant).
    AlreadyLocked,
    /// `unlock` was called by a task that isn't the current holder.
    NotHeldByCaller,
    /// `task` (or the current holder, when boosting) doesn't identify a
    /// currently live task in the given [`Scheduler`].
    UnknownTask,
}

/// A lock that boosts its current holder's priority to a contending
/// waiter's priority for as long as the waiter is blocked on it, and
/// restores the holder's original priority on release.
///
/// This module owns only the priority-boost bookkeeping — actually
/// blocking a waiting task (parking it until the lock frees) is a
/// scheduler/dispatcher responsibility this kernel doesn't have yet (see
/// this module's own doc comment); `try_lock` reports contention via
/// [`LockError::AlreadyLocked`] rather than blocking itself.
pub struct PriorityInheritingLock {
    holder: Option<TaskId>,
}

impl PriorityInheritingLock {
    /// Creates an unlocked lock. `const fn`: usable in a `static`
    /// initializer, no heap allocation.
    pub const fn new() -> Self {
        PriorityInheritingLock { holder: None }
    }

    /// Attempts to acquire the lock for `task` (currently running at
    /// `task_priority` per `scheduler`).
    ///
    /// If the lock is free, `task` becomes the holder and this returns
    /// `Ok(())`. If it's already held by a different task, this boosts the
    /// holder's priority to `task_priority` when that's higher than the
    /// holder's current priority (priority inheritance) and returns
    /// [`LockError::AlreadyLocked`] — the caller is responsible for
    /// whatever blocking/retry discipline it uses while contended, since
    /// this module has no dispatcher to park it against (see the module
    /// doc comment).
    ///
    /// A boost stamps a [`Spoor`] (`Category::Lock`, `Action::Boost`) into
    /// `journal`, `TARGET` the holder's task-pool index and `COST` the
    /// boosted-to priority value — see the module doc comment
    /// (`STORY-P0-06-03`).
    pub fn try_lock<const N: usize, const J: usize>(
        &mut self,
        scheduler: &mut Scheduler<N>,
        journal: &mut SpoorJournal<J>,
        task: TaskId,
        task_priority: Priority,
    ) -> Result<(), LockError> {
        match self.holder {
            None => {
                self.holder = Some(task);
                Ok(())
            }
            Some(holder) => {
                let holder_priority =
                    scheduler.priority_of(holder).ok_or(LockError::UnknownTask)?;
                if task_priority > holder_priority {
                    // Records only what is inherited. Nothing about the
                    // holder's *own* priority is captured, so there is no
                    // stored value for a later degrade to invalidate
                    // (`STORY-P1-04-04`, closing `LE-22`).
                    scheduler
                        .inherit_priority(holder, task_priority)
                        .ok_or(LockError::UnknownTask)?;
                    journal.append(Spoor::stamp(
                        Category::Lock,
                        Actor::Kernel,
                        Action::Boost,
                        Outcome::Ok,
                        holder.index() as u16,
                        task_priority.value() as u32,
                    ));
                }
                Err(LockError::AlreadyLocked)
            }
        }
    }

    /// Releases the lock, currently held by `task`.
    ///
    /// Restores `task`'s priority to what it was before any inheritance
    /// boost this contention cycle applied (a no-op if the lock was never
    /// contended while held) — deterministic release, per
    /// `STORY-P0-02-03` acceptance criterion 2: no path leaves `task`
    /// permanently boosted.
    ///
    /// A restore stamps a [`Spoor`] (`Category::Lock`, `Action::Restore`)
    /// into `journal`, `TARGET` `task`'s task-pool index and `COST` the
    /// restored-to priority value — no spoor is stamped when there was
    /// nothing to restore (`STORY-P0-06-03`).
    ///
    /// # How this composes with a WCET degrade (`STORY-P1-04-04`, `LE-22`)
    ///
    /// It releases the *inheritance* and nothing else. The task falls back to
    /// its own priority **as it stands now**, so a [`crate::wcet`] degrade
    /// applied while this lock was held survives the release — the task leaves
    /// `unlock` at its floor, not at the priority it had before the overrun.
    ///
    /// The converse also holds, and is the more important half: a degrade
    /// applied to a boosted holder does not disturb the boost, so a waiter
    /// blocked on this lock keeps the priority it needs until the holder
    /// actually releases.
    ///
    /// Neither subsystem writes the quantity the scheduler reads. It is
    /// `max(base, inherited)`, derived on demand, which is why there is no
    /// stored decision left for either to invalidate. **A conditional restore
    /// would not have been a fix** — it stores the same decision and merely
    /// narrows the window in which it is stale.
    pub fn unlock<const N: usize, const J: usize>(
        &mut self,
        scheduler: &mut Scheduler<N>,
        journal: &mut SpoorJournal<J>,
        task: TaskId,
    ) -> Result<(), LockError> {
        match self.holder {
            Some(holder) if holder == task => {
                let was_boosted =
                    scheduler.inherited_priority_of(task).ok_or(LockError::UnknownTask)?.is_some();
                let fell_back_to =
                    scheduler.release_inheritance(task).ok_or(LockError::UnknownTask)?;
                // Only a release that actually changed the effective priority
                // is an audit event, matching this journal's "what changed"
                // scope. The stamped `COST` is the priority the task really
                // lands on — including a degrade applied while it held the
                // lock, which the previous implementation would have
                // overwritten and mis-reported.
                if was_boosted {
                    journal.append(Spoor::stamp(
                        Category::Lock,
                        Actor::Kernel,
                        Action::Restore,
                        Outcome::Ok,
                        task.index() as u16,
                        fell_back_to.value() as u32,
                    ));
                }
                self.holder = None;
                Ok(())
            }
            _ => Err(LockError::NotHeldByCaller),
        }
    }
}

impl Default for PriorityInheritingLock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sched::{OverrunPolicy, WcetBudgetTicks};

    #[allow(clippy::empty_loop)]
    extern "C" fn dummy_entry() -> ! {
        loop {}
    }

    const BUDGET: WcetBudgetTicks = WcetBudgetTicks(1000);

    fn priority(value: u8) -> Priority {
        Priority::try_new(value).expect("value is in range")
    }

    // STORY-P0-02-03 AC1: the classic three-task priority-inversion
    // scenario — low holds the lock, high contends for it, medium is a
    // third, independent task that outranks low's *original* priority but
    // (per priority inheritance) must not outrank low's *boosted*
    // priority, so a real dispatcher would run low-holding-the-lock to
    // completion ahead of medium rather than letting medium repeatedly
    // preempt it and starve high. See this module's doc comment for why
    // this test asserts on the resulting priority values rather than on
    // live preemption (no ready-queue dispatcher exists yet to preempt
    // anything).
    #[test]
    fn contention_boosts_the_holder_above_the_medium_priority_task() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<4> = SpoorJournal::new();
        let low = sched
            .create_task(priority(5), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();
        let medium = sched
            .create_task(priority(15), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();
        let high = sched
            .create_task(priority(25), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();

        let mut lock = PriorityInheritingLock::new();
        assert_eq!(lock.try_lock(&mut sched, &mut journal, low, priority(5)), Ok(()));

        // Before contention: low's priority is unchanged, and (the classic
        // inversion problem) sits below medium's.
        assert_eq!(sched.priority_of(low), Some(priority(5)));
        assert!(sched.priority_of(low) < sched.priority_of(medium));

        // High contends for the lock low holds.
        assert_eq!(
            lock.try_lock(&mut sched, &mut journal, high, priority(25)),
            Err(LockError::AlreadyLocked)
        );

        // Priority inheritance: low is now boosted to high's priority,
        // outranking medium — a real dispatcher would no longer let medium
        // preempt low ahead of it finishing and releasing the lock high
        // needs.
        assert_eq!(sched.priority_of(low), Some(priority(25)));
        assert!(sched.priority_of(low) > sched.priority_of(medium));
        // medium's own priority is untouched by contention it isn't party to.
        assert_eq!(sched.priority_of(medium), Some(priority(15)));

        // STORY-P0-06-03: the unlocking-free acquire stamps nothing (no
        // boost happened), the granting acquire stamps nothing either —
        // only the boost itself did.
        let spoors: std::vec::Vec<Spoor> = journal.iter().collect();
        assert_eq!(spoors.len(), 1);
        assert_eq!(spoors[0].category(), Category::Lock);
        assert_eq!(spoors[0].action(), Action::Boost);
        assert_eq!(spoors[0].outcome(), Outcome::Ok);
        assert_eq!(spoors[0].target(), low.index() as u16);
        assert_eq!(spoors[0].cost(), 25);
    }

    // STORY-P0-02-03 AC2: releasing the lock restores the holder's
    // original priority exactly — no path leaves it permanently boosted.
    #[test]
    fn unlock_restores_the_holders_original_priority() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<4> = SpoorJournal::new();
        let low = sched
            .create_task(priority(5), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();
        let high = sched
            .create_task(priority(25), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();

        let mut lock = PriorityInheritingLock::new();
        lock.try_lock(&mut sched, &mut journal, low, priority(5)).unwrap();
        lock.try_lock(&mut sched, &mut journal, high, priority(25)).unwrap_err();
        assert_eq!(sched.priority_of(low), Some(priority(25)));

        assert_eq!(lock.unlock(&mut sched, &mut journal, low), Ok(()));
        assert_eq!(sched.priority_of(low), Some(priority(5)));

        // STORY-P0-06-03: boost then restore — exactly two spoors, in
        // order, the restore naming the pre-boost priority it returned to.
        let spoors: std::vec::Vec<Spoor> = journal.iter().collect();
        assert_eq!(spoors.len(), 2);
        assert_eq!(spoors[0].action(), Action::Boost);
        assert_eq!(spoors[1].action(), Action::Restore);
        assert_eq!(spoors[1].target(), low.index() as u16);
        assert_eq!(spoors[1].cost(), 5);
    }

    // A lock that was never contended releases as a no-op on priority —
    // there's nothing to restore.
    #[test]
    fn unlock_without_contention_leaves_priority_unchanged() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let mut journal: SpoorJournal<4> = SpoorJournal::new();
        let low = sched
            .create_task(priority(5), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();

        let mut lock = PriorityInheritingLock::new();
        lock.try_lock(&mut sched, &mut journal, low, priority(5)).unwrap();
        assert_eq!(lock.unlock(&mut sched, &mut journal, low), Ok(()));
        assert_eq!(sched.priority_of(low), Some(priority(5)));

        // STORY-P0-06-03: nothing to boost, nothing to restore — the
        // journal stays empty rather than recording a no-op event.
        assert!(journal.is_empty());
    }

    // A lower-priority contender must not de-boost (or otherwise disturb)
    // the holder — inheritance only ever raises, never lowers, priority.
    #[test]
    fn a_lower_priority_contender_does_not_change_the_holders_priority() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<4> = SpoorJournal::new();
        let holder = sched
            .create_task(priority(20), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();
        let low_contender = sched
            .create_task(priority(5), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();

        let mut lock = PriorityInheritingLock::new();
        lock.try_lock(&mut sched, &mut journal, holder, priority(20)).unwrap();
        lock.try_lock(&mut sched, &mut journal, low_contender, priority(5)).unwrap_err();

        assert_eq!(sched.priority_of(holder), Some(priority(20)));
        // Releasing after a non-boosting contention still succeeds and
        // changes nothing.
        assert_eq!(lock.unlock(&mut sched, &mut journal, holder), Ok(()));
        assert_eq!(sched.priority_of(holder), Some(priority(20)));

        // STORY-P0-06-03: a contention that never boosts (contender
        // outranked) stamps nothing.
        assert!(journal.is_empty());
    }

    // Repeated contention from progressively higher-priority waiters keeps
    // boosting the holder, but a single unlock always restores the
    // *original* (pre-contention) priority, never an intermediate boosted
    // value — no path leaves the holder boosted after release.
    #[test]
    fn repeated_boosts_still_restore_the_original_priority_on_unlock() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<4> = SpoorJournal::new();
        let low = sched
            .create_task(priority(5), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();
        let mid_waiter = sched
            .create_task(priority(15), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();
        let high_waiter = sched
            .create_task(priority(25), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();

        let mut lock = PriorityInheritingLock::new();
        lock.try_lock(&mut sched, &mut journal, low, priority(5)).unwrap();
        lock.try_lock(&mut sched, &mut journal, mid_waiter, priority(15)).unwrap_err();
        assert_eq!(sched.priority_of(low), Some(priority(15)));
        lock.try_lock(&mut sched, &mut journal, high_waiter, priority(25)).unwrap_err();
        assert_eq!(sched.priority_of(low), Some(priority(25)));

        assert_eq!(lock.unlock(&mut sched, &mut journal, low), Ok(()));
        assert_eq!(sched.priority_of(low), Some(priority(5)));

        // STORY-P0-06-03: two escalating boosts (15, then 25), then a
        // single restore back to the *original* priority (5) — never an
        // intermediate boosted value, mirroring AC2's own priority-value
        // guarantee at the audit-trail level.
        let spoors: std::vec::Vec<Spoor> = journal.iter().collect();
        assert_eq!(spoors.len(), 3);
        assert_eq!(spoors[0].action(), Action::Boost);
        assert_eq!(spoors[0].cost(), 15);
        assert_eq!(spoors[1].action(), Action::Boost);
        assert_eq!(spoors[1].cost(), 25);
        assert_eq!(spoors[2].action(), Action::Restore);
        assert_eq!(spoors[2].cost(), 5);
    }

    // Attempting to unlock a lock held by a different task fails closed
    // rather than releasing someone else's lock.
    #[test]
    fn unlock_by_a_non_holder_is_rejected() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let mut journal: SpoorJournal<4> = SpoorJournal::new();
        let holder = sched
            .create_task(priority(10), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();
        let impostor = sched
            .create_task(priority(10), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();

        let mut lock = PriorityInheritingLock::new();
        lock.try_lock(&mut sched, &mut journal, holder, priority(10)).unwrap();
        assert_eq!(
            lock.unlock(&mut sched, &mut journal, impostor),
            Err(LockError::NotHeldByCaller)
        );
    }

    // A reentrant lock attempt by the current holder is rejected (this
    // lock is not reentrant), not silently granted or deadlocked forever.
    #[test]
    fn reentrant_lock_attempt_by_the_holder_is_rejected() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let mut journal: SpoorJournal<4> = SpoorJournal::new();
        let holder = sched
            .create_task(priority(10), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();

        let mut lock = PriorityInheritingLock::new();
        lock.try_lock(&mut sched, &mut journal, holder, priority(10)).unwrap();
        assert_eq!(
            lock.try_lock(&mut sched, &mut journal, holder, priority(10)),
            Err(LockError::AlreadyLocked)
        );
    }

    // ---- `TEST-P1-04-04-A`: degrade and inheritance compose (`LE-22`) ----
    //
    // Every composition test below fails on the pre-`STORY-P1-04-04`
    // implementation, which is the point. The old code captured the holder's
    // pre-boost priority at boost time and replayed it on unlock, so a degrade
    // applied in between was silently discarded — and the degrade, writing the
    // same field, silently discarded the boost.

    /// Drives `task` past its budget through the **real** enforcement path.
    ///
    /// `wcet::account_tick` is what the timer ISR calls and the only entry
    /// point that both detects an overrun and applies the declared policy;
    /// `record_tick` alone only detects. Testing the composition against
    /// anything less than the real path would prove nothing about the system
    /// that ships.
    fn overrun<const N: usize, const J: usize>(
        sched: &mut Scheduler<N>,
        journal: &mut SpoorJournal<J>,
        task: TaskId,
    ) {
        for _ in 0..3 {
            crate::wcet::account_tick(sched, journal, Some(task));
        }
    }

    /// The three-task scenario plus an overrun. `low` (5, degrade floor 2)
    /// holds; `high` (25) contends; `medium` (15) is uninvolved and must never
    /// outrank the boosted holder.
    #[allow(clippy::type_complexity)]
    fn contended_holder_that_overruns(
    ) -> (Scheduler<4>, SpoorJournal<16>, PriorityInheritingLock, TaskId, TaskId) {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<16> = SpoorJournal::new();
        let low = sched
            .create_task(
                priority(5),
                WcetBudgetTicks(2),
                OverrunPolicy::Degrade(priority(2)),
                dummy_entry,
            )
            .expect("slot available");
        let high = sched
            .create_task(priority(25), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .expect("slot available");
        let medium = sched
            .create_task(priority(15), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .expect("slot available");

        let mut lock = PriorityInheritingLock::new();
        lock.try_lock(&mut sched, &mut journal, low, priority(5)).expect("lock is free");
        assert_eq!(
            lock.try_lock(&mut sched, &mut journal, high, priority(25)),
            Err(LockError::AlreadyLocked),
            "high contends"
        );
        assert_eq!(sched.priority_of(low), Some(priority(25)), "boosted to the waiter");
        (sched, journal, lock, low, medium)
    }

    // Clause 2. The assertion the old implementation fails: after a degrade
    // taken while boosted, `unlock` must land the task on its *floor*, not on
    // the priority it held before the overrun that degraded it.
    #[test]
    fn a_degrade_taken_while_boosted_survives_the_unlock() {
        let (mut sched, mut journal, mut lock, low, _medium) = contended_holder_that_overruns();
        // Three ticks against a budget of 2, through the real enforcement
        // path -- `account_tick` is what the timer ISR calls, and it is the
        // only caller that both detects and applies.
        overrun(&mut sched, &mut journal, low);

        // Clause 3: the boost is intact while `high` is still blocked.
        assert_eq!(
            sched.priority_of(low),
            Some(priority(25)),
            "the waiter still needs the holder to finish"
        );
        assert_eq!(sched.base_priority_of(low), Some(priority(2)), "but the degrade landed");

        lock.unlock(&mut sched, &mut journal, low).expect("holder releases");

        assert_eq!(
            sched.priority_of(low),
            Some(priority(2)),
            "falls to the degraded floor, NOT to the pre-boost 5 — this is LE-22"
        );
        assert_eq!(sched.inherited_priority_of(low), Some(None), "nothing left inherited");
    }

    // Clause 3, as the property that matters rather than as values: an
    // uninvolved medium task must not outrank the holder at any point between
    // the boost and the release, including across the degrade.
    #[test]
    fn an_overrun_never_lets_an_uninvolved_task_outrank_the_boosted_holder() {
        let (mut sched, mut journal, mut lock, low, medium) = contended_holder_that_overruns();
        assert!(sched.priority_of(low) > sched.priority_of(medium), "before the overrun");
        overrun(&mut sched, &mut journal, low);
        assert!(
            sched.priority_of(low) > sched.priority_of(medium),
            "and after it — the degrade must not re-create the inversion the boost prevents"
        );

        // Only once the waiter is released does medium legitimately outrank it.
        lock.unlock(&mut sched, &mut journal, low).expect("holder releases");
        assert!(sched.priority_of(low) < sched.priority_of(medium));
    }

    // Clause 1: the two writers cannot reach each other's field, in both
    // directions, with no lock or overrun machinery involved.
    #[test]
    fn the_two_priority_writers_are_independent() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let task = sched
            .create_task(priority(10), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .expect("slot available");

        sched.inherit_priority(task, priority(20)).expect("task is live");
        sched.set_base_priority(task, priority(3)).expect("task is live");
        assert_eq!(sched.priority_of(task), Some(priority(20)), "degrade cannot cancel a boost");

        assert_eq!(sched.release_inheritance(task), Some(priority(3)));
        assert_eq!(
            sched.priority_of(task),
            Some(priority(3)),
            "and releasing the boost cannot undo the degrade"
        );
    }

    // Clause 4: inheritance raises and never lowers, structurally.
    #[test]
    fn inheritance_never_lowers_an_already_higher_inherited_priority() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let task = sched
            .create_task(priority(5), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .expect("slot available");

        sched.inherit_priority(task, priority(25)).expect("task is live");
        sched.inherit_priority(task, priority(12)).expect("task is live");
        assert_eq!(
            sched.priority_of(task),
            Some(priority(25)),
            "a second, lower waiter cannot pull the holder down"
        );
    }

    // Clause 4: an inherited priority below the task's own is inert, so a
    // boost can never demote anyone even if a caller asks for one.
    #[test]
    fn an_inherited_priority_below_the_base_changes_nothing() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let task = sched
            .create_task(priority(20), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .expect("slot available");
        sched.inherit_priority(task, priority(4)).expect("task is live");
        assert_eq!(sched.priority_of(task), Some(priority(20)));
    }

    // The audit trail has to report where the task actually landed. The old
    // implementation stamped the value it replayed, which after a degrade was
    // a priority the task never held.
    #[test]
    fn the_release_spoor_records_the_priority_the_task_actually_lands_on() {
        let (mut sched, mut journal, mut lock, low, _medium) = contended_holder_that_overruns();
        overrun(&mut sched, &mut journal, low);
        lock.unlock(&mut sched, &mut journal, low).expect("holder releases");

        let restore = journal
            .iter()
            .filter(|spoor| spoor.action() == Action::Restore)
            .last()
            .expect("a boosted holder releasing stamps a Restore");
        assert_eq!(restore.cost(), 2, "the floor it landed on, not the 5 it left");
    }
}
