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
//! **Scope note, and what changed in `STORY-P1-04-01`.** The functions in
//! this module are *cooperative*: they switch into a task and wait for that
//! task to switch back. Nothing here forces a yield, and that is still true.
//!
//! What is no longer true is that nothing in the kernel can. Dispatch became
//! genuinely preemptive in `STORY-P1-04-01` without a line of this module
//! changing, and the reason is worth understanding before modifying either
//! side: a timer interrupt suspends the running task by calling
//! [`switch`](crate::context::switch) *from interrupt context* into the same
//! `dispatcher_ctx` this module's own switch call is suspended at
//! ([`crate::preempt::on_timer_tick`]). Control therefore returns to
//! [`run_once`] at exactly the point it would have if the task had yielded —
//! and the task, still `TaskState::Running`, is returned to `Ready` by the
//! code below, which is precisely the right behaviour for a preempted task
//! and required no special case. The dispatcher does not know or need to
//! know how it got control back.
//!
//! The consequence for anyone editing this module: **the caller's
//! `dispatcher_ctx` is now reachable from an interrupt**, so the invariant
//! that it is the caller's own currently-suspended slot is load-bearing
//! against a second, asynchronous writer rather than only against this
//! module's own discipline.
//!
//! Blocking a task (e.g. on a contended [`crate::lock::PriorityInheritingLock`])
//! is the caller's responsibility, done between rounds via
//! `Scheduler::set_state` — this module has no automatic parking of its
//! own, matching `crate::lock`'s own doc comment ("the caller is
//! responsible for whatever blocking/retry discipline it uses while
//! contended").

use crate::context::{switch, Context};
use crate::sched::{Scheduler, TaskId, TaskState};

/// How a dispatch round must transfer control into the selected task
/// (`STORY-P1-03-02`, review D7): the pure, host-testable half of `CR3`
/// awareness. `switch_address_space` itself is a real `mov cr3` that no
/// host process can retire, so the *decision* is factored out here and the
/// hardware arm is proven under Tier 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchPlan {
    /// `Tcb::address_space` is `None`: a plain [`switch`], exactly what
    /// every task before this Story got — no `CR3` touch at all.
    Plain,
    /// `Tcb::address_space` is `Some`: install this `CR3` via
    /// [`crate::context::switch_address_space`] before the register swap.
    InstallAddressSpace(u64),
}

/// The dispatch arm `address_space` selects — the whole decision, kept as
/// one pure function so both arms are pinned by host tests even though only
/// Tier 0 can execute the install arm's `CR3` write.
pub const fn switch_plan(address_space: Option<u64>) -> SwitchPlan {
    match address_space {
        None => SwitchPlan::Plain,
        Some(cr3) => SwitchPlan::InstallAddressSpace(cr3),
    }
}

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

/// [`run_once`], `CR3`-aware (`STORY-P1-03-02`): selects exactly as
/// [`run_once`] does, then transfers control per [`switch_plan`] over the
/// selected task's `Tcb::address_space` — a plain [`switch`] for a task
/// with no dedicated space (every pre-existing Story's tasks, bit-for-bit
/// the behavior [`run_once`] has always had), or
/// [`crate::context::switch_address_space`] installing the task's own
/// `CR3` first when it has one.
///
/// A separate function rather than a change to [`run_once`], deliberately:
/// [`run_once`]'s existing tests are the no-regression guard for the
/// `Plain` arm, and this function is the one place the plan meets the
/// hardware.
///
/// # Safety
/// [`run_once`]'s contract, plus [`crate::context::switch_address_space`]'s
/// for any task whose `address_space` is `Some`: that value must be the
/// physical, page-aligned address of a fully populated PML4 mapping
/// everything the incoming task's saved registers/stack — and the kernel
/// code/IDT servicing it — need, or the install is an immediate,
/// unrecoverable fault.
#[cfg(all(target_arch = "x86_64", not(target_os = "windows")))]
pub unsafe fn run_once_in_space<const N: usize>(
    scheduler: &mut Scheduler<N>,
    dispatcher_ctx: *mut Context,
    contexts: *mut [Context; N],
) -> Option<TaskId> {
    let task = scheduler.highest_priority_ready()?;
    scheduler.set_state(task, TaskState::Running);
    let plan = switch_plan(scheduler.address_space_of(task).flatten());
    let idx = task.index();
    // SAFETY: identical to `run_once`'s — `idx < N` since the `TaskId` came
    // from this same `Scheduler<N>`, and the two context pointers satisfy
    // the switch contract per this function's own.
    let task_ctx = unsafe { (*contexts).as_mut_ptr().add(idx) };
    match plan {
        // SAFETY: forwarded from this function's own contract.
        SwitchPlan::Plain => unsafe { switch(dispatcher_ctx, task_ctx) },
        // SAFETY: forwarded from this function's own contract (the `Some`
        // clause above).
        SwitchPlan::InstallAddressSpace(cr3) => unsafe {
            crate::context::switch_address_space(dispatcher_ctx, task_ctx, cr3)
        },
    }
    if scheduler.state_of(task) == Some(TaskState::Running) {
        scheduler.set_state(task, TaskState::Ready);
    }
    Some(task)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lock::PriorityInheritingLock;
    use crate::sched::{OverrunPolicy, Priority, WcetBudgetTicks};
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
        let low = sched
            .create_task(
                priority(5),
                WcetBudgetTicks(1000),
                OverrunPolicy::TripToSafeState,
                low_entry,
            )
            .unwrap();
        let medium = sched
            .create_task(
                priority(15),
                WcetBudgetTicks(1000),
                OverrunPolicy::TripToSafeState,
                medium_entry,
            )
            .unwrap();
        let high = sched
            .create_task(
                priority(25),
                WcetBudgetTicks(1000),
                OverrunPolicy::TripToSafeState,
                low_entry,
            )
            .unwrap();
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

    // STORY-P1-03-02 AC I1 (review D7): both arms of the CR3 decision,
    // pinned on the host. `None` — the default every task ever created by
    // any pre-existing Story has — plans a plain switch; `Some` plans an
    // install of exactly that CR3.
    #[test]
    fn the_switch_plan_covers_both_arms_and_defaults_to_plain() {
        assert_eq!(switch_plan(None), SwitchPlan::Plain);
        assert_eq!(switch_plan(Some(0x1234_5000)), SwitchPlan::InstallAddressSpace(0x1234_5000));
    }

    // The scheduler read the plan consumes: a live task with no space, a
    // live task with one, and an unknown task each answer distinguishably.
    #[test]
    fn address_space_of_distinguishes_none_some_and_unknown() {
        let mut sched: Scheduler<N> = Scheduler::new();
        let plain = sched
            .create_task(
                priority(3),
                WcetBudgetTicks(100),
                OverrunPolicy::TripToSafeState,
                low_entry,
            )
            .unwrap();
        let spaced = sched
            .create_task(
                priority(3),
                WcetBudgetTicks(100),
                OverrunPolicy::TripToSafeState,
                low_entry,
            )
            .unwrap();
        sched.set_address_space(spaced, 0x9000).unwrap();

        assert_eq!(sched.address_space_of(plain), Some(None));
        assert_eq!(sched.address_space_of(spaced), Some(Some(0x9000)));
        assert_eq!(switch_plan(sched.address_space_of(plain).flatten()), SwitchPlan::Plain);
        assert_eq!(
            switch_plan(sched.address_space_of(spaced).flatten()),
            SwitchPlan::InstallAddressSpace(0x9000)
        );
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
