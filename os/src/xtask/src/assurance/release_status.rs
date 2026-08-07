//! `LE-84` — the assurance release status, decomposed by machine.
//!
//! The dashboard publishes two numbers, `0/97` Stories assurance-verified and
//! `20/460` release gates with dated evidence, and until this module existed
//! neither was decomposed anywhere. Four consecutive handovers quoted the
//! second one and none of them said that **half of its denominator cannot be
//! closed by construction**: 230 of the 460 belong to domains whose subsystem
//! does not exist, and `goals/assurance/README.md` is explicit that not one of
//! those can be closed.
//!
//! So `20/460` invites *"4% done, 440 to go"*, which is wrong in both
//! directions at once. It overstates the work remaining, because 230 of it is
//! not work; and it understates the indictment, because against the
//! denominator that can actually be closed the figure is `20/220` — and none
//! of those 200 empty gates is waiting on the qualification decision, on the
//! board, or on anyone.
//!
//! # Why this is code and not a paragraph
//!
//! Handover `09A` computed all of this by hand and **its first printing of the
//! ledger did not reconcile**: it subtracted three overlapping buckets and
//! produced 164 where the answer is 220, because the 46 hardware-only gates
//! and half the `G04` are *inside* the 230 rather than beside it. A reader had
//! to repair it. That is the argument for deriving a number rather than
//! writing it down, made against the very document making it — the same
//! argument `LE-30` made for the dashboard and `STORY-P0-01-05` made for the
//! register itself.
//!
//! Hence the shape below: **nested, never subtracted.** Each bucket states its
//! own total and its children sum to it, so a bucket that overlaps another is
//! a compile-time impossibility rather than an arithmetic slip.

use std::collections::{BTreeMap, BTreeSet};

use super::*;
use crate::bound_provenance;
use crate::performance_catalogue::{self, ReleaseGate, UNIMPLEMENTED_READINESS};

/// Release guardrails whose **mechanism does not exist in the kernel**, as
/// distinct from ones that merely have not been measured.
///
/// This is the distinction the register cannot express and this module exists
/// partly to surface. `readiness` is tracked per *domain*, so a `prototype`
/// domain still contains guardrails describing machinery nobody has built —
/// load, queueing, isolation and campaign shapes. You cannot flood a budget
/// that does not exist, and `G19`/`G21` are precisely `FEAT-P1-05`'s unbuilt
/// containment mechanism: `Tcb` carries no containment class, the pool is one
/// flat capacity with no reservation floor, and a repository-wide search for a
/// scheduling or allocation reserve finds none.
///
/// **One precision, added 2026-08-07 after this list was re-verified guardrail
/// by guardrail against the current tree.** All seven are still correctly
/// listed. But "unbuilt containment mechanism" unqualified would overstate the
/// case, and an overstatement here is the kind that gets quoted: `kernel::fault`
/// *does* contain a fault to the task that raised it — three real faults, each
/// contained, with the scheduler still dispatching afterwards, gated in CI by
/// `qemu-x86_64 --fixture=fault`. What `G19`/`G21` require and this project
/// does not have is the *class* and the *reservation floor* — containment under
/// competing load, and exhaustion of a finite resource contained to the class
/// that exhausted it. Single-task fault containment is real; per-class resource
/// containment is not.
///
/// **This list is a declared judgement, not a derived fact**, and the printed
/// output says so. The catalogue has no per-guardrail readiness column; adding
/// one is the real fix and is what `LE-84` asks be considered. Until then the
/// judgement is at least in one place, under review, rather than re-made from
/// scratch by each session that asks what is reachable.
const MECHANISM_ABSENT_GUARDRAILS: [&str; 7] = [
    "G13", // queue residence p99
    "G14", // queue processing maximum
    "G15", // sustained throughput floor
    "G16", // burst and backpressure safety
    "G19", // isolation under competing load
    "G21", // exhaustion and fault containment
    "G22", // 72-hour soak
];

