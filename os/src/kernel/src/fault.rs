//! Fault disposition policy and audit (`STORY-P1-02-01`).
//!
//! The kernel half of exception handling: `hal_x86_64::fault` captures *what
//! happened*, this module decides *what to do about it* and records the
//! decision. Deliberately a pure function over a captured report — no
//! `unsafe`, no hardware dependency, no I/O — so every combination of vector
//! and faulting context is host-testable, which is the only way a fault path
//! ever gets adversarial test coverage at all.
//!
//! **Two arms, both reachable, both fail-closed.**
//!
//! - A fault inside a **task** terminates that task and nothing else. The
//!   scheduler keeps running everything that did not fault — the containment
//!   property `FEAT-P1-03`'s per-task address spaces depend on (a live `CR3`
//!   switch with no fault containment behind it is strictly more dangerous
//!   than today's identity map).
//! - A fault in **kernel context** halts the system. There is no task to
//!   contain it to, and a kernel that has just violated one of its own
//!   invariants cannot know which of the others still hold.
//!
//! **There is no `Resume` arm, on purpose.** `STORY-P1-02-01`'s second
//! acceptance criterion forbids speculative "maybe recoverable" paths, and
//! this kernel has no recoverable fault case: no demand paging, no
//! copy-on-write, no guard-page stack growth. An unreachable resume arm in a
//! fault path is a liability rather than future-proofing — the day a genuine
//! recoverable case exists, it arrives with its own Story, its own
//! enumeration and its own test, together with the register save/restore the
//! entry stubs would then need.
//!
//! **The report is evidence, never authority.** [`Disposition::of`] reads
//! exactly one field: which context was running. It never consults the error
//! code, the faulting address, or the faulting instruction pointer, all of
//! which come from arbitrary and possibly attacker-steered execution
//! (`BND-04`). They are reported; they do not decide.

use crate::sched::TaskId;
use crate::spoor::{Action, Actor, Category, Outcome, Spoor};

/// Which execution context a fault interrupted.
///
/// Note what this is *not*: a hardware privilege level. Everything still runs
/// at CPL 0 in one identity-mapped address space, so this records which
/// context the kernel believes it had switched into, not a `CS`-derived ring.
/// Real privilege separation is `FEAT-P1-03`'s, and conflating the two here
/// would be a claim this Story cannot back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultingContext {
    /// A scheduled task was running.
    Task(TaskId),
    /// The kernel itself was running — no task to contain the fault to.
    Kernel,
}

/// One captured fault, reduced to what the policy is allowed to see.
///
/// The raw `hal_x86_64::fault::FaultFrame` deliberately does **not** appear
/// here. Passing it in would make it trivially available to a future policy
/// arm, and the one rule this module exists to hold is that a fault frame
/// never becomes an input to an authority decision. The vector is carried for
/// the audit record only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaultReport {
    /// The raw vector number, for the record.
    pub vector: u64,
    /// Which context was running.
    pub context: FaultingContext,
}

/// What the kernel does about a captured fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// Mark this task `Finished` and keep scheduling everything else.
    TerminateTask(TaskId),
    /// Stop: the fault was in kernel context and cannot be contained.
    HaltSystem,
}

impl Disposition {
    /// Decides what to do about `report`.
    ///
    /// One line of logic, deliberately: every additional input to this
    /// function would be a fault-frame field influencing an authority
    /// decision.
    pub const fn of(report: &FaultReport) -> Self {
        match report.context {
            FaultingContext::Task(task) => Disposition::TerminateTask(task),
            FaultingContext::Kernel => Disposition::HaltSystem,
        }
    }

    /// Whether this disposition lets the rest of the system keep running.
    pub const fn system_survives(self) -> bool {
        matches!(self, Disposition::TerminateTask(_))
    }
}

