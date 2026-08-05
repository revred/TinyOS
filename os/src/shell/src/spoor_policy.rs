//! The spoor-journaling policy decorator (`LE-56`, hand-2026-07-30/04A §0).
//!
//! A wrapper [`VerbPolicy`] that forwards every verdict to the real
//! [`GrantSet`] and journals each *denial* as a kernel
//! [`Spoor`](kernel::spoor::Spoor) — capturing the audit fact at the same
//! decision point `verbs::execute` consults, with zero changes to the shell
//! library's shipped code. This file is deliberately **not** a module of the
//! `shell` library crate: the library stays `no_std`, kernel-free and
//! flavour-agnostic (`EPIC-P2` §3.2's one-core rule). It is included by
//! `#[path]` from the two places that may know about the kernel:
//!
//! - `fixture_batch_main.rs` — the QEMU fixture binary installs the decorator
//!   over the parity policy, so the target run journals real spoors;
//! - the shell library's `#[cfg(test)]`-only include — host-side unit tests
//!   over the decorator as plain code (it never enters the shipped library).
//!
//! Shell-crate types are imported through `super`, so each includer re-exports
//! them; `kernel` spells the same in both crates.
//!
//! The journal here is *atomic* rather than [`kernel::spoor_journal::
//! SpoorJournal`] because the decorator sits behind `World`'s
//! `&(dyn VerbPolicy + Sync)` seam: appends happen through `&self`. Record
//! format is identical — each slot holds `Spoor::to_bits()`'s `u64`, the same
//! 8-byte record `SPOORJ01` journals store — so nothing here invents a second
//! wire format (`STORY-P0-06-01`'s format is untouched, per the plan's
//! bounds).
//!
//! Scope, stated: this decorator audits **policy denials** (`allows == false`).
//! The supervisor-scope refusal inside `task-kill` is a separate seam and a
//! named later concern — the parity `.TCB` exercises no such refusal, so the
//! journal-corroborates-denials invariant holds exactly for the parity run.

use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use kernel::spoor::{Action, Actor, Category, Outcome, Spoor};

use super::{GrantSet, SpoorRow, SpoorView, VerbKind, VerbPolicy};

/// A fixed-capacity, append-only journal of denial spoors, appendable through
/// `&self` (the policy seam is `&dyn VerbPolicy`). Slots hold raw
/// [`Spoor::to_bits`] records. `len` counts every append ever made; appends
/// beyond `N` keep counting but drop the record — loudly detectable, since the
/// corroboration check compares `len` against the denial counter and a size
/// chosen below capacity can never mask a count. The fixture sizes `N` at the
/// batch line budget, the most denials one run can produce.
pub struct DenialJournal<const N: usize> {
    slots: [AtomicU64; N],
    appended: AtomicUsize,
}

impl<const N: usize> DenialJournal<N> {
    /// An empty journal, usable in a `static` initializer.
    pub const fn new() -> Self {
        DenialJournal { slots: [const { AtomicU64::new(0) }; N], appended: AtomicUsize::new(0) }
    }

    /// Append one spoor. Never blocks, never panics; past capacity the count
    /// still advances (see the type's doc comment).
    fn append(&self, spoor: Spoor) {
        let index = self.appended.fetch_add(1, Ordering::SeqCst);
        if index < N {
            self.slots[index].store(spoor.to_bits(), Ordering::SeqCst);
        }
    }

    /// Number of spoors ever appended (including any dropped past capacity).
    pub fn len(&self) -> usize {
        self.appended.load(Ordering::SeqCst)
    }

    /// Whether nothing has been journaled.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The `index`-th appended spoor (oldest first), if it was retained and
    /// decodes — every retained record was written via `to_bits`, so decode
    /// failure is unreachable in practice but never assumed away.
    pub fn entry(&self, index: usize) -> Option<Spoor> {
        if index >= self.len() || index >= N {
            return None;
        }
        Spoor::decode(self.slots[index].load(Ordering::SeqCst)).ok()
    }
}

impl<const N: usize> Default for DenialJournal<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// The kernel taxonomy's render names — the spellings the `SPOOR` verb shows
/// and the register (`goals/context/terminal-gap.tsv`) decides. Exhaustive
/// matches: a new kernel variant fails compilation here rather than rendering
/// a wrong name.
fn category_name(category: Category) -> &'static str {
    match category {
        Category::Scheduling => "scheduling",
        Category::Lock => "lock",
        Category::Wcet => "wcet",
        Category::Dispatch => "dispatch",
        Category::Exec => "exec",
        Category::Memory => "memory",
        Category::Boot => "boot",
        Category::Fault => "fault",
        Category::Actuation => "actuation",
        Category::Shell => "shell",
        Category::Thermal => "thermal",
    }
}

