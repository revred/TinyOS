//! Spoor: universal 64-bit audit atom (`STORY-P0-06-01`).
//!
//! A [`Spoor`] is a fixed-size, hierarchically bit-packed audit/action
//! record — not a log line, not a string, not an event stream. The shape
//! and discipline are borrowed deliberately from the sibling
//! `Sharc.Blue` project's own `spoor` primitive
//! (`Sharc.Bluekind\Blue.Reef\src\shape\spoor.rs`, `docs/ThePlan/Spoor.md`):
//! "What iblock did for code, spoor does for action... stamp immediately
//! at execution point" — never buffered, never batched, never carrying a
//! heap-allocated field, cheap enough to emit from any real-time path.
//!
//! **What's borrowed vs. what's TinyOS's own.** The 64-bit layout itself —
//! field widths and bit positions (`CAT|WHO|ACT|OUT|TARGET|COST`) — and
//! the [`Outcome`] vocabulary are adopted verbatim, since both are
//! genuinely generic (any project's audit record needs "where did this
//! field start," and `Sharc.Blue`'s outcome vocabulary — ok/empty/chose/
//! capped/failed/skipped/superseded/partial — is domain-neutral). The
//! [`Category`] and [`Action`] vocabularies are TinyOS's own: `CAT`/`ACT`
//! are per-project taxonomies within a shared wire format, the same way
//! two projects using an identical network header format still define
//! their own port-number assignments — reusing `Sharc.Blue`'s literal
//! render/mutate/classify category set would make no sense for a kernel.
//!
//! ```text
//!  63    59    55    51    47          31              0
//!   [ CAT ][ WHO ][ ACT ][ OUT ][  TARGET  ][   COST     ]
//!   4 bits 4 bits 4 bits 4 bits  16 bits     32 bits
//! ```

/// Errors [`Spoor::decode`] fails closed with, per
/// `agent/CODING_STANDARDS.md`'s "no stringly-typed errors" rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpoorError {
    /// The `CAT` nibble doesn't match a known [`Category`] variant.
    UnknownCategory,
    /// The `WHO` nibble doesn't match a known [`Actor`] variant.
    UnknownActor,
    /// The `ACT` nibble doesn't match a known [`Action`] variant.
    UnknownAction,
    /// The `OUT` nibble doesn't match a known [`Outcome`] variant.
    UnknownOutcome,
}

/// The `CAT` field: which TinyOS subsystem emitted this spoor. TinyOS's
/// own vocabulary (see this module's doc comment for why `Sharc.Blue`'s
/// own category set isn't reused) — additive: a new subsystem that starts
/// emitting spoors gets a new variant, never a repurposed existing one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    /// `kernel::sched` — task creation, priority/state changes.
    Scheduling,
    /// `kernel::lock` — priority-inheriting lock contention/release.
    Lock,
    /// `kernel::wcet` — WCET budget tick attribution/overrun detection.
    Wcet,
    /// `kernel::dispatch` — dispatch-round task selection.
    Dispatch,
    /// `exec` — PE loading, address-space mapping, Win32 shim calls.
    Exec,
    /// `kernel::mem` — pool allocation/exhaustion.
    Memory,
    /// Boot-time initialization (ACPI topology discovery, etc.).
    Boot,
    /// `kernel::fault` — captured CPU exceptions and their disposition
    /// (`STORY-P1-02-01`). A new variant rather than a reuse of `Boot` or
    /// `Scheduling`: a fault is neither, and an auditor filtering for faults
    /// must never have to guess which other category one was folded into.
    Fault,
    /// `kernel::actuation` — a command reaching (or being refused at) the
    /// output boundary, and a missed actuation deadline (`STORY-P1-06-01`). A
    /// new variant on the same grounds `Fault` took one: an actuation decision
    /// is neither a scheduling decision nor a budget one, and the auditor who
    /// most needs to filter for "did anything reach the actuator" is the one
    /// who can least afford to guess which category it was folded into.
    Actuation,
}

impl Category {
    const fn to_bits(self) -> u8 {
        match self {
            Category::Scheduling => 0,
            Category::Lock => 1,
            Category::Wcet => 2,
            Category::Dispatch => 3,
            Category::Exec => 4,
            Category::Memory => 5,
            Category::Boot => 6,
            Category::Fault => 7,
            Category::Actuation => 8,
        }
    }

