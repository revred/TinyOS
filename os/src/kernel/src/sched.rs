//! Fixed-priority task creation (`STORY-P0-02-01`).
//!
//! `Scheduler<N>` owns a [`Pool`](crate::mem::Pool)`<Tcb, N>` of task control
//! blocks, so creating a task carries the same no-heap, fail-closed
//! allocation contract `mem.rs`'s `Pool` already provides (`STORY-P0-03-01`,
//! `STORY-P0-03-03`) — this Story doesn't reinvent bounded allocation, it
//! reuses it. Context switching (`STORY-P0-02-02`), priority inheritance
//! (`STORY-P0-02-03`), and WCET enforcement (`STORY-P0-02-04`) are explicitly
//! out of scope here: this module only creates and stores task control
//! blocks with a validated priority and a WCET budget placeholder.

use crate::mem::{Pool, PoolError, PoolHandle};

/// Identifies a task previously created by [`Scheduler::create_task`].
///
/// A newtype over [`PoolHandle`] (per the newtype style note in
/// `agent/CODING_STANDARDS.md`) rather than a bare integer, so a `TaskId`
/// can't be confused with an arbitrary index or forged by a caller — only
/// [`Scheduler::create_task`] hands one out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskId(PoolHandle);

impl TaskId {
    /// This task's underlying pool-slot index — exposed (read-only) so
    /// `kernel::dispatch` can key an external, parallel array of
    /// [`crate::context::Context`] values by task, one per pool slot,
    /// without this module owning or knowing about `Context` itself
    /// (Dependency Inversion: `sched` has no dependency on `context`).
    pub const fn index(self) -> usize {
        self.0.index()
    }
}

/// A task's dispatch state — whether it's eligible to be selected and run
/// next (`kernel::dispatch::run_once`), currently running, waiting on
/// something (e.g. a contended [`crate::lock::PriorityInheritingLock`]),
/// or done.
///
/// Every newly created task starts [`TaskState::Ready`]. Transitioning a
/// task to [`TaskState::Blocked`]/[`TaskState::Finished`] is the caller's
/// responsibility (e.g. the lock-contention integration point in
/// `kernel::dispatch`'s own tests) — this module has no dispatcher loop of
/// its own to drive those transitions automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// Eligible to be selected and run next.
    Ready,
    /// Currently the task `kernel::dispatch::run_once` last switched into.
    Running,
    /// Waiting on something (e.g. lock contention) — not selectable.
    Blocked,
    /// Will never run again — not selectable.
    Finished,
}

/// Lowest valid [`Priority`] value.
///
/// 0 is the least-urgent priority a task may declare, not "unset" or
/// "invalid" — every created task has some priority, so there is no
/// separate sentinel value.
pub const PRIORITY_MIN: u8 = 0;

/// Highest valid [`Priority`] value.
///
/// 31 keeps the whole priority space inside 5 bits, wide enough for a
/// fixed-priority RT scheduler to give every distinct criticality class in
/// the current design (boot/idle through interrupt-adjacent RT tasks) its
/// own level with headroom to spare, while staying small enough that a
/// future priority-bitmap ready-queue (one bit per level, a common
/// fixed-priority scheduler implementation technique) fits in a single
/// `u32`.
pub const PRIORITY_MAX: u8 = 31;

/// A task's static scheduling priority, in `PRIORITY_MIN..=PRIORITY_MAX`.
///
/// Higher numeric value means higher priority (more urgent). Constructed
/// only via [`Priority::try_new`], which rejects an out-of-range value with
/// a typed [`PriorityError`] instead of silently clamping it — a silently
/// clamped priority would let a task run at a criticality level its creator
/// never asked for, which `agent/CODING_STANDARDS.md`'s Safety-first
/// priority ordering treats as a defect, not a convenience.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Priority(u8);

impl Priority {
    /// Validates `value` against `PRIORITY_MIN..=PRIORITY_MAX` and wraps it.
    ///
    /// `const fn` so valid priorities can be named as compile-time
    /// constants; still returns `Result` rather than panicking on an
    /// out-of-range value, since not every caller's input is known at
    /// compile time.
    pub const fn try_new(value: u8) -> Result<Self, PriorityError> {
        if value > PRIORITY_MAX {
            Err(PriorityError::OutOfRange)
        } else {
            Ok(Priority(value))
        }
    }

