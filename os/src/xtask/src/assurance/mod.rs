//! Integrity validation for TinyOS's performance-and-security assurance spine.
//!
//! # How this module is laid out
//!
//! This file holds three things and nothing else: the register **shapes**
//! (headers, field counts and expected populations), the **data model** the
//! validators exchange, and [`walk_spine`] — the one function that states the
//! order every check runs in. The checks themselves live next door, one module
//! per register family, so that adding a rule to one register is an edit to one
//! file rather than a search through four thousand lines:
//!
//! | module | owns |
//! |---|---|
//! | [`security_spine`] | containment classes, boundary tests, security controls, Protection Domain contracts, code-admission gates, the class matrix |
//! | [`contracts`] | the Feature and Story contract registers |
//! | [`context`] | application/platform destinations and landing zones |
//! | [`registers`] | guardrail evidence, open debt, and the no-heap gate |
//! | [`loose_ends`] | the defect register and its session citations |
//! | [`status`] | `Status:` headers and the three checks that hold them to something outside themselves |
//! | [`documents`] | Test and Report documents and their join back to Stories |
//! | [`ids`] | id grammar and list-membership |
//! | [`common`] | the TSV and filesystem primitives the rest are written in |
//!
//! **The order in [`walk_spine`] is load-bearing and is not alphabetical.** A
//! later check reads what an earlier one returned — the no-heap gate runs
//! before the evidence register that records it, so a `PERF-Dnn-G11` row can
//! never outlive the property; the platform register is read before the bound
//! claims, so a platform absent from it is unqualified rather than presumed
//! clean. Reordering these is a semantic change, not a cosmetic one.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::{bound_provenance, dashboard, performance_catalogue};

mod common;
mod context;
mod contracts;
mod documents;
mod ids;
mod loose_ends;
mod registers;
mod release_status;
mod security_spine;
mod status;

use common::*;
use context::*;
use contracts::*;
use documents::*;
use ids::*;
use loose_ends::*;
use registers::*;
use security_spine::*;
use status::*;

pub use release_status::{
    decompose as release_status, render as render_release_status, Implemented, ReleaseStatus,
};
pub use status::{artifact_statuses, ArtifactStatus};
const CONTAINMENT_HEADER: &str =
    "id\tname\tpurpose\tdefault_authority\tinput_rule\tfailure_rule\trequired_evidence";
const BOUNDARY_TEST_HEADER: &str =
    "id\tclasses\tobjective\tattack\tsuccess_criterion\tsecurity_controls\tperformance_guardrails";
const SECURITY_HEADER: &str =
    "id\tcontrol\tthreat\tinvariant\tcontainment_classes\trequired_evidence\tphase\tgate";
const PROTECTION_DOMAIN_HEADER: &str =
    "id\tinvariant\tscope\tenforcement\tfailure_rule\tsecurity_controls\tboundary_tests\tperformance_guardrails";
const CODE_ADMISSION_HEADER: &str =
    "id\tstage\tinput_classes\toutput_classes\tmandatory_check\tfailure_rule\tsecurity_controls\tboundary_tests\tperformance_guardrails";
const CLASS_COMMUNICATION_HEADER: &str =
    "source\ttarget\tdecision\tpath\tauthority_transfer\tfailure_rule\tboundary_tests";
const APPLICATION_PLATFORM_HEADER: &str =
    "id\tname\tcategory\tsupport_level\troadmap_horizon\texecution_model\tcontainment_classes\tperformance_domains\tsecurity_controls\tnetwork_posture\tcode_policy\trequired_evidence";
const LANDING_ZONE_HEADER: &str =
    "id\tname\toutcome\troadmap_horizon\tgoals\tperformance_domains\tapplications\tsecurity_controls\tcontainment_classes\tclaim_gate";
const FEATURE_CONTRACT_HEADER: &str =
    "feature_id\timplementation_classes\tsubject_classes\tauthority_posture\thostile_inputs\tboundary_tests\tprotection_domain_contracts\tcode_admission_gates\trequired_boundary_evidence";
const LOOSE_END_HEADER: &str =
    "le_id\tsummary\torigin\towner_path\townership\tstate\traised_in\tclosed_in";
const CONTRACT_HEADER: &str =
    "story_id\tfeature_id\tperformance_domains\tsecurity_controls\tcontainment_classes\tstate\trationale";
