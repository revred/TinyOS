//! Priority-ordered cooperative dispatch loop (`STORY-P0-02-05`).
//!
//! The piece `STORY-P0-02-03` (priority inheritance) and `STORY-P0-02-04`
//! (WCET enforcement) both named as a missing prerequisite for their own
//! *behavioral* guarantees: something that actually runs tasks in priority
//! order via `context::switch`, rather than only bookkeeping who *should*
//! run. [`run_once`] selects the highest-priority [`crate::sched::TaskState::Ready`]
//! task (`Scheduler::highest_priority_ready`) and switches into it; the
//! task yields back cooperatively by calling [`crate::context::switch`]
//! itself, into the caller-supplied `dispatcher_ctx`.
//!
//! **Scope note.** This is a *cooperative* dispatcher, not a preemptive
//! one: there is still no timer interrupt / IDT in this kernel
//! (`STORY-P0-05-02`'s named gap), so nothing forces a running task to
//! yield — it must call `switch` back itself. This is enough to
//! behaviorally prove `STORY-P0-02-03`'s and `STORY-P0-02-04`'s own claims
//! for a task that yields at controlled points (this module's own test
//! does exactly that: a boosted lock holder is actually *chosen and run*
//! ahead of an uninvolved, higher-static-priority Ready task) — but it
//! does not prove true preemption of a task that never yields voluntarily,
//! which still needs the timer/IDT this kernel doesn't have.
//!
//! Blocking a task (e.g. on a contended [`crate::lock::PriorityInheritingLock`])
//! is the caller's responsibility, done between rounds via
//! `Scheduler::set_state` — this module has no automatic parking of its
//! own, matching `crate::lock`'s own doc comment ("the caller is
//! responsible for whatever blocking/retry discipline it uses while
//! contended").

use crate::context::{switch, Context};
use crate::sched::{Scheduler, TaskId, TaskState};