    /// Returns the wrapped, already-validated priority value.
    pub const fn value(self) -> u8 {
        self.0
    }
}

/// Errors [`Priority::try_new`] fails closed with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorityError {
    /// `value` fell outside `PRIORITY_MIN..=PRIORITY_MAX`.
    OutOfRange,
}

/// A task's worst-case execution time budget, in an implementation-defined
/// time unit (ticks).
///
/// This Story only stores the budget so it isn't lost between task creation
/// and the enforcement logic `STORY-P0-02-04` adds later — no admission
/// control or preemption-on-overrun happens here yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WcetBudgetTicks(pub u32);

/// A task's entry point: the function the scheduler transfers control to
/// when the task first runs.
///
/// `extern "C" fn() -> !` matches `main.rs`'s `kernel_main` idiom (a task
/// never returns; it exits via a scheduler call, not a Rust `return`).
pub type TaskEntry = extern "C" fn() -> !;

/// A task control block: everything the scheduler needs to know about one
/// created task.
///
/// Fields are `pub` within the crate's control (constructed only through
/// [`Scheduler::create_task`]) rather than exposing a public constructor,
/// so a `Tcb` can never exist with an unvalidated `Priority` — the type
/// system, not caller discipline, enforces that invariant.
#[derive(Debug, Clone, Copy)]
pub struct Tcb {
    priority: Priority,
    wcet_budget: WcetBudgetTicks,
    entry: TaskEntry,
    /// Ticks attributed to this task since its last budget-window reset —
    /// `STORY-P0-02-04`'s enforcement bookkeeping (`crate::wcet`). Starts
    /// at 0 for every newly created task.
    ticks_consumed: u32,
    /// This task's current dispatch state — `STORY-P0-02-05`'s bookkeeping
    /// (`crate::dispatch`). Starts at [`TaskState::Ready`] for every newly
    /// created task.
    state: TaskState,
}

impl Tcb {
    /// The task's static scheduling priority.
    pub const fn priority(&self) -> Priority {
        self.priority
    }

    /// The task's WCET budget, enforced by `crate::wcet::record_tick`.
    pub const fn wcet_budget(&self) -> WcetBudgetTicks {
        self.wcet_budget
    }

    /// The task's current dispatch state.
    pub const fn state(&self) -> TaskState {
        self.state
    }

    /// The task's entry point.
    pub const fn entry(&self) -> TaskEntry {
        self.entry
    }

    /// Ticks attributed to this task in its current budget window.
    pub const fn ticks_consumed(&self) -> u32 {
        self.ticks_consumed
    }
}

/// Errors [`Scheduler::create_task`] fails closed with, per
/// `agent/CODING_STANDARDS.md`'s "no stringly-typed errors" rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskCreateError {
    /// Every task slot is occupied; no side effects occurred.
    Exhausted,
}

impl From<PoolError> for TaskCreateError {
    fn from(err: PoolError) -> Self {
        match err {
            PoolError::Exhausted => TaskCreateError::Exhausted,
            // `Scheduler::create_task` only ever calls `Pool::alloc`, which
            // never returns `InvalidHandle` (that variant is `free`-only),
            // so this arm is unreachable in practice but kept exhaustive
            // rather than assumed away.
            PoolError::InvalidHandle => TaskCreateError::Exhausted,
        }
    }
}

/// Fixed-priority task creation backed by a bounded `Pool<Tcb, N>`.
///
/// `N` is the maximum number of live tasks this scheduler instance can
/// hold, chosen by the caller at the type level (mirrors `mem.rs`'s
/// `Pool<T, N>` pattern) rather than grown dynamically, so task creation
/// carries no allocation-time variance per `agent/CODING_STANDARDS.md`'s
/// real-time discipline section.
pub struct Scheduler<const N: usize> {
    tasks: Pool<Tcb, N>,
}