/// The full nested decomposition of the release-gate denominator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseStatus {
    /// Domains at least one Story contract selects, with their readiness.
    pub in_play_domains: Vec<(String, String)>,
    /// Release guardrails per domain — 23 of the 25, `G24`/`G25` being claim
    /// gates.
    pub release_guardrails_per_domain: usize,
    /// The published denominator: in-play domains × release guardrails.
    pub in_play: usize,
    /// The half that cannot be closed because there is nothing to close it
    /// against.
    pub without_subsystem: WithoutSubsystem,
    /// The half that can.
    pub implemented: Implemented,
}

/// Gates in domains whose `readiness` says the subsystem does not exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WithoutSubsystem {
    /// `(domain, readiness)`, in id order.
    pub domains: Vec<(String, String)>,
    /// The bucket total.
    pub gates: usize,
    /// Hardware-only gates **contained in** this bucket, not beside it.
    pub hardware_only: usize,
    /// `G04` bound-class gates **contained in** this bucket, not beside it.
    pub bound_class: usize,
}

/// Gates in domains that exist and can therefore be measured.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Implemented {
    /// `(domain, readiness)`, in id order.
    pub domains: Vec<(String, String)>,
    /// The bucket total.
    pub gates: usize,
    /// Gates here that only a board could move.
    ///
    /// **This is the `09A` §3 finding, derived rather than asserted**, and it
    /// is expected to be zero: every release guardrail in every implemented
    /// in-play domain is `Host` or `T0` tier. The board was never the
    /// constraint on this register.
    pub hardware_only: usize,
    /// `G04`, barred by `ADR 0005` while zero platforms are qualified.
    ///
    /// Correctly barred — a bound quoted from an unqualified platform is the
    /// exact failure the ADR exists to prevent.
    pub bound_class_barred: usize,
    /// Not barred by `G04`, by the board, or by an absent subsystem.
    pub open: usize,
    /// Of the open gates, those carrying evidence.
    pub evidenced: usize,
    /// Of the *empty* gates, those carrying a `refused` row: measured, read
    /// against the target, and declined. A subset of [`Self::empty`], never a
    /// sibling of it.
    pub refused: usize,
    /// Open gates carrying nothing.
    pub empty: usize,
    /// Empty because the mechanism has not been built.
    pub mechanism_absent: usize,
    /// Empty because nobody has measured. **Available today.**
    pub measurable_today: usize,
    /// The same implemented half, split by domain, so that
    /// [`Self::measurable_today`] is a WORKLIST rather than a quantity.
    ///
    /// Added 2026-08-07. The aggregate had been published for four handovers
    /// and nobody could act on it, because "125 gates are measurable today"
    /// names no gate: a reader who wanted to start had to re-derive the split
    /// by hand, which is the `09A` failure this module exists to prevent. The
    /// rows carry guardrail ids, not just counts, for the same reason.
    pub per_domain: Vec<DomainWork>,
}

/// One implemented in-play domain's share of the closable half.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainWork {
    /// `Dnn`.
    pub domain: String,
    /// The domain's `readiness` column verbatim.
    pub readiness: String,
    /// Closable gates here carrying evidence.
    pub evidenced: usize,
    /// Closable gates here carrying nothing and needing a mechanism first.
    pub mechanism_absent: usize,
    /// Guardrail ids here that are unmeasured and measurable today, in id
    /// order. **This is the work.**
    pub measurable_today: Vec<String>,
}