/// Selects the highest-priority Ready task in `scheduler` and runs it for
/// one cooperative slice: switches from `dispatcher_ctx` into
/// `contexts[task.index()]`, and resumes here (updating nothing else) the
/// moment that task switches back.
///
/// A task still `TaskState::Running` when it yields back (i.e. one that
/// didn't transition itself to `Blocked`/`Finished` before yielding) is
/// returned to `TaskState::Ready` — the default cooperative round-robin
/// behavior. A task that *did* change its own state (there is no way for
/// task code running on its own stack to safely call back into `scheduler`
/// today, so in practice this transition happens via the caller, between
/// `run_once` calls, exactly as this module's own test does around lock
/// contention) is left as that caller set it.
///
/// Returns the [`TaskId`] that ran, or `None` if no task is currently
/// Ready.
///
/// # Safety
/// `contexts[task.index()]` for whichever task [`Scheduler::highest_priority_ready`]
/// selects must be a valid [`Context`] — either freshly built by
/// [`Context::new`] and not yet resumed, or previously suspended by a
/// `switch` back into `dispatcher_ctx` and not resumed since — mirroring
/// [`switch`]'s own safety contract. `dispatcher_ctx` must be the caller's
/// own currently-suspended slot.
pub unsafe fn run_once<const N: usize>(
    scheduler: &mut Scheduler<N>,
    dispatcher_ctx: *mut Context,
    contexts: *mut [Context; N],
) -> Option<TaskId> {
    let task = scheduler.highest_priority_ready()?;
    scheduler.set_state(task, TaskState::Running);
    let idx = task.index();
    // SAFETY: `contexts` is a valid pointer to an `[Context; N]` per this
    // function's own contract, and `idx < N` since it came from a `TaskId`
    // this same `Scheduler<N>` issued (its backing `Pool<Tcb, N>` never
    // hands out an index `>= N`); `task_ctx` and `dispatcher_ctx` satisfy
    // `switch`'s own contract per this function's contract above.
    let task_ctx = unsafe { (*contexts).as_mut_ptr().add(idx) };
    // SAFETY: forwarded from this function's own contract.
    unsafe { switch(dispatcher_ctx, task_ctx) };
    if scheduler.state_of(task) == Some(TaskState::Running) {
        scheduler.set_state(task, TaskState::Ready);
    }
    Some(task)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lock::PriorityInheritingLock;
    use crate::sched::{Priority, WcetBudgetTicks};
    use crate::spoor_journal::SpoorJournal;

    const STACK_SIZE: usize = 4096;
    const N: usize = 4;

    fn priority(value: u8) -> Priority {
        Priority::try_new(value).expect("value is in range")
    }

    static mut DISPATCHER_CTX: Context = Context::zeroed();
    static mut CONTEXTS: [Context; N] = [Context::zeroed(); N];
    static mut STACK_LOW: [u8; STACK_SIZE] = [0; STACK_SIZE];
    static mut STACK_MEDIUM: [u8; STACK_SIZE] = [0; STACK_SIZE];
    static mut RUN_LOG: [u8; 8] = [0; 8];
    static mut RUN_LOG_LEN: usize = 0;

    // Task functions yield back to the dispatcher by switching into the
    // well-known `DISPATCHER_CTX`/`CONTEXTS` statics — the same
    // static-storage discipline `context.rs`'s own host tests and
    // `context_switch_fixture.rs` already use for the identical
    // move-safety reason (a `Context`/stack must never move once a
    // `switch` has been taken into it).

    extern "C" fn low_entry() -> ! {
        loop {
            // SAFETY: this test is the sole user of these statics, run
            // single-threaded/serially; each `&mut`/raw-pointer use below
            // is dropped before the next `switch` hands control elsewhere.
            unsafe {
                RUN_LOG[RUN_LOG_LEN] = 0;
                RUN_LOG_LEN += 1;
                switch(&raw mut CONTEXTS[0], &raw mut DISPATCHER_CTX);
            }
        }
    }

    extern "C" fn medium_entry() -> ! {
        loop {
            // SAFETY: see `low_entry`.
            unsafe {
                RUN_LOG[RUN_LOG_LEN] = 1;
                RUN_LOG_LEN += 1;
                switch(&raw mut CONTEXTS[1], &raw mut DISPATCHER_CTX);
            }
        }
    }

    // STORY-P0-02-05 / behavioral closure of STORY-P0-02-03's own named
    // gap: without contention, the dispatcher picks the higher
    // *static*-priority medium task over low. Once a third (high-priority,
    // never actually run) task contends for a lock low holds — boosting
    // low's priority above medium's via `PriorityInheritingLock` — the
    // *next* round picks low instead, proving the boost isn't just a
    // recorded number but actually changes which task a real dispatch
    // round selects and runs (a genuine `context::switch` into low's own
    // task function, not a priority-value assertion standing in for it).
    #[test]
    #[allow(static_mut_refs, clippy::deref_addrof)]
    fn dispatcher_runs_the_boosted_holder_ahead_of_an_uninvolved_ready_task_after_contention() {
        let mut sched: Scheduler<N> = Scheduler::new();
        let low = sched.create_task(priority(5), WcetBudgetTicks(1000), low_entry).unwrap();
        let medium = sched.create_task(priority(15), WcetBudgetTicks(1000), medium_entry).unwrap();
        let high = sched.create_task(priority(25), WcetBudgetTicks(1000), low_entry).unwrap();
        assert_eq!(low.index(), 0, "Pool::alloc's first-free-slot order underpins this test");
        assert_eq!(medium.index(), 1);
        // `high` never actually runs in this test (no `Context` of its own
        // is ever initialized for it) — it exists only as the `TaskId`
        // that contends for `low`'s lock below. Marking it `Blocked`
        // immediately keeps it out of every `highest_priority_ready`
        // selection despite outranking both other tasks, so `run_once`
        // never tries to switch into its (deliberately never-initialized)
        // context slot.
        sched.set_state(high, TaskState::Blocked).unwrap();

        // SAFETY: sole test touching these statics.
        unsafe {
            RUN_LOG_LEN = 0;
            CONTEXTS[0] = Context::new(&mut *&raw mut STACK_LOW, low_entry).unwrap();
            CONTEXTS[1] = Context::new(&mut *&raw mut STACK_MEDIUM, medium_entry).unwrap();
        }

        // Baseline: no contention yet — medium (static priority 15)
        // outranks low (5), so the dispatcher picks medium first.
        // SAFETY: contexts/dispatcher_ctx satisfy `run_once`'s contract —
        // both freshly initialized/zeroed and not concurrently used.
        let first = unsafe { run_once(&mut sched, &raw mut DISPATCHER_CTX, &raw mut CONTEXTS) };
        assert_eq!(first, Some(medium));

        // Low acquires a lock (out of band — representing it already holds
        // a resource before this scenario's contention begins).
        let mut lock = PriorityInheritingLock::new();
        let mut journal: SpoorJournal<4> = SpoorJournal::new();
        assert_eq!(lock.try_lock(&mut sched, &mut journal, low, priority(5)), Ok(()));

        // High contends for the lock low holds: boosts low to high's
        // priority (25). `high` was already marked Blocked above (this
        // module's own scope: it doesn't auto-park a contending task, the
        // caller does).
        assert_eq!(
            lock.try_lock(&mut sched, &mut journal, high, priority(25)),
            Err(crate::lock::LockError::AlreadyLocked)
        );
        assert_eq!(sched.priority_of(low), Some(priority(25)));

        // Now: low (boosted to 25) outranks medium (15) — the dispatcher
        // must pick low next, not medium again, proving the boost actually
        // changes a real scheduling decision.
        // SAFETY: see above.
        let second = unsafe { run_once(&mut sched, &raw mut DISPATCHER_CTX, &raw mut CONTEXTS) };
        assert_eq!(second, Some(low));

        // Low releases the lock, restoring its original priority; medium
        // (now the only Ready task left, since low returns to Ready too
        // but at its restored priority 5) is picked again.
        assert_eq!(lock.unlock(&mut sched, &mut journal, low), Ok(()));
        assert_eq!(sched.priority_of(low), Some(priority(5)));
        // SAFETY: see above.
        let third = unsafe { run_once(&mut sched, &raw mut DISPATCHER_CTX, &raw mut CONTEXTS) };
        assert_eq!(third, Some(medium));

        // SAFETY: sole test touching this static; read after all switches
        // above have returned.
        let log = unsafe { &RUN_LOG[..RUN_LOG_LEN] };
        assert_eq!(log, &[1, 0, 1], "medium, then boosted-low, then medium again");
    }

    // With no Ready task at all, `run_once` returns `None` and switches
    // into nothing.
    #[test]
    #[allow(static_mut_refs, clippy::deref_addrof)]
    fn run_once_against_an_empty_scheduler_returns_none() {
        let mut sched: Scheduler<N> = Scheduler::new();
        // SAFETY: sole test touching these statics for this call; no
        // switch occurs since `highest_priority_ready` short-circuits
        // `run_once` before ever dereferencing `contexts`/`dispatcher_ctx`.
        let result = unsafe { run_once(&mut sched, &raw mut DISPATCHER_CTX, &raw mut CONTEXTS) };
        assert_eq!(result, None);
    }
}
