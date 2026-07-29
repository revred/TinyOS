//! The bounded decision-to-actuation path (`STORY-P1-06-01`,
//! `TEST-P1-06-01-A`) — `G-PA-1`'s flagship primitive.
//!
//! One [`ActuationPort`] is one actuator line with one declared owner and one
//! declared deadline. A command reaches the line only if the caller **is** the
//! declared actuation task, that task is **running**, the activation window is
//! **armed**, and the declared deadline has **not** passed. Every other
//! outcome is a refusal that leaves the line untouched and stamps a spoor.
//!
//! ## The deadline is not the WCET budget
//!
//! `STORY-P1-04-02` built budget enforcement and closed its own scope note
//! honestly: *"A declared deadline is a different quantity from a declared
//! execution budget, and this Story enforces the latter."* This module is the
//! other one, and the two count different things:
//!
//! | Quantity | Counts | Advances while descheduled? | Enforced by |
//! |---|---|---|---|
//! | [`crate::sched::WcetBudgetTicks`] | ticks *attributed* to the task | **no** | [`crate::wcet::account_tick`] → declared [`crate::sched::OverrunPolicy`] |
//! | [`DeadlineTicks`] | ticks since [`ActuationPort::arm`] | **yes** | [`ActuationPort::on_tick`] → the emit is refused |
//!
//! A task that is preempted and starved meets its budget perfectly and misses
//! its deadline badly. That divergence is the entire reason this is a separate
//! mechanism rather than a second reading of the budget counter, and it is
//! pinned by a host test rather than left as an argument.
//!
//! ## Why the authority check is first, and why the line cannot refuse
//!
//! [`ActuationPort::emit`] checks the caller's identity **before** it consults
//! the window. An unauthorized caller therefore gets the same answer whether
//! the window is open, closed or was never armed — so a refusal cannot be used
//! as an oracle for the RT task's timing state (`SEC-14`, `PD-14`'s
//! no-ambient-authority posture read forward into a timing side channel).
//!
//! And [`hal::actuation::OutputLine`] deliberately has no refusal of its own:
//! *whether* to actuate is decided here, once. A line that could also refuse
//! would put one decision in two places and let them disagree — the same
//! reasoning [`crate::wcet::disposition_for`] uses for reading exactly one
//! input.
//!
//! ## Real-time discipline
//!
//! No allocation, no lock, no unbounded loop, no panic on any path. Every
//! function here is straight-line over fixed-size state, callable from a timer
//! ISR ([`ActuationPort::on_tick`]) or from RT task context with interrupts
//! masked ([`ActuationPort::emit`]) — and it must be one of those two, because
//! the port's state is read by both.

use crate::sched::{Scheduler, TaskId, TaskState};
use crate::spoor::{Action, Actor, Category, Outcome, Spoor};
use crate::spoor_journal::SpoorJournal;
use hal::actuation::OutputLine;

/// A declared **relative deadline**, in timer ticks, measured from the
/// activation [`ActuationPort::arm`] starts.
///
/// A newtype rather than a bare `u32` for the same reason
/// [`crate::sched::WcetBudgetTicks`] is one: the two are both "a number of
/// ticks" and mean entirely different things, and a call site that swapped them
/// would compile and be wrong in a way no test could name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeadlineTicks(pub u32);

/// Why an actuation command did not reach the line.
///
/// Per `agent/CODING_STANDARDS.md`'s "no stringly-typed errors" rule, and
/// deliberately without an "other" arm: every variant here names a decision
/// this module actually took.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActuationError {
    /// The caller is not the declared actuation task, or is no longer running.
    /// The line was not written and nothing about the window was consulted.
    NotAuthorized,
    /// No activation is armed. Fails closed: an actuation with no activation
    /// behind it is a command nobody decided to issue.
    NotArmed,
    /// The declared deadline passed before the command was presented. **The
    /// late command was prevented, not logged** — this is the refusal
    /// `STORY-P1-06-01` acceptance criterion 2 is about.
    DeadlineMissed,
}