const CONTAINMENT_FIELD_COUNT: usize = 7;
const BOUNDARY_TEST_FIELD_COUNT: usize = 7;
const SECURITY_FIELD_COUNT: usize = 8;
const PROTECTION_DOMAIN_FIELD_COUNT: usize = 8;
const CODE_ADMISSION_FIELD_COUNT: usize = 9;
const CLASS_COMMUNICATION_FIELD_COUNT: usize = 7;
const APPLICATION_PLATFORM_FIELD_COUNT: usize = 12;
const LANDING_ZONE_FIELD_COUNT: usize = 10;
const FEATURE_CONTRACT_FIELD_COUNT: usize = 9;
const CONTRACT_FIELD_COUNT: usize = 7;
const LOOSE_END_FIELD_COUNT: usize = 8;
const GUARDRAIL_EVIDENCE_HEADER: &str = "guardrail_id\tdomain\tstory_id\tevidence_kind\t\
     evidence_path\tplatform_id\tirq_state\tcore_count\timage_kind\trecorded_in\tnote";
const GUARDRAIL_EVIDENCE_FIELD_COUNT: usize = 11;

/// The closed `irq_state` vocabulary (`ADR 0015` decision 2).
///
/// The four measurement-condition columns exist because every timing number
/// this project held on 2026-08-06 was produced with interrupts masked, on a
/// single core, from a fixture rather than the shipping image — and
/// `guardrail-evidence.tsv` had nowhere to say so. The conditions lived in
/// free-text `note` prose, so nothing checked them and nothing could refuse a
/// row for them: the `LE-89`/`LE-91` family a fourth time, a fact recorded
/// *beside* the thing it determines rather than derived from it.
///
/// **`unrecorded` is a value, not a gap.** It is the accurate statement that a
/// row was filed without saying what it was measured under, and it must never
/// be tidied into a guess — retro-fitting a condition is the exact failure the
/// ADR exists to stop. `n-a` is different and the difference is load-bearing:
/// structural evidence is a property of the code rather than of a run, so its
/// conditions do not *apply* rather than being unknown.
const IRQ_STATES: [&str; 4] = ["live", "masked", "n-a", UNRECORDED];

/// The closed `image_kind` vocabulary. `fixture` is the measurement binary;
/// `shipping` is the `os` image a deployment would run (`LE-20`, `LE-85`).
const IMAGE_KINDS: [&str; 4] = ["fixture", "shipping", "n-a", UNRECORDED];

/// Filed before conditions were recorded. Never a guess.
const UNRECORDED: &str = "unrecorded";

/// The condition does not apply, as against being unknown. Structural evidence
/// is a property of the code rather than of a run, so it has no interrupt state
/// to record — and collapsing that into [`UNRECORDED`] would make the honest
/// value mean two different things.
const NOT_APPLICABLE: &str = "n-a";
/// The closed `evidence_kind` vocabulary.
///
/// `structural` is a property the compiler or the type system enforces, so it
/// holds in every state rather than in a sampled one; `measured` is a number a
/// run produced; `refused` is a number a run produced that was then **read
/// against the gate's `target` column and declined**.
const EVIDENCE_KINDS: [&str; 3] = ["structural", "measured", EVIDENCE_KIND_REFUSED];
const EVIDENCE_KIND_REFUSED: &str = "refused";
const OPEN_DEBT_HEADER: &str = "story_id\tdomain\treadiness\treason\trecorded_in";
const OPEN_DEBT_FIELD_COUNT: usize = 5;
/// Crates that ship inside the image, and therefore must contain no heap.
///
/// `xtask` is deliberately absent: it is a host tool, it links `std`, and it
/// allocates freely. Conflating it with the shipped crates would either make the
/// no-heap gate unpassable or make it meaningless.
const SHIPPED_CRATES: [&str; 7] =
    ["hal", "hal-arm64", "hal-x86_64", "exec", "kernel", "motion", "os"];
/// Placeholder for a field that has no value yet.
///
/// The TSV convention in this directory is that every field is non-empty, so an
/// open loose end records `-` rather than an empty `closed_in`.
const LOOSE_END_UNSET: &str = "-";
const CONTAINMENT_CLASS_COUNT: usize = 5;
const BOUNDARY_TEST_COUNT: usize = 20;
const SECURITY_CONTROL_COUNT: usize = 20;
const PROTECTION_DOMAIN_CONTRACT_COUNT: usize = 14;
const CODE_ADMISSION_GATE_COUNT: usize = 14;
const CLASS_COMMUNICATION_PAIR_COUNT: usize = CONTAINMENT_CLASS_COUNT * CONTAINMENT_CLASS_COUNT;
const APPLICATION_PLATFORM_COUNT: usize = 19;
const LANDING_ZONE_COUNT: usize = 9;
const PERFORMANCE_GUARDRAILS_PER_DOMAIN: usize = 25;

