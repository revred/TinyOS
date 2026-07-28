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

/// What a task declared must happen to it if it exceeds its
/// [`WcetBudgetTicks`] (`STORY-P1-04-02`).
///
/// Declared by whoever creates the task, at the same moment as the budget it
/// governs, and never defaulted — see [`Scheduler::create_task`]. The
/// enforcement half lives in [`crate::wcet`], which re-exports this type as
/// `wcet::OverrunPolicy`; it is *defined* here because it is task-declaration
/// state that a [`Tcb`] holds and because [`OverrunPolicy::Degrade`] carries
/// a [`Priority`], and this module deliberately depends on nothing (compare
/// the `sched` → `context` non-dependency this file's own `TaskId::index`
/// doc records).
///
/// There is no `Ignore` arm, and adding one would be a change to this
/// enumeration rather than an omission in a `match` — which is the whole
/// reason the choice is an enumeration at all. See
/// [`crate::wcet::disposition_for`] for the decision table and
/// `TEST-P1-04-02-A` clause 2 for why it is *not* `crate::fault::Disposition`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverrunPolicy {
    /// Re-initialize the task to its entry point, reset its budget window,
    /// and return it to [`TaskState::Ready`]: it runs again from the
    /// beginning, having lost whatever it had accumulated.
    Restart,
    /// Drop the task's priority to this declared floor and reset its budget
    /// window. It keeps running, but can no longer preempt anything above
    /// the floor.
    ///
    /// The floor must not exceed the task's own creation priority —
    /// [`Scheduler::create_task`] rejects that with
    /// [`TaskCreateError::DegradeFloorAbovePriority`] rather than clamping,
    /// because an overrun that *raised* a task's priority would turn a
    /// missed budget into a privilege escalation.
    Degrade(Priority),
    /// Finish the task and enter the system's declared safe state. At Tier 0
    /// that is a reported, fail-closed stop — the precedent
    /// `crate::fault::Disposition::HaltSystem` already set.
    TripToSafeState,
}

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
    /// The task's **own** priority: what it was created with, and the only
    /// field `crate::wcet`'s degrade lowers (`STORY-P1-04-02`).
    ///
    /// Split out from a single `priority` field by `STORY-P1-04-04`, closing
    /// `LE-22`. Two subsystems used to write one field — the lock replayed a
    /// captured value on unlock, the WCET path wrote a declared floor on
    /// overrun — so whichever wrote last silently discarded the other's
    /// decision. Neither writes the other's field now, and neither writes the
    /// value anybody reads.
    base_priority: Priority,
    /// The highest priority currently *inherited* from a waiter blocked on a
    /// lock this task holds (`STORY-P0-02-03`), or `None` when nothing is
    /// inherited.
    ///
    /// Written only by [`Scheduler::inherit_priority`] and cleared only by
    /// [`Scheduler::release_inheritance`], both of which `crate::lock` owns.
    inherited_priority: Option<Priority>,
    wcet_budget: WcetBudgetTicks,
    /// What this task declared should happen to it if it exceeds
    /// `wcet_budget` — `STORY-P1-04-02`. Supplied at creation alongside the
    /// budget it governs, with no default, so a task can never hold a budget
    /// whose consequence was decided by whoever wrote the enforcement path
    /// rather than by whoever declared the task.
    overrun_policy: OverrunPolicy,
    entry: TaskEntry,
    /// Ticks attributed to this task since its last budget-window reset —
    /// `STORY-P0-02-04`'s enforcement bookkeeping (`crate::wcet`). Starts
    /// at 0 for every newly created task.
    ticks_consumed: u32,
    /// This task's current dispatch state — `STORY-P0-02-05`'s bookkeeping
    /// (`crate::dispatch`). Starts at [`TaskState::Ready`] for every newly
    /// created task.
    state: TaskState,
    /// This task's private address space, as a `CR3` value (the physical
    /// address of its PML4) — `STORY-P1-03-01`'s bookkeeping. `None` for
    /// every newly created task, meaning "no dedicated space; a switch into
    /// this task must not touch `CR3`" — the default that keeps every
    /// existing Story's tasks running exactly as they do today, since
    /// nothing yet installs a per-task page-table tree on the real dispatch
    /// path (that step, and the W^X-correct kernel mappings it needs to be
    /// safe there, are `FEAT-P1-03`'s remaining work).
    address_space: Option<u64>,
}