/// Derives the decomposition from the committed registers.
pub fn decompose(repo_root: &Path) -> Result<ReleaseStatus, String> {
    let readiness = performance_catalogue::domain_readiness(repo_root)?;
    let in_play_domains = in_play_domains(repo_root)?;
    let gates = performance_catalogue::release_gates(repo_root, &in_play_domains)?;
    let (evidenced, refused) = evidenced_and_refused_gates(repo_root)?;

    let domains_with_readiness = |unimplemented: bool| -> Vec<(String, String)> {
        in_play_domains
            .iter()
            .filter(|domain| {
                readiness.get(*domain).is_some_and(|value| is_unimplemented(value) == unimplemented)
            })
            .map(|domain| (domain.clone(), readiness.get(domain).cloned().unwrap_or_default()))
            .collect()
    };

    let (absent, present): (Vec<&ReleaseGate>, Vec<&ReleaseGate>) =
        gates.iter().partition(|gate| is_unimplemented(&gate.readiness));

    let bound_class_barred = present.iter().filter(|gate| is_bound(gate)).count();
    let open: Vec<&&ReleaseGate> = present.iter().filter(|gate| !is_bound(gate)).collect();
    let evidenced_count = open.iter().filter(|gate| evidenced.contains(&gate.id)).count();
    let empty: Vec<&&&ReleaseGate> =
        open.iter().filter(|gate| !evidenced.contains(&gate.id)).collect();
    let mechanism_absent = empty
        .iter()
        .filter(|gate| MECHANISM_ABSENT_GUARDRAILS.contains(&gate.guardrail.as_str()))
        .count();

    // The same partition again, per domain. Derived from the SAME `open`,
    // `evidenced` and `empty` sets above rather than recomputed from the
    // catalogue, so the rows cannot disagree with the totals they sum to —
    // `the_per_domain_worklist_reconciles_with_the_totals` asserts it.
    let per_domain: Vec<DomainWork> = domains_with_readiness(false)
        .into_iter()
        .map(|(domain, readiness)| {
            let mine = |gate: &&&ReleaseGate| gate.domain == domain;
            let mut measurable_today: Vec<String> = empty
                .iter()
                .filter(|gate| mine(gate))
                .filter(|gate| !MECHANISM_ABSENT_GUARDRAILS.contains(&gate.guardrail.as_str()))
                .map(|gate| gate.guardrail.clone())
                .collect();
            measurable_today.sort();
            DomainWork {
                evidenced: open
                    .iter()
                    .filter(|gate| gate.domain == domain && evidenced.contains(&gate.id))
                    .count(),
                mechanism_absent: empty
                    .iter()
                    .filter(|gate| mine(gate))
                    .filter(|gate| MECHANISM_ABSENT_GUARDRAILS.contains(&gate.guardrail.as_str()))
                    .count(),
                measurable_today,
                domain,
                readiness,
            }
        })
        .collect();

    let release_guardrails_per_domain =
        if in_play_domains.is_empty() { 0 } else { gates.len() / in_play_domains.len() };

    Ok(ReleaseStatus {
        in_play_domains: in_play_domains
            .iter()
            .map(|domain| (domain.clone(), readiness.get(domain).cloned().unwrap_or_default()))
            .collect(),
        release_guardrails_per_domain,
        in_play: gates.len(),
        without_subsystem: WithoutSubsystem {
            domains: domains_with_readiness(true),
            gates: absent.len(),
            hardware_only: absent.iter().filter(|gate| gate.is_hardware_only()).count(),
            bound_class: absent.iter().filter(|gate| is_bound(gate)).count(),
        },
        implemented: Implemented {
            domains: domains_with_readiness(false),
            gates: present.len(),
            hardware_only: present.iter().filter(|gate| gate.is_hardware_only()).count(),
            bound_class_barred,
            open: open.len(),
            evidenced: evidenced_count,
            refused: empty.iter().filter(|gate| refused.contains(&gate.id)).count(),
            empty: empty.len(),
            mechanism_absent,
            measurable_today: empty.len() - mechanism_absent,
            per_domain,
        },
    })
}

fn is_unimplemented(readiness: &str) -> bool {
    UNIMPLEMENTED_READINESS.contains(&readiness)
}

fn is_bound(gate: &ReleaseGate) -> bool {
    bound_provenance::is_bound_class(&gate.id)
}