/// The audit pair every fault emits: the capture, then the decision.
///
/// Two spoors rather than one, following the module's established
/// entry/exit convention (`spoor.rs`'s own doc comment): the first records
/// that a fault happened at all, the second what was done about it. A single
/// combined atom could not distinguish "a fault was captured but the
/// disposition never ran" — which is exactly the shape a bug in this path
/// would take.
///
/// Neither spoor carries the faulting address, the error code, or any
/// register content. `PD-12` scopes a fault record to class/actor/action/
/// outcome; an audit atom is not a debugging channel, and the full frame goes
/// to the bounded serial report instead.
pub fn audit(report: &FaultReport, disposition: Disposition) -> [Spoor; 2] {
    let target = match report.context {
        FaultingContext::Task(task) => task.index() as u16,
        FaultingContext::Kernel => 0,
    };
    let captured = Spoor::stamp(
        Category::Fault,
        Actor::Kernel,
        Action::Fault,
        Outcome::Failed,
        target,
        report.vector as u32,
    );
    let decided = Spoor::stamp(
        Category::Fault,
        Actor::Kernel,
        Action::Terminate,
        match disposition {
            // The faulting task was contained: the *disposition* succeeded,
            // even though the fault that prompted it did not.
            Disposition::TerminateTask(_) => Outcome::Ok,
            // Nothing was contained; the system stopped.
            Disposition::HaltSystem => Outcome::Failed,
        },
        target,
        report.vector as u32,
    );
    [captured, decided]
}

/// The audit pair a **double fault** emits (`STORY-P1-02-02`).
///
/// Separate from [`audit`] and from [`Disposition`] entirely, and the
/// separation is the design:
///
/// - [`Disposition::of`] is not called, not extended, and does not gain a
///   vector-dependent branch. Its load-bearing invariant is that it reads
///   exactly one field — which context was running — and a `match` on the
///   vector would end that, for the one vector where the answer was never in
///   doubt.
/// - A double fault means the primary fault path itself failed while the CPU
///   was delivering a fault. There is no arm to choose between: nothing can be
///   contained, because the machinery that would do the containing is what just
///   broke. The absence of a `DoubleFaultDisposition` type is the same refusal
///   `STORY-P1-02-01` made about a `Resume` arm — an enumeration with one
///   variant is a decision that isn't one.
///
/// `context` is still carried, for **attribution only**: an auditor reading the
/// journal should be able to see which task was running when the escalation
/// began. It does not change the outcome, and the host tests below assert that
/// the disposition spoor is `Failed` in both contexts — a double fault inside a
/// task is not a contained fault, and must never audit as one.
///
/// Like [`audit`], neither spoor carries an address, an error code, or any
/// register content (`PD-12`).
pub fn audit_double_fault(context: FaultingContext) -> [Spoor; 2] {
    let target = match context {
        FaultingContext::Task(task) => task.index() as u16,
        FaultingContext::Kernel => 0,
    };
    let vector = DOUBLE_FAULT_VECTOR as u32;
    [
        Spoor::stamp(
            Category::Fault,
            Actor::Kernel,
            Action::Fault,
            Outcome::Failed,
            target,
            vector,
        ),
        Spoor::stamp(
            Category::Fault,
            Actor::Kernel,
            Action::Terminate,
            // `Failed` in both contexts. The disposition spoor answers "was
            // this contained?", and the answer is no, whoever was running.
            Outcome::Failed,
            target,
            vector,
        ),
    ]
}