/// Where the current activation stands against its declared deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineStatus {
    /// No activation is armed.
    Idle,
    /// Armed, with this many ticks elapsed — still inside the declared
    /// deadline.
    WithinDeadline(u32),
    /// Armed, and the declared deadline has passed. Any command presented from
    /// here is refused.
    Missed,
}

/// One actuator line, its declared owner, and its declared deadline.
///
/// Constructed by [`ActuationPort::declare`] and **never re-pointed**: there is
/// no setter for the owner, the deadline or the line. That is the whole of the
/// "no ambient path to the output primitive" property — an authority that one
/// `pub fn` could move is not a containment property, it is a convention.
#[derive(Debug)]
pub struct ActuationPort<L: OutputLine> {
    line: L,
    owner: TaskId,
    deadline: DeadlineTicks,
    /// Ticks elapsed in the current activation, or `None` when none is armed.
    elapsed: Option<u32>,
    /// Whether the current activation's miss has already been stamped, so a
    /// long overrun produces one audit record and not one per tick.
    missed: bool,
    emitted: u32,
    refused: u32,
}

impl<L: OutputLine> ActuationPort<L> {
    /// Declares `owner` as the only task that may drive `line`, with `deadline`
    /// ticks allowed between an activation and its command.
    ///
    /// Not a `const fn`: the owner is a [`TaskId`] the scheduler issues at
    /// runtime, so a port that could be built in a `static` initializer would
    /// have to be built without one — which is exactly the ambient, ownerless
    /// port this type exists to make unrepresentable.
    pub fn declare(line: L, owner: TaskId, deadline: DeadlineTicks) -> Self {
        ActuationPort {
            line,
            owner,
            deadline,
            elapsed: None,
            missed: false,
            emitted: 0,
            refused: 0,
        }
    }

    /// The declared actuation task. Read-only, for evidence.
    pub fn owner(&self) -> TaskId {
        self.owner
    }

    /// The declared relative deadline.
    pub fn deadline(&self) -> DeadlineTicks {
        self.deadline
    }

    /// The output line, for a fixture to read back what it recorded. There is
    /// no `&mut` accessor: a caller that could reach the line directly would be
    /// the ambient path.
    pub fn line(&self) -> &L {
        &self.line
    }

    /// How many commands have reached the line.
    pub fn emitted(&self) -> u32 {
        self.emitted
    }

    /// How many commands were refused, for any reason.
    pub fn refused(&self) -> u32 {
        self.refused
    }

    /// Where the current activation stands.
    pub fn status(&self) -> DeadlineStatus {
        match (self.elapsed, self.missed) {
            (None, _) => DeadlineStatus::Idle,
            (Some(_), true) => DeadlineStatus::Missed,
            (Some(elapsed), false) => DeadlineStatus::WithinDeadline(elapsed),
        }
    }

    /// Starts a fresh activation: the decision has begun, and the clock the
    /// deadline is measured against starts now.
    ///
    /// Stamps nothing. An activation is not a boundary decision — it is the
    /// *start* of the interval one will be taken over — and stamping every arm
    /// would turn the journal into a call-count log, which
    /// [`crate::lock`]'s own "audit trail of what changed" discipline
    /// explicitly refuses.
    pub fn arm(&mut self) {
        self.elapsed = Some(0);
        self.missed = false;
    }

    /// Abandons the current activation without actuating, returning whether
    /// there was one to abandon.
    ///
    /// **Why this exists rather than letting an unsatisfied activation lapse.**
    /// An activation nobody cancels stays armed, expires a few ticks later, and
    /// reports a missed deadline belonging to no decision anybody took — a
    /// phantom miss, which is worse than a silent one because it is evidence
    /// that looks real. A cancelled decision has to be cancelled explicitly.
    ///
    /// Stamps nothing, for the same reason [`ActuationPort::arm`] does not:
    /// nothing crossed the boundary, and nothing was refused at it.
    pub fn disarm(&mut self) -> bool {
        let was_armed = self.elapsed.is_some();
        self.elapsed = None;
        self.missed = false;
        was_armed
    }