/// Summary returned after every assurance-spine integrity check passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssuranceSummary {
    /// Number of Feature files with exactly one containment contract.
    pub feature_count: usize,
    /// Number of Story files with exactly one assurance contract.
    pub story_count: usize,
    /// Number of canonical containment classes.
    pub containment_class_count: usize,
    /// Number of canonical cross-class boundary tests.
    pub boundary_test_count: usize,
    /// Number of canonical security controls.
    pub security_control_count: usize,
    /// Number of canonical Protection Domain invariants.
    pub protection_domain_contract_count: usize,
    /// Number of mandatory remote-code admission gates.
    pub code_admission_gate_count: usize,
    /// Number of source/target entries in the complete C0-C4 matrix.
    pub class_communication_pair_count: usize,
    /// Number of application and platform destinations in the holistic context.
    pub application_platform_count: usize,
    /// Number of whole-system landing zones joining all four assurance planes.
    pub landing_zone_count: usize,
    /// Number of selected application/domain/guardrail contracts.
    pub selected_application_performance_contracts: usize,
    /// Number of selected Story/domain/guardrail contracts.
    ///
    /// One selected domain expands to all 25 performance guardrails.
    pub selected_performance_contracts: usize,
    /// Number of Test documents connected to a mapped Story.
    pub test_count: usize,
    /// Number of Report documents connected to a mapped Story or Test.
    pub report_count: usize,
    /// Number of loose ends in the register, closed and open together.
    pub loose_end_count: usize,
    /// Number of loose ends still open — the project's live defect count.
    pub open_loose_end_count: usize,
    /// Number of Epic/Feature/Story documents with a parseable `Status:` header.
    pub status_header_count: usize,
    /// Number of unfinished Stories examined for `LE-65`'s other half: a Story
    /// claiming every criterion is met may not also read `In progress`.
    pub unfinished_story_count: usize,
    /// Number of passing Reports cross-checked against their Stories' headers
    /// (`LE-65`).
    pub passing_report_count: usize,
    /// Number of `PERF-Dnn-Gnn` release gates with dated evidence recorded.
    ///
    /// This is a count of gates that have *evidence*, never a score and never a
    /// pass rate. A gate absent from the register is unevidenced, which is what
    /// it is; it is never "passed". No Story's assurance state is derived from
    /// this number — that conversion still requires every applicable gate.
    pub guardrail_evidence_count: usize,
    /// Of [`Self::guardrail_evidence_count`], the gates whose evidence was
    /// measured under conditions a deployment will actually meet — interrupts
    /// live, shipping image, qualified platform (`ADR 0015` decision 2).
    ///
    /// Published **beside** the evidence count rather than replacing it,
    /// because the difference between the two numbers is the finding. On
    /// 2026-08-06 it was 25 and 0.
    pub realtime_evidence_count: usize,
    /// Number of `(Story, domain)` selections initialised as stated open debt
    /// because the domain's subsystem does not exist yet (`LE-35`).
    pub open_debt_count: usize,
    /// Number of measuring platforms in the qualification register.
    pub platform_count: usize,
    /// Number of platforms holding a secure-world qualification record.
    ///
    /// `ADR 0005` decision 3 states this is zero and that a platform with no
    /// record is not qualified rather than presumed clean. It is printed
    /// alongside the total so a reader sees the ratio rather than a bare count.
    pub qualified_platform_count: usize,
    /// Number of bound-class evidence rows whose provenance was checked
    /// (`LE-33`).
    ///
    /// Printed even when zero, deliberately. A gate that has never examined
    /// anything and a gate that examined things and found them clean produce
    /// the same silence otherwise, and `ADR 0005`'s trap section is the
    /// argument for never letting those two look alike.
    pub bound_claim_count: usize,
    /// Number of Feature Stories-table rows cross-checked against the
    /// referenced Story's own `Status:` header (`LE-44`).
    pub feature_story_row_count: usize,
    /// Number of `Cargo.toml` manifests under `os/` proven not to reach
    /// outside `os/` by `path =` dependency or workspace membership
    /// (`ADR 0008`). The gate that keeps `external/` reference trees from
    /// silently becoming build inputs.
    pub external_manifest_count: usize,
    /// Number of dashboard Story status badges cross-checked against
    /// `list-status` (`LE-30`).
    ///
    /// The generated stat tiles and the spine-count sentence are checked on
    /// the same pass and have nothing to count -- they either match or the
    /// run fails -- so this is the one dashboard figure worth printing.
    pub dashboard_badge_count: usize,
}