    const fn from_bits(bits: u8) -> Result<Self, SpoorError> {
        match bits {
            0 => Ok(Category::Scheduling),
            1 => Ok(Category::Lock),
            2 => Ok(Category::Wcet),
            3 => Ok(Category::Dispatch),
            4 => Ok(Category::Exec),
            5 => Ok(Category::Memory),
            6 => Ok(Category::Boot),
            7 => Ok(Category::Fault),
            8 => Ok(Category::Actuation),
            _ => Err(SpoorError::UnknownCategory),
        }
    }
}

/// The `WHO` field: which actor performed the action. Distinct from
/// [`Category`] (which subsystem) — an actor can act across categories
/// (e.g. a future ACI-mediated caller acting on `Exec`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Actor {
    /// The kernel itself (scheduler, lock, WCET, dispatch subsystems).
    Kernel,
    /// A loaded executable's own request, mediated by `exec::win32_shim`.
    Exec,
}

impl Actor {
    const fn to_bits(self) -> u8 {
        match self {
            Actor::Kernel => 0,
            Actor::Exec => 1,
        }
    }

    const fn from_bits(bits: u8) -> Result<Self, SpoorError> {
        match bits {
            0 => Ok(Actor::Kernel),
            1 => Ok(Actor::Exec),
            _ => Err(SpoorError::UnknownActor),
        }
    }
}

/// The `ACT` field: the verb — what kind of action this spoor records.
/// TinyOS's own vocabulary, sized to what Phase 0's already-implemented
/// subsystems (`kernel::lock`, `kernel::wcet`, `kernel::dispatch`) actually
/// do today, not a speculative superset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// A task was created (`Scheduler::create_task`).
    Create,
    /// A lock holder's priority was boosted (`PriorityInheritingLock`).
    Boost,
    /// A lock holder's priority was restored on release.
    Restore,
    /// A task transitioned to `Blocked`.
    Block,
    /// A dispatch round selected a task to run (`dispatch::run_once`).
    Select,
    /// A WCET budget overrun was detected (`wcet::record_tick`).
    Overrun,
    /// A WCET budget window was reset (`wcet::reset_budget_window`).
    ResetBudget,
    /// A CPU exception was captured (`kernel::fault`).
    Fault,
    /// A faulting task was terminated, or the system halted because the
    /// fault could not be contained to one (`kernel::fault`). Also the verb
    /// for a WCET overrun whose declared policy was `TripToSafeState`, where
    /// the outcome really is termination (`STORY-P1-04-02`).
    Terminate,
    /// A task that overran its WCET budget was rewound to its entry point
    /// under the `Restart` policy it declared (`kernel::wcet`).
    Restart,
    /// A task that overran its WCET budget had its priority lowered to its
    /// declared floor under the `Degrade` policy (`kernel::wcet`).
    Degrade,
    /// An actuation command was presented at the output boundary
    /// (`kernel::actuation`). The [`Outcome`] carries what happened to it, and
    /// the three cases are deliberately one verb rather than three:
    /// [`Outcome::Ok`] — it reached the line; [`Outcome::Failed`] — refused,
    /// the caller was not the declared actuation task; [`Outcome::Capped`] —
    /// refused, the declared deadline had passed, which is a bound being
    /// enforced rather than an error. `TARGET` is the *caller*, not the owner,
    /// so a refused attempt names who attempted it.
    Actuate,
    /// A declared actuation deadline expired before the command was emitted
    /// (`kernel::actuation`). Stamped by the monitor on the tick that crosses
    /// the deadline, exactly once per activation — distinct from
    /// [`Action::Actuate`] with [`Outcome::Capped`], which is the later refusal
    /// of a specific command. One says the window closed; the other says
    /// something tried to drive the line after it had.
    Deadline,
}

impl Action {
    const fn to_bits(self) -> u8 {
        match self {
            Action::Create => 0,
            Action::Boost => 1,
            Action::Restore => 2,
            Action::Block => 3,
            Action::Select => 4,
            Action::Overrun => 5,
            Action::ResetBudget => 6,
            Action::Fault => 7,
            Action::Terminate => 8,
            Action::Restart => 9,
            Action::Degrade => 10,
            Action::Actuate => 11,
            Action::Deadline => 12,
        }
    }