    /// Charges one timer tick to the current activation and reports where it
    /// leaves it.
    ///
    /// Detection is on the tick that takes elapsed **past** the declared
    /// deadline; elapsed exactly equal to it is not yet a miss. That is
    /// [`crate::wcet::record_tick`]'s rule for budgets, held here for deadlines
    /// so the two can never drift into disagreeing about what "exceeded" means.
    ///
    /// The miss stamps exactly once per activation.
    ///
    /// Called from the timer ISR. Bounded, allocation-free, and it takes no
    /// scheduler borrow at all — so it can run beside
    /// [`crate::wcet::account_tick`] in the same hook without the two
    /// contending for the same `&mut`.
    pub fn on_tick<const J: usize>(&mut self, journal: &mut SpoorJournal<J>) -> DeadlineStatus {
        let Some(elapsed) = self.elapsed else {
            return DeadlineStatus::Idle;
        };
        // Saturating rather than wrapping: an activation left armed for four
        // billion ticks is already missed, and a wrapped counter would report
        // it as freshly armed — the one arithmetic mistake on this path that
        // produces a *late actuation* rather than a spurious refusal.
        let elapsed = elapsed.saturating_add(1);
        self.elapsed = Some(elapsed);
        if elapsed <= self.deadline.0 {
            return DeadlineStatus::WithinDeadline(elapsed);
        }
        if !self.missed {
            self.missed = true;
            journal.append(Spoor::stamp(
                Category::Actuation,
                Actor::Kernel,
                Action::Deadline,
                Outcome::Failed,
                self.owner.index() as u16,
                elapsed,
            ));
        }
        DeadlineStatus::Missed
    }

    /// Presents `command` at the output boundary on behalf of `caller`.
    ///
    /// Reaches the line only if all four hold: `caller` is the declared owner,
    /// the scheduler says that task is [`TaskState::Running`], an activation is
    /// armed, and its deadline has not passed. On success the activation is
    /// **consumed** — one armed window is one actuation, never a licence to
    /// keep driving the line.
    ///
    /// The order of the checks is load-bearing; see the module doc. Every arm,
    /// including the successful one, stamps.
    ///
    /// Takes `&Scheduler` rather than trusting the caller's own claim about who
    /// it is: identity is kernel-derived (`PD-02`), and a `TaskId` is data an
    /// RT task could hold a stale copy of.
    pub fn emit<const N: usize, const J: usize>(
        &mut self,
        scheduler: &Scheduler<N>,
        journal: &mut SpoorJournal<J>,
        caller: TaskId,
        command: u8,
    ) -> Result<(), ActuationError> {
        // Authority first, and nothing about the window read before it.
        if caller != self.owner || scheduler.state_of(caller) != Some(TaskState::Running) {
            return Err(self.refuse(journal, caller, Outcome::Failed, ActuationError::NotAuthorized));
        }
        match self.status() {
            DeadlineStatus::Idle => {
                Err(self.refuse(journal, caller, Outcome::Failed, ActuationError::NotArmed))
            }
            DeadlineStatus::Missed => {
                // `Capped` rather than `Failed`: nothing failed. A declared
                // bound was enforced, which is the same shape a WCET degrade
                // stamps and the same word `crate::wcet::apply` uses for it.
                Err(self.refuse(journal, caller, Outcome::Capped, ActuationError::DeadlineMissed))
            }
            DeadlineStatus::WithinDeadline(_) => {
                // The write happens here and in no other branch — one call, one
                // actuation, and the audit record is stamped after the command
                // has actually left, so a journal entry can never claim an
                // actuation the line did not take.
                self.line.write_command(command);
                self.elapsed = None;
                self.missed = false;
                self.emitted = self.emitted.saturating_add(1);
                journal.append(Spoor::stamp(
                    Category::Actuation,
                    Actor::Kernel,
                    Action::Actuate,
                    Outcome::Ok,
                    caller.index() as u16,
                    command as u32,
                ));
                Ok(())
            }
        }
    }