fn actor_name(actor: Actor) -> &'static str {
    match actor {
        Actor::Kernel => "kernel",
        Actor::Exec => "exec",
        Actor::Session => "session",
    }
}

fn action_name(action: Action) -> &'static str {
    match action {
        Action::Create => "create",
        Action::Boost => "boost",
        Action::Restore => "restore",
        Action::Block => "block",
        Action::Select => "select",
        Action::Overrun => "overrun",
        Action::ResetBudget => "reset-budget",
        Action::Fault => "fault",
        Action::Terminate => "terminate",
        Action::Restart => "restart",
        Action::Degrade => "degrade",
        Action::Actuate => "actuate",
        Action::Deadline => "deadline",
        Action::VerbDenied => "verb-denied",
        Action::Observe => "observe",
    }
}

fn outcome_name(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Ok => "ok",
        Outcome::Empty => "empty",
        Outcome::Chose => "chose",
        Outcome::Capped => "capped",
        Outcome::Failed => "failed",
        Outcome::Skipped => "skipped",
        Outcome::Superseded => "superseded",
        Outcome::Partial => "partial",
    }
}

/// The journal *is* a renderable view: the `SPOOR` verb shows exactly what
/// the decorator journaled, mapped through the kernel taxonomy's render
/// names. Both halves of the parity lane install this same impl, which is
/// what makes the golden byte-comparison hold across host and target.
impl<const N: usize> SpoorView for DenialJournal<N> {
    fn source(&self) -> &'static str {
        "policy-seam denial journal"
    }
    fn len(&self) -> usize {
        DenialJournal::len(self)
    }
    fn is_empty(&self) -> bool {
        DenialJournal::is_empty(self)
    }
    fn entry(&self, index: usize) -> Option<SpoorRow> {
        let spoor = DenialJournal::entry(self, index)?;
        Some(SpoorRow {
            category: category_name(spoor.category()),
            actor: actor_name(spoor.who()),
            action: action_name(spoor.action()),
            outcome: outcome_name(spoor.outcome()),
            target: spoor.target(),
            cost: spoor.cost(),
        })
    }
}

/// The decorator: forwards to the wrapped [`GrantSet`] and journals each
/// denial as `(Category::Shell, Actor::Session, Action::VerbDenied,
/// Outcome::Failed, target = the denied verb's kind discriminant)`. It must
/// never alter authorisation — the verdict returned is exactly the wrapped
/// policy's, proven by test.
pub struct SpoorPolicy<'a, const N: usize> {
    inner: &'a GrantSet,
    journal: &'a DenialJournal<N>,
}

impl<'a, const N: usize> SpoorPolicy<'a, N> {
    /// Wrap `inner`, journaling denials into `journal`.
    pub const fn new(inner: &'a GrantSet, journal: &'a DenialJournal<N>) -> Self {
        SpoorPolicy { inner, journal }
    }
}