#[derive(Debug, PartialEq, Eq)]
struct ContractIndex {
    stories: BTreeSet<String>,
    features: BTreeSet<String>,
    details_by_story: BTreeMap<String, StoryContract>,
    selected_performance_contracts: usize,
}

#[derive(Debug, PartialEq, Eq)]
struct StoryContract {
    feature_id: String,
    performance_domains: BTreeSet<String>,
    security_controls: BTreeSet<String>,
    containment_classes: BTreeSet<String>,
    state: String,
}

#[derive(Debug, PartialEq, Eq)]
struct SecurityIndex {
    controls: BTreeSet<String>,
    classes_by_control: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Debug, PartialEq, Eq)]
struct FeatureContractIndex {
    features: BTreeSet<String>,
    classes_by_feature: BTreeMap<String, BTreeSet<String>>,
    boundary_tests_by_feature: BTreeMap<String, BTreeSet<String>>,
    protection_domains_by_feature: BTreeMap<String, BTreeSet<String>>,
    code_admission_by_feature: BTreeMap<String, BTreeSet<String>>,
}

#[derive(Debug, PartialEq, Eq)]
struct CharterContractIndex {
    ids: BTreeSet<String>,
    security_controls: BTreeSet<String>,
    boundary_tests: BTreeSet<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct ApplicationPlatformIndex {
    ids: BTreeSet<String>,
    classes_by_application: BTreeMap<String, BTreeSet<String>>,
    domains_by_application: BTreeMap<String, BTreeSet<String>>,
    controls_by_application: BTreeMap<String, BTreeSet<String>>,
    selected_performance_contracts: usize,
}

impl ApplicationPlatformIndex {
    fn len(&self) -> usize {
        self.ids.len()
    }
}

#[derive(Debug, PartialEq, Eq)]
struct LandingZoneIndex {
    ids: BTreeSet<String>,
}

#[derive(Debug, PartialEq, Eq)]
struct GuardrailEvidenceIndex {
    /// Distinct `PERF-Dnn-Gnn` gates carrying at least one row.
    ///
    /// **Gates, not rows, and the distinction is not pedantic.** The register's
    /// unit is the `(guardrail, story)` pair — two Stories that both select a
    /// domain each file their own row for the same gate, by design, and the
    /// duplicate check permits exactly that. So the row count and the number of
    /// gates that have evidence are different quantities, and the one the
    /// dashboard publishes is the *gate* count: its tile reads "Release gates
    /// with dated evidence" over a denominator of gates in play.
    ///
    /// It counted rows until 2026-08-05, when filing `PERF-D07-G11` for
    /// `STORY-P1-10-02` and `-04` — a gate `STORY-P0-03-01` had already
    /// evidenced — moved the published figure by two while covering nothing
    /// new. That is a numerator inflating against a fixed denominator, in the
    /// exact statistic `06A` §6 chose as one of the project's two measures of
    /// itself, and it inflates fastest precisely when a domain is popular
    /// rather than when evidence is added.
    count: usize,
    /// Of [`Self::count`], the gates whose evidence was measured under
    /// conditions a deployment will actually meet — interrupts **live**, on the
    /// **shipping** image, on a platform holding a secure-world qualification
    /// record (`ADR 0015` decision 2, `ADR 0005`).
    ///
    /// **This is deliberately a second number rather than a filter on the
    /// first.** The existing rows keep their value as *mechanism* evidence:
    /// they show the mechanism works and what it costs under stated conditions.
    /// What they stop doing is standing as evidence about a running system.
    /// Publishing both makes the gap visible instead of reclassifying it away —
    /// and on the day this landed the value was **0**, which is the honest
    /// answer and the reason the column exists.
    realtime_count: usize,
    /// Gates carrying at least one `refused` row: **measured, read against the
    /// gate's `target` column, and declined.**
    ///
    /// A third state the register could not express until 2026-08-05, and the
    /// absence cost a real thing. `STORY-P1-06-01` measured `PERF-D03-G20`,
    /// found 55% run-to-run p99 CV, and refused the filing *in Report prose* —
    /// so from the register that Story is indistinguishable from one nobody
    /// started, and `09A` §8 step 1's "file what already exists" would have
    /// re-filed it or re-derived the refusal from scratch. `LE-85`.
    ///
    /// Refused gates are **not** in [`Self::count`]. A refusal is the opposite
    /// of evidence, and counting it would be `LE-83`'s numerator defect wearing
    /// the opposite sign.
    refused_gates: BTreeSet<String>,
    story_domain_pairs: BTreeSet<(String, String)>,
    bound_rows: Vec<bound_provenance::BoundEvidenceRow>,
}

#[derive(Debug, PartialEq, Eq)]
struct LooseEndIndex {
    ids: BTreeSet<String>,
    open_count: usize,
    /// Every `raised_in`/`closed_in` as `(id, field name, raw value)`, for
    /// [`validate_loose_end_citations`] to resolve against `session/` (`LE-51`).
    citations: Vec<(String, &'static str, String)>,
}

impl LandingZoneIndex {
    fn len(&self) -> usize {
        self.ids.len()
    }
}

impl CharterContractIndex {
    fn len(&self) -> usize {
        self.ids.len()
    }
}

/// Whether a spine run also holds `goals/index.html` to the numbers it just
/// computed.
///
/// `Skip` exists for exactly one caller and the reason is a usability trap:
/// `emit-dashboard` prints the block that *fixes* a stale page, so if it ran
/// the dashboard check it would refuse to run precisely when it is needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DashboardPolicy {
    Check,
    Skip,
}

/// Validates both catalogues and the Story-level join relative to `repo_root`.
pub fn check_assurance_spine(repo_root: &Path) -> Result<AssuranceSummary, String> {
    walk_spine(repo_root, DashboardPolicy::Check).map(|(summary, _)| summary)
}

/// The spine numbers `goals/index.html` renders, without holding the page to
/// them -- what `emit-dashboard` needs in order to print a correct block.
pub fn dashboard_facts(repo_root: &Path) -> Result<dashboard::DashboardFacts, String> {
    walk_spine(repo_root, DashboardPolicy::Skip).map(|(_, facts)| facts)
}

fn walk_spine(
    repo_root: &Path,
    dashboard_policy: DashboardPolicy,
) -> Result<(AssuranceSummary, dashboard::DashboardFacts), String> {
    crate::performance_catalogue::check_catalogue(repo_root)
        .map_err(|error| format!("performance catalogue: {error}"))?;

    let external_isolation = crate::external_isolation::check_external_isolation(repo_root)
        .map_err(|error| format!("external isolation: {error}"))?;

    let charter_path = repo_root.join("SECURITY_CHARTER.md");
    let charter_contents = fs::read_to_string(&charter_path)
        .map_err(|error| format!("failed to read {}: {error}", charter_path.display()))?;
    validate_security_charter_document(&charter_contents)?;

    let containment_path = repo_root.join("goals").join("security").join("containment-classes.tsv");
    let containment_contents = fs::read_to_string(&containment_path)
        .map_err(|error| format!("failed to read {}: {error}", containment_path.display()))?;
    let containment_classes = validate_containment_classes(&containment_contents)?;

    let security_path = repo_root.join("goals").join("security").join("controls.tsv");
    let security_contents = fs::read_to_string(&security_path)
        .map_err(|error| format!("failed to read {}: {error}", security_path.display()))?;
    let security = validate_security_controls(&security_contents, &containment_classes)?;

    let boundary_test_path = repo_root.join("goals").join("security").join("containment-tests.tsv");
    let boundary_test_contents = fs::read_to_string(&boundary_test_path)
        .map_err(|error| format!("failed to read {}: {error}", boundary_test_path.display()))?;
    let boundary_tests =
        validate_boundary_tests(&boundary_test_contents, &containment_classes, &security.controls)?;

    let protection_domain_path =
        repo_root.join("goals").join("security").join("protection-domain-contracts.tsv");
    let protection_domain_contents = fs::read_to_string(&protection_domain_path)
        .map_err(|error| format!("failed to read {}: {error}", protection_domain_path.display()))?;
    let protection_domains = validate_protection_domain_contracts(
        &protection_domain_contents,
        &containment_classes,
        &security.controls,
        &boundary_tests,
    )?;

    let code_admission_path =
        repo_root.join("goals").join("security").join("code-admission-gates.tsv");
    let code_admission_contents = fs::read_to_string(&code_admission_path)
        .map_err(|error| format!("failed to read {}: {error}", code_admission_path.display()))?;
    let code_admission = validate_code_admission_gates(
        &code_admission_contents,
        &containment_classes,
        &security.controls,
        &boundary_tests,
    )?;

    let class_communication_path =
        repo_root.join("goals").join("security").join("class-communication-matrix.tsv");
    let class_communication_contents =
        fs::read_to_string(&class_communication_path).map_err(|error| {
            format!("failed to read {}: {error}", class_communication_path.display())
        })?;
    let class_communication_pairs = validate_class_communication_matrix(
        &class_communication_contents,
        &containment_classes,
        &boundary_tests,
    )?;

    validate_charter_coverage(
        &protection_domains,
        &code_admission,
        &security.controls,
        &boundary_tests,
    )?;

    let application_platform_path =
        repo_root.join("goals").join("context").join("application-platforms.tsv");
    let application_platform_contents =
        fs::read_to_string(&application_platform_path).map_err(|error| {
            format!("failed to read {}: {error}", application_platform_path.display())
        })?;
    let application_platforms = validate_application_platforms(
        &application_platform_contents,
        &containment_classes,
        &security.controls,
    )?;

    let landing_zone_path = repo_root.join("goals").join("context").join("landing-zones.tsv");
    let landing_zone_contents = fs::read_to_string(&landing_zone_path)
        .map_err(|error| format!("failed to read {}: {error}", landing_zone_path.display()))?;
    let landing_zones = validate_landing_zones(
        &landing_zone_contents,
        &application_platforms,
        &containment_classes,
        &security.controls,
    )?;

    let feature_contract_path =
        repo_root.join("goals").join("assurance").join("feature-contracts.tsv");
    let feature_contract_contents = fs::read_to_string(&feature_contract_path)
        .map_err(|error| format!("failed to read {}: {error}", feature_contract_path.display()))?;
    let feature_contracts = validate_feature_contracts(
        &feature_contract_contents,
        &containment_classes,
        &boundary_tests,
        &protection_domains.ids,
        &code_admission.ids,
    )?;

    let contract_path = repo_root.join("goals").join("assurance").join("story-contracts.tsv");
    let contract_contents = fs::read_to_string(&contract_path)
        .map_err(|error| format!("failed to read {}: {error}", contract_path.display()))?;
    let contracts = validate_story_contracts(
        &contract_contents,
        &security,
        &containment_classes,
        &feature_contracts.classes_by_feature,
    )?;

    let story_dir = repo_root.join("goals").join("stories");
    let story_files = markdown_ids(&story_dir, "STORY-")?;
    compare_exact_coverage("Story", &story_files, &contracts.stories)?;

    let feature_dir = repo_root.join("goals").join("features");
    let feature_files = markdown_ids(&feature_dir, "FEAT-")?;
    compare_exact_coverage("Feature containment", &feature_files, &feature_contracts.features)?;
    compare_exact_coverage("Feature", &feature_files, &contracts.features)?;

    let test_dir = repo_root.join("goals").join("tests");
    let test_files = markdown_ids(&test_dir, "TEST-")?;
    validate_test_coverage(&test_dir, &test_files, &contracts, &feature_contracts)?;

    let report_dir = repo_root.join("goals").join("reports");
    let report_files = markdown_ids(&report_dir, "REPORT-")?;
    validate_report_coverage(&report_dir, &report_files, &test_files, &contracts.stories)?;

    let loose_end_path = repo_root.join("goals").join("assurance").join("loose-ends.tsv");
    let loose_end_contents = fs::read_to_string(&loose_end_path)
        .map_err(|error| format!("failed to read {}: {error}", loose_end_path.display()))?;
    let loose_ends = validate_loose_ends(&loose_end_contents)?;
    validate_loose_end_references(repo_root, &loose_ends.ids)?;
    validate_loose_end_citations(repo_root, &loose_ends.citations)?;

    // The no-heap gate runs before the evidence register reads it, so a
    // `PERF-Dnn-G11` row can never outlive the property it records.
    validate_no_heap(repo_root)?;

    // `LE-33`: a bound-class gate cannot be closed from a source `ADR 0004` or
    // `ADR 0005` disqualifies. The platform register is read first because a
    // platform absent from it is unqualified, never presumed clean.
    //
    // Moved above the evidence register on 2026-08-06 (`ADR 0015` decision 2):
    // an evidence row now names the platform it was measured on, and a register
    // that validates a name against a list it has not read yet validates
    // nothing.
    let platform_path = repo_root.join("goals").join("assurance").join("qualified-platforms.tsv");
    let platform_contents = fs::read_to_string(&platform_path)
        .map_err(|error| format!("failed to read {}: {error}", platform_path.display()))?;
    let platforms = bound_provenance::validate_platforms(&platform_contents, &report_files)?;

    let evidence_path = repo_root.join("goals").join("assurance").join("guardrail-evidence.tsv");
    let evidence_contents = fs::read_to_string(&evidence_path)
        .map_err(|error| format!("failed to read {}: {error}", evidence_path.display()))?;
    let evidence = validate_guardrail_evidence(&evidence_contents, &contracts, &platforms)?;

    // `LE-35`: a domain whose subsystem does not exist yet cannot be selected
    // as a satisfiable obligation, only as stated open debt.
    let readiness = performance_catalogue::domain_readiness(repo_root)?;
    let open_debt_path = repo_root.join("goals").join("assurance").join("open-debt.tsv");
    let open_debt_contents = fs::read_to_string(&open_debt_path)
        .map_err(|error| format!("failed to read {}: {error}", open_debt_path.display()))?;
    let open_debt = validate_open_debt(
        &open_debt_contents,
        &contracts,
        &readiness,
        &evidence.story_domain_pairs,
    )?;
    validate_open_debt_coverage(&contracts, &readiness, &open_debt)?;

    let bound_claim_count =
        bound_provenance::check_bound_evidence(repo_root, &evidence.bound_rows, &platforms)?;

    let statuses = validate_status_headers(repo_root)?;
    // `LE-44`: the headers above are individually well-formed; this is the
    // check that they agree with what their Features say about them.
    let feature_story_row_count = validate_feature_story_tables(repo_root, &statuses)?;
    // `LE-65`: and this is the check that a header agrees with the Story's own
    // filed evidence — the direction every other gate here misses, which is
    // how `STORY-P0-01-08` read `Specified` for four days after its Report
    // recorded Pass with everything green.
    let passing_report_count =
        validate_specified_headers_against_reports(&report_dir, &report_files, &statuses)?;
    // `LE-65`'s other half (`06A` §4.2): and this is the check in the opposite
    // direction — a Story that says every criterion is met may not also say it
    // is unfinished. Seven `EPIC-P1` Stories did exactly that on 2026-08-05.
    let unfinished_story_count = validate_unclaimed_satisfied_stories(&statuses)?;

    // `LE-30`: and this is the check that the page a reader meets first agrees
    // with all of the above. Nine sessions hand-synced it; none of them was
    // wrong on purpose, which is the argument for a machine rather than a
    // tenth careful reader.
    let in_play_domains: BTreeSet<String> = contracts
        .details_by_story
        .values()
        .flat_map(|contract| contract.performance_domains.iter().cloned())
        .collect();
    let reach = performance_catalogue::release_gate_reach(repo_root, &in_play_domains)?;
    /// `LE-108`. Named rather than inlined because the prose sentence it feeds
    /// speaks of the two determinism Epics specifically, and a substring test
    /// against `"STORY-P"` alone would silently absorb `EPIC-P9`'s ten.
    fn is_p0p1_story(id: &str) -> bool {
        id.starts_with("STORY-P0-") || id.starts_with("STORY-P1-")
    }

    let assurance_verified =
        contracts.details_by_story.values().filter(|contract| contract.state == "verified").count();
    // `LE-84`. The decomposition the dashboard publishes is the SAME call
    // `xtask assurance-status` makes, so the two can never disagree.
    let release_decomposition = release_status::decompose(repo_root)?;

    // `STORY-P0-01-09`: the Overall-progress numerics. The Epic population is
    // derived from disk — Epic documents plus the backlog phase table — and
    // the Story state counts from the same headers the badge check reads, so
    // no human retypes either.
    let epic_dir = repo_root.join("goals").join("epics");
    let epic_docs = markdown_ids(&epic_dir, "EPIC-")?;
    let backlog_path = epic_dir.join("backlog.md");
    let backlog_contents = fs::read_to_string(&backlog_path)
        .map_err(|error| format!("failed to read {}: {error}", backlog_path.display()))?;
    let roadmap = dashboard::roadmap_epics(&epic_docs, &backlog_contents);
    let epics_decomposed = dashboard::decomposed_epics(&roadmap, &contracts.stories);
    let story_state_count = |state: &str| {
        statuses
            .iter()
            .filter(|status| status.id.starts_with("STORY-") && status.state == state)
            .count()
    };

    let facts = dashboard::DashboardFacts {
        catalogue_cells: PERFORMANCE_GUARDRAILS_PER_DOMAIN * PERFORMANCE_GUARDRAILS_PER_DOMAIN,
        containment_classes: containment_classes.len(),
        boundary_tests: boundary_tests.len(),
        protection_domains: protection_domains.len(),
        code_admission_gates: code_admission.len(),
        class_paths: class_communication_pairs.len(),
        stories: contracts.stories.len(),
        security_controls: security.controls.len(),
        application_targets: application_platforms.len(),
        landing_zones: landing_zones.len(),
        assurance_verified,
        evidenced_gates: evidence.count,
        in_play_gates: reach.in_play,
        reachable_gates: reach.reachable,
        // `LE-84`: derived from the same decomposition `xtask assurance-status`
        // prints, never recomputed here. Handover `09A` computed this ledger by
        // hand and its first printing did not reconcile — it produced 164 where
        // the answer is 220, because two buckets are nested rather than
        // adjacent. One derivation, two consumers.
        closable_gates: release_decomposition.implemented.open,
        measurable_today: release_decomposition.implemented.measurable_today,
        // `LE-108`: derived from the same `Status:` headers the badge check
        // reads, so the prose sentence cannot drift from the documents again.
        p0p1_stories: statuses.iter().filter(|status| is_p0p1_story(&status.id)).count(),
        p0p1_settled: statuses
            .iter()
            .filter(|status| is_p0p1_story(&status.id))
            .filter(|status| matches!(status.state.as_str(), "Verified" | "Functionally Verified"))
            .count(),
        platforms: platforms.count(),
        qualified_platforms: platforms.qualified_count(),
        features: feature_contracts.features.len(),
        tests: test_files.len(),
        reports: report_files.len(),
        loose_ends: loose_ends.ids.len(),
        open_loose_ends: loose_ends.open_count,
        epics_total: roadmap.len(),
        epics_decomposed,
        stories_verified: story_state_count("Verified"),
        stories_functionally_verified: story_state_count("Functionally Verified"),
        stories_specified: story_state_count("Specified"),
        stories_in_progress: story_state_count("In progress"),
    };
    let dashboard_summary = match dashboard_policy {
        DashboardPolicy::Check => dashboard::check_dashboard(repo_root, &facts, &statuses)?,
        DashboardPolicy::Skip => dashboard::DashboardSummary { badges_checked: 0 },
    };

    Ok((
        AssuranceSummary {
            guardrail_evidence_count: evidence.count,
            realtime_evidence_count: evidence.realtime_count,
            open_debt_count: open_debt.len(),
            platform_count: platforms.count(),
            qualified_platform_count: platforms.qualified_count(),
            bound_claim_count,
            feature_story_row_count,
            unfinished_story_count,
            passing_report_count,
            dashboard_badge_count: dashboard_summary.badges_checked,
            external_manifest_count: external_isolation.manifest_count,
            feature_count: feature_contracts.features.len(),
            story_count: contracts.stories.len(),
            containment_class_count: containment_classes.len(),
            boundary_test_count: boundary_tests.len(),
            security_control_count: security.controls.len(),
            protection_domain_contract_count: protection_domains.len(),
            code_admission_gate_count: code_admission.len(),
            class_communication_pair_count: class_communication_pairs.len(),
            application_platform_count: application_platforms.len(),
            landing_zone_count: landing_zones.len(),
            selected_application_performance_contracts: application_platforms
                .selected_performance_contracts,
            selected_performance_contracts: contracts.selected_performance_contracts,
            test_count: test_files.len(),
            report_count: report_files.len(),
            loose_end_count: loose_ends.ids.len(),
            open_loose_end_count: loose_ends.open_count,
            status_header_count: statuses.len(),
        },
        facts,
    ))
}

#[cfg(test)]
mod spine_tests;