    /// Records a refusal: counts it, stamps it, and returns the error. Written
    /// once rather than three times so that "no refusal is silent" is a
    /// property of the code's shape and not of three call sites remembering.
    ///
    /// `TARGET` is the **caller**, not the owner, so an unauthorized attempt
    /// names who attempted it.
    fn refuse<const J: usize>(
        &mut self,
        journal: &mut SpoorJournal<J>,
        caller: TaskId,
        outcome: Outcome,
        error: ActuationError,
    ) -> ActuationError {
        self.refused = self.refused.saturating_add(1);
        journal.append(Spoor::stamp(
            Category::Actuation,
            Actor::Kernel,
            Action::Actuate,
            outcome,
            caller.index() as u16,
            self.elapsed.unwrap_or(0),
        ));
        error
    }
}

#[cfg(test)]
mod tests {
    use crate::actuation::{ActuationError, ActuationPort, DeadlineStatus, DeadlineTicks};
    use crate::sched::{OverrunPolicy, Priority, Scheduler, TaskId, TaskState, WcetBudgetTicks};
    use crate::spoor::{Action, Actor, Category, Outcome, Spoor};
    use crate::spoor_journal::SpoorJournal;
    use crate::wcet;
    use hal::actuation::OutputLine;
    use std::vec::Vec;

    #[allow(clippy::empty_loop)]
    extern "C" fn dummy_entry() -> ! {
        loop {}
    }

    /// The host test double for the output boundary. Counts writes
    /// independently of anything the port itself records, because "the
    /// actuator moved" must be observable somewhere the port does not own.
    #[derive(Default)]
    struct RecordingLine {
        writes: u32,
        last: Option<u8>,
    }

    impl OutputLine for RecordingLine {
        const NAME: &'static str = "recording";

        fn write_command(&mut self, command: u8) {
            self.writes += 1;
            self.last = Some(command);
        }
    }

    fn priority(value: u8) -> Priority {
        Priority::try_new(value).expect("value is in range")
    }

    fn task(sched: &mut Scheduler<4>, p: u8, budget: u32) -> TaskId {
        sched
            .create_task(
                priority(p),
                WcetBudgetTicks(budget),
                OverrunPolicy::TripToSafeState,
                dummy_entry,
            )
            .expect("a slot is free")
    }

    /// The declared actuation task, running, with a port declared over it.
    fn armed_port(
        sched: &mut Scheduler<4>,
        deadline: u32,
    ) -> (TaskId, ActuationPort<RecordingLine>) {
        let owner = task(sched, 25, 1_000);
        sched.set_state(owner, TaskState::Running).expect("the task is live");
        let mut port =
            ActuationPort::declare(RecordingLine::default(), owner, DeadlineTicks(deadline));
        port.arm();
        (owner, port)
    }

    // Clause 1: the happy path writes the line exactly once, with exactly the
    // command word it was given, and disarms.
    #[test]
    fn an_authorized_emit_inside_the_window_writes_the_line_exactly_once() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<16> = SpoorJournal::new();
        let (owner, mut port) = armed_port(&mut sched, 4);

        assert_eq!(port.emit(&sched, &mut journal, owner, 0xA5), Ok(()));
        assert_eq!(port.line().writes, 1);
        assert_eq!(port.line().last, Some(0xA5));
        assert_eq!(port.emitted(), 1);