/// A domain is *in play* when at least one Story contract selects it.
///
/// Read straight from the column rather than through
/// [`super::validate_story_contracts`], which needs the security and class
/// indexes to run. The risk of a second reader is drift, and the test
/// [`tests::the_decomposition_agrees_with_what_the_dashboard_publishes`] is
/// what removes it: this module's totals are pinned to the authoritative
/// spine walk rather than merely believed.
fn in_play_domains(repo_root: &Path) -> Result<BTreeSet<String>, String> {
    let path = repo_root.join("goals").join("assurance").join("story-contracts.tsv");
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let mut domains = BTreeSet::new();
    for line in contents.lines().skip(1) {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let Some(field) = line.split('\t').nth(2) else {
            continue;
        };
        for domain in field.split(',').map(str::trim).filter(|value| !value.is_empty()) {
            domains.insert(domain.to_string());
        }
    }
    Ok(domains)
}

/// Gates carrying evidence, and gates carrying a reasoned refusal.
///
/// The two sets are disjoint per *row* and need not be per *gate*: a gate one
/// Story refused may be evidenced by another. Where both are true the gate is
/// evidenced, and the refusal stays visible.
fn evidenced_and_refused_gates(
    repo_root: &Path,
) -> Result<(BTreeSet<String>, BTreeSet<String>), String> {
    let path = repo_root.join("goals").join("assurance").join("guardrail-evidence.tsv");
    let contents = fs::read_to_string(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let mut evidenced = BTreeSet::new();
    let mut refused = BTreeSet::new();
    for line in contents.lines().skip(1) {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        let (Some(id), Some(kind)) = (fields.first(), fields.get(3)) else {
            continue;
        };
        if *kind == EVIDENCE_KIND_REFUSED {
            refused.insert((*id).to_string());
        } else {
            evidenced.insert((*id).to_string());
        }
    }
    Ok((evidenced, refused))
}

/// Renders the decomposition as the nested ledger.
pub fn render(status: &ReleaseStatus) -> String {
    let mut out = String::new();
    let absent = &status.without_subsystem;
    let present = &status.implemented;

    out.push_str(&format!(
        "release gates in play ({} in-play domains x {} release guardrails)   {}\n",
        status.in_play_domains.len(),
        status.release_guardrails_per_domain,
        status.in_play
    ));
    out.push_str("|\n");
    out.push_str(&format!(
        "+- in the {} domains whose SUBSYSTEM DOES NOT EXIST                  {}\n",
        absent.domains.len(),
        absent.gates
    ));
    out.push_str(&format!("|     {}\n", render_domains(&absent.domains)));
    out.push_str("|     -- this bucket already CONTAINS:\n");
    out.push_str(&format!(
        "|          all {} hardware-only (T1/T2) gates in play\n",
        absent.hardware_only
    ));
    out.push_str(&format!(
        "|          {} of the {} G04 bound-class gates\n",
        absent.bound_class,
        absent.bound_class + present.bound_class_barred
    ));
    out.push_str("|\n");
    out.push_str(&format!(
        "+- in the {} IMPLEMENTED in-play domains                             {}\n",
        present.domains.len(),
        present.gates
    ));
    out.push_str(&format!("      {}\n", render_domains(&present.domains)));
    out.push_str(&format!(
        "   +- G04, barred by ADR 0005 while 0 platforms are qualified         {}\n",
        present.bound_class_barred
    ));
    out.push_str(&format!(
        "   +- needing a board (T1/T2 with no Host or T0 tier)                 {}\n",
        present.hardware_only
    ));
    out.push_str(&format!(
        "   +- not barred by G04, by the board, or by an absent subsystem      {}\n",
        present.open
    ));
    out.push_str(&format!(
        "      +- carrying evidence                                           {}\n",
        present.evidenced
    ));
    out.push_str(&format!(
        "      +- carrying nothing                                            {}\n",
        present.empty
    ));
    out.push_str(&format!(
        "         +- mechanism not built (declared, see source)               {}\n",
        present.mechanism_absent
    ));
    out.push_str(&format!(
        "         +- unmeasured, and MEASURABLE TODAY                         {}\n",
        present.measurable_today
    ));
    out.push_str(&format!(
        "         (of the empty, {} carry a reasoned REFUSAL rather than silence)\n",
        present.refused
    ));

    // The worklist. A count nobody can act on is a count nobody acts on, and
    // this one had been published for four handovers.
    let mut ranked: Vec<&DomainWork> = present.per_domain.iter().collect();
    ranked.sort_by(|a, b| {
        a.measurable_today
            .len()
            .cmp(&b.measurable_today.len())
            .then_with(|| a.domain.cmp(&b.domain))
    });
    out.push('\n');
    out.push_str("THE 125, BY DOMAIN — nearest to complete first.\n");
    out.push_str("Each row: gates already evidenced, gates needing a mechanism first, then the\n");
    out.push_str("guardrails that are measurement work available today.\n\n");
    out.push_str("  domain  readiness              evd  mech  today  guardrails\n");
    for work in ranked {
        out.push_str(&format!(
            "  {:<6}  {:<20}  {:>3}  {:>4}  {:>5}  {}\n",
            work.domain,
            work.readiness,
            work.evidenced,
            work.mechanism_absent,
            work.measurable_today.len(),
            if work.measurable_today.is_empty() {
                "-".to_string()
            } else {
                work.measurable_today.join(" ")
            }
        ));
    }

    // The rollup that actually says where the leverage is. The rows above
    // repeat the same guardrails across ten domains, so "125 gates" is not 125
    // pieces of work -- it is a much smaller number of MEASUREMENTS, each owed
    // by several domains. A guardrail owed by nine domains is one harness arm
    // that moves nine gates; a guardrail owed by one is a domain-specific job.
    // Reading the by-domain table alone hides that completely.
    let mut by_guardrail: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for work in &present.per_domain {
        for guardrail in &work.measurable_today {
            by_guardrail.entry(guardrail.as_str()).or_default().push(work.domain.as_str());
        }
    }
    let mut rollup: Vec<(&&str, &Vec<&str>)> = by_guardrail.iter().collect();
    rollup.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(b.0)));
    out.push('\n');
    out.push_str(&format!(
        "THE SAME 125, BY GUARDRAIL — {} distinct measurements, not 125 jobs.\n",
        rollup.len()
    ));
    out.push_str(
        "Widest first: a guardrail owed by many domains is one arm that moves many gates.\n\n",
    );
    out.push_str("  guardrail  domains  owed by\n");
    for (guardrail, domains) in rollup {
        out.push_str(&format!("  {:<9}  {:>7}  {}\n", guardrail, domains.len(), domains.join(" ")));
    }

    out.push('\n');
    out.push_str(&format!(
        "The defensible headline: {} of the {} are blocked by neither the qualification\n\
         decision nor the board. {} are measurement work available today; {} need a\n\
         mechanism built first. {} of the {} closable gates carry anything at all.\n",
        present.empty,
        status.in_play,
        present.measurable_today,
        present.mechanism_absent,
        present.evidenced,
        present.open
    ));
    out.push('\n');
    out.push_str(
        "Two caveats this output makes rather than hides. The mechanism-not-built split is a\n\
         DECLARED list of guardrails, not a derived fact -- readiness is per domain and cannot\n\
         say this per guardrail. And a gate carrying evidence is a gate someone MEASURED, never\n\
         a gate that PASSED: this is a count of evidence and is not a score.\n",
    );
    out
}