impl Tcb {
    /// The priority everything that *schedules* this task reads:
    /// `max(base, inherited)`, evaluated on demand.
    ///
    /// **Derived, never stored** — `STORY-P1-04-04`'s whole point. A stored
    /// effective priority is a decision two subsystems can invalidate, which
    /// is the defect `LE-22` registered; a computed one cannot go stale
    /// because there is nothing to go stale.
    ///
    /// `max` also makes "inheritance raises and never lowers" structural
    /// rather than a guard at each call site that someone can forget.
    pub const fn priority(&self) -> Priority {
        match self.inherited_priority {
            Some(inherited) if inherited.value() > self.base_priority.value() => inherited,
            _ => self.base_priority,
        }
    }

    /// The task's own priority, ignoring any inheritance — what `degrade`
    /// lowers and what the task falls back to when its last waiter is
    /// released.
    pub const fn base_priority(&self) -> Priority {
        self.base_priority
    }

    /// The highest priority currently inherited from a waiter, or `None`.
    pub const fn inherited_priority(&self) -> Option<Priority> {
        self.inherited_priority
    }

    /// The task's WCET budget, enforced by `crate::wcet::record_tick`.
    pub const fn wcet_budget(&self) -> WcetBudgetTicks {
        self.wcet_budget
    }

    /// What this task declared should happen if it exceeds
    /// [`Tcb::wcet_budget`] (`STORY-P1-04-02`).
    pub const fn overrun_policy(&self) -> OverrunPolicy {
        self.overrun_policy
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

    /// This task's `CR3` value, or `None` if it has no dedicated address
    /// space (`STORY-P1-03-01`).
    pub const fn address_space(&self) -> Option<u64> {
        self.address_space
    }
}

/// Errors [`Scheduler::create_task`] fails closed with, per
/// `agent/CODING_STANDARDS.md`'s "no stringly-typed errors" rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskCreateError {
    /// Every task slot is occupied; no side effects occurred.
    Exhausted,
    /// The task declared [`OverrunPolicy::Degrade`] with a floor *above* its
    /// own creation priority; no side effects occurred (`STORY-P1-04-02`).
    ///
    /// Rejected rather than clamped, for the same reason
    /// [`Priority::try_new`] rejects rather than clamps: a degrade that
    /// raised a task's priority would make exceeding a deadline a route to
    /// running at a criticality level nobody granted. Degrade means degrade.
    DegradeFloorAbovePriority,
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

    /// Creates a task with the given `priority`, `wcet_budget`,
    /// `overrun_policy`, and `entry` point, returning a [`TaskId`] that
    /// identifies it.
    ///
    /// `overrun_policy` is a parameter rather than a defaulted field
    /// (`STORY-P1-04-02` acceptance criterion 2): a task that held a budget
    /// with no declared consequence would be a task whose overrun behaviour
    /// was decided by whoever wrote the enforcement path. Every call site in
    /// this workspace therefore states the consequence it wants, out loud, at
    /// the point the budget is declared.
    ///
    /// Fails closed with [`TaskCreateError::Exhausted`] and no side effects
    /// if every task slot is occupied — never panics, mirroring
    /// `Pool::alloc`'s exhaustion contract (`STORY-P0-03-03`) — and with
    /// [`TaskCreateError::DegradeFloorAbovePriority`], checked *before* any
    /// slot is claimed, if the declared degrade floor outranks `priority`.
    pub fn create_task(
        &mut self,
        priority: Priority,
        wcet_budget: WcetBudgetTicks,
        overrun_policy: OverrunPolicy,
        entry: TaskEntry,
    ) -> Result<TaskId, TaskCreateError> {
        if let OverrunPolicy::Degrade(floor) = overrun_policy {
            if floor > priority {
                return Err(TaskCreateError::DegradeFloorAbovePriority);
            }
        }
        let tcb = Tcb {
            base_priority: priority,
            inherited_priority: None,
            wcet_budget,
            overrun_policy,
            entry,
            ticks_consumed: 0,
            state: TaskState::Ready,
            address_space: None,
        };
        let handle = self.tasks.alloc(tcb)?;
        Ok(TaskId(handle))
    }