        // Disarmed by the emit: a second command with no fresh activation is
        // refused. One armed window is one actuation, never a licence to keep
        // driving the line.
        assert_eq!(
            port.emit(&sched, &mut journal, owner, 0x5A),
            Err(ActuationError::NotArmed),
            "one activation is one actuation"
        );
        assert_eq!(port.line().writes, 1, "the refused command must not reach the line");
        assert_eq!(port.line().last, Some(0xA5));
    }

    // Clause 2: a caller that is not the declared task is refused, and the
    // line is untouched. This is the "no ambient path" claim.
    #[test]
    fn an_unauthorized_caller_is_refused_and_the_line_is_never_written() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<16> = SpoorJournal::new();
        let (_owner, mut port) = armed_port(&mut sched, 4);
        let intruder = task(&mut sched, 5, 1_000);
        sched.set_state(intruder, TaskState::Running).expect("the task is live");

        assert_eq!(
            port.emit(&sched, &mut journal, intruder, 0xFF),
            Err(ActuationError::NotAuthorized)
        );
        assert_eq!(port.line().writes, 0, "an unauthorized identity never reaches the line");
        assert_eq!(port.line().last, None);
        assert_eq!(port.emitted(), 0);
        assert_eq!(port.refused(), 1);
        // And the window it was refused from is still armed and unchanged: a
        // refusal must not consume the owner's activation either.
        assert_eq!(port.status(), DeadlineStatus::WithinDeadline(0));
    }

    // Clause 2, the ordering half: the authority check runs *before* the
    // deadline state is consulted, so an unauthorized caller is refused
    // identically whether the window is open, expired, or was never armed —
    // and therefore learns nothing about it from the answer it gets.
    #[test]
    fn authority_is_checked_before_the_deadline_state_is_consulted() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<16> = SpoorJournal::new();
        let owner = task(&mut sched, 25, 1_000);
        sched.set_state(owner, TaskState::Running).expect("the task is live");
        let intruder = task(&mut sched, 5, 1_000);
        sched.set_state(intruder, TaskState::Running).expect("the task is live");
        let mut port =
            ActuationPort::declare(RecordingLine::default(), owner, DeadlineTicks(1));

        // Never armed.
        assert_eq!(
            port.emit(&sched, &mut journal, intruder, 1),
            Err(ActuationError::NotAuthorized)
        );
        // Armed and open.
        port.arm();
        assert_eq!(
            port.emit(&sched, &mut journal, intruder, 2),
            Err(ActuationError::NotAuthorized)
        );
        // Armed and expired.
        port.on_tick(&mut journal);
        port.on_tick(&mut journal);
        assert_eq!(port.status(), DeadlineStatus::Missed);
        assert_eq!(
            port.emit(&sched, &mut journal, intruder, 3),
            Err(ActuationError::NotAuthorized),
            "the same answer in all three states, or the refusal is an oracle"
        );
        assert_eq!(port.line().writes, 0);
    }

    // Clause 2: the declaration is immutable for the port's whole life. There
    // is no setter, so this test is the place a future one would have to be
    // added — and adding it would break the containment property rather than
    // extend the API.
    #[test]
    fn the_declared_owner_cannot_be_changed() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let (owner, port) = armed_port(&mut sched, 4);
        let other = task(&mut sched, 5, 1_000);

        assert_eq!(port.owner(), owner);
        assert_ne!(port.owner(), other);
        // The whole API surface that could move authority, enumerated: there
        // is none. If this assertion ever needs updating, read clause 2 first.
        assert_eq!(port.owner(), owner);
    }

    // Clause 2/5: an owner that is not `Running` is refused. This is what
    // makes prevention independent of the task never being scheduled again —
    // a tripped task's own identity cannot actuate.
    #[test]
    fn an_owner_that_is_not_running_is_refused() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<16> = SpoorJournal::new();
        let (owner, mut port) = armed_port(&mut sched, 4);

        sched.set_state(owner, TaskState::Finished).expect("the task is live");
        assert_eq!(
            port.emit(&sched, &mut journal, owner, 0x11),
            Err(ActuationError::NotAuthorized)
        );
        assert_eq!(port.line().writes, 0, "a tripped task cannot actuate");

        // And an owner the scheduler no longer knows at all fails closed the
        // same way rather than panicking or being treated as authorized.
        sched.free_task_for_test(owner);
        assert_eq!(
            port.emit(&sched, &mut journal, owner, 0x22),
            Err(ActuationError::NotAuthorized)
        );
        assert_eq!(port.line().writes, 0);
    }

    // Clause 3: expiry is exact. Elapsed *equal to* the declared deadline is
    // not yet a miss; the tick that takes it past is. Same discipline
    // `wcet::record_tick` holds for budgets.
    #[test]
    fn the_deadline_expires_on_the_tick_that_crosses_it_and_not_before() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<16> = SpoorJournal::new();
        let (owner, mut port) = armed_port(&mut sched, 3);

        for elapsed in 1..=3 {
            assert_eq!(
                port.on_tick(&mut journal),
                DeadlineStatus::WithinDeadline(elapsed),
                "elapsed equal to the declared deadline is not a miss"
            );
        }
        assert!(journal.is_empty(), "a within-deadline tick stamps nothing");
        assert_eq!(port.on_tick(&mut journal), DeadlineStatus::Missed);
        assert_eq!(
            port.emit(&sched, &mut journal, owner, 0x77),
            Err(ActuationError::DeadlineMissed)
        );
        assert_eq!(port.line().writes, 0);
    }

    // Clause 3: the miss is stamped exactly once per activation, however many
    // further ticks arrive — an audit trail of what changed, not a tick log.
    #[test]
    fn a_missed_deadline_is_stamped_once_per_activation() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<32> = SpoorJournal::new();
        let (_owner, mut port) = armed_port(&mut sched, 1);

        for _ in 0..20 {
            port.on_tick(&mut journal);
        }
        let misses = journal
            .iter()
            .filter(|spoor| {
                spoor.category() == Category::Actuation && spoor.action() == Action::Deadline
            })
            .count();
        assert_eq!(misses, 1, "twenty ticks past the deadline is still one missed deadline");

        // A fresh activation is a fresh miss.
        port.arm();
        assert_eq!(port.status(), DeadlineStatus::WithinDeadline(0));
        for _ in 0..5 {
            port.on_tick(&mut journal);
        }
        let misses = journal
            .iter()
            .filter(|spoor| {
                spoor.category() == Category::Actuation && spoor.action() == Action::Deadline
            })
            .count();
        assert_eq!(misses, 2);
    }

    // Clause 3, the claim that makes a deadline monitor worth building: the
    // deadline advances while the armed task is descheduled, and its WCET
    // budget does not. A task starved to death meets its budget perfectly.
    #[test]
    fn the_deadline_advances_while_the_budget_does_not() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<32> = SpoorJournal::new();
        let (owner, mut port) = armed_port(&mut sched, 4);
        let hog = task(&mut sched, 30, 1_000_000);
        sched.set_state(hog, TaskState::Running).expect("the task is live");

        // Ten ticks, every one of them charged to the hog. The owner is armed
        // and descheduled throughout.
        for _ in 0..10 {
            wcet::account_tick(&mut sched, &mut journal, Some(hog));
            port.on_tick(&mut journal);
        }

        assert_eq!(
            sched.wcet_state(owner).map(|(consumed, _)| consumed),
            Some(0),
            "not one tick was attributed to the armed task"
        );
        assert_eq!(
            port.status(),
            DeadlineStatus::Missed,
            "wall time does not stop because a task lost the CPU"
        );
        assert_eq!(sched.wcet_state(hog).map(|(consumed, _)| consumed), Some(10));
    }

    // An abandoned activation produces no phantom miss: a cancelled decision is
    // cancelled, and the ticks that follow belong to nobody.
    #[test]
    fn an_abandoned_activation_cannot_expire_later() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<16> = SpoorJournal::new();
        let (owner, mut port) = armed_port(&mut sched, 2);

        assert!(port.disarm(), "there was an activation to abandon");
        assert_eq!(port.status(), DeadlineStatus::Idle);
        for _ in 0..50 {
            assert_eq!(port.on_tick(&mut journal), DeadlineStatus::Idle);
        }
        assert!(journal.is_empty(), "no decision was taken, so nothing is stamped");
        assert!(!port.disarm(), "and there is nothing left to abandon");

        // And the port is still usable: abandoning is not a terminal state.
        port.arm();
        assert_eq!(port.emit(&sched, &mut journal, owner, 0x0F), Ok(()));
        assert_eq!(port.line().writes, 1);
    }

    // Clause 4: a late command is prevented, not logged. Asserted against the
    // line's own write count — the only place "the actuator moved" exists.
    #[test]
    fn a_late_command_never_reaches_the_line() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<16> = SpoorJournal::new();
        let (owner, mut port) = armed_port(&mut sched, 2);

        port.on_tick(&mut journal);
        port.on_tick(&mut journal);
        assert_eq!(port.emit(&sched, &mut journal, owner, 0x01), Ok(()), "still inside");
        assert_eq!(port.line().writes, 1);

        port.arm();
        for _ in 0..3 {
            port.on_tick(&mut journal);
        }
        assert_eq!(
            port.emit(&sched, &mut journal, owner, 0x02),
            Err(ActuationError::DeadlineMissed)
        );
        assert_eq!(port.line().writes, 1, "the late command is prevented, not merely recorded");
        assert_eq!(port.line().last, Some(0x01), "and it did not overwrite the last good one");
    }

    // Clause 6: every outcome is distinguishable in the audit trail —
    // emitted, refused-unauthorized, refused-late.
    #[test]
    fn the_audit_trail_distinguishes_emitted_from_each_kind_of_refusal() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let mut journal: SpoorJournal<32> = SpoorJournal::new();
        let (owner, mut port) = armed_port(&mut sched, 2);
        let intruder = task(&mut sched, 5, 1_000);
        sched.set_state(intruder, TaskState::Running).expect("the task is live");

        port.emit(&sched, &mut journal, owner, 0x33).expect("inside the window");
        port.arm();
        let _ = port.emit(&sched, &mut journal, intruder, 0x44);
        for _ in 0..3 {
            port.on_tick(&mut journal);
        }
        let _ = port.emit(&sched, &mut journal, owner, 0x55);

        let spoors: Vec<Spoor> = journal
            .iter()
            .filter(|spoor| spoor.category() == Category::Actuation)
            .collect();
        assert_eq!(spoors.len(), 4, "emit, unauthorized refusal, deadline miss, late refusal");

        assert_eq!(spoors[0].who(), Actor::Kernel);
        assert_eq!(spoors[0].action(), Action::Actuate);
        assert_eq!(spoors[0].outcome(), Outcome::Ok);
        assert_eq!(spoors[0].target(), owner.index() as u16);
        assert_eq!(spoors[0].cost(), 0x33, "the command word that reached the line");

        assert_eq!(spoors[1].action(), Action::Actuate);
        assert_eq!(spoors[1].outcome(), Outcome::Failed, "refused: not the declared task");
        assert_eq!(spoors[1].target(), intruder.index() as u16, "the caller, not the owner");

        assert_eq!(spoors[2].action(), Action::Deadline);
        assert_eq!(spoors[2].outcome(), Outcome::Failed);
        assert_eq!(spoors[2].target(), owner.index() as u16);

        assert_eq!(spoors[3].action(), Action::Actuate);
        assert_eq!(spoors[3].outcome(), Outcome::Capped, "refused: the window had closed");
        assert_eq!(spoors[3].target(), owner.index() as u16);
    }

    // Clause 6, stated structurally: there is no arm of the emit path that
    // neither writes the line nor stamps a refusal. Driven over the whole
    // error enumeration, so an arm added later fails to compile here rather
    // than passing silently — the same guard `wcet`'s own "no ignore branch"
    // test holds.
    #[test]
    fn no_emit_path_is_silent() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let owner = task(&mut sched, 25, 1_000);
        let intruder = task(&mut sched, 5, 1_000);
        sched.set_state(owner, TaskState::Running).expect("the task is live");
        sched.set_state(intruder, TaskState::Running).expect("the task is live");

        for error in
            [ActuationError::NotAuthorized, ActuationError::NotArmed, ActuationError::DeadlineMissed]
        {
            // Named exhaustively rather than with a wildcard: this is one of
            // the places a fourth arm must be handled.
            match error {
                ActuationError::NotAuthorized
                | ActuationError::NotArmed
                | ActuationError::DeadlineMissed => {}
            }

            let mut journal: SpoorJournal<16> = SpoorJournal::new();
            let mut port =
                ActuationPort::declare(RecordingLine::default(), owner, DeadlineTicks(2));
            let caller = match error {
                ActuationError::NotAuthorized => intruder,
                _ => owner,
            };
            match error {
                ActuationError::NotAuthorized => port.arm(),
                ActuationError::NotArmed => {}
                ActuationError::DeadlineMissed => {
                    port.arm();
                    for _ in 0..3 {
                        port.on_tick(&mut journal);
                    }
                }
            }
            let before = journal.len();
            assert_eq!(port.emit(&sched, &mut journal, caller, 0x99), Err(error));
            assert_eq!(port.line().writes, 0, "{error:?} must not reach the line");
            assert!(journal.len() > before, "{error:?} stamped nothing");
        }
    }
}