    const fn from_bits(bits: u8) -> Result<Self, SpoorError> {
        match bits {
            0 => Ok(Action::Create),
            1 => Ok(Action::Boost),
            2 => Ok(Action::Restore),
            3 => Ok(Action::Block),
            4 => Ok(Action::Select),
            5 => Ok(Action::Overrun),
            6 => Ok(Action::ResetBudget),
            7 => Ok(Action::Fault),
            8 => Ok(Action::Terminate),
            9 => Ok(Action::Restart),
            10 => Ok(Action::Degrade),
            11 => Ok(Action::Actuate),
            12 => Ok(Action::Deadline),
            _ => Err(SpoorError::UnknownAction),
        }
    }
}

/// The `OUT` field: the outcome. Adopted verbatim from `Sharc.Blue`'s own
/// outcome vocabulary (domain-neutral — every project's audit atoms need
/// these same eight shapes of "what happened").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Succeeded normally.
    Ok,
    /// Succeeded, but there was nothing to do.
    Empty,
    /// Succeeded by choosing among alternatives (e.g. a ready-queue pick).
    Chose,
    /// Succeeded, but a bound/ceiling was hit (e.g. a budget capped).
    Capped,
    /// Did not succeed.
    Failed,
    /// Deliberately not attempted.
    Skipped,
    /// Superseded by a later action before completing.
    Superseded,
    /// Partially completed.
    Partial,
}

impl Outcome {
    const fn to_bits(self) -> u8 {
        match self {
            Outcome::Ok => 0,
            Outcome::Empty => 1,
            Outcome::Chose => 2,
            Outcome::Capped => 3,
            Outcome::Failed => 4,
            Outcome::Skipped => 5,
            Outcome::Superseded => 6,
            Outcome::Partial => 7,
        }
    }

    const fn from_bits(bits: u8) -> Result<Self, SpoorError> {
        match bits {
            0 => Ok(Outcome::Ok),
            1 => Ok(Outcome::Empty),
            2 => Ok(Outcome::Chose),
            3 => Ok(Outcome::Capped),
            4 => Ok(Outcome::Failed),
            5 => Ok(Outcome::Skipped),
            6 => Ok(Outcome::Superseded),
            7 => Ok(Outcome::Partial),
            _ => Err(SpoorError::UnknownOutcome),
        }
    }
}

const WHO_SHIFT: u32 = 56;
const CAT_SHIFT: u32 = 60;
const ACT_SHIFT: u32 = 52;
const OUT_SHIFT: u32 = 48;
const TARGET_SHIFT: u32 = 32;
const NIBBLE_MASK: u64 = 0xF;
const TARGET_MASK: u64 = 0xFFFF;
const COST_MASK: u64 = 0xFFFF_FFFF;

/// A 64-bit universal audit atom: `CAT|WHO|ACT|OUT|TARGET|COST`, packed
/// into a single `u64` — 8 bytes, `Copy`, no heap, no allocation, cheap
/// enough to stamp from any real-time path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spoor(u64);

impl Spoor {
    /// Stamps a spoor from its typed fields — the only public
    /// constructor: a `Spoor` can never exist with an invalid or
    /// unaccounted-for field, since every field here is already a
    /// validated enum (or, for `target`/`cost`, a value that fits its bit
    /// width exactly, so there is nothing to reject).
    ///
    /// Used both for a single fire-and-forget event and, called twice, for
    /// a bracketed action (an "entry" stamp when the action starts — cost
    /// 0, outcome a placeholder like [`Outcome::Empty`] — and an "exit"
    /// stamp with the real outcome/cost when it completes), mirroring
    /// `Sharc.Blue`'s own "ENTRY + EXIT spoor per atom call" pattern. This
    /// module doesn't enforce that pairing structurally (there is no
    /// separate `entry`/`exit` type) — it's a usage convention the caller
    /// follows, the same way `Sharc.Blue`'s own atoms do.
    pub const fn stamp(
        category: Category,
        who: Actor,
        action: Action,
        outcome: Outcome,
        target: u16,
        cost: u32,
    ) -> Self {
        let bits = ((category.to_bits() as u64) << CAT_SHIFT)
            | ((who.to_bits() as u64) << WHO_SHIFT)
            | ((action.to_bits() as u64) << ACT_SHIFT)
            | ((outcome.to_bits() as u64) << OUT_SHIFT)
            | ((target as u64) << TARGET_SHIFT)
            | (cost as u64);
        Spoor(bits)
    }