impl<const N: usize> Scheduler<N> {
    /// Creates a scheduler with no tasks yet. `const fn`: usable in a
    /// `static` initializer, no heap allocation.
    pub const fn new() -> Self {
        Scheduler { tasks: Pool::new() }
    }

    /// Creates a task with the given `priority`, `wcet_budget`, and `entry`
    /// point, returning a [`TaskId`] that identifies it.
    ///
    /// Fails closed with [`TaskCreateError::Exhausted`] and no side effects
    /// if every task slot is occupied — never panics, mirroring
    /// `Pool::alloc`'s exhaustion contract (`STORY-P0-03-03`).
    pub fn create_task(
        &mut self,
        priority: Priority,
        wcet_budget: WcetBudgetTicks,
        entry: TaskEntry,
    ) -> Result<TaskId, TaskCreateError> {
        let tcb = Tcb { priority, wcet_budget, entry, ticks_consumed: 0, state: TaskState::Ready };
        let handle = self.tasks.alloc(tcb)?;
        Ok(TaskId(handle))
    }

    /// The current (possibly priority-inheritance-boosted, `STORY-P0-02-03`)
    /// priority of `task`, or `None` if `task` doesn't identify a currently
    /// live task.
    pub fn priority_of(&mut self, task: TaskId) -> Option<Priority> {
        self.tasks.get_mut(task.0).map(|tcb| tcb.priority())
    }

    /// Sets `task`'s current priority, used by [`crate::lock`]'s priority
    /// inheritance to boost a lock holder to a waiter's priority, and to
    /// restore it afterward. Returns `None` (no side effect) if `task`
    /// doesn't identify a currently live task.
    pub fn set_priority(&mut self, task: TaskId, priority: Priority) -> Option<()> {
        let tcb = self.tasks.get_mut(task.0)?;
        tcb.priority = priority;
        Some(())
    }

    /// `task`'s current `(ticks_consumed, wcet_budget)`, used by
    /// `crate::wcet::record_tick` to check an attribution against budget.
    /// `None` if `task` doesn't identify a currently live task.
    pub fn wcet_state(&mut self, task: TaskId) -> Option<(u32, WcetBudgetTicks)> {
        self.tasks.get_mut(task.0).map(|tcb| (tcb.ticks_consumed, tcb.wcet_budget))
    }

    /// Attributes `ticks` more of consumption to `task`, returning its new
    /// running total. `None` (no side effect) if `task` doesn't identify a
    /// currently live task.
    pub fn add_ticks_consumed(&mut self, task: TaskId, ticks: u32) -> Option<u32> {
        let tcb = self.tasks.get_mut(task.0)?;
        tcb.ticks_consumed = tcb.ticks_consumed.saturating_add(ticks);
        Some(tcb.ticks_consumed)
    }

    /// Resets `task`'s consumed-ticks counter to 0, starting a fresh WCET
    /// budget window (e.g. at the start of each new scheduling period).
    /// `None` (no side effect) if `task` doesn't identify a currently live
    /// task.
    pub fn reset_ticks_consumed(&mut self, task: TaskId) -> Option<()> {
        let tcb = self.tasks.get_mut(task.0)?;
        tcb.ticks_consumed = 0;
        Some(())
    }

    /// Test-only helper for other modules' test suites (e.g.
    /// `crate::wcet`'s) that need an "unknown task" `TaskId` — a `TaskId`
    /// that was once valid but no longer identifies a live task, without
    /// exposing a real, non-test task-destruction API this Story doesn't
    /// otherwise need.
    #[cfg(test)]
    pub(crate) fn free_task_for_test(&mut self, task: TaskId) {
        let _ = self.tasks.free(task.0);
    }

    /// `task`'s current dispatch state, or `None` if `task` doesn't
    /// identify a currently live task.
    pub fn state_of(&self, task: TaskId) -> Option<TaskState> {
        self.tasks.iter_occupied().find(|(handle, _)| *handle == task.0).map(|(_, tcb)| tcb.state)
    }

    /// Sets `task`'s current dispatch state. Returns `None` (no side
    /// effect) if `task` doesn't identify a currently live task.
    pub fn set_state(&mut self, task: TaskId, state: TaskState) -> Option<()> {
        let tcb = self.tasks.get_mut(task.0)?;
        tcb.state = state;
        Some(())
    }