fn render_domains(domains: &[(String, String)]) -> String {
    domains
        .iter()
        .map(|(domain, readiness)| format!("{domain} {readiness}"))
        .collect::<Vec<_>>()
        .join(" | ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("xtask manifest lives at os/src/xtask")
            .to_path_buf()
    }

    fn committed() -> ReleaseStatus {
        decompose(&repo_root()).expect("the committed registers decompose")
    }

    /// The property `09A`'s hand-written ledger did not have.
    ///
    /// Every parent equals the sum of its children. Nothing is subtracted, so
    /// no bucket can overlap another and be counted twice — which is exactly
    /// how a hand ledger produced 164 where the answer is 220.
    #[test]
    /// The per-domain rows must sum to the totals they decompose.
    ///
    /// This is the `09A` failure made impossible rather than warned about:
    /// that handover subtracted overlapping buckets by hand and printed 164
    /// where the answer is 220, and a reader had to repair it. A worklist that
    /// does not add up to its own headline is worse than no worklist, because
    /// somebody will work from it.
    #[test]
    fn the_per_domain_worklist_reconciles_with_the_totals() {
        let status = committed();
        let present = &status.implemented;

        assert_eq!(
            present.per_domain.len(),
            present.domains.len(),
            "every implemented in-play domain gets exactly one row"
        );
        let summed_today: usize =
            present.per_domain.iter().map(|work| work.measurable_today.len()).sum();
        assert_eq!(
            summed_today, present.measurable_today,
            "the per-domain worklist must sum to the published measurable-today figure"
        );
        assert_eq!(
            present.per_domain.iter().map(|work| work.evidenced).sum::<usize>(),
            present.evidenced,
            "the per-domain evidenced counts must sum to the published total"
        );
        assert_eq!(
            present.per_domain.iter().map(|work| work.mechanism_absent).sum::<usize>(),
            present.mechanism_absent,
            "the per-domain mechanism-absent counts must sum to the published total"
        );
        assert!(
            summed_today > 0,
            "a worklist that names nothing would satisfy every sum above; this asserts \
             the rows carry actual guardrails"
        );
        for work in &present.per_domain {
            assert!(
                work.measurable_today
                    .iter()
                    .all(|g| !MECHANISM_ABSENT_GUARDRAILS.contains(&g.as_str())),
                "{}: a guardrail cannot be both measurable today and mechanism-absent",
                work.domain
            );
        }
    }

    #[test]
    fn the_ledger_reconciles_at_every_level() {
        let status = committed();
        let absent = &status.without_subsystem;
        let present = &status.implemented;

        assert_eq!(
            absent.gates + present.gates,
            status.in_play,
            "the two top-level buckets must partition the denominator"
        );
        assert_eq!(
            present.bound_class_barred + present.open,
            present.gates,
            "an implemented gate is barred by G04 or it is open"
        );
        assert_eq!(
            present.evidenced + present.empty,
            present.open,
            "an open gate carries evidence or it does not"
        );
        assert_eq!(
            present.mechanism_absent + present.measurable_today,
            present.empty,
            "an empty gate is unbuilt or it is merely unmeasured"
        );
        assert!(
            present.refused <= present.empty,
            "a refusal is a subset of the empty gates, never a sibling bucket"
        );
        assert_eq!(
            status.in_play_domains.len(),
            absent.domains.len() + present.domains.len(),
            "every in-play domain lands in exactly one bucket"
        );
    }

    /// `09A` §1's ledger, pinned — the **shape** of the denominator, which is
    /// a property of the catalogue and the contracts and moves only when
    /// someone selects a new domain or a subsystem changes readiness.
    ///
    /// Deliberately excludes the evidence-dependent figures. Those are
    /// supposed to move, and a test that pinned them would have to be edited
    /// every time a row is filed — which is how a pin stops being read and
    /// starts being updated reflexively.
    #[test]
    fn the_committed_tree_matches_the_09a_decomposition() {
        let status = committed();
        assert_eq!(status.in_play_domains.len(), 20);
        assert_eq!(status.release_guardrails_per_domain, 23);
        assert_eq!(status.in_play, 460);

        assert_eq!(status.without_subsystem.gates, 230, "half the denominator has no subsystem");
        assert_eq!(status.without_subsystem.hardware_only, 46);
        assert_eq!(status.without_subsystem.bound_class, 10);

        let present = &status.implemented;
        assert_eq!(present.gates, 230);
        assert_eq!(present.bound_class_barred, 10);
        assert_eq!(present.open, 220, "the denominator that can actually be closed");
    }

    /// The figures that are supposed to move, checked for consistency with
    /// each other rather than pinned to a value.
    ///
    /// The one hard assertion is the floor: **evidence may not go backwards.**
    /// 21 gates carried evidence on 2026-08-06 and a register that loses a row
    /// is a register someone edited by hand and did not mean to.
    #[test]
    fn evidence_coverage_is_self_consistent_and_never_regresses() {
        let present = committed().implemented;
        assert!(
            present.evidenced >= 21,
            "evidence ratchets: {} gates carry rows, and 21 did on 2026-08-06",
            present.evidenced
        );
        assert!(
            present.measurable_today > present.evidenced,
            "while this holds, the bottleneck is nobody measuring rather than anything blocking"
        );
    }

    /// `09A` §3's headline, and the finding that most contradicts the four
    /// handovers before it: **the board unblocks nothing on this register.**
    ///
    /// Derived here rather than asserted in prose. Every one of the 23 release
    /// guardrails in every implemented in-play domain is `Host` or `T0` tier;
    /// every hardware-only gate in play lives in a domain whose subsystem does
    /// not exist, where a board cannot help either.
    #[test]
    fn no_implemented_in_play_domain_has_a_gate_only_a_board_could_move() {
        let status = committed();
        assert_eq!(
            status.implemented.hardware_only, 0,
            "if this ever becomes non-zero, the board is genuinely blocking a closable gate \
             and 09A §3 stops being true"
        );
        assert_eq!(
            status.without_subsystem.hardware_only, 46,
            "and all 46 in-play hardware-only gates are inside the bucket a board cannot help"
        );
    }

    /// The check that makes a second reader of these TSVs safe.
    ///
    /// This module parses `story-contracts.tsv` and `guardrail-evidence.tsv`
    /// itself rather than through the full spine walk, so its numbers are
    /// pinned to the authoritative ones instead of merely resembling them.
    /// Without this, the decomposition could drift from the dashboard and both
    /// would look right.
    #[test]
    fn the_decomposition_agrees_with_what_the_dashboard_publishes() {
        let root = repo_root();
        let status = decompose(&root).expect("decomposes");
        let facts = super::super::dashboard_facts(&root).expect("the spine walks");
        assert_eq!(
            status.in_play, facts.in_play_gates,
            "the decomposition's denominator is the published denominator"
        );
        assert_eq!(
            status.implemented.evidenced + status.without_subsystem_evidenced_count(&root),
            facts.evidenced_gates,
            "every evidenced gate lands somewhere in the decomposition"
        );
    }

    #[test]
    fn the_rendered_ledger_names_its_own_caveats() {
        let rendered = render(&committed());
        assert!(rendered.contains("DECLARED list"), "the judgement must be labelled as one");
        assert!(rendered.contains("is not a score"), "{rendered}");
        assert!(rendered.contains("MEASURABLE TODAY"), "{rendered}");
    }

    impl ReleaseStatus {
        /// Evidenced gates sitting in the unbuildable half.
        ///
        /// Expected to be zero and asserted rather than assumed: evidence
        /// filed against a domain whose subsystem does not exist would be
        /// evidence about nothing, which is `LE-35`'s whole subject.
        fn without_subsystem_evidenced_count(&self, repo_root: &Path) -> usize {
            let (evidenced, _) = evidenced_and_refused_gates(repo_root).expect("register reads");
            let absent: BTreeSet<&String> =
                self.without_subsystem.domains.iter().map(|(domain, _)| domain).collect();
            let count = evidenced
                .iter()
                .filter(|id| id.split('-').nth(1).is_some_and(|d| absent.contains(&d.to_string())))
                .count();
            assert_eq!(count, 0, "LE-35 forbids evidence in a domain that does not exist");
            count
        }
    }

    /// Guard: the mechanism-absent list must name guardrails that actually
    /// exist in the catalogue, or the split silently counts zero of them.
    #[test]
    fn every_declared_mechanism_absent_guardrail_is_a_real_release_guardrail() {
        let status = committed();
        assert!(status.implemented.mechanism_absent > 0, "a typo'd list would count nothing");
        assert_eq!(
            status.implemented.mechanism_absent,
            MECHANISM_ABSENT_GUARDRAILS.len() * status.implemented.domains.len(),
            "each declared guardrail should be empty in every implemented domain; if this \
             fails, one of them has been evidenced and the list needs revisiting"
        );
    }
}