impl<const N: usize> VerbPolicy for SpoorPolicy<'_, N> {
    fn allows(&self, session: &str, verb: VerbKind) -> bool {
        let verdict = self.inner.allows(session, verb);
        if !verdict {
            self.journal.append(Spoor::stamp(
                Category::Shell,
                Actor::Session,
                Action::VerbDenied,
                Outcome::Failed,
                verb as u16,
                0,
            ));
        }
        verdict
    }

    fn supervisor(&self, session: &str) -> bool {
        self.inner.supervisor(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel::spoor::{Action, Actor, Category, Outcome};

    const GRANTED: &[VerbKind] = &[VerbKind::List, VerbKind::Echo, VerbKind::PrintCwd];
    static INNER: GrantSet =
        GrantSet { granted: GRANTED, withheld: Some(VerbKind::Echo), supervisor: false };

    /// Every verb kind, so neutrality is proven over the whole vocabulary
    /// rather than a convenient sample.
    const ALL_VERBS: &[VerbKind] = &[
        VerbKind::List,
        VerbKind::ChangeDir,
        VerbKind::PrintCwd,
        VerbKind::Copy,
        VerbKind::Move,
        VerbKind::Delete,
        VerbKind::MakeDir,
        VerbKind::RemoveDir,
        VerbKind::ViewFile,
        VerbKind::FindText,
        VerbKind::SortStream,
        VerbKind::Page,
        VerbKind::TreeView,
        VerbKind::AttribView,
        VerbKind::Env,
        VerbKind::Echo,
        VerbKind::ClearScreen,
        VerbKind::VersionInfo,
        VerbKind::VolumeInfo,
        VerbKind::MemInfo,
        VerbKind::TaskList,
        VerbKind::TaskKill,
    ];

    /// SP1 — one denied verb appends exactly one spoor, with the decided
    /// shape: `(Category::Shell, Actor::Session, Action::VerbDenied,
    /// Outcome::Failed, target = the denied verb's discriminant)`.
    #[test]
    fn sp1_a_denied_verb_journals_exactly_one_spoor_with_the_decided_shape() {
        let journal: DenialJournal<8> = DenialJournal::new();
        let policy = SpoorPolicy::new(&INNER, &journal);

        assert!(!policy.allows("PARITY", VerbKind::ClearScreen), "ungranted verb must deny");
        assert_eq!(journal.len(), 1, "exactly one denial, exactly one append");

        let spoor = journal.entry(0).expect("the appended spoor decodes");
        assert_eq!(spoor.category(), Category::Shell);
        assert_eq!(spoor.who(), Actor::Session);
        assert_eq!(spoor.action(), Action::VerbDenied);
        assert_eq!(spoor.outcome(), Outcome::Failed);
        assert_eq!(spoor.target(), VerbKind::ClearScreen as u16, "TARGET names the denied verb");
    }

    /// SP2 — a granted verb passes through and appends nothing.
    #[test]
    fn sp2_a_granted_verb_journals_nothing() {
        let journal: DenialJournal<8> = DenialJournal::new();
        let policy = SpoorPolicy::new(&INNER, &journal);

        assert!(policy.allows("PARITY", VerbKind::List));
        assert!(policy.allows("PARITY", VerbKind::PrintCwd));
        assert!(journal.is_empty(), "granted verbs must never touch the journal");
    }

    /// SP3 — authorisation neutrality: over the whole verb vocabulary the
    /// decorator's verdict is identical to the wrapped policy's, and so is the
    /// supervisor answer. Auditing must never change a verdict.
    #[test]
    fn sp3_the_decorator_never_alters_the_wrapped_verdict() {
        let journal: DenialJournal<32> = DenialJournal::new();
        let policy = SpoorPolicy::new(&INNER, &journal);

        for &verb in ALL_VERBS {
            assert_eq!(
                policy.allows("PARITY", verb),
                INNER.allows("PARITY", verb),
                "verdict altered for {verb:?}"
            );
        }
        assert_eq!(policy.supervisor("PARITY"), INNER.supervisor("PARITY"));
        let denied = ALL_VERBS.iter().filter(|&&v| !INNER.allows("PARITY", v)).count();
        assert_eq!(journal.len(), denied, "one append per denial, none otherwise");
    }

    /// SP4 — the withheld verb (granted in the list, withheld by the slot) is
    /// a denial like any other: it journals. This is the exact denial the
    /// parity `.TCB` exercises (`CLS`), so the shape is pinned here too.
    #[test]
    fn sp4_the_withheld_verb_is_a_journaled_denial() {
        let journal: DenialJournal<8> = DenialJournal::new();
        let policy = SpoorPolicy::new(&INNER, &journal);

        assert!(!policy.allows("PARITY", VerbKind::Echo), "withheld overrides granted");
        assert_eq!(journal.len(), 1);
        let spoor = journal.entry(0).expect("decodes");
        assert_eq!(spoor.target(), VerbKind::Echo as u16);
    }

    /// SP5 — end to end through the real seam: a batch run over a decorated
    /// policy journals exactly the denials `verbs::execute` counts — the
    /// invariant the fixture asserts in-guest (step 3 of the plan).
    #[test]
    fn sp5_a_batch_run_over_the_decorator_journals_exactly_the_counted_denials() {
        use crate::batch;
        use crate::verbs::{Env, World};
        use crate::volume::RamVolume;

        static BATCH_INNER: GrantSet = GrantSet {
            granted: &[VerbKind::Echo, VerbKind::PrintCwd],
            withheld: Some(VerbKind::ClearScreen),
            supervisor: false,
        };
        static JOURNAL: DenialJournal<8> = DenialJournal::new();
        static POLICY: SpoorPolicy<'static, 8> = SpoorPolicy::new(&BATCH_INNER, &JOURNAL);
        let mut world = World {
            volume: RamVolume::new(Some("TINYOS"), (0x1234, 0xABCD)),
            env: Env::new(),
            cwd: 0,
            echo: true,
            policy: &POLICY,
            session: "PARITY",
            tasks: &[],
            spoors: &crate::verbs::NoSpoors,
            denials: 0,
        };
        let mut out = String::new();
        let stats =
            batch::run(&mut world, "@ECHO OFF\nCLS\nVER\nCD\n", &mut out).expect("batch runs");
        // CLS is withheld, VER is ungranted — two denials; ECHO OFF and CD run.
        assert_eq!(stats.denials, 2, "CLS and VER deny, CD runs: {out}");
        assert_eq!(JOURNAL.len() as u32, stats.denials, "journal corroborates the counter");
    }
}