/// Vector 8, re-exported at the policy layer so the kernel's audit record and
/// the HAL's stub cannot disagree about which vector a double fault is.
pub const DOUBLE_FAULT_VECTOR: u64 = hal_x86_64::fault::DOUBLE_FAULT_VECTOR;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sched::{Priority, Scheduler, WcetBudgetTicks};

    // A task has to be created by a real `Scheduler` — `TaskId` has no public
    // constructor, deliberately (see `sched::TaskId`), so these tests build
    // one rather than fabricating ids the rest of the kernel could never see.
    #[allow(clippy::empty_loop)]
    extern "C" fn dummy_entry() -> ! {
        loop {}
    }

    fn scheduler_with(count: usize) -> (Scheduler<8>, [TaskId; 8]) {
        let mut scheduler: Scheduler<8> = Scheduler::new();
        let priority = Priority::try_new(4).expect("4 is a valid priority");
        let mut ids = [None; 8];
        for slot in ids.iter_mut().take(count) {
            *slot = Some(
                scheduler
                    .create_task(priority, WcetBudgetTicks(1_000), dummy_entry)
                    .expect("slot available"),
            );
        }
        let first = ids[0].expect("at least one task");
        (scheduler, ids.map(|id| id.unwrap_or(first)))
    }

    // Clause 3: exactly two arms, decided only by which context faulted.
    #[test]
    fn a_fault_in_a_task_terminates_that_task_only() {
        let (_scheduler, tasks) = scheduler_with(3);
        for vector in [6u64, 13, 14] {
            let report = FaultReport { vector, context: FaultingContext::Task(tasks[1]) };
            assert_eq!(Disposition::of(&report), Disposition::TerminateTask(tasks[1]));
            assert!(Disposition::of(&report).system_survives());
        }
    }

    #[test]
    fn a_fault_in_kernel_context_halts_the_system() {
        for vector in [6u64, 13, 14] {
            let report = FaultReport { vector, context: FaultingContext::Kernel };
            assert_eq!(Disposition::of(&report), Disposition::HaltSystem);
            assert!(!Disposition::of(&report).system_survives());
        }
    }

    // Clause 2: the vector is recorded but never decides. Two faults from the
    // same context must reach the same disposition whatever fired them —
    // including a vector this kernel does not even wire.
    #[test]
    fn the_vector_is_recorded_but_never_changes_the_decision() {
        let (_scheduler, tasks) = scheduler_with(2);
        let victim = FaultingContext::Task(tasks[0]);
        let baseline = Disposition::of(&FaultReport { vector: 14, context: victim });
        for vector in [0u64, 6, 8, 13, 14, 255, u64::MAX] {
            assert_eq!(
                Disposition::of(&FaultReport { vector, context: victim }),
                baseline,
                "vector {vector} changed the disposition"
            );
        }
    }

    #[test]
    fn every_task_is_contained_to_itself() {
        let (_scheduler, tasks) = scheduler_with(4);
        for task in tasks.iter().take(4) {
            let report = FaultReport { vector: 13, context: FaultingContext::Task(*task) };
            assert_eq!(Disposition::of(&report), Disposition::TerminateTask(*task));
        }
    }

    // Clause 5: the audit pair, and what it must not carry.
    #[test]
    fn a_contained_fault_audits_as_a_failed_fault_with_a_successful_disposition() {
        let (_scheduler, tasks) = scheduler_with(4);
        let victim = tasks[3];
        let report = FaultReport { vector: 14, context: FaultingContext::Task(victim) };
        let [captured, decided] = audit(&report, Disposition::of(&report));

        assert_eq!(captured.category(), Category::Fault);
        assert_eq!(captured.who(), Actor::Kernel);
        assert_eq!(captured.action(), Action::Fault);
        assert_eq!(captured.outcome(), Outcome::Failed);
        assert_eq!(captured.target(), victim.index() as u16);

        assert_eq!(decided.action(), Action::Terminate);
        assert_eq!(decided.outcome(), Outcome::Ok);
        assert_eq!(decided.target(), victim.index() as u16);
    }

    #[test]
    fn an_uncontained_fault_audits_its_disposition_as_failed_too() {
        let report = FaultReport { vector: 13, context: FaultingContext::Kernel };
        let [captured, decided] = audit(&report, Disposition::of(&report));
        assert_eq!(captured.outcome(), Outcome::Failed);
        assert_eq!(decided.action(), Action::Terminate);
        assert_eq!(decided.outcome(), Outcome::Failed);
    }

    #[test]
    fn a_fault_spoor_carries_no_address_and_no_error_code() {
        // The atom has exactly two payload fields (target, cost). If a
        // faulting address or error code were ever smuggled in, it would have
        // to be one of them — so the payload is pinned here: the task index
        // and the vector, and nothing else.
        let (_scheduler, tasks) = scheduler_with(2);
        let report = FaultReport { vector: 14, context: FaultingContext::Task(tasks[1]) };
        let first = audit(&report, Disposition::of(&report));
        assert_eq!(first, audit(&report, Disposition::of(&report)));
        assert_eq!(first[0].cost(), 14);
        assert_eq!(first[0].target(), tasks[1].index() as u16);
    }

    // `TEST-P1-02-02-A` clause 5: the double fault is audited, attributed, and
    // never claims containment — in either context.
    #[test]
    fn a_double_fault_never_audits_as_contained_whatever_was_running() {
        let (_scheduler, tasks) = scheduler_with(3);
        for context in [FaultingContext::Task(tasks[2]), FaultingContext::Kernel] {
            let [captured, decided] = audit_double_fault(context);
            assert_eq!(captured.category(), Category::Fault);
            assert_eq!(captured.action(), Action::Fault);
            assert_eq!(captured.outcome(), Outcome::Failed);
            assert_eq!(decided.action(), Action::Terminate);
            assert_eq!(
                decided.outcome(),
                Outcome::Failed,
                "a double fault inside a task is still not a contained fault"
            );
            assert_eq!(captured.cost(), DOUBLE_FAULT_VECTOR as u32);
        }
    }

    // Attribution, not disposition: the task index is recorded so an auditor
    // can see what was running, and it changes nothing else.
    #[test]
    fn a_double_fault_records_which_task_was_running() {
        let (_scheduler, tasks) = scheduler_with(4);
        let [captured, decided] = audit_double_fault(FaultingContext::Task(tasks[3]));
        assert_eq!(captured.target(), tasks[3].index() as u16);
        assert_eq!(decided.target(), tasks[3].index() as u16);
        let [kernel_captured, _] = audit_double_fault(FaultingContext::Kernel);
        assert_eq!(kernel_captured.target(), 0);
    }

    // Clause 5, stated as a test rather than as a comment: the containment
    // policy is untouched by this Story. `Disposition::of` still reaches the
    // same two arms for vector 8 as for anything else, precisely because it
    // never sees the vector — and nothing routes a double fault through it.
    #[test]
    fn the_double_fault_vector_does_not_reach_the_containment_policy() {
        assert_eq!(DOUBLE_FAULT_VECTOR, 8);
        let (_scheduler, tasks) = scheduler_with(2);
        let victim = FaultingContext::Task(tasks[0]);
        assert_eq!(
            Disposition::of(&FaultReport { vector: DOUBLE_FAULT_VECTOR, context: victim }),
            Disposition::of(&FaultReport { vector: 14, context: victim }),
            "the policy must still be blind to the vector"
        );
    }

    #[test]
    fn double_fault_spoors_round_trip_through_the_journal_encoding() {
        let (_scheduler, tasks) = scheduler_with(6);
        for spoor in audit_double_fault(FaultingContext::Task(tasks[4])) {
            let decoded = Spoor::decode(spoor.to_bits()).expect("a stamped spoor must decode");
            assert_eq!(decoded.category(), Category::Fault);
            assert_eq!(decoded.outcome(), Outcome::Failed);
            assert_eq!(decoded.target(), tasks[4].index() as u16);
        }
    }

    #[test]
    fn fault_spoors_round_trip_through_the_journal_encoding() {
        let (_scheduler, tasks) = scheduler_with(6);
        let report = FaultReport { vector: 6, context: FaultingContext::Task(tasks[5]) };
        for spoor in audit(&report, Disposition::of(&report)) {
            let decoded = Spoor::decode(spoor.to_bits()).expect("a stamped spoor must decode");
            assert_eq!(decoded.category(), Category::Fault);
            assert_eq!(decoded.who(), Actor::Kernel);
            assert_eq!(decoded.outcome(), spoor.outcome());
            assert_eq!(decoded.target(), tasks[5].index() as u16);
        }
    }
}