    /// Attaches `cr3` (a physical PML4 address, e.g. `exec::AddressSpace`'s
    /// own `cr3()` accessor) to `task` as its dedicated address space
    /// (`STORY-P1-03-01`). Returns `None` (no side effect) if `task` doesn't
    /// identify a currently live task.
    pub fn set_address_space(&mut self, task: TaskId, cr3: u64) -> Option<()> {
        let tcb = self.tasks.get_mut(task.0)?;
        tcb.address_space = Some(cr3);
        Some(())
    }

    /// The current (possibly priority-inheritance-boosted, `STORY-P0-02-03`)
    /// priority of `task`, or `None` if `task` doesn't identify a currently
    /// live task.
    pub fn priority_of(&mut self, task: TaskId) -> Option<Priority> {
        self.tasks.get_mut(task.0).map(|tcb| tcb.priority())
    }

    /// Sets `task`'s **own** priority, leaving any inherited priority intact.
    ///
    /// The only writer is `crate::wcet::apply`'s degrade arm. It is named for
    /// the field it writes rather than for the quantity anybody reads, because
    /// `LE-22` was precisely a caller believing it had set the scheduling
    /// priority when it had set one of two inputs to it.
    ///
    /// **This cannot cancel a boost**: a task degraded while a waiter is
    /// blocked on a lock it holds keeps running at the waiter's priority until
    /// it releases, and *then* falls to the degraded base. Returns `None` (no
    /// side effect) if `task` doesn't identify a currently live task.
    pub fn set_base_priority(&mut self, task: TaskId, priority: Priority) -> Option<()> {
        let tcb = self.tasks.get_mut(task.0)?;
        tcb.base_priority = priority;
        Some(())
    }

    /// Raises `task`'s inherited priority to `priority` if that is higher than
    /// what it already inherits (`STORY-P0-02-03`).
    ///
    /// Idempotent and monotonic within one contention cycle: a second, lower
    /// waiter cannot pull a holder down, and re-boosting to the same value
    /// changes nothing. Returns `None` (no side effect) if `task` doesn't
    /// identify a currently live task.
    ///
    /// **This cannot cancel a degrade**: it writes only `inherited_priority`,
    /// so a task's own priority is exactly where `crate::wcet` left it.
    pub fn inherit_priority(&mut self, task: TaskId, priority: Priority) -> Option<()> {
        let tcb = self.tasks.get_mut(task.0)?;
        let raise = match tcb.inherited_priority {
            Some(current) => priority.value() > current.value(),
            None => true,
        };
        if raise {
            tcb.inherited_priority = Some(priority);
        }
        Some(())
    }