    /// The raw packed bits — used to write this spoor into a journal
    /// (`STORY-P0-06-02`) or transmit it, without needing to decode it
    /// first.
    pub const fn to_bits(self) -> u64 {
        self.0
    }

    /// Decodes a raw `u64` (e.g. read back from a journal) into a
    /// [`Spoor`], failing closed with the first unrecognized field found
    /// (`CAT`, then `WHO`, then `ACT`, then `OUT`, in bit order) rather
    /// than silently accepting an unknown discriminant.
    pub const fn decode(bits: u64) -> Result<Self, SpoorError> {
        let cat_bits = ((bits >> CAT_SHIFT) & NIBBLE_MASK) as u8;
        let who_bits = ((bits >> WHO_SHIFT) & NIBBLE_MASK) as u8;
        let act_bits = ((bits >> ACT_SHIFT) & NIBBLE_MASK) as u8;
        let out_bits = ((bits >> OUT_SHIFT) & NIBBLE_MASK) as u8;

        if let Err(e) = Category::from_bits(cat_bits) {
            return Err(e);
        }
        if let Err(e) = Actor::from_bits(who_bits) {
            return Err(e);
        }
        if let Err(e) = Action::from_bits(act_bits) {
            return Err(e);
        }
        if let Err(e) = Outcome::from_bits(out_bits) {
            return Err(e);
        }
        Ok(Spoor(bits))
    }

    /// This spoor's category.
    pub const fn category(self) -> Category {
        match Category::from_bits(((self.0 >> CAT_SHIFT) & NIBBLE_MASK) as u8) {
            Ok(c) => c,
            // A `Spoor` only ever exists via `stamp` (always-valid fields)
            // or `decode` (validated fields) — this arm is unreachable in
            // practice, kept exhaustive per `sched.rs::TaskCreateError`'s
            // own precedent rather than assumed away.
            Err(_) => Category::Scheduling,
        }
    }

    /// This spoor's actor.
    pub const fn who(self) -> Actor {
        match Actor::from_bits(((self.0 >> WHO_SHIFT) & NIBBLE_MASK) as u8) {
            Ok(a) => a,
            Err(_) => Actor::Kernel,
        }
    }

    /// This spoor's action.
    pub const fn action(self) -> Action {
        match Action::from_bits(((self.0 >> ACT_SHIFT) & NIBBLE_MASK) as u8) {
            Ok(a) => a,
            Err(_) => Action::Create,
        }
    }

    /// This spoor's outcome.
    pub const fn outcome(self) -> Outcome {
        match Outcome::from_bits(((self.0 >> OUT_SHIFT) & NIBBLE_MASK) as u8) {
            Ok(o) => o,
            Err(_) => Outcome::Ok,
        }
    }

    /// This spoor's target (a 16-bit hash/id of what was acted on).
    pub const fn target(self) -> u16 {
        ((self.0 >> TARGET_SHIFT) & TARGET_MASK) as u16
    }