    /// Iterates over every currently live task, yielding each one's
    /// [`TaskId`] alongside a shared reference to its [`Tcb`].
    pub fn iter_tasks(&self) -> impl Iterator<Item = (TaskId, &Tcb)> {
        self.tasks.iter_occupied().map(|(handle, tcb)| (TaskId(handle), tcb))
    }

    /// Selects the highest-priority [`TaskState::Ready`] task, or `None` if
    /// none is Ready — the scheduling decision `kernel::dispatch::run_once`
    /// acts on.
    ///
    /// Ties break toward the *most recently created* of the tied tasks
    /// (`Iterator::max_by_key`'s own documented last-wins tie-break over
    /// this module's creation-order iteration) — deterministic, but not
    /// itself a scheduling guarantee beyond "some Ready task at the
    /// highest present priority is chosen every time."
    pub fn highest_priority_ready(&self) -> Option<TaskId> {
        self.iter_tasks()
            .filter(|(_, tcb)| tcb.state == TaskState::Ready)
            .max_by_key(|(_, tcb)| tcb.priority)
            .map(|(id, _)| id)
    }
}

impl<const N: usize> Default for Scheduler<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A valid `TaskEntry` function pointer for test fixtures; never actually
    // called (no scheduler/context-switch exists yet to call it), so the
    // empty spin body clippy normally flags as CPU-wasting is moot here.
    #[allow(clippy::empty_loop)]
    extern "C" fn dummy_entry() -> ! {
        loop {}
    }

    const BUDGET: WcetBudgetTicks = WcetBudgetTicks(1000);

    fn low_priority() -> Priority {
        Priority::try_new(1).expect("1 is in range")
    }

    // Creating a task returns a distinguishable TaskId per task
    // (STORY-P0-02-01 acceptance criterion 1).
    #[test]
    fn create_task_returns_distinguishable_task_ids() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let a = sched.create_task(low_priority(), BUDGET, dummy_entry).expect("slot available");
        let b = sched.create_task(low_priority(), BUDGET, dummy_entry).expect("slot available");
        assert_ne!(a, b);
    }

    // Task creation against a full pool fails closed, repeatedly, and
    // recovers after a slot frees — mirrors
    // mem.rs::exhausted_pool_fails_closed_without_side_effects
    // (STORY-P0-02-01 acceptance criterion 2, STORY-P0-03-03's contract).
    #[test]
    fn exhausted_scheduler_fails_closed_and_recovers_after_free() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let a = sched.create_task(low_priority(), BUDGET, dummy_entry).unwrap();
        let _b = sched.create_task(low_priority(), BUDGET, dummy_entry).unwrap();

        assert_eq!(
            sched.create_task(low_priority(), BUDGET, dummy_entry),
            Err(TaskCreateError::Exhausted)
        );
        // Repeated exhaustion fails the same way every time, not just once.
        assert_eq!(
            sched.create_task(low_priority(), BUDGET, dummy_entry),
            Err(TaskCreateError::Exhausted)
        );

        // Freeing the underlying pool slot proves exhaustion was transient
        // occupancy state, not a poisoned/latched scheduler.
        sched.tasks.free(a.0).expect("a was a valid, occupied handle");
        let c = sched.create_task(low_priority(), BUDGET, dummy_entry);
        assert!(c.is_ok(), "a freed slot should be allocatable again");
    }

    // Priority construction rejects out-of-range values without panicking
    // (STORY-P0-02-01 acceptance criterion 3).
    #[test]
    fn priority_construction_rejects_out_of_range_values() {
        assert_eq!(Priority::try_new(PRIORITY_MAX + 1), Err(PriorityError::OutOfRange));
        assert_eq!(Priority::try_new(u8::MAX), Err(PriorityError::OutOfRange));
    }

    // Priority construction accepts the full valid range including its
    // boundaries (STORY-P0-02-01 acceptance criterion 3).
    #[test]
    fn priority_construction_accepts_full_valid_range_including_boundaries() {
        assert_eq!(Priority::try_new(PRIORITY_MIN).map(Priority::value), Ok(PRIORITY_MIN));
        assert_eq!(Priority::try_new(PRIORITY_MAX).map(Priority::value), Ok(PRIORITY_MAX));
        for value in PRIORITY_MIN..=PRIORITY_MAX {
            assert!(Priority::try_new(value).is_ok(), "priority {value} should be valid");
        }
    }

    // A created task retains the priority, WCET budget, and entry point it
    // was created with — the TCB isn't losing fields on the way into the
    // pool.
    #[test]
    fn created_task_tcb_retains_priority_and_budget() {
        let mut sched: Scheduler<1> = Scheduler::new();
        let priority = Priority::try_new(17).unwrap();
        let budget = WcetBudgetTicks(2500);
        let id = sched.create_task(priority, budget, dummy_entry).unwrap();
        let tcb = sched.tasks.free(id.0).expect("just-created task should be present");
        assert_eq!(tcb.priority(), priority);
        assert_eq!(tcb.wcet_budget(), budget);
        assert!(core::ptr::eq(tcb.entry() as *const (), dummy_entry as *const ()));
    }

    // STORY-P0-02-05: a newly created task starts Ready, and is therefore
    // selectable by `highest_priority_ready`.
    #[test]
    fn newly_created_task_starts_ready_and_is_selectable() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let task = sched.create_task(low_priority(), BUDGET, dummy_entry).unwrap();
        assert_eq!(sched.state_of(task), Some(TaskState::Ready));
        assert_eq!(sched.highest_priority_ready(), Some(task));
    }

    // STORY-P0-02-05: among several Ready tasks, the highest-priority one
    // is selected, regardless of creation order.
    #[test]
    fn highest_priority_ready_selects_the_highest_priority_among_several() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let low = sched.create_task(Priority::try_new(5).unwrap(), BUDGET, dummy_entry).unwrap();
        let high = sched.create_task(Priority::try_new(25).unwrap(), BUDGET, dummy_entry).unwrap();
        let medium =
            sched.create_task(Priority::try_new(15).unwrap(), BUDGET, dummy_entry).unwrap();
        let _ = (low, medium);

        assert_eq!(sched.highest_priority_ready(), Some(high));
    }

    // A Blocked or Finished task is never selected, even if it outranks
    // every Ready task.
    #[test]
    fn blocked_and_finished_tasks_are_never_selected() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let blocked_high =
            sched.create_task(Priority::try_new(31).unwrap(), BUDGET, dummy_entry).unwrap();
        let finished_high =
            sched.create_task(Priority::try_new(30).unwrap(), BUDGET, dummy_entry).unwrap();
        let ready_low =
            sched.create_task(Priority::try_new(1).unwrap(), BUDGET, dummy_entry).unwrap();

        sched.set_state(blocked_high, TaskState::Blocked).unwrap();
        sched.set_state(finished_high, TaskState::Finished).unwrap();

        assert_eq!(sched.highest_priority_ready(), Some(ready_low));
    }

    // No Ready task at all (empty scheduler, or every task Blocked/Finished)
    // selects nothing, rather than panicking or picking an ineligible task.
    #[test]
    fn no_ready_task_selects_none() {
        let sched: Scheduler<2> = Scheduler::new();
        assert_eq!(sched.highest_priority_ready(), None);

        let mut sched: Scheduler<2> = Scheduler::new();
        let only = sched.create_task(low_priority(), BUDGET, dummy_entry).unwrap();
        sched.set_state(only, TaskState::Blocked).unwrap();
        assert_eq!(sched.highest_priority_ready(), None);
    }

    // `state_of`/`set_state` against an unknown task fail closed.
    #[test]
    fn state_of_and_set_state_against_an_unknown_task_fail_closed() {
        let mut sched: Scheduler<1> = Scheduler::new();
        let task = sched.create_task(low_priority(), BUDGET, dummy_entry).unwrap();
        sched.free_task_for_test(task);

        assert_eq!(sched.state_of(task), None);
        assert_eq!(sched.set_state(task, TaskState::Ready), None);
    }
}