    /// Drops every priority `task` inherits, returning the effective priority
    /// it falls back to — which is its **current** base, including any degrade
    /// applied while it was boosted.
    ///
    /// Returns `None` (no side effect) if `task` doesn't identify a currently
    /// live task.
    ///
    /// **Known limit (`LE-49`).** This clears the task's inheritance outright,
    /// so a task holding two contended locks loses the second lock's boost when
    /// it releases the first. Correcting that needs per-lock inheritance
    /// records, which needs blocking waiters, which this kernel does not have —
    /// `crate::lock::PriorityInheritingLock::try_lock` reports contention rather
    /// than parking. It is a smaller hole than the one it replaces: the previous
    /// code wrote back a stale *absolute* value in the same scenario, which
    /// could raise a task above any priority it was entitled to.
    pub fn release_inheritance(&mut self, task: TaskId) -> Option<Priority> {
        let tcb = self.tasks.get_mut(task.0)?;
        tcb.inherited_priority = None;
        Some(tcb.base_priority)
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

    /// `task`'s dedicated address space (`Tcb::address_space`), or the
    /// outer `None` if `task` doesn't identify a currently live task —
    /// the read `kernel::dispatch`'s `CR3`-aware selection consumes
    /// (`STORY-P1-03-02`), shaped like [`Scheduler::state_of`].
    pub fn address_space_of(&self, task: TaskId) -> Option<Option<u64>> {
        self.tasks
            .iter_occupied()
            .find(|(handle, _)| *handle == task.0)
            .map(|(_, tcb)| tcb.address_space)
    }

    /// `task`'s current (possibly boosted) priority, or `None` if `task`
    /// doesn't identify a currently live task — the shared-reference twin of
    /// [`Scheduler::priority_of`], shaped like [`Scheduler::state_of`].
    ///
    /// `STORY-P1-04-01` needs this: the timer ISR's preemption decision reads
    /// the running task's priority from an interrupt context that must not
    /// take a `&mut` to the scheduler at all, since the dispatcher it
    /// interrupted may hold one.
    pub fn live_priority_of(&self, task: TaskId) -> Option<Priority> {
        self.tasks
            .iter_occupied()
            .find(|(handle, _)| *handle == task.0)
            .map(|(_, tcb)| tcb.priority())
    }

    /// `task`'s own priority, ignoring inheritance, or `None` if `task`
    /// doesn't identify a currently live task.
    pub fn base_priority_of(&mut self, task: TaskId) -> Option<Priority> {
        self.tasks.get_mut(task.0).map(|tcb| tcb.base_priority())
    }

    /// What `task` currently inherits from a waiter, or `None` if `task`
    /// doesn't identify a currently live task. The inner `Option` is `None`
    /// when the task inherits nothing — the distinction `crate::lock` needs to
    /// tell "released a boost" from "there was no boost".
    pub fn inherited_priority_of(&mut self, task: TaskId) -> Option<Option<Priority>> {
        self.tasks.get_mut(task.0).map(|tcb| tcb.inherited_priority())
    }

    /// The [`OverrunPolicy`] `task` declared at creation, or `None` if `task`
    /// doesn't identify a currently live task (`STORY-P1-04-02`).
    ///
    /// This is the *only* input `crate::wcet::disposition_for` consumes, and
    /// it is immutable for a task's whole life: there is no setter, so an
    /// overrunning task cannot influence what happens to it by overrunning.
    pub fn overrun_policy_of(&self, task: TaskId) -> Option<OverrunPolicy> {
        self.tasks
            .iter_occupied()
            .find(|(handle, _)| *handle == task.0)
            .map(|(_, tcb)| tcb.overrun_policy)
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
            // `priority()`, not the base field: selection is exactly the place
            // a boosted holder must outrank an uninvolved medium task, which
            // is what priority inheritance exists to make true.
            .max_by_key(|(_, tcb)| tcb.priority())
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
        let a = sched
            .create_task(low_priority(), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .expect("slot available");
        let b = sched
            .create_task(low_priority(), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .expect("slot available");
        assert_ne!(a, b);
    }

    // Task creation against a full pool fails closed, repeatedly, and
    // recovers after a slot frees — mirrors
    // mem.rs::exhausted_pool_fails_closed_without_side_effects
    // (STORY-P0-02-01 acceptance criterion 2, STORY-P0-03-03's contract).
    #[test]
    fn exhausted_scheduler_fails_closed_and_recovers_after_free() {
        let mut sched: Scheduler<2> = Scheduler::new();
        let a = sched
            .create_task(low_priority(), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();
        let _b = sched
            .create_task(low_priority(), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();

        assert_eq!(
            sched.create_task(low_priority(), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry),
            Err(TaskCreateError::Exhausted)
        );
        // Repeated exhaustion fails the same way every time, not just once.
        assert_eq!(
            sched.create_task(low_priority(), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry),
            Err(TaskCreateError::Exhausted)
        );

        // Freeing the underlying pool slot proves exhaustion was transient
        // occupancy state, not a poisoned/latched scheduler.
        sched.tasks.free(a.0).expect("a was a valid, occupied handle");
        let c =
            sched.create_task(low_priority(), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry);
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
        let id = sched
            .create_task(priority, budget, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();
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
        let task = sched
            .create_task(low_priority(), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();
        assert_eq!(sched.state_of(task), Some(TaskState::Ready));
        assert_eq!(sched.highest_priority_ready(), Some(task));
    }

    // STORY-P0-02-05: among several Ready tasks, the highest-priority one
    // is selected, regardless of creation order.
    #[test]
    fn highest_priority_ready_selects_the_highest_priority_among_several() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let low = sched
            .create_task(
                Priority::try_new(5).unwrap(),
                BUDGET,
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();
        let high = sched
            .create_task(
                Priority::try_new(25).unwrap(),
                BUDGET,
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();
        let medium = sched
            .create_task(
                Priority::try_new(15).unwrap(),
                BUDGET,
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();
        let _ = (low, medium);

        assert_eq!(sched.highest_priority_ready(), Some(high));
    }

    // A Blocked or Finished task is never selected, even if it outranks
    // every Ready task.
    #[test]
    fn blocked_and_finished_tasks_are_never_selected() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let blocked_high = sched
            .create_task(
                Priority::try_new(31).unwrap(),
                BUDGET,
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();
        let finished_high = sched
            .create_task(
                Priority::try_new(30).unwrap(),
                BUDGET,
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();
        let ready_low = sched
            .create_task(
                Priority::try_new(1).unwrap(),
                BUDGET,
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .unwrap();

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
        let only = sched
            .create_task(low_priority(), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();
        sched.set_state(only, TaskState::Blocked).unwrap();
        assert_eq!(sched.highest_priority_ready(), None);
    }

    // `state_of`/`set_state` against an unknown task fail closed.
    #[test]
    fn state_of_and_set_state_against_an_unknown_task_fail_closed() {
        let mut sched: Scheduler<1> = Scheduler::new();
        let task = sched
            .create_task(low_priority(), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();
        sched.free_task_for_test(task);

        assert_eq!(sched.state_of(task), None);
        assert_eq!(sched.set_state(task, TaskState::Ready), None);
    }

    // STORY-P1-03-01: a newly created task has no dedicated address space —
    // the default that keeps every pre-existing Story's tasks unaffected.
    #[test]
    fn a_newly_created_task_has_no_address_space_by_default() {
        let mut sched: Scheduler<1> = Scheduler::new();
        let task = sched
            .create_task(low_priority(), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();
        let tcb = sched.tasks.free(task.0).expect("just-created task should be present");
        assert_eq!(tcb.address_space(), None);
    }

    // STORY-P1-03-01: `set_address_space` attaches a CR3 value that
    // `Tcb::address_space` then reports back.
    #[test]
    fn set_address_space_attaches_a_cr3_value_to_a_task() {
        let mut sched: Scheduler<1> = Scheduler::new();
        let task = sched
            .create_task(low_priority(), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();
        assert_eq!(sched.set_address_space(task, 0x1234_5000), Some(()));
        let tcb = sched.tasks.free(task.0).expect("just-created task should be present");
        assert_eq!(tcb.address_space(), Some(0x1234_5000));
    }

    // `set_address_space` against an unknown task fails closed.
    #[test]
    fn set_address_space_against_an_unknown_task_fails_closed() {
        let mut sched: Scheduler<1> = Scheduler::new();
        let task = sched
            .create_task(low_priority(), BUDGET, OverrunPolicy::TripToSafeState, dummy_entry)
            .unwrap();
        sched.free_task_for_test(task);
        assert_eq!(sched.set_address_space(task, 0x1000), None);
    }
}