    /// This spoor's cost (elapsed microseconds, or a payload value).
    pub const fn cost(self) -> u32 {
        (self.0 & COST_MASK) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_CATEGORIES: [Category; 9] = [
        Category::Scheduling,
        Category::Lock,
        Category::Wcet,
        Category::Dispatch,
        Category::Exec,
        Category::Memory,
        Category::Boot,
        Category::Fault,
        Category::Actuation,
    ];
    const ALL_ACTORS: [Actor; 2] = [Actor::Kernel, Actor::Exec];
    const ALL_ACTIONS: [Action; 13] = [
        Action::Create,
        Action::Boost,
        Action::Restore,
        Action::Block,
        Action::Select,
        Action::Overrun,
        Action::ResetBudget,
        Action::Fault,
        Action::Terminate,
        Action::Restart,
        Action::Degrade,
        Action::Actuate,
        Action::Deadline,
    ];
    const ALL_OUTCOMES: [Outcome; 8] = [
        Outcome::Ok,
        Outcome::Empty,
        Outcome::Chose,
        Outcome::Capped,
        Outcome::Failed,
        Outcome::Skipped,
        Outcome::Superseded,
        Outcome::Partial,
    ];
    const TARGET_SAMPLES: [u16; 4] = [0, 1, 12345, u16::MAX];
    const COST_SAMPLES: [u32; 4] = [0, 1, 123_456_789, u32::MAX];

    // STORY-P0-06-01 acceptance criterion 4: round-trip property over the
    // full range of every field (every enum variant × every target/cost
    // sample) — encoding then decoding returns the exact original tuple,
    // with no lossy bit overlap between fields.
    #[test]
    fn every_field_combination_round_trips_through_stamp_and_accessors() {
        for &category in &ALL_CATEGORIES {
            for &who in &ALL_ACTORS {
                for &action in &ALL_ACTIONS {
                    for &outcome in &ALL_OUTCOMES {
                        for &target in &TARGET_SAMPLES {
                            for &cost in &COST_SAMPLES {
                                let spoor =
                                    Spoor::stamp(category, who, action, outcome, target, cost);
                                assert_eq!(spoor.category(), category);
                                assert_eq!(spoor.who(), who);
                                assert_eq!(spoor.action(), action);
                                assert_eq!(spoor.outcome(), outcome);
                                assert_eq!(spoor.target(), target);
                                assert_eq!(spoor.cost(), cost);
                            }
                        }
                    }
                }
            }
        }
    }

    // A `Spoor` is exactly 8 bytes (STORY-P0-06-01 acceptance criterion 1).
    #[test]
    fn spoor_is_exactly_eight_bytes() {
        assert_eq!(core::mem::size_of::<Spoor>(), 8);
    }

    // `decode` round-trips through `to_bits` for a validly-stamped spoor.
    #[test]
    fn decode_round_trips_a_validly_stamped_spoors_bits() {
        let original =
            Spoor::stamp(Category::Lock, Actor::Kernel, Action::Boost, Outcome::Ok, 42, 100);
        let decoded = Spoor::decode(original.to_bits()).expect("validly stamped bits decode");
        assert_eq!(decoded, original);
    }

    // Each field's decode failure is distinguishable and fails closed
    // rather than silently accepting/wrapping an unknown discriminant
    // (STORY-P0-06-01 acceptance criterion 2).
    #[test]
    fn decode_rejects_an_unknown_category_nibble() {
        // Category 0..=8 are valid (7 = `Fault`, added by `STORY-P1-02-01`;
        // 8 = `Actuation`, added by `STORY-P1-06-01`); 9 is not yet assigned.
        let bits = 9u64 << CAT_SHIFT;
        assert_eq!(Spoor::decode(bits), Err(SpoorError::UnknownCategory));
    }

    #[test]
    fn decode_rejects_an_unknown_actor_nibble() {
        // Actor 0..=1 are valid; 2 is not yet assigned. Category 0 (valid)
        // so only the WHO field is under test.
        let bits = 2u64 << WHO_SHIFT;
        assert_eq!(Spoor::decode(bits), Err(SpoorError::UnknownActor));
    }

    #[test]
    fn decode_rejects_an_unknown_action_nibble() {
        // Action 0..=12 are valid (7 = `Fault`, 8 = `Terminate`, added by
        // `STORY-P1-02-01`; 9 = `Restart`, 10 = `Degrade`, added by
        // `STORY-P1-04-02`; 11 = `Actuate`, 12 = `Deadline`, added by
        // `STORY-P1-06-01`); 13 is not yet assigned. The nibble's remaining
        // range is 13..=15, so the vocabulary has three verbs of headroom
        // before the `ACT` field itself has to widen — worth watching, since
        // widening it is a wire-format change to every stored spoor.
        let bits = 13u64 << ACT_SHIFT;
        assert_eq!(Spoor::decode(bits), Err(SpoorError::UnknownAction));
    }

    #[test]
    fn decode_rejects_an_unknown_outcome_nibble() {
        // Outcome 0..=7 are valid; 8 (of the nibble's full 0..=15 range)
        // is not yet assigned.
        let bits = 8u64 << OUT_SHIFT;
        assert_eq!(Spoor::decode(bits), Err(SpoorError::UnknownOutcome));
    }
}
