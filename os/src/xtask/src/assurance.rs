//! Integrity validation for TinyOS's performance-and-security assurance spine.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::{bound_provenance, dashboard, performance_catalogue};

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
const GUARDRAIL_EVIDENCE_HEADER: &str =
    "guardrail_id\tdomain\tstory_id\tevidence_kind\tevidence_path\trecorded_in\tnote";
const GUARDRAIL_EVIDENCE_FIELD_COUNT: usize = 7;
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
    /// Number of `PERF-Dnn-Gnn` release gates with dated evidence recorded.
    ///
    /// This is a count of gates that have *evidence*, never a score and never a
    /// pass rate. A gate absent from the register is unevidenced, which is what
    /// it is; it is never "passed". No Story's assurance state is derived from
    /// this number — that conversion still requires every applicable gate.
    pub guardrail_evidence_count: usize,
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
    count: usize,
    story_domain_pairs: BTreeSet<(String, String)>,
    bound_rows: Vec<bound_provenance::BoundEvidenceRow>,
}

#[derive(Debug, PartialEq, Eq)]
struct LooseEndIndex {
    ids: BTreeSet<String>,
    open_count: usize,
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

    // The no-heap gate runs before the evidence register reads it, so a
    // `PERF-Dnn-G11` row can never outlive the property it records.
    validate_no_heap(repo_root)?;

    let evidence_path = repo_root.join("goals").join("assurance").join("guardrail-evidence.tsv");
    let evidence_contents = fs::read_to_string(&evidence_path)
        .map_err(|error| format!("failed to read {}: {error}", evidence_path.display()))?;
    let evidence = validate_guardrail_evidence(&evidence_contents, &contracts)?;

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

    // `LE-33`: a bound-class gate cannot be closed from a source `ADR 0004` or
    // `ADR 0005` disqualifies. The platform register is read first because a
    // platform absent from it is unqualified, never presumed clean.
    let platform_path = repo_root.join("goals").join("assurance").join("qualified-platforms.tsv");
    let platform_contents = fs::read_to_string(&platform_path)
        .map_err(|error| format!("failed to read {}: {error}", platform_path.display()))?;
    let platforms = bound_provenance::validate_platforms(&platform_contents, &report_files)?;
    let bound_claim_count =
        bound_provenance::check_bound_evidence(repo_root, &evidence.bound_rows, &platforms)?;

    let statuses = validate_status_headers(repo_root)?;
    // `LE-44`: the headers above are individually well-formed; this is the
    // check that they agree with what their Features say about them.
    let feature_story_row_count = validate_feature_story_tables(repo_root, &statuses)?;

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
    let assurance_verified =
        contracts.details_by_story.values().filter(|contract| contract.state == "verified").count();
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
        features: feature_contracts.features.len(),
        tests: test_files.len(),
        reports: report_files.len(),
        loose_ends: loose_ends.ids.len(),
        open_loose_ends: loose_ends.open_count,
    };
    let dashboard_summary = match dashboard_policy {
        DashboardPolicy::Check => dashboard::check_dashboard(repo_root, &facts, &statuses)?,
        DashboardPolicy::Skip => dashboard::DashboardSummary { badges_checked: 0 },
    };

    Ok((
        AssuranceSummary {
            guardrail_evidence_count: evidence.count,
            open_debt_count: open_debt.len(),
            platform_count: platforms.count(),
            qualified_platform_count: platforms.qualified_count(),
            bound_claim_count,
            feature_story_row_count,
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

fn validate_security_charter_document(contents: &str) -> Result<(), String> {
    const REQUIRED_REFERENCES: [&str; 8] = [
        "goals/security/protection-domain-contracts.tsv",
        "goals/security/code-admission-gates.tsv",
        "goals/security/class-communication-matrix.tsv",
        "goals/context/application-platforms.tsv",
        "goals/context/landing-zones.tsv",
        "xtask check-assurance-spine",
        "Revoke before reuse",
        "baseline-debt",
    ];
    for required in REQUIRED_REFERENCES {
        if !contents.contains(required) {
            return Err(format!(
                "SECURITY_CHARTER.md is missing required charter text `{required}`"
            ));
        }
    }
    Ok(())
}

fn validate_containment_classes(contents: &str) -> Result<BTreeSet<String>, String> {
    let mut lines = contents.lines();
    let header = lines
        .next()
        .ok_or_else(|| "containment class catalogue is empty".to_string())?
        .trim_end_matches('\r');
    if header != CONTAINMENT_HEADER {
        return Err(format!(
            "unexpected containment header; expected exactly `{CONTAINMENT_HEADER}`"
        ));
    }

    let mut classes = BTreeSet::new();
    for (zero_based_index, raw_line) in lines.enumerate() {
        let line_number = zero_based_index + 2;
        let fields =
            non_empty_tsv_fields(raw_line, line_number, CONTAINMENT_FIELD_COUNT, "containment")?;
        let class = fields[0];
        validate_containment_id(class, line_number)?;
        if !classes.insert(class.to_string()) {
            return Err(format!("containment line {line_number}: duplicate class `{class}`"));
        }
    }

    for number in 0..CONTAINMENT_CLASS_COUNT {
        let expected = format!("C{number}");
        if !classes.contains(&expected) {
            return Err(format!("missing containment class `{expected}`"));
        }
    }
    if classes.len() != CONTAINMENT_CLASS_COUNT {
        return Err(format!(
            "expected exactly {CONTAINMENT_CLASS_COUNT} containment classes, found {}",
            classes.len()
        ));
    }
    Ok(classes)
}

fn validate_boundary_tests(
    contents: &str,
    containment_classes: &BTreeSet<String>,
    security_controls: &BTreeSet<String>,
) -> Result<BTreeSet<String>, String> {
    let mut lines = contents.lines();
    let header = lines
        .next()
        .ok_or_else(|| "containment boundary-test catalogue is empty".to_string())?
        .trim_end_matches('\r');
    if header != BOUNDARY_TEST_HEADER {
        return Err(format!(
            "unexpected boundary-test header; expected exactly `{BOUNDARY_TEST_HEADER}`"
        ));
    }

    let mut tests = BTreeSet::new();
    for (zero_based_index, raw_line) in lines.enumerate() {
        let line_number = zero_based_index + 2;
        let fields = non_empty_tsv_fields(
            raw_line,
            line_number,
            BOUNDARY_TEST_FIELD_COUNT,
            "boundary-test",
        )?;
        let test = fields[0];
        validate_boundary_test_id(test, line_number)?;
        if !tests.insert(test.to_string()) {
            return Err(format!("boundary-test line {line_number}: duplicate test `{test}`"));
        }
        validate_containment_list(fields[1], line_number, containment_classes)?;
        validate_security_list(fields[5], line_number, security_controls)?;

        let mut guardrails = BTreeSet::new();
        for guardrail in fields[6].split(',') {
            validate_performance_guardrail_id(guardrail, line_number)?;
            if !guardrails.insert(guardrail) {
                return Err(format!(
                    "boundary-test line {line_number}: duplicate performance guardrail `{guardrail}`"
                ));
            }
        }
    }

    for number in 1..=BOUNDARY_TEST_COUNT {
        let expected = format!("BND-{number:02}");
        if !tests.contains(&expected) {
            return Err(format!("missing containment boundary test `{expected}`"));
        }
    }
    if tests.len() != BOUNDARY_TEST_COUNT {
        return Err(format!(
            "expected exactly {BOUNDARY_TEST_COUNT} containment boundary tests, found {}",
            tests.len()
        ));
    }
    Ok(tests)
}

fn validate_feature_contracts(
    contents: &str,
    containment_classes: &BTreeSet<String>,
    boundary_tests: &BTreeSet<String>,
    protection_domain_contracts: &BTreeSet<String>,
    code_admission_gates: &BTreeSet<String>,
) -> Result<FeatureContractIndex, String> {
    let mut lines = contents.lines();
    let header = lines
        .next()
        .ok_or_else(|| "Feature containment contract catalogue is empty".to_string())?
        .trim_end_matches('\r');
    if header != FEATURE_CONTRACT_HEADER {
        return Err(format!(
            "unexpected Feature contract header; expected exactly `{FEATURE_CONTRACT_HEADER}`"
        ));
    }

    let mut features = BTreeSet::new();
    let mut classes_by_feature = BTreeMap::new();
    let mut boundary_tests_by_feature = BTreeMap::new();
    let mut protection_domains_by_feature = BTreeMap::new();
    let mut code_admission_by_feature = BTreeMap::new();
    let mut selected_boundary_tests = BTreeSet::new();
    let mut selected_protection_domains = BTreeSet::new();
    let mut selected_code_admission = BTreeSet::new();
    for (zero_based_index, raw_line) in lines.enumerate() {
        let line_number = zero_based_index + 2;
        let fields = non_empty_tsv_fields(
            raw_line,
            line_number,
            FEATURE_CONTRACT_FIELD_COUNT,
            "Feature contract",
        )?;
        let feature = fields[0];
        validate_feature_id(feature, line_number)?;
        if !features.insert(feature.to_string()) {
            return Err(format!(
                "Feature contract line {line_number}: duplicate Feature `{feature}`"
            ));
        }

        let implementation =
            validate_containment_list(fields[1], line_number, containment_classes)?;
        let subjects = validate_containment_list(fields[2], line_number, containment_classes)?;
        let feature_classes = implementation.union(&subjects).cloned().collect();
        classes_by_feature.insert(feature.to_string(), feature_classes);

        let mut row_tests = BTreeSet::new();
        for boundary_test in fields[5].split(',') {
            validate_boundary_test_id(boundary_test, line_number)?;
            if !boundary_tests.contains(boundary_test) {
                return Err(format!(
                    "Feature contract line {line_number}: unknown boundary test `{boundary_test}`"
                ));
            }
            if !row_tests.insert(boundary_test) {
                return Err(format!(
                    "Feature contract line {line_number}: duplicate boundary test `{boundary_test}`"
                ));
            }
            selected_boundary_tests.insert(boundary_test.to_string());
        }
        boundary_tests_by_feature
            .insert(feature.to_string(), row_tests.into_iter().map(str::to_string).collect());

        let row_protection_domains = validate_numbered_contract_list(
            fields[6],
            line_number,
            "PD-",
            PROTECTION_DOMAIN_CONTRACT_COUNT,
            protection_domain_contracts,
            "Protection Domain contract",
        )?;
        selected_protection_domains.extend(row_protection_domains.iter().cloned());
        protection_domains_by_feature.insert(feature.to_string(), row_protection_domains);

        let row_code_admission = validate_numbered_contract_list(
            fields[7],
            line_number,
            "RCG-",
            CODE_ADMISSION_GATE_COUNT,
            code_admission_gates,
            "code-admission gate",
        )?;
        selected_code_admission.extend(row_code_admission.iter().cloned());
        code_admission_by_feature.insert(feature.to_string(), row_code_admission);
    }

    let unowned: Vec<&String> = boundary_tests.difference(&selected_boundary_tests).collect();
    if !unowned.is_empty() {
        return Err(format!(
            "containment boundary tests selected by no Feature: {}",
            join_ids(&unowned)
        ));
    }

    let unowned_protection_domains: Vec<&String> =
        protection_domain_contracts.difference(&selected_protection_domains).collect();
    if !unowned_protection_domains.is_empty() {
        return Err(format!(
            "Protection Domain contracts selected by no Feature: {}",
            join_ids(&unowned_protection_domains)
        ));
    }
    let unowned_code_admission: Vec<&String> =
        code_admission_gates.difference(&selected_code_admission).collect();
    if !unowned_code_admission.is_empty() {
        return Err(format!(
            "code-admission gates selected by no Feature: {}",
            join_ids(&unowned_code_admission)
        ));
    }

    Ok(FeatureContractIndex {
        features,
        classes_by_feature,
        boundary_tests_by_feature,
        protection_domains_by_feature,
        code_admission_by_feature,
    })
}

fn validate_security_controls(
    contents: &str,
    containment_classes: &BTreeSet<String>,
) -> Result<SecurityIndex, String> {
    let mut lines = contents.lines();
    let header = lines
        .next()
        .ok_or_else(|| "security control catalogue is empty".to_string())?
        .trim_end_matches('\r');
    if header != SECURITY_HEADER {
        return Err(format!("unexpected security header; expected exactly `{SECURITY_HEADER}`"));
    }

    let mut controls = BTreeSet::new();
    let mut classes_by_control = BTreeMap::new();
    for (zero_based_index, raw_line) in lines.enumerate() {
        let line_number = zero_based_index + 2;
        let fields = non_empty_tsv_fields(raw_line, line_number, SECURITY_FIELD_COUNT, "security")?;

        let id = fields[0];
        validate_security_id(id, line_number)?;
        if !controls.insert(id.to_string()) {
            return Err(format!("security line {line_number}: duplicate control `{id}`"));
        }
        let classes = validate_containment_list(fields[4], line_number, containment_classes)?;
        classes_by_control.insert(id.to_string(), classes);
        if fields[7] != "release" {
            return Err(format!(
                "security line {line_number}: gate must be `release`, found `{}`",
                fields[7]
            ));
        }
    }

    for number in 1..=SECURITY_CONTROL_COUNT {
        let expected = format!("SEC-{number:02}");
        if !controls.contains(&expected) {
            return Err(format!("missing security control `{expected}`"));
        }
    }
    if controls.len() != SECURITY_CONTROL_COUNT {
        return Err(format!(
            "expected exactly {SECURITY_CONTROL_COUNT} security controls, found {}",
            controls.len()
        ));
    }
    Ok(SecurityIndex { controls, classes_by_control })
}

fn validate_protection_domain_contracts(
    contents: &str,
    containment_classes: &BTreeSet<String>,
    security_controls: &BTreeSet<String>,
    boundary_tests: &BTreeSet<String>,
) -> Result<CharterContractIndex, String> {
    let mut lines = contents.lines();
    let header = lines
        .next()
        .ok_or_else(|| "Protection Domain contract catalogue is empty".to_string())?
        .trim_end_matches('\r');
    if header != PROTECTION_DOMAIN_HEADER {
        return Err(format!(
            "unexpected Protection Domain header; expected exactly `{PROTECTION_DOMAIN_HEADER}`"
        ));
    }

    let mut index = CharterContractIndex {
        ids: BTreeSet::new(),
        security_controls: BTreeSet::new(),
        boundary_tests: BTreeSet::new(),
    };
    for (zero_based_index, raw_line) in lines.enumerate() {
        let line_number = zero_based_index + 2;
        let fields = non_empty_tsv_fields(
            raw_line,
            line_number,
            PROTECTION_DOMAIN_FIELD_COUNT,
            "Protection Domain",
        )?;
        validate_numbered_charter_id(
            fields[0],
            "PD-",
            PROTECTION_DOMAIN_CONTRACT_COUNT,
            line_number,
        )?;
        if !index.ids.insert(fields[0].to_string()) {
            return Err(format!(
                "Protection Domain line {line_number}: duplicate contract `{}`",
                fields[0]
            ));
        }
        validate_containment_list(fields[2], line_number, containment_classes)?;
        index.security_controls.extend(validate_security_list(
            fields[5],
            line_number,
            security_controls,
        )?);
        index.boundary_tests.extend(validate_boundary_list(
            fields[6],
            line_number,
            boundary_tests,
            "Protection Domain",
        )?);
        validate_performance_guardrail_list(fields[7], line_number, "Protection Domain")?;
    }
    validate_complete_numbered_charter(
        &index.ids,
        "PD-",
        PROTECTION_DOMAIN_CONTRACT_COUNT,
        "Protection Domain contract",
    )?;
    Ok(index)
}

fn validate_code_admission_gates(
    contents: &str,
    containment_classes: &BTreeSet<String>,
    security_controls: &BTreeSet<String>,
    boundary_tests: &BTreeSet<String>,
) -> Result<CharterContractIndex, String> {
    let mut lines = contents.lines();
    let header = lines
        .next()
        .ok_or_else(|| "code-admission gate catalogue is empty".to_string())?
        .trim_end_matches('\r');
    if header != CODE_ADMISSION_HEADER {
        return Err(format!(
            "unexpected code-admission header; expected exactly `{CODE_ADMISSION_HEADER}`"
        ));
    }

    let mut index = CharterContractIndex {
        ids: BTreeSet::new(),
        security_controls: BTreeSet::new(),
        boundary_tests: BTreeSet::new(),
    };
    let mut classes_by_gate = BTreeMap::new();
    for (zero_based_index, raw_line) in lines.enumerate() {
        let line_number = zero_based_index + 2;
        let fields = non_empty_tsv_fields(
            raw_line,
            line_number,
            CODE_ADMISSION_FIELD_COUNT,
            "code-admission",
        )?;
        validate_numbered_charter_id(fields[0], "RCG-", CODE_ADMISSION_GATE_COUNT, line_number)?;
        if !index.ids.insert(fields[0].to_string()) {
            return Err(format!(
                "code-admission line {line_number}: duplicate gate `{}`",
                fields[0]
            ));
        }
        let inputs = validate_containment_list(fields[2], line_number, containment_classes)?;
        let outputs = validate_containment_list(fields[3], line_number, containment_classes)?;
        classes_by_gate.insert(fields[0].to_string(), (inputs, outputs));
        index.security_controls.extend(validate_security_list(
            fields[6],
            line_number,
            security_controls,
        )?);
        index.boundary_tests.extend(validate_boundary_list(
            fields[7],
            line_number,
            boundary_tests,
            "code-admission",
        )?);
        validate_performance_guardrail_list(fields[8], line_number, "code-admission")?;
    }
    validate_complete_numbered_charter(
        &index.ids,
        "RCG-",
        CODE_ADMISSION_GATE_COUNT,
        "code-admission gate",
    )?;

    let promotion = classes_by_gate
        .get("RCG-09")
        .ok_or_else(|| "missing code-admission promotion gate `RCG-09`".to_string())?;
    if !promotion.0.contains("C4") || promotion.1 != BTreeSet::from(["C3".to_string()]) {
        return Err(
            "RCG-09 must destroy a C4 inspection domain and produce only a fresh C3 domain"
                .to_string(),
        );
    }
    let seal = classes_by_gate
        .get("RCG-11")
        .ok_or_else(|| "missing executable-seal gate `RCG-11`".to_string())?;
    if seal.1 != BTreeSet::from(["C3".to_string()]) {
        return Err("RCG-11 executable sealing must produce only C3".to_string());
    }
    Ok(index)
}

fn validate_class_communication_matrix(
    contents: &str,
    containment_classes: &BTreeSet<String>,
    boundary_tests: &BTreeSet<String>,
) -> Result<BTreeSet<(String, String)>, String> {
    let mut lines = contents.lines();
    let header = lines
        .next()
        .ok_or_else(|| "class communication matrix is empty".to_string())?
        .trim_end_matches('\r');
    if header != CLASS_COMMUNICATION_HEADER {
        return Err(format!(
            "unexpected class communication header; expected exactly `{CLASS_COMMUNICATION_HEADER}`"
        ));
    }

    let mut pairs = BTreeSet::new();
    for (zero_based_index, raw_line) in lines.enumerate() {
        let line_number = zero_based_index + 2;
        let fields = non_empty_tsv_fields(
            raw_line,
            line_number,
            CLASS_COMMUNICATION_FIELD_COUNT,
            "class communication",
        )?;
        validate_containment_id(fields[0], line_number)?;
        validate_containment_id(fields[1], line_number)?;
        if !containment_classes.contains(fields[0]) || !containment_classes.contains(fields[1]) {
            return Err(format!(
                "class communication line {line_number}: unknown pair `{}->{}`",
                fields[0], fields[1]
            ));
        }
        let pair = (fields[0].to_string(), fields[1].to_string());
        if !pairs.insert(pair.clone()) {
            return Err(format!(
                "class communication line {line_number}: duplicate pair `{}->{}`",
                pair.0, pair.1
            ));
        }
        if !matches!(fields[2], "deny" | "handoff" | "internal" | "mediated") {
            return Err(format!(
                "class communication line {line_number}: unknown decision `{}`",
                fields[2]
            ));
        }
        if !matches!(fields[4], "none" | "boot-state-only" | "rights-reduced" | "one-shot-only") {
            return Err(format!(
                "class communication line {line_number}: unknown authority transfer `{}`",
                fields[4]
            ));
        }
        if fields[2] == "deny" && fields[4] != "none" {
            return Err(format!(
                "class communication line {line_number}: a denied path cannot transfer authority"
            ));
        }
        validate_boundary_list(fields[6], line_number, boundary_tests, "class communication")?;

        let expected_decision = if fields[0] == "C0" && fields[1] == "C1" {
            "handoff"
        } else if fields[0] == "C0" || fields[1] == "C0" {
            "deny"
        } else if fields[0] == "C1" && fields[1] == "C1" {
            "internal"
        } else {
            "mediated"
        };
        if fields[2] != expected_decision {
            return Err(format!(
                "class communication line {line_number}: `{}->{}` must be `{expected_decision}`, found `{}`",
                fields[0], fields[1], fields[2]
            ));
        }
        if fields[0] == "C4" && fields[4] != "none" {
            return Err(format!(
                "class communication line {line_number}: C4 cannot transfer authority"
            ));
        }
    }

    for source in containment_classes {
        for target in containment_classes {
            if !pairs.contains(&(source.clone(), target.clone())) {
                return Err(format!("class communication matrix is missing `{source}->{target}`"));
            }
        }
    }
    if pairs.len() != CLASS_COMMUNICATION_PAIR_COUNT {
        return Err(format!(
            "expected exactly {CLASS_COMMUNICATION_PAIR_COUNT} class communication pairs, found {}",
            pairs.len()
        ));
    }
    Ok(pairs)
}

fn validate_charter_coverage(
    protection_domains: &CharterContractIndex,
    code_admission: &CharterContractIndex,
    security_controls: &BTreeSet<String>,
    boundary_tests: &BTreeSet<String>,
) -> Result<(), String> {
    let selected_controls: BTreeSet<String> = protection_domains
        .security_controls
        .union(&code_admission.security_controls)
        .cloned()
        .collect();
    let missing_controls: Vec<&String> = security_controls.difference(&selected_controls).collect();
    if !missing_controls.is_empty() {
        return Err(format!(
            "security controls disconnected from the governing charter: {}",
            join_ids(&missing_controls)
        ));
    }

    let selected_boundaries: BTreeSet<String> =
        protection_domains.boundary_tests.union(&code_admission.boundary_tests).cloned().collect();
    let missing_boundaries: Vec<&String> =
        boundary_tests.difference(&selected_boundaries).collect();
    if !missing_boundaries.is_empty() {
        return Err(format!(
            "boundary tests disconnected from the governing charter: {}",
            join_ids(&missing_boundaries)
        ));
    }
    Ok(())
}

fn validate_application_platforms(
    contents: &str,
    containment_classes: &BTreeSet<String>,
    security_controls: &BTreeSet<String>,
) -> Result<ApplicationPlatformIndex, String> {
    let mut lines = contents.lines();
    let header = lines
        .next()
        .ok_or_else(|| "application platform catalogue is empty".to_string())?
        .trim_end_matches('\r');
    if header != APPLICATION_PLATFORM_HEADER {
        return Err(format!(
            "unexpected application platform header; expected exactly `{APPLICATION_PLATFORM_HEADER}`"
        ));
    }

    let mut index = ApplicationPlatformIndex {
        ids: BTreeSet::new(),
        classes_by_application: BTreeMap::new(),
        domains_by_application: BTreeMap::new(),
        controls_by_application: BTreeMap::new(),
        selected_performance_contracts: 0,
    };
    for (zero_based_index, raw_line) in lines.enumerate() {
        let line_number = zero_based_index + 2;
        let fields = non_empty_tsv_fields(
            raw_line,
            line_number,
            APPLICATION_PLATFORM_FIELD_COUNT,
            "application platform",
        )?;
        let id = fields[0];
        validate_numbered_context_id(
            id,
            "APP-",
            APPLICATION_PLATFORM_COUNT,
            line_number,
            "application platform",
        )?;
        if !index.ids.insert(id.to_string()) {
            return Err(format!("application platform line {line_number}: duplicate id `{id}`"));
        }
        if !matches!(
            fields[2],
            "core"
                | "control"
                | "ai"
                | "framework"
                | "runtime"
                | "game"
                | "browser"
                | "remote"
                | "compatibility"
                | "fleet"
                | "lab"
        ) {
            return Err(format!(
                "application platform line {line_number}: unknown category `{}`",
                fields[2]
            ));
        }
        if !matches!(
            fields[3],
            "core-native"
                | "native-txe"
                | "managed-aot"
                | "isolated-runtime"
                | "compatibility-guest"
                | "browser-hosted"
        ) {
            return Err(format!(
                "application platform line {line_number}: unknown support level `{}`",
                fields[3]
            ));
        }
        validate_context_horizon(fields[4], line_number, "application platform")?;
        let classes = validate_containment_list(fields[6], line_number, containment_classes)?;
        let domains = validate_domain_list(fields[7], line_number, "application platform")?;
        let controls = validate_security_list(fields[8], line_number, security_controls)?;
        index.selected_performance_contracts += domains.len() * PERFORMANCE_GUARDRAILS_PER_DOMAIN;
        index.classes_by_application.insert(id.to_string(), classes);
        index.domains_by_application.insert(id.to_string(), domains);
        index.controls_by_application.insert(id.to_string(), controls);
    }

    validate_complete_numbered_context(
        &index.ids,
        "APP-",
        APPLICATION_PLATFORM_COUNT,
        "application platform",
    )?;
    Ok(index)
}

fn validate_landing_zones(
    contents: &str,
    applications: &ApplicationPlatformIndex,
    containment_classes: &BTreeSet<String>,
    security_controls: &BTreeSet<String>,
) -> Result<LandingZoneIndex, String> {
    let mut lines = contents.lines();
    let header = lines
        .next()
        .ok_or_else(|| "landing-zone catalogue is empty".to_string())?
        .trim_end_matches('\r');
    if header != LANDING_ZONE_HEADER {
        return Err(format!(
            "unexpected landing-zone header; expected exactly `{LANDING_ZONE_HEADER}`"
        ));
    }

    let mut ids = BTreeSet::new();
    let mut selected_applications = BTreeSet::new();
    for (zero_based_index, raw_line) in lines.enumerate() {
        let line_number = zero_based_index + 2;
        let fields =
            non_empty_tsv_fields(raw_line, line_number, LANDING_ZONE_FIELD_COUNT, "landing zone")?;
        let id = fields[0];
        validate_numbered_context_id(id, "LZ-", LANDING_ZONE_COUNT, line_number, "landing zone")?;
        if !ids.insert(id.to_string()) {
            return Err(format!("landing zone line {line_number}: duplicate id `{id}`"));
        }
        validate_context_horizon(fields[3], line_number, "landing zone")?;
        validate_goal_list(fields[4], line_number)?;
        let row_domains = validate_domain_list(fields[5], line_number, "landing zone")?;
        let row_applications =
            validate_application_list(fields[6], line_number, &applications.ids)?;
        let row_controls = validate_security_list(fields[7], line_number, security_controls)?;
        let row_classes = validate_containment_list(fields[8], line_number, containment_classes)?;

        let mut application_domains = BTreeSet::new();
        let mut application_controls = BTreeSet::new();
        let mut application_classes = BTreeSet::new();
        for application in &row_applications {
            application_domains.extend(
                applications
                    .domains_by_application
                    .get(application)
                    .expect("validated application has domains")
                    .iter()
                    .cloned(),
            );
            application_controls.extend(
                applications
                    .controls_by_application
                    .get(application)
                    .expect("validated application has controls")
                    .iter()
                    .cloned(),
            );
            application_classes.extend(
                applications
                    .classes_by_application
                    .get(application)
                    .expect("validated application has classes")
                    .iter()
                    .cloned(),
            );
        }
        let disconnected_domains: Vec<&String> =
            row_domains.difference(&application_domains).collect();
        if !disconnected_domains.is_empty() {
            return Err(format!(
                "landing zone line {line_number}: performance domains unsupported by its applications: {}",
                join_ids(&disconnected_domains)
            ));
        }
        let disconnected_controls: Vec<&String> =
            row_controls.difference(&application_controls).collect();
        if !disconnected_controls.is_empty() {
            return Err(format!(
                "landing zone line {line_number}: security controls unsupported by its applications: {}",
                join_ids(&disconnected_controls)
            ));
        }
        let disconnected_classes: Vec<&String> =
            row_classes.difference(&application_classes).collect();
        if !disconnected_classes.is_empty() {
            return Err(format!(
                "landing zone line {line_number}: containment classes unsupported by its applications: {}",
                join_ids(&disconnected_classes)
            ));
        }
        selected_applications.extend(row_applications);
    }

    validate_complete_numbered_context(&ids, "LZ-", LANDING_ZONE_COUNT, "landing zone")?;
    let unowned_applications: Vec<&String> =
        applications.ids.difference(&selected_applications).collect();
    if !unowned_applications.is_empty() {
        return Err(format!(
            "application platforms selected by no landing zone: {}",
            join_ids(&unowned_applications)
        ));
    }
    Ok(LandingZoneIndex { ids })
}

/// Validates the loose-ends register: the project's machine-readable defect list.
///
/// The register exists because `LE-*` ids were previously carried only in session
/// handover prose, which the session convention forbids editing once a newer dated
/// folder exists — so the canonical list fragmented across handovers and could not
/// be queried. Ids must be contiguous from `LE-01`, because a gap is the signature
/// of exactly that fragmentation.
fn validate_loose_ends(contents: &str) -> Result<LooseEndIndex, String> {
    const OWNERSHIP: [&str; 3] = ["owned", "unowned", "deferred-with-trigger"];
    const STATES: [&str; 2] = ["open", "closed"];

    let mut lines = contents.lines();
    let header = lines
        .next()
        .ok_or_else(|| "loose-ends register is empty".to_string())?
        .trim_end_matches('\r');
    if header != LOOSE_END_HEADER {
        return Err(format!("unexpected loose-ends header; expected exactly `{LOOSE_END_HEADER}`"));
    }

    let mut ids = BTreeSet::new();
    let mut open_count = 0;
    for (zero_based_index, raw_line) in lines.enumerate() {
        let line_number = zero_based_index + 2;
        let fields =
            non_empty_tsv_fields(raw_line, line_number, LOOSE_END_FIELD_COUNT, "loose-ends")?;

        let id = fields[0];
        let Some(number) = id.strip_prefix("LE-").filter(|suffix| is_two_digits(suffix)) else {
            return Err(format!(
                "loose-ends line {line_number}: `{id}` is not a valid id (expected `LE-NN`)"
            ));
        };
        if !ids.insert(id.to_string()) {
            return Err(format!("loose-ends line {line_number}: duplicate id `{id}`"));
        }
        if number.parse::<usize>().unwrap_or(0) != ids.len() {
            return Err(format!(
                "loose-ends line {line_number}: `{id}` is out of order or leaves a gap; ids must \
                 run contiguously from `LE-01`"
            ));
        }

        let ownership = fields[4];
        if !OWNERSHIP.contains(&ownership) {
            return Err(format!(
                "loose-ends line {line_number}: unknown ownership `{ownership}` (expected one of {})",
                OWNERSHIP.join(", ")
            ));
        }

        let state = fields[5];
        if !STATES.contains(&state) {
            return Err(format!(
                "loose-ends line {line_number}: unknown state `{state}` (expected one of {})",
                STATES.join(", ")
            ));
        }

        // A closed loose end must say where it closed, and an open one must not
        // claim to have closed anywhere. Without this the register can report a
        // defect as resolved with no evidence behind the claim.
        let closed_in = fields[7];
        match (state, closed_in == LOOSE_END_UNSET) {
            ("closed", true) => {
                return Err(format!(
                    "loose-ends line {line_number}: `{id}` is closed but records no `closed_in`"
                ));
            }
            ("open", false) => {
                return Err(format!(
                    "loose-ends line {line_number}: `{id}` is open but records `closed_in` \
                     `{closed_in}`"
                ));
            }
            _ => {}
        }
        if state == "open" {
            open_count += 1;
        }
    }

    if ids.is_empty() {
        return Err("loose-ends register has no entries".to_string());
    }
    Ok(LooseEndIndex { ids, open_count })
}

/// Validates the guardrail evidence register and returns the number of gates
/// carrying dated evidence.
///
/// `TEST-P0-01-05-A` clause 1. The check that gives the register its value is
/// the last one: **a Story may only record evidence in a domain its own
/// contract selects.** Without it the register would accept evidence filed
/// against a gate nobody was ever obliged to close, which is a more convincing
/// way to be wrong than having no register at all.
///
/// Clause 2: no aggregate is computed here and no Story state is derived from
/// these rows. The count is a count of evidence, not a score.
fn validate_guardrail_evidence(
    contents: &str,
    contracts: &ContractIndex,
) -> Result<GuardrailEvidenceIndex, String> {
    let mut lines = contents.lines();
    let header = lines
        .next()
        .ok_or_else(|| "guardrail evidence register is empty".to_string())?
        .trim_end_matches('\r');
    if header != GUARDRAIL_EVIDENCE_HEADER {
        return Err(format!(
            "unexpected guardrail-evidence header; expected exactly `{GUARDRAIL_EVIDENCE_HEADER}`"
        ));
    }

    let mut seen = BTreeSet::new();
    let mut story_domain_pairs = BTreeSet::new();
    let mut bound_rows = Vec::new();
    for (zero_based_index, raw_line) in lines.enumerate() {
        let line_number = zero_based_index + 2;
        let fields = non_empty_tsv_fields(
            raw_line,
            line_number,
            GUARDRAIL_EVIDENCE_FIELD_COUNT,
            "guardrail-evidence",
        )?;

        let guardrail_id = fields[0];
        validate_performance_guardrail_id(guardrail_id, line_number)?;

        // `PERF-D04-G11` names its own domain, so a row claiming a different one
        // is internally inconsistent and every later check would be reading a
        // domain the id does not refer to.
        let domain = fields[1];
        let id_domain = guardrail_id.split('-').nth(1).unwrap_or_default();
        if id_domain != domain {
            return Err(format!(
                "guardrail-evidence line {line_number}: `{guardrail_id}` is a `{id_domain}` \
                 guardrail but the row records domain `{domain}`"
            ));
        }

        let story_id = fields[2];
        let Some(contract) = contracts.details_by_story.get(story_id) else {
            return Err(format!(
                "guardrail-evidence line {line_number}: `{story_id}` has no contract row"
            ));
        };
        if !contract.performance_domains.contains(domain) {
            return Err(format!(
                "guardrail-evidence line {line_number}: `{story_id}` records evidence in `{domain}` \
                 but its contract selects {}",
                join_owned_ids(&contract.performance_domains)
            ));
        }

        if !seen.insert((guardrail_id.to_string(), story_id.to_string())) {
            return Err(format!(
                "guardrail-evidence line {line_number}: duplicate evidence for `{guardrail_id}` \
                 from `{story_id}`"
            ));
        }
        story_domain_pairs.insert((story_id.to_string(), domain.to_string()));

        // Bound-class rows are handed to `bound_provenance`, which is where
        // `ADR 0004`'s and `ADR 0005`'s refusals live. Filtering here rather
        // than there keeps the register's parser in one place.
        if bound_provenance::is_bound_class(guardrail_id) {
            bound_rows.push(bound_provenance::BoundEvidenceRow {
                guardrail_id: guardrail_id.to_string(),
                story_id: story_id.to_string(),
                evidence_path: fields[4].to_string(),
            });
        }
    }

    Ok(GuardrailEvidenceIndex { count: seen.len(), story_domain_pairs, bound_rows })
}

/// Validates `goals/assurance/open-debt.tsv` — `LE-35`'s register.
///
/// The rule this enforces was set as a precedent by Handover 25 and never
/// written down: **selecting a performance domain pulls all 25 of its
/// guardrails into the selecting Story's contract, and where the subsystem does
/// not exist not one of them can be closed.** Left implicit, the contract
/// presents as satisfiable and the cheapest lie available becomes recording all
/// 25.
///
/// Both directions are refused, and the second matters as much as the first: a
/// debt row for a domain that *is* implemented would let debt excuse a real
/// obligation.
fn validate_open_debt(
    contents: &str,
    contracts: &ContractIndex,
    readiness: &BTreeMap<String, String>,
    evidence_pairs: &BTreeSet<(String, String)>,
) -> Result<BTreeSet<(String, String)>, String> {
    let mut lines = contents.lines();
    let header = lines
        .next()
        .ok_or_else(|| "open-debt register is empty".to_string())?
        .trim_end_matches('\r');
    if header != OPEN_DEBT_HEADER {
        return Err(format!("unexpected open-debt header; expected exactly `{OPEN_DEBT_HEADER}`"));
    }

    let mut pairs = BTreeSet::new();
    for (zero_based_index, raw_line) in lines.enumerate() {
        let line_number = zero_based_index + 2;
        let fields =
            non_empty_tsv_fields(raw_line, line_number, OPEN_DEBT_FIELD_COUNT, "open-debt")?;

        let story_id = fields[0];
        let domain = fields[1];
        let declared_readiness = fields[2];
        validate_domain_id(domain, line_number)?;

        let Some(contract) = contracts.details_by_story.get(story_id) else {
            return Err(format!("open-debt line {line_number}: `{story_id}` has no contract row"));
        };
        if !contract.performance_domains.contains(domain) {
            return Err(format!(
                "open-debt line {line_number}: `{story_id}` records debt in `{domain}` but its \
                 contract selects {}",
                join_owned_ids(&contract.performance_domains)
            ));
        }

        let Some(actual_readiness) = readiness.get(domain) else {
            return Err(format!(
                "open-debt line {line_number}: `{domain}` is not a catalogue domain"
            ));
        };
        if declared_readiness != actual_readiness {
            return Err(format!(
                "open-debt line {line_number}: `{domain}` is recorded at readiness \
                 `{declared_readiness}` but the catalogue says `{actual_readiness}`"
            ));
        }
        if !performance_catalogue::UNIMPLEMENTED_READINESS.contains(&actual_readiness.as_str()) {
            return Err(format!(
                "open-debt line {line_number}: `{story_id}` records `{domain}` as open debt, but \
                 `{domain}` is at readiness `{actual_readiness}` and its guardrails are real \
                 obligations. Debt may name a subsystem that does not exist; it may not excuse \
                 one that does"
            ));
        }

        if !pairs.insert((story_id.to_string(), domain.to_string())) {
            return Err(format!(
                "open-debt line {line_number}: duplicate debt row for `{story_id}` / `{domain}`"
            ));
        }
        // A gate cannot be simultaneously unclosable and closed. This is the
        // check that stops the register pair from drifting into a contradiction
        // nobody reading either file alone would notice.
        if evidence_pairs.contains(&(story_id.to_string(), domain.to_string())) {
            return Err(format!(
                "open-debt line {line_number}: `{story_id}` records `{domain}` as open debt and \
                 also files guardrail evidence in it. A domain whose subsystem does not exist \
                 cannot have produced evidence"
            ));
        }
    }

    Ok(pairs)
}

/// Refuses a Story contract that selects a domain whose subsystem does not
/// exist without initialising it as stated open debt (`LE-35`, the forward
/// direction).
fn validate_open_debt_coverage(
    contracts: &ContractIndex,
    readiness: &BTreeMap<String, String>,
    debt: &BTreeSet<(String, String)>,
) -> Result<(), String> {
    for (story_id, contract) in &contracts.details_by_story {
        for domain in &contract.performance_domains {
            let Some(actual) = readiness.get(domain) else {
                continue;
            };
            if !performance_catalogue::UNIMPLEMENTED_READINESS.contains(&actual.as_str()) {
                continue;
            }
            if !debt.contains(&(story_id.clone(), domain.clone())) {
                return Err(format!(
                    "story-contracts: `{story_id}` selects `{domain}`, whose readiness is \
                     `{actual}` — the subsystem does not exist, so not one of its 25 guardrails \
                     can be closed. Selecting it initialises stated open debt: add a row to \
                     goals/assurance/open-debt.tsv (LE-35)"
                ));
            }
        }
    }
    Ok(())
}

/// Fails if any shipped crate could allocate.
///
/// `TEST-P0-01-05-A` clause 3, and the evidence behind every `PERF-Dnn-G11` row
/// in the register. `G11` asks for zero heap allocations per steady-state work
/// unit; this system has no heap at all, which is a stronger property and a
/// compiler-enforced one — a `no_std` crate with no `#[global_allocator]` cannot
/// use `alloc` and would fail to build if it tried.
///
/// The property was true by design. This makes it true on purpose: the day
/// someone adds an allocator, the `G11` evidence is withdrawn by CI rather than
/// silently invalidated by a change nobody connected to it.
///
/// `#[cfg(test)]` code is exempt deliberately. Host tests link `std` on purpose
/// and `kernel::measure`'s tests use `String` today; the claim is about the
/// shipped image, and conflating the two would make the gate either unpassable
/// or meaningless.
fn validate_no_heap(repo_root: &Path) -> Result<(), String> {
    const FORBIDDEN: [&str; 3] = ["#[global_allocator]", "extern crate alloc", "use alloc::"];

    for crate_name in SHIPPED_CRATES {
        let crate_src = repo_root.join("os").join("src").join(crate_name).join("src");
        let mut sources = Vec::new();
        collect_rust_sources(&crate_src, &mut sources)?;
        if sources.is_empty() {
            return Err(format!("no-heap gate: {crate_name} has no Rust sources to check"));
        }

        let mut declares_no_std = false;
        for path in sources {
            let contents = fs::read_to_string(&path)
                .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
            if contents.contains("no_std") {
                declares_no_std = true;
            }
            let mut in_test_module = false;
            for (zero_based_index, line) in contents.lines().enumerate() {
                if line.trim_start().starts_with("#[cfg(test)]") {
                    in_test_module = true;
                    continue;
                }
                if in_test_module {
                    continue;
                }
                for needle in FORBIDDEN {
                    if line.contains(needle) {
                        return Err(format!(
                            "no-heap gate: {}:{} contains `{needle}` outside `#[cfg(test)]`; \
                             every `PERF-Dnn-G11` row in guardrail-evidence.tsv rests on this \
                             system having no heap, so add an allocator only by withdrawing that \
                             evidence first",
                            path.display(),
                            zero_based_index + 1
                        ));
                    }
                }
            }
        }
        if !declares_no_std {
            return Err(format!(
                "no-heap gate: crate `{crate_name}` declares no `no_std`, so it links the host \
                 allocator and cannot support a `G11` claim"
            ));
        }
    }
    Ok(())
}

/// Collects `.rs` files under a directory, recursively.
fn collect_rust_sources(directory: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("failed to read a directory entry: {error}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_sources(&path, output)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            output.push(path);
        }
    }
    Ok(())
}

/// Fails if any `LE-*` token in the live documents has no row in the register.
///
/// Only `goals/` and `docs/` are scanned. `session/` is deliberately excluded: those
/// dated folders are an immutable historical record that the session convention says
/// is never edited, so a token frozen in an old handover must not gate the register.
fn validate_loose_end_references(
    repo_root: &Path,
    ids: &BTreeSet<String>,
) -> Result<usize, String> {
    let mut markdown = Vec::new();
    for directory in ["goals", "docs"] {
        collect_markdown(&repo_root.join(directory), &mut markdown)?;
    }

    let mut reference_count = 0;
    for path in markdown {
        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        for (zero_based_index, line) in contents.lines().enumerate() {
            for token in loose_end_tokens(line) {
                reference_count += 1;
                if !ids.contains(&token) {
                    return Err(format!(
                        "{}:{}: `{token}` has no row in goals/assurance/loose-ends.tsv",
                        path.display(),
                        zero_based_index + 1
                    ));
                }
            }
        }
    }
    Ok(reference_count)
}

/// Extracts every `LE-NN` token from one line.
fn loose_end_tokens(line: &str) -> Vec<String> {
    let bytes = line.as_bytes();
    let mut tokens = Vec::new();
    for (index, _) in line.match_indices("LE-") {
        // `SAMPLE-01` and similar must not match: require a non-alphanumeric before
        // the `L`, so only a standalone token counts.
        if index > 0 {
            let previous = bytes[index - 1];
            if previous.is_ascii_alphanumeric() || previous == b'-' || previous == b'_' {
                continue;
            }
        }
        let digits = &line[index + 3..];
        if digits.len() >= 2 && digits.as_bytes()[..2].iter().all(u8::is_ascii_digit) {
            tokens.push(format!("LE-{}", &digits[..2]));
        }
    }
    tokens
}

fn collect_markdown(directory: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry
            .map_err(|error| format!("failed to enumerate {}: {error}", directory.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_markdown(&path, output)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("md") {
            output.push(path);
        }
    }
    Ok(())
}

/// The controlled vocabulary a `Status:` header may open with.
///
/// Ordered longest-first so that `Functionally Verified` is never truncated to
/// `Verified` by a prefix match.
const STATUS_STATES: [&str; 6] = [
    "Functionally Verified",
    "Functionally complete",
    "In progress",
    "Specified",
    "Complete",
    "Verified",
];

/// One artifact's machine-readable state, extracted from its `Status:` header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactStatus {
    /// `EPIC-P1`, `FEAT-P1-04`, `STORY-P1-04-03`.
    pub id: String,
    /// One of [`STATUS_STATES`].
    pub state: String,
    /// Everything after the state — dates, tiers, Reports, caveats.
    pub detail: String,
}

/// Extracts the state from a `Status:` header line.
///
/// The header stays human-readable prose; only its opening is constrained. The
/// state runs from `**` to the first terminator, so
/// `Status: **Verified** (locally; CI run pending)` and
/// `Status: **Specified, not yet started. Gated on `LE-09`.**` both parse.
fn parse_status_line(line: &str) -> Result<(String, String), String> {
    let body = line
        .strip_prefix("Status:")
        .ok_or_else(|| "status line must start with `Status:`".to_string())?
        .trim_start();
    let body = body
        .strip_prefix("**")
        .ok_or_else(|| "status line must open with a bold state, `Status: **...`".to_string())?;

    for state in STATUS_STATES {
        let Some(rest) = body.strip_prefix(state) else {
            continue;
        };
        // A state must be followed by a terminator, never by more word
        // characters — otherwise `Complete` would match `Completely rewritten`.
        let terminated = rest.is_empty()
            || rest.starts_with("**")
            || rest.starts_with(" —")
            || rest.starts_with(',')
            || rest.starts_with(" (")
            || rest.starts_with('.');
        if terminated {
            // The remainder is prose meant for a reader, so strip the bold
            // markers and the separator that joined it to the state.
            let detail = rest
                .trim()
                .trim_start_matches("**")
                .trim()
                .trim_start_matches(['—', ','])
                .trim()
                .trim_end_matches("**")
                .trim()
                .to_string();
            return Ok((state.to_string(), detail));
        }
    }

    Err(format!(
        "status must open with one of {}; found `{}`",
        STATUS_STATES.join(", "),
        body.chars().take(40).collect::<String>()
    ))
}

/// Reads and validates the `Status:` header of every Epic, Feature and Story.
///
/// The headers were previously free prose in fourteen distinct shapes, so the
/// dashboard had to be hand-maintained against seventy documents and drifted.
/// Constraining only the opening keeps the prose while making the state
/// queryable.
fn validate_status_headers(repo_root: &Path) -> Result<Vec<ArtifactStatus>, String> {
    let mut statuses = Vec::new();
    for (directory, prefix) in [("epics", "EPIC-"), ("features", "FEAT-"), ("stories", "STORY-")] {
        let path = repo_root.join("goals").join(directory);
        let mut paths = Vec::new();
        collect_markdown(&path, &mut paths)?;
        paths.sort();
        for file in paths {
            let Some(stem) = file.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            // `backlog.md` carries a status line but is a table of undecomposed
            // rows rather than an artifact with an id, so it is not constrained.
            if !stem.starts_with(prefix) {
                continue;
            }
            let contents = fs::read_to_string(&file)
                .map_err(|error| format!("failed to read {}: {error}", file.display()))?;
            let line = contents
                .lines()
                .find(|line| line.starts_with("Status:"))
                .ok_or_else(|| format!("{stem}: no `Status:` header"))?;
            let (state, detail) =
                parse_status_line(line).map_err(|error| format!("{stem}: {error}"))?;
            statuses.push(ArtifactStatus { id: stem.to_string(), state, detail });
        }
    }
    if statuses.is_empty() {
        return Err("no Epic, Feature or Story documents found".to_string());
    }
    Ok(statuses)
}

/// Reads every artifact status for `list-status`.
pub fn artifact_statuses(repo_root: &Path) -> Result<Vec<ArtifactStatus>, String> {
    validate_status_headers(repo_root)
}

/// Extracts the Story id and status cell from one Feature Stories-table row.
///
/// Rows look like `| [`STORY-P1-07-01`](../stories/….md) | summary | status |`.
/// Anything that is not such a row — the header, the `|---|` rule, prose
/// containing a pipe — yields `None` rather than an error, because a Feature's
/// body is prose and only this one table shape is constrained.
fn parse_feature_story_row(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if !line.starts_with("| [`STORY-") {
        return None;
    }
    let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
    if cells.len() < 3 {
        return None;
    }
    let id = cells[0].trim_start_matches("[`").split('`').next()?.to_string();
    Some((id, cells[cells.len() - 1].to_string()))
}

/// Every `criterion N` / `criteria N and M` number a status cell or header
/// mentions.
///
/// A set, not a list: `criteria 3 and 4` and `criteria 4 and 3` say the same
/// thing, and a check that disagreed about that would be noise rather than a
/// gate.
fn criterion_numbers(text: &str) -> BTreeSet<u32> {
    let lowered = text.to_ascii_lowercase();
    let mut numbers = BTreeSet::new();
    let mut rest = lowered.as_str();
    while let Some(position) = rest.find("criteri") {
        rest = &rest[position..];
        let after_keyword = match (rest.strip_prefix("criteria"), rest.strip_prefix("criterion")) {
            (Some(tail), _) => tail,
            (None, Some(tail)) => tail,
            (None, None) => {
                rest = &rest["criteri".len()..];
                continue;
            }
        };
        // Numbers run until the first token that is neither a number nor one of
        // the words that join them, so `criteria 3 and 4 need a board` stops at
        // `need` rather than swallowing the sentence.
        for token in after_keyword.split(|c: char| c.is_whitespace() || c == ',') {
            let token = token.trim_matches(|c: char| !c.is_ascii_alphanumeric());
            if token.is_empty() || token == "and" {
                continue;
            }
            match token.parse::<u32>() {
                Ok(number) => {
                    numbers.insert(number);
                }
                Err(_) => break,
            }
        }
        rest = after_keyword;
    }
    numbers
}

/// Cross-checks every Feature Stories-table row against the referenced Story's
/// own `Status:` header — `LE-44`.
///
/// `check-assurance-spine` already validated all 84 status headers, and already
/// read every Feature document; what it never did was compare the two. So a
/// Feature and its Story could disagree about a Story's state indefinitely with
/// every gate green, which is exactly what happened twice: `FEAT-P1-07` said
/// `STORY-P1-07-01` needed a board for *"criteria 2 and 4"* where the Story said
/// 3 and 4 — understating it on precisely the criterion that produces `Q1`
/// qualification evidence — and `FEAT-P1-03` recorded `STORY-P1-03-02` as
/// `Verified` for four days while the Story's own header still read
/// `In progress`.
///
/// Two things are compared and nothing else. The **state word exactly**:
/// `Functionally Verified` and `Verified` are distinct states in this project's
/// vocabulary, one carrying assurance debt whose reader will not go looking for
/// it, so they do not satisfy each other. And the **criterion numbers as a
/// set**. Everything else in both cells stays free prose.
fn validate_feature_story_tables(
    repo_root: &Path,
    statuses: &[ArtifactStatus],
) -> Result<usize, String> {
    let by_id: BTreeMap<&str, &ArtifactStatus> =
        statuses.iter().map(|status| (status.id.as_str(), status)).collect();

    let feature_dir = repo_root.join("goals").join("features");
    let mut paths = Vec::new();
    collect_markdown(&feature_dir, &mut paths)?;
    paths.sort();

    let mut checked = 0;
    for file in paths {
        let Some(stem) = file.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if !stem.starts_with("FEAT-") {
            continue;
        }
        let contents = fs::read_to_string(&file)
            .map_err(|error| format!("failed to read {}: {error}", file.display()))?;
        for line in contents.lines() {
            let Some((story_id, cell)) = parse_feature_story_row(line) else {
                continue;
            };
            let Some(status) = by_id.get(story_id.as_str()) else {
                return Err(format!(
                    "{stem}: Stories table names `{story_id}`, which has no Story document"
                ));
            };
            compare_feature_story_row(stem, &story_id, &cell, status)?;
            checked += 1;
        }
    }

    if checked == 0 {
        return Err("no Feature Stories-table rows found to cross-check".to_string());
    }
    Ok(checked)
}

/// One Feature-table row against one Story header — the comparison itself,
/// separated from the file walk so it can be driven directly by tests.
///
/// `TEST-P0-01-07-A` clause 2 is why: against the committed tree this function
/// returns `Ok` for all 59 rows, so a green run says nothing about whether it
/// can reject. The tests supply the disagreements the tree no longer has.
fn compare_feature_story_row(
    feature: &str,
    story_id: &str,
    cell: &str,
    status: &ArtifactStatus,
) -> Result<(), String> {
    // Some tables bold the state and some do not; both are prose choices, so
    // the cell is normalised to the header shape rather than one of them being
    // declared wrong.
    let normalised = format!("Status: **{}", cell.trim_start_matches("**"));
    let cell_state = parse_status_line(&normalised)
        .map_err(|error| {
            format!(
                "{feature}: the Stories-table status for `{story_id}` (`{cell}`) does not open \
                 with a known state: {error}"
            )
        })?
        .0;
    if cell_state != status.state {
        return Err(format!(
            "{feature}: Stories table records `{story_id}` as `{cell_state}`, but that Story's \
             own `Status:` header says `{}`. The Story is authoritative about its own state \
             (LE-44)",
            status.state
        ));
    }

    let table_criteria = criterion_numbers(cell);
    let story_criteria = criterion_numbers(&status.detail);
    if table_criteria != story_criteria {
        return Err(format!(
            "{feature}: Stories table says `{story_id}` blocks on criteria {table_criteria:?}, \
             but that Story's own header says {story_criteria:?} (LE-44)"
        ));
    }
    Ok(())
}

fn validate_numbered_context_id(
    id: &str,
    prefix: &str,
    maximum: usize,
    line_number: usize,
    kind: &str,
) -> Result<(), String> {
    let Some(number) = id.strip_prefix(prefix) else {
        return Err(format!("{kind} line {line_number}: `{id}` is not a valid {prefix}NN id"));
    };
    if !is_two_digits(number) {
        return Err(format!("{kind} line {line_number}: `{id}` is not a valid {prefix}NN id"));
    }
    let number = number
        .parse::<usize>()
        .map_err(|error| format!("{kind} line {line_number}: invalid `{id}`: {error}"))?;
    if !(1..=maximum).contains(&number) {
        return Err(format!(
            "{kind} line {line_number}: `{id}` falls outside {prefix}01..{maximum:02}"
        ));
    }
    Ok(())
}

fn validate_complete_numbered_context(
    ids: &BTreeSet<String>,
    prefix: &str,
    expected_count: usize,
    kind: &str,
) -> Result<(), String> {
    for number in 1..=expected_count {
        let expected = format!("{prefix}{number:02}");
        if !ids.contains(&expected) {
            return Err(format!("missing {kind} `{expected}`"));
        }
    }
    if ids.len() != expected_count {
        return Err(format!("expected exactly {expected_count} {kind}s, found {}", ids.len()));
    }
    Ok(())
}

fn validate_context_horizon(value: &str, line_number: usize, kind: &str) -> Result<(), String> {
    if !matches!(value, "now" | "next" | "later" | "research") {
        return Err(format!("{kind} line {line_number}: unknown roadmap horizon `{value}`"));
    }
    Ok(())
}

fn validate_goal_list(value: &str, line_number: usize) -> Result<BTreeSet<String>, String> {
    let known_families = ["PA", "AI", "RC", "RT", "HW", "DX", "PC", "SEC", "APP"];
    let mut goals = BTreeSet::new();
    for goal in value.split(',') {
        let parts: Vec<&str> = goal.split('-').collect();
        if parts.len() != 3
            || parts[0] != "G"
            || !known_families.contains(&parts[1])
            || parts[2].is_empty()
            || !parts[2].bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(format!("landing zone line {line_number}: invalid goal id `{goal}`"));
        }
        if !goals.insert(goal.to_string()) {
            return Err(format!("landing zone line {line_number}: duplicate goal `{goal}`"));
        }
    }
    Ok(goals)
}

fn validate_domain_list(
    value: &str,
    line_number: usize,
    kind: &str,
) -> Result<BTreeSet<String>, String> {
    let mut domains = BTreeSet::new();
    for domain in value.split(',') {
        validate_domain_id(domain, line_number)?;
        if !domains.insert(domain.to_string()) {
            return Err(format!("{kind} line {line_number}: duplicate domain `{domain}`"));
        }
    }
    Ok(domains)
}

fn validate_application_list(
    value: &str,
    line_number: usize,
    applications: &BTreeSet<String>,
) -> Result<BTreeSet<String>, String> {
    let mut selected = BTreeSet::new();
    for application in value.split(',') {
        validate_numbered_context_id(
            application,
            "APP-",
            APPLICATION_PLATFORM_COUNT,
            line_number,
            "landing zone",
        )?;
        if !applications.contains(application) {
            return Err(format!(
                "landing zone line {line_number}: unknown application platform `{application}`"
            ));
        }
        if !selected.insert(application.to_string()) {
            return Err(format!(
                "landing zone line {line_number}: duplicate application platform `{application}`"
            ));
        }
    }
    Ok(selected)
}

fn validate_numbered_charter_id(
    id: &str,
    prefix: &str,
    maximum: usize,
    line_number: usize,
) -> Result<(), String> {
    let Some(number) = id.strip_prefix(prefix) else {
        return Err(format!("line {line_number}: `{id}` is not a valid {prefix}NN id"));
    };
    if !is_two_digits(number) {
        return Err(format!("line {line_number}: `{id}` is not a valid {prefix}NN id"));
    }
    let number = number
        .parse::<usize>()
        .map_err(|error| format!("line {line_number}: invalid `{id}`: {error}"))?;
    if !(1..=maximum).contains(&number) {
        return Err(format!("line {line_number}: `{id}` falls outside {prefix}01..{maximum:02}"));
    }
    Ok(())
}

fn validate_complete_numbered_charter(
    ids: &BTreeSet<String>,
    prefix: &str,
    expected_count: usize,
    kind: &str,
) -> Result<(), String> {
    for number in 1..=expected_count {
        let expected = format!("{prefix}{number:02}");
        if !ids.contains(&expected) {
            return Err(format!("missing {kind} `{expected}`"));
        }
    }
    if ids.len() != expected_count {
        return Err(format!("expected exactly {expected_count} {kind}s, found {}", ids.len()));
    }
    Ok(())
}

fn validate_boundary_list(
    value: &str,
    line_number: usize,
    boundary_tests: &BTreeSet<String>,
    kind: &str,
) -> Result<BTreeSet<String>, String> {
    let mut selected = BTreeSet::new();
    for boundary in value.split(',') {
        validate_boundary_test_id(boundary, line_number)?;
        if !boundary_tests.contains(boundary) {
            return Err(format!("{kind} line {line_number}: unknown boundary test `{boundary}`"));
        }
        if !selected.insert(boundary.to_string()) {
            return Err(format!("{kind} line {line_number}: duplicate boundary test `{boundary}`"));
        }
    }
    Ok(selected)
}

fn validate_numbered_contract_list(
    value: &str,
    line_number: usize,
    prefix: &str,
    maximum: usize,
    catalogue: &BTreeSet<String>,
    kind: &str,
) -> Result<BTreeSet<String>, String> {
    let mut selected = BTreeSet::new();
    for id in value.split(',') {
        validate_numbered_charter_id(id, prefix, maximum, line_number)?;
        if !catalogue.contains(id) {
            return Err(format!("Feature contract line {line_number}: unknown {kind} `{id}`"));
        }
        if !selected.insert(id.to_string()) {
            return Err(format!("Feature contract line {line_number}: duplicate {kind} `{id}`"));
        }
    }
    Ok(selected)
}

fn validate_performance_guardrail_list(
    value: &str,
    line_number: usize,
    kind: &str,
) -> Result<(), String> {
    let mut selected = BTreeSet::new();
    for guardrail in value.split(',') {
        validate_performance_guardrail_id(guardrail, line_number)?;
        if !selected.insert(guardrail) {
            return Err(format!(
                "{kind} line {line_number}: duplicate performance guardrail `{guardrail}`"
            ));
        }
    }
    Ok(())
}

fn validate_story_contracts(
    contents: &str,
    security: &SecurityIndex,
    containment_classes: &BTreeSet<String>,
    classes_by_feature: &BTreeMap<String, BTreeSet<String>>,
) -> Result<ContractIndex, String> {
    let mut lines = contents.lines();
    let header = lines
        .next()
        .ok_or_else(|| "Story contract catalogue is empty".to_string())?
        .trim_end_matches('\r');
    if header != CONTRACT_HEADER {
        return Err(format!("unexpected contract header; expected exactly `{CONTRACT_HEADER}`"));
    }

    let mut stories = BTreeSet::new();
    let mut features = BTreeSet::new();
    let mut details_by_story = BTreeMap::new();
    let mut selected_performance_contracts = 0usize;

    for (zero_based_index, raw_line) in lines.enumerate() {
        let line_number = zero_based_index + 2;
        let fields = non_empty_tsv_fields(raw_line, line_number, CONTRACT_FIELD_COUNT, "contract")?;

        let story_id = fields[0];
        let feature_id = fields[1];
        let expected_feature = feature_for_story(story_id, line_number)?;
        if feature_id != expected_feature {
            return Err(format!(
                "contract line {line_number}: Story `{story_id}` belongs to `{expected_feature}`, not `{feature_id}`"
            ));
        }
        if !stories.insert(story_id.to_string()) {
            return Err(format!("contract line {line_number}: duplicate Story `{story_id}`"));
        }
        features.insert(feature_id.to_string());

        let mut domains = BTreeSet::new();
        for domain in fields[2].split(',') {
            validate_domain_id(domain, line_number)?;
            if !domains.insert(domain.to_string()) {
                return Err(format!(
                    "contract line {line_number}: duplicate performance domain `{domain}`"
                ));
            }
        }
        selected_performance_contracts = selected_performance_contracts
            .checked_add(domains.len() * PERFORMANCE_GUARDRAILS_PER_DOMAIN)
            .ok_or_else(|| "selected performance contract count overflowed".to_string())?;

        let row_controls = validate_security_list(fields[3], line_number, &security.controls)?;
        let row_classes = validate_containment_list(fields[4], line_number, containment_classes)?;
        let feature_classes = classes_by_feature.get(feature_id).ok_or_else(|| {
            format!(
                "contract line {line_number}: Feature `{feature_id}` has no containment contract"
            )
        })?;
        let classes_outside_feature: Vec<&String> =
            row_classes.difference(feature_classes).collect();
        if !classes_outside_feature.is_empty() {
            return Err(format!(
                "contract line {line_number}: Story `{story_id}` selects classes outside Feature `{feature_id}`: {}",
                join_ids(&classes_outside_feature)
            ));
        }
        for control in &row_controls {
            let control_classes = security
                .classes_by_control
                .get(control)
                .ok_or_else(|| format!("missing class mapping for security control `{control}`"))?;
            if row_classes.is_disjoint(control_classes) {
                return Err(format!(
                    "contract line {line_number}: security control `{control}` applies to none of Story `{story_id}`'s containment classes"
                ));
            }
        }

        if !matches!(fields[5], "specified" | "baseline-debt" | "verified") {
            return Err(format!(
                "contract line {line_number}: state must be `specified`, `baseline-debt`, or `verified`, found `{}`",
                fields[5]
            ));
        }
        details_by_story.insert(
            story_id.to_string(),
            StoryContract {
                feature_id: feature_id.to_string(),
                performance_domains: domains,
                security_controls: row_controls,
                containment_classes: row_classes,
                state: fields[5].to_string(),
            },
        );
    }

    Ok(ContractIndex { stories, features, details_by_story, selected_performance_contracts })
}

fn feature_for_story(story_id: &str, line_number: usize) -> Result<String, String> {
    let parts: Vec<&str> = story_id.split('-').collect();
    if parts.len() != 4
        || parts[0] != "STORY"
        || !is_phase_id(parts[1])
        || !is_two_digits(parts[2])
        || !is_two_digits(parts[3])
    {
        return Err(format!(
            "contract line {line_number}: `{story_id}` is not a valid STORY-Pn-NN-NN id"
        ));
    }
    Ok(format!("FEAT-{}-{}", parts[1], parts[2]))
}

fn validate_feature_id(feature_id: &str, line_number: usize) -> Result<(), String> {
    let parts: Vec<&str> = feature_id.split('-').collect();
    if parts.len() != 3 || parts[0] != "FEAT" || !is_phase_id(parts[1]) || !is_two_digits(parts[2])
    {
        return Err(format!(
            "Feature contract line {line_number}: `{feature_id}` is not a valid FEAT-Pn-NN id"
        ));
    }
    Ok(())
}

fn validate_containment_id(class: &str, line_number: usize) -> Result<(), String> {
    let bytes = class.as_bytes();
    if bytes.len() != 2
        || bytes[0] != b'C'
        || !bytes[1].is_ascii_digit()
        || usize::from(bytes[1] - b'0') >= CONTAINMENT_CLASS_COUNT
    {
        return Err(format!(
            "line {line_number}: `{class}` is not a valid C0..C4 containment class"
        ));
    }
    Ok(())
}

fn validate_boundary_test_id(test: &str, line_number: usize) -> Result<(), String> {
    let Some(number) = test.strip_prefix("BND-") else {
        return Err(format!("line {line_number}: `{test}` is not a valid BND-01..BND-20 id"));
    };
    if !is_two_digits(number) {
        return Err(format!("line {line_number}: `{test}` is not a valid BND-01..BND-20 id"));
    }
    let number = number
        .parse::<usize>()
        .map_err(|error| format!("line {line_number}: invalid `{test}`: {error}"))?;
    if !(1..=BOUNDARY_TEST_COUNT).contains(&number) {
        return Err(format!("line {line_number}: `{test}` falls outside BND-01..BND-20"));
    }
    Ok(())
}

fn validate_domain_id(domain: &str, line_number: usize) -> Result<(), String> {
    let bytes = domain.as_bytes();
    if bytes.len() != 3 || bytes[0] != b'D' || !bytes[1..].iter().all(u8::is_ascii_digit) {
        return Err(format!("contract line {line_number}: `{domain}` is not a valid D01..D25 id"));
    }
    let number = domain[1..]
        .parse::<usize>()
        .map_err(|error| format!("contract line {line_number}: invalid `{domain}`: {error}"))?;
    if !(1..=25).contains(&number) {
        return Err(format!("contract line {line_number}: `{domain}` falls outside D01..D25"));
    }
    Ok(())
}

fn validate_performance_guardrail_id(guardrail: &str, line_number: usize) -> Result<(), String> {
    let parts: Vec<&str> = guardrail.split('-').collect();
    if parts.len() != 3 || parts[0] != "PERF" {
        return Err(format!(
            "boundary-test line {line_number}: `{guardrail}` is not a valid PERF-Dnn-Gnn id"
        ));
    }
    validate_domain_id(parts[1], line_number)?;
    let Some(guardrail_number) = parts[2].strip_prefix('G') else {
        return Err(format!(
            "boundary-test line {line_number}: `{guardrail}` is not a valid PERF-Dnn-Gnn id"
        ));
    };
    if !is_two_digits(guardrail_number) {
        return Err(format!(
            "boundary-test line {line_number}: `{guardrail}` is not a valid PERF-Dnn-Gnn id"
        ));
    }
    let number = guardrail_number.parse::<usize>().map_err(|error| {
        format!("boundary-test line {line_number}: invalid `{guardrail}`: {error}")
    })?;
    if !(1..=PERFORMANCE_GUARDRAILS_PER_DOMAIN).contains(&number) {
        return Err(format!(
            "boundary-test line {line_number}: `{guardrail}` falls outside G01..G25"
        ));
    }
    Ok(())
}

fn validate_security_id(control: &str, line_number: usize) -> Result<(), String> {
    let Some(number) = control.strip_prefix("SEC-") else {
        return Err(format!("line {line_number}: `{control}` is not a valid SEC-01..SEC-20 id"));
    };
    if !is_two_digits(number) {
        return Err(format!("line {line_number}: `{control}` is not a valid SEC-01..SEC-20 id"));
    }
    let number = number
        .parse::<usize>()
        .map_err(|error| format!("line {line_number}: invalid `{control}`: {error}"))?;
    if !(1..=SECURITY_CONTROL_COUNT).contains(&number) {
        return Err(format!("line {line_number}: `{control}` falls outside SEC-01..SEC-20"));
    }
    Ok(())
}

fn validate_containment_list(
    value: &str,
    line_number: usize,
    containment_classes: &BTreeSet<String>,
) -> Result<BTreeSet<String>, String> {
    let mut classes = BTreeSet::new();
    for class in value.split(',') {
        validate_containment_id(class, line_number)?;
        if !containment_classes.contains(class) {
            return Err(format!("line {line_number}: unknown containment class `{class}`"));
        }
        if !classes.insert(class.to_string()) {
            return Err(format!("line {line_number}: duplicate containment class `{class}`"));
        }
    }
    Ok(classes)
}

fn validate_security_list(
    value: &str,
    line_number: usize,
    security_controls: &BTreeSet<String>,
) -> Result<BTreeSet<String>, String> {
    let mut controls = BTreeSet::new();
    for control in value.split(',') {
        validate_security_id(control, line_number)?;
        if !security_controls.contains(control) {
            return Err(format!("line {line_number}: unknown security control `{control}`"));
        }
        if !controls.insert(control.to_string()) {
            return Err(format!("line {line_number}: duplicate security control `{control}`"));
        }
    }
    Ok(controls)
}

fn non_empty_tsv_fields<'a>(
    raw_line: &'a str,
    line_number: usize,
    expected_count: usize,
    kind: &str,
) -> Result<Vec<&'a str>, String> {
    let line = raw_line.trim_end_matches('\r');
    if line.is_empty() {
        return Err(format!("{kind} line {line_number}: blank rows are not allowed"));
    }
    let fields: Vec<&str> = line.split('\t').collect();
    if fields.len() != expected_count {
        return Err(format!(
            "{kind} line {line_number}: expected {expected_count} fields, found {}",
            fields.len()
        ));
    }
    if fields.iter().any(|field| field.trim().is_empty()) {
        return Err(format!("{kind} line {line_number}: every field must be non-empty"));
    }
    Ok(fields)
}

fn is_phase_id(value: &str) -> bool {
    value.strip_prefix('P').is_some_and(|suffix| {
        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
    })
}

fn is_two_digits(value: &str) -> bool {
    value.len() == 2 && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn markdown_ids(directory: &Path, prefix: &str) -> Result<BTreeSet<String>, String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
    let mut ids = BTreeSet::new();
    for entry in entries {
        let entry = entry
            .map_err(|error| format!("failed to enumerate {}: {error}", directory.display()))?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if stem.starts_with(prefix) {
            ids.insert(stem.to_string());
        }
    }
    Ok(ids)
}

fn compare_exact_coverage(
    kind: &str,
    files: &BTreeSet<String>,
    contracts: &BTreeSet<String>,
) -> Result<(), String> {
    let missing: Vec<&String> = files.difference(contracts).collect();
    if !missing.is_empty() {
        return Err(format!("{kind} files missing assurance contracts: {}", join_ids(&missing)));
    }
    let stale: Vec<&String> = contracts.difference(files).collect();
    if !stale.is_empty() {
        return Err(format!(
            "assurance contracts reference nonexistent {kind} files: {}",
            join_ids(&stale)
        ));
    }
    Ok(())
}

fn validate_test_coverage(
    test_dir: &Path,
    tests: &BTreeSet<String>,
    contracts: &ContractIndex,
    feature_contracts: &FeatureContractIndex,
) -> Result<(), String> {
    for test in tests {
        let parts: Vec<&str> = test.split('-').collect();
        if parts.len() != 5
            || parts[0] != "TEST"
            || !is_phase_id(parts[1])
            || !is_two_digits(parts[2])
            || !is_two_digits(parts[3])
            || parts[4].len() != 1
            || !parts[4].bytes().all(|byte| byte.is_ascii_uppercase())
        {
            return Err(format!("`{test}` is not a valid TEST-Pn-NN-NN-A id"));
        }
        let story = format!("STORY-{}-{}-{}", parts[1], parts[2], parts[3]);
        let Some(contract) = contracts.details_by_story.get(&story) else {
            return Err(format!("Test `{test}` has no mapped Story `{story}`"));
        };
        let path = test_dir.join(format!("{test}.md"));
        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        if !contents.lines().any(|line| {
            line.starts_with("Assurance contract:")
                && line.contains("goals/assurance/story-contracts.tsv")
        }) {
            return Err(format!(
                "Test `{test}` has no `Assurance contract:` link to story-contracts.tsv"
            ));
        }
        validate_test_metadata(
            test,
            &contents,
            "Performance domains:",
            &contract.performance_domains,
        )?;
        validate_test_metadata(test, &contents, "Security controls:", &contract.security_controls)?;
        validate_test_metadata(
            test,
            &contents,
            "Containment classes:",
            &contract.containment_classes,
        )?;
        let expected_boundary_tests = feature_contracts
            .boundary_tests_by_feature
            .get(&contract.feature_id)
            .ok_or_else(|| {
                format!(
                    "Test `{test}`'s Feature `{}` has no boundary-test contract",
                    contract.feature_id
                )
            })?;
        validate_test_metadata(test, &contents, "Boundary tests:", expected_boundary_tests)?;
        let expected_protection_domains = feature_contracts
            .protection_domains_by_feature
            .get(&contract.feature_id)
            .ok_or_else(|| {
                format!(
                    "Test `{test}`'s Feature `{}` has no Protection Domain contract",
                    contract.feature_id
                )
            })?;
        validate_test_metadata(
            test,
            &contents,
            "Protection Domain contracts:",
            expected_protection_domains,
        )?;
        let expected_code_admission = feature_contracts
            .code_admission_by_feature
            .get(&contract.feature_id)
            .ok_or_else(|| {
                format!(
                    "Test `{test}`'s Feature `{}` has no code-admission contract",
                    contract.feature_id
                )
            })?;
        validate_test_metadata(test, &contents, "Code admission gates:", expected_code_admission)?;
        validate_test_metadata(
            test,
            &contents,
            "Assurance state:",
            &BTreeSet::from([contract.state.clone()]),
        )?;
    }
    Ok(())
}

fn validate_test_metadata(
    test: &str,
    contents: &str,
    field: &str,
    expected: &BTreeSet<String>,
) -> Result<(), String> {
    let line = contents
        .lines()
        .find(|line| line.starts_with(field))
        .ok_or_else(|| format!("Test `{test}` has no `{field}` field"))?;
    let mut actual = BTreeSet::new();
    for (index, value) in line.split('`').enumerate() {
        if index % 2 == 1 && !value.is_empty() && !actual.insert(value.to_string()) {
            return Err(format!("Test `{test}` repeats `{value}` in its `{field}` field"));
        }
    }
    if &actual != expected {
        return Err(format!(
            "Test `{test}` `{field}` metadata does not match its assurance contract; expected {}, found {}",
            join_owned_ids(expected),
            join_owned_ids(&actual)
        ));
    }
    Ok(())
}

fn validate_report_coverage(
    report_dir: &Path,
    reports: &BTreeSet<String>,
    tests: &BTreeSet<String>,
    stories: &BTreeSet<String>,
) -> Result<(), String> {
    for report in reports {
        let path = report_dir.join(format!("{report}.md"));
        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        let covered = contents
            .lines()
            .find(|line| line.contains("Test(s) covered:"))
            .ok_or_else(|| format!("Report `{report}` has no `Test(s) covered:` field"))?;
        let names_a_test = tests.iter().any(|test| covered.contains(test));
        let names_a_story = stories.iter().any(|story| covered.contains(story));
        if !names_a_test && !names_a_story {
            return Err(format!(
                "Report `{report}`'s `Test(s) covered:` field references no mapped Story or Test"
            ));
        }
    }
    Ok(())
}

fn join_ids(ids: &[&String]) -> String {
    ids.iter().map(|id| id.as_str()).collect::<Vec<_>>().join(", ")
}

fn join_owned_ids(ids: &BTreeSet<String>) -> String {
    ids.iter().map(String::as_str).collect::<Vec<_>>().join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn containment_fixture() -> String {
        let mut fixture = String::from(CONTAINMENT_HEADER);
        fixture.push('\n');
        for number in 0..CONTAINMENT_CLASS_COUNT {
            fixture.push_str(&format!(
                "C{number}\tname\tpurpose\tauthority\tinput\tfailure\tevidence\n"
            ));
        }
        fixture
    }

    fn security_fixture() -> String {
        let mut fixture = String::from(SECURITY_HEADER);
        fixture.push('\n');
        for number in 1..=SECURITY_CONTROL_COUNT {
            fixture.push_str(&format!(
                "SEC-{number:02}\tcontrol\tthreat\tinvariant\tC0\tevidence\tP0\trelease\n"
            ));
        }
        fixture
    }

    fn boundary_test_fixture() -> String {
        let mut fixture = String::from(BOUNDARY_TEST_HEADER);
        fixture.push('\n');
        for number in 1..=BOUNDARY_TEST_COUNT {
            fixture.push_str(&format!(
                "BND-{number:02}\tC0\tobjective\tattack\tsuccess\tSEC-01\tPERF-D01-G01\n"
            ));
        }
        fixture
    }

    fn application_platform_fixture() -> String {
        let mut fixture = String::from(APPLICATION_PLATFORM_HEADER);
        fixture.push('\n');
        for number in 1..=APPLICATION_PLATFORM_COUNT {
            fixture.push_str(&format!(
                "APP-{number:02}\tname\tframework\tnative-txe\tlater\texecution\tC3\tD25\tSEC-04\tnetwork\tpolicy\tevidence\n"
            ));
        }
        fixture
    }

    fn landing_zone_fixture() -> String {
        let mut fixture = String::from(LANDING_ZONE_HEADER);
        fixture.push('\n');
        for number in 1..=LANDING_ZONE_COUNT {
            let applications = if number == 1 {
                (1..=APPLICATION_PLATFORM_COUNT)
                    .map(|application| format!("APP-{application:02}"))
                    .collect::<Vec<_>>()
                    .join(",")
            } else {
                format!("APP-{number:02}")
            };
            fixture.push_str(&format!(
                "LZ-{number:02}\tname\toutcome\tlater\tG-APP-01\tD25\t{applications}\tSEC-04\tC3\tclaim\n"
            ));
        }
        fixture
    }

    #[test]
    fn complete_containment_catalogue_passes() {
        let classes = validate_containment_classes(&containment_fixture())
            .expect("complete containment classes are valid");
        assert_eq!(classes.len(), CONTAINMENT_CLASS_COUNT);
    }

    #[test]
    fn missing_containment_class_fails() {
        let fixture = containment_fixture()
            .replace("C4\tname\tpurpose\tauthority\tinput\tfailure\tevidence\n", "");
        let error = validate_containment_classes(&fixture)
            .expect_err("missing containment class must fail");
        assert!(error.contains("C4"));
    }

    #[test]
    fn complete_security_catalogue_passes() {
        let classes = validate_containment_classes(&containment_fixture())
            .expect("complete containment classes are valid");
        let security = validate_security_controls(&security_fixture(), &classes)
            .expect("complete controls are valid");
        assert_eq!(security.controls.len(), SECURITY_CONTROL_COUNT);
    }

    #[test]
    fn missing_security_control_fails() {
        let classes = validate_containment_classes(&containment_fixture())
            .expect("complete containment classes are valid");
        let fixture = security_fixture()
            .replace("SEC-20\tcontrol\tthreat\tinvariant\tC0\tevidence\tP0\trelease\n", "");
        let error =
            validate_security_controls(&fixture, &classes).expect_err("missing control must fail");
        assert!(error.contains("SEC-20"));
    }

    #[test]
    fn missing_boundary_test_fails() {
        let classes = validate_containment_classes(&containment_fixture())
            .expect("complete containment classes are valid");
        let security = validate_security_controls(&security_fixture(), &classes)
            .expect("complete controls are valid");
        let fixture = boundary_test_fixture()
            .replace("BND-20\tC0\tobjective\tattack\tsuccess\tSEC-01\tPERF-D01-G01\n", "");
        let error = validate_boundary_tests(&fixture, &classes, &security.controls)
            .expect_err("missing boundary test must fail");
        assert!(error.contains("BND-20"));
    }

    #[test]
    fn complete_protection_domain_catalogue_passes() {
        let classes = validate_containment_classes(&containment_fixture())
            .expect("complete containment classes are valid");
        let security = validate_security_controls(&security_fixture(), &classes)
            .expect("complete controls are valid");
        let boundary_tests =
            validate_boundary_tests(&boundary_test_fixture(), &classes, &security.controls)
                .expect("complete boundary tests are valid");
        let mut fixture = String::from(
            "id\tinvariant\tscope\tenforcement\tfailure_rule\tsecurity_controls\tboundary_tests\tperformance_guardrails\n",
        );
        for number in 1..=14 {
            fixture.push_str(&format!(
                "PD-{number:02}\tinvariant\tC0\tenforcement\tfailure\tSEC-01\tBND-01\tPERF-D01-G01\n"
            ));
        }

        let contracts = validate_protection_domain_contracts(
            &fixture,
            &classes,
            &security.controls,
            &boundary_tests,
        )
        .expect("complete Protection Domain catalogue is valid");
        assert_eq!(contracts.len(), 14);
    }

    #[test]
    fn missing_code_admission_gate_fails() {
        let classes = validate_containment_classes(&containment_fixture())
            .expect("complete containment classes are valid");
        let security = validate_security_controls(&security_fixture(), &classes)
            .expect("complete controls are valid");
        let boundary_tests =
            validate_boundary_tests(&boundary_test_fixture(), &classes, &security.controls)
                .expect("complete boundary tests are valid");
        let mut fixture = String::from(
            "id\tstage\tinput_classes\toutput_classes\tmandatory_check\tfailure_rule\tsecurity_controls\tboundary_tests\tperformance_guardrails\n",
        );
        for number in 1..14 {
            fixture.push_str(&format!(
                "RCG-{number:02}\tstage\tC0\tC0\tcheck\tfailure\tSEC-01\tBND-01\tPERF-D01-G01\n"
            ));
        }

        let error =
            validate_code_admission_gates(&fixture, &classes, &security.controls, &boundary_tests)
                .expect_err("missing RCG-14 must fail");
        assert!(error.contains("RCG-14"));
    }

    #[test]
    fn class_communication_matrix_requires_every_ordered_pair() {
        let classes = validate_containment_classes(&containment_fixture())
            .expect("complete containment classes are valid");
        let security = validate_security_controls(&security_fixture(), &classes)
            .expect("complete controls are valid");
        let boundary_tests =
            validate_boundary_tests(&boundary_test_fixture(), &classes, &security.controls)
                .expect("complete boundary tests are valid");
        let mut fixture = String::from(
            "source\ttarget\tdecision\tpath\tauthority_transfer\tfailure_rule\tboundary_tests\n",
        );
        for source in 0..CONTAINMENT_CLASS_COUNT {
            for target in 0..CONTAINMENT_CLASS_COUNT {
                if source == 4 && target == 4 {
                    continue;
                }
                let (decision, authority) = if source == 0 && target == 1 {
                    ("handoff", "boot-state-only")
                } else if source == 0 || target == 0 {
                    ("deny", "none")
                } else if source == 1 && target == 1 {
                    ("internal", "none")
                } else {
                    ("mediated", "none")
                };
                fixture.push_str(&format!(
                    "C{source}\tC{target}\t{decision}\tpath\t{authority}\tfailure\tBND-01\n"
                ));
            }
        }

        let error = validate_class_communication_matrix(&fixture, &classes, &boundary_tests)
            .expect_err("missing C4 to C4 pair must fail");
        assert!(error.contains("C4->C4"));
    }

    #[test]
    fn complete_application_and_landing_catalogues_pass() {
        let classes = validate_containment_classes(&containment_fixture())
            .expect("complete containment classes are valid");
        let security = validate_security_controls(&security_fixture(), &classes)
            .expect("complete controls are valid");
        let applications = validate_application_platforms(
            &application_platform_fixture(),
            &classes,
            &security.controls,
        )
        .expect("complete application catalogue is valid");
        let landing_zones = validate_landing_zones(
            &landing_zone_fixture(),
            &applications,
            &classes,
            &security.controls,
        )
        .expect("complete landing-zone catalogue is valid");

        assert_eq!(applications.len(), APPLICATION_PLATFORM_COUNT);
        assert_eq!(landing_zones.len(), LANDING_ZONE_COUNT);
    }

    #[test]
    fn landing_zone_rejects_unknown_application() {
        let classes = validate_containment_classes(&containment_fixture())
            .expect("complete containment classes are valid");
        let security = validate_security_controls(&security_fixture(), &classes)
            .expect("complete controls are valid");
        let applications = validate_application_platforms(
            &application_platform_fixture(),
            &classes,
            &security.controls,
        )
        .expect("complete application catalogue is valid");
        let fixture = landing_zone_fixture().replace("APP-01", "APP-20");
        let error = validate_landing_zones(&fixture, &applications, &classes, &security.controls)
            .expect_err("unknown application target must fail");
        assert!(error.contains("APP-20"));
    }

    #[test]
    fn malformed_contract_reference_fails() {
        let classes = validate_containment_classes(&containment_fixture())
            .expect("complete containment classes are valid");
        let security = validate_security_controls(&security_fixture(), &classes)
            .expect("complete controls are valid");
        let feature_classes =
            BTreeMap::from([("FEAT-P0-01".to_string(), BTreeSet::from(["C0".to_string()]))]);
        let fixture = format!(
            "{CONTRACT_HEADER}\nSTORY-P0-01-01\tFEAT-P0-01\tD26\tSEC-01\tC0\tspecified\trationale\n"
        );
        let error = validate_story_contracts(&fixture, &security, &classes, &feature_classes)
            .expect_err("unknown domain must fail");
        assert!(error.contains("D26"));
    }

    /// The closed catalogues, asserted exactly.
    ///
    /// Each of these is fixed by a charter document rather than by how much work
    /// has landed: five containment classes, twenty boundary tests, twenty
    /// security controls, fourteen Protection Domain contracts, fourteen
    /// code-admission gates, the complete 5x5 class matrix, nineteen
    /// application/platform targets, nine landing zones. Changing one is a
    /// deliberate charter amendment, and this test **should** fail when it
    /// happens — that is the whole point of pinning them.
    #[test]
    fn committed_assurance_spine_catalogues_are_exact() {
        let summary = committed_summary();
        assert_eq!(summary.containment_class_count, 5);
        assert_eq!(summary.boundary_test_count, 20);
        assert_eq!(summary.security_control_count, 20);
        assert_eq!(summary.protection_domain_contract_count, 14);
        assert_eq!(summary.code_admission_gate_count, 14);
        assert_eq!(summary.class_communication_pair_count, 25);
        assert_eq!(summary.application_platform_count, 19);
        assert_eq!(summary.landing_zone_count, 9);
        assert!(summary.selected_performance_contracts >= 625);
        assert!(summary.selected_application_performance_contracts >= 625);
    }

    /// How much work has landed, asserted as **floors and relationships** — never
    /// as totals.
    ///
    /// These counts grow with every Story, so an exact total is a number that
    /// must be re-synced by every change that adds a document, including changes
    /// made concurrently in another working tree. That churned five times in the
    /// session of 2026-07-28 and broke `main` once, because the symptom was
    /// treated each time and the pattern was not named. It is named here: **a
    /// population count is a floor, not a total.**
    ///
    /// The floors still catch the failure that matters — documents are added,
    /// never deleted, so a shrinking count means an artifact was lost or a
    /// contract row was dropped. Raise a floor deliberately when a milestone is
    /// worth pinning; do not raise it reflexively to match today's tree.
    #[test]
    fn committed_assurance_spine_population_never_shrinks() {
        let summary = committed_summary();

        // Floors as of 2026-07-28 (FEAT-P1-07 specified, STORY-P0-01-04 Verified).
        assert!(summary.feature_count >= 23, "features: {}", summary.feature_count);
        assert!(summary.story_count >= 56, "stories: {}", summary.story_count);
        assert!(summary.test_count >= 43, "tests: {}", summary.test_count);
        assert!(summary.report_count >= 44, "reports: {}", summary.report_count);
        assert!(summary.loose_end_count >= 27, "loose ends: {}", summary.loose_end_count);

        // Relationships that must hold at any size. Every Feature is decomposed
        // into at least one Story, and a loose end cannot be open unless it
        // exists — the second is trivially true of the register's own parser and
        // is asserted so that a future change to how open-ness is counted cannot
        // quietly invert it.
        assert!(summary.story_count >= summary.feature_count);
        assert!(summary.open_loose_end_count <= summary.loose_end_count);
    }

    fn committed_summary() -> AssuranceSummary {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("xtask manifest lives at os/src/xtask")
            .to_path_buf();
        check_assurance_spine(&repo_root).expect("committed assurance spine must be valid")
    }

    fn evidence_contracts() -> ContractIndex {
        let fixture = format!(
            "{CONTRACT_HEADER}\n\
             STORY-P0-01-01\tFEAT-P0-01\tD01\tSEC-19\tC0\tbaseline-debt\trationale\n\
             STORY-P0-02-01\tFEAT-P0-02\tD05\tSEC-19\tC0\tbaseline-debt\trationale\n"
        );
        let security = SecurityIndex {
            controls: BTreeSet::from(["SEC-19".to_string()]),
            classes_by_control: BTreeMap::from([(
                "SEC-19".to_string(),
                BTreeSet::from(["C0".to_string()]),
            )]),
        };
        let classes = BTreeSet::from(["C0".to_string()]);
        let feature_classes = BTreeMap::from([
            ("FEAT-P0-01".to_string(), BTreeSet::from(["C0".to_string()])),
            ("FEAT-P0-02".to_string(), BTreeSet::from(["C0".to_string()])),
        ]);
        validate_story_contracts(&fixture, &security, &classes, &feature_classes)
            .expect("fixture contracts are valid")
    }

    fn evidence_row(guardrail: &str, domain: &str, story: &str) -> String {
        format!("{guardrail}\t{domain}\t{story}\tstructural\tpath\t2026-07-28\tnote\n")
    }

    // ---- `TEST-P0-01-07-A` clause 3: `LE-35`, the open-debt rule -----------
    //
    // Every test here is a positive control. Against the committed tree the
    // debt register is complete, so `check-assurance-spine` returning green
    // proves nothing about whether these refusals work.

    /// Two Stories: one selecting an implemented domain, one selecting a
    /// design-readiness domain that therefore owes stated debt.
    fn debt_contracts() -> ContractIndex {
        let fixture = format!(
            "{CONTRACT_HEADER}\n\
             STORY-P0-01-01\tFEAT-P0-01\tD01\tSEC-19\tC0\tbaseline-debt\trationale\n\
             STORY-P0-02-01\tFEAT-P0-02\tD05\tSEC-19\tC0\tbaseline-debt\trationale\n"
        );
        let security = SecurityIndex {
            controls: BTreeSet::from(["SEC-19".to_string()]),
            classes_by_control: BTreeMap::from([(
                "SEC-19".to_string(),
                BTreeSet::from(["C0".to_string()]),
            )]),
        };
        let classes = BTreeSet::from(["C0".to_string()]);
        let feature_classes = BTreeMap::from([
            ("FEAT-P0-01".to_string(), BTreeSet::from(["C0".to_string()])),
            ("FEAT-P0-02".to_string(), BTreeSet::from(["C0".to_string()])),
        ]);
        validate_story_contracts(&fixture, &security, &classes, &feature_classes)
            .expect("fixture contracts are valid")
    }

    /// `D01` is built; `D05` stands in here for a subsystem that is not.
    fn debt_readiness() -> BTreeMap<String, String> {
        BTreeMap::from([
            ("D01".to_string(), "prototype".to_string()),
            ("D05".to_string(), "design".to_string()),
        ])
    }

    fn debt_register(rows: &str) -> String {
        format!("{OPEN_DEBT_HEADER}\n{rows}")
    }

    #[test]
    fn selecting_a_design_readiness_domain_without_stating_debt_is_refused() {
        let debt = validate_open_debt(
            &debt_register(""),
            &debt_contracts(),
            &debt_readiness(),
            &BTreeSet::new(),
        )
        .expect("an empty register is well-formed");
        let error = validate_open_debt_coverage(&debt_contracts(), &debt_readiness(), &debt)
            .expect_err("an undeclared design selection must be refused");
        assert!(error.contains("STORY-P0-02-01"), "{error}");
        assert!(error.contains("readiness is `design`"), "{error}");
    }

    /// The acceptance case. Without it every refusal below would also pass
    /// against a validator that refused unconditionally.
    #[test]
    fn a_design_readiness_selection_with_a_matching_debt_row_is_accepted() {
        let rows = "STORY-P0-02-01\tD05\tdesign\tthe subsystem does not exist\t2026-07-28\n";
        let debt = validate_open_debt(
            &debt_register(rows),
            &debt_contracts(),
            &debt_readiness(),
            &BTreeSet::new(),
        )
        .expect("a matching debt row is exactly what the rule asks for");
        assert_eq!(debt.len(), 1);
        validate_open_debt_coverage(&debt_contracts(), &debt_readiness(), &debt)
            .expect("coverage is complete");
    }

    #[test]
    fn debt_recorded_against_an_implemented_domain_is_refused() {
        let rows = "STORY-P0-01-01\tD01\tprototype\tnot really debt\t2026-07-28\n";
        let error = validate_open_debt(
            &debt_register(rows),
            &debt_contracts(),
            &debt_readiness(),
            &BTreeSet::new(),
        )
        .expect_err("debt may not excuse a real obligation");
        assert!(error.contains("may not excuse one that does"), "{error}");
    }

    #[test]
    fn a_debt_row_disagreeing_with_the_catalogue_readiness_is_refused() {
        let rows = "STORY-P0-02-01\tD05\tunbuilt\twrong readiness\t2026-07-28\n";
        let error = validate_open_debt(
            &debt_register(rows),
            &debt_contracts(),
            &debt_readiness(),
            &BTreeSet::new(),
        )
        .expect_err("a drifted readiness must be refused");
        assert!(error.contains("the catalogue says `design`"), "{error}");
    }

    #[test]
    fn debt_in_a_domain_the_story_does_not_select_is_refused() {
        let rows = "STORY-P0-01-01\tD05\tdesign\tnot selected\t2026-07-28\n";
        let error = validate_open_debt(
            &debt_register(rows),
            &debt_contracts(),
            &debt_readiness(),
            &BTreeSet::new(),
        )
        .expect_err("debt in an unselected domain must be refused");
        assert!(error.contains("its contract selects"), "{error}");
    }

    #[test]
    fn a_pair_that_is_both_open_debt_and_evidenced_is_refused() {
        let rows = "STORY-P0-02-01\tD05\tdesign\tthe subsystem does not exist\t2026-07-28\n";
        let evidenced = BTreeSet::from([("STORY-P0-02-01".to_string(), "D05".to_string())]);
        let error = validate_open_debt(
            &debt_register(rows),
            &debt_contracts(),
            &debt_readiness(),
            &evidenced,
        )
        .expect_err("a gate cannot be unclosable and closed at once");
        assert!(error.contains("cannot have produced evidence"), "{error}");
    }

    // ---- `TEST-P0-01-07-A` clause 5: `LE-44`, Feature/Story agreement ------

    fn status(state: &str, detail: &str) -> ArtifactStatus {
        ArtifactStatus {
            id: "STORY-P1-07-01".to_string(),
            state: state.to_string(),
            detail: detail.to_string(),
        }
    }

    #[test]
    fn a_feature_story_row_parses_its_id_and_status_cell() {
        let row = "| [`STORY-P1-07-01`](../stories/STORY-P1-07-01.md) | summary | In progress — \
                   criteria 3 and 4 need a board |";
        let (id, cell) = parse_feature_story_row(row).expect("a Stories-table row parses");
        assert_eq!(id, "STORY-P1-07-01");
        assert!(cell.starts_with("In progress"), "{cell}");
    }

    #[test]
    fn a_table_rule_and_ordinary_prose_are_not_story_rows() {
        assert!(parse_feature_story_row("|---|---|---|").is_none());
        assert!(parse_feature_story_row("Order matters | and is not negotiable").is_none());
    }

    #[test]
    fn criterion_numbers_are_a_set_and_stop_at_the_first_non_number() {
        assert_eq!(criterion_numbers("criteria 3 and 4 need a board"), BTreeSet::from([3, 4]));
        assert_eq!(criterion_numbers("criterion 2 needs a board"), BTreeSet::from([2]));
        assert_eq!(criterion_numbers("criteria 4 and 3"), BTreeSet::from([3, 4]));
        assert!(criterion_numbers("Verified (Tier 0 + Host)").is_empty());
    }

    /// `LE-44`'s originating instance, in the direction it actually occurred:
    /// the Feature understated which criteria a board session must close, on
    /// precisely the criterion that produces `Q1` qualification evidence.
    #[test]
    fn a_feature_disagreeing_about_criteria_is_refused() {
        let error = compare_feature_story_row(
            "FEAT-P1-07",
            "STORY-P1-07-01",
            "In progress — host half Green, criteria 2 and 4 need a board",
            &status("In progress", "host-testable half Green; criteria 3 and 4 blocked on a board"),
        )
        .expect_err("a criteria disagreement must be refused");
        assert!(error.contains("{2, 4}"), "{error}");
        assert!(error.contains("{3, 4}"), "{error}");
    }

    /// The sharper instance this check found on the committed tree:
    /// `FEAT-P1-03` recorded `STORY-P1-03-02` as `Verified` for four days while
    /// the Story's own header still read `In progress`.
    #[test]
    fn a_feature_disagreeing_about_the_state_word_is_refused() {
        let error = compare_feature_story_row(
            "FEAT-P1-03",
            "STORY-P1-03-02",
            "Verified (Tier 0 + Host; assurance `baseline-debt`)",
            &status("In progress", "acceptance criteria hardened after pre-implementation review"),
        )
        .expect_err("a state disagreement must be refused");
        assert!(error.contains("records `STORY-P1-03-02` as `Verified`"), "{error}");
    }

    /// `Functionally Verified` carries assurance debt that a reader of plain
    /// `Verified` will not go looking for. They are distinct states in this
    /// project's own vocabulary and do not satisfy each other.
    #[test]
    fn functionally_verified_does_not_satisfy_verified() {
        let error = compare_feature_story_row(
            "FEAT-P1-01",
            "STORY-P1-01-01",
            "Verified (Tier 0 + Host; assurance `baseline-debt`)",
            &status("Functionally Verified", "assurance state `baseline-debt`"),
        )
        .expect_err("the two states are not interchangeable");
        assert!(error.contains("Functionally Verified"), "{error}");
    }

    #[test]
    fn an_agreeing_row_is_accepted_whether_or_not_the_cell_is_bolded() {
        compare_feature_story_row(
            "FEAT-P1-07",
            "STORY-P1-07-01",
            "In progress — host half Green, criteria 3 and 4 need a board",
            &status("In progress", "host-testable half Green; criteria 3 and 4 blocked on a board"),
        )
        .expect("agreement is accepted");
        compare_feature_story_row(
            "FEAT-P1-04",
            "STORY-P1-04-01",
            "**Verified** (Tier 0 + Host, 2026-07-28; assurance `baseline-debt`)",
            &status("Verified", "Tier 0 + Host — assurance `baseline-debt`"),
        )
        .expect("a bolded cell is a prose choice, not a defect");
    }

    // `TEST-P0-01-05-A` clause 1.
    #[test]
    fn guardrail_evidence_counts_valid_rows() {
        let fixture = format!(
            "{GUARDRAIL_EVIDENCE_HEADER}\n{}{}",
            evidence_row("PERF-D01-G11", "D01", "STORY-P0-01-01"),
            evidence_row("PERF-D05-G11", "D05", "STORY-P0-02-01"),
        );
        let evidence = validate_guardrail_evidence(&fixture, &evidence_contracts())
            .expect("valid register passes");
        assert_eq!(evidence.count, 2);
        assert!(evidence.bound_rows.is_empty(), "no G11 row is bound-class");
    }

    // `TEST-P0-01-05-A` clause 1: the check the register exists for. Evidence
    // filed against a gate the Story's own contract never selected is more
    // convincing than having no register at all, and therefore worse.
    #[test]
    fn guardrail_evidence_rejects_a_domain_the_story_does_not_select() {
        let fixture = format!(
            "{GUARDRAIL_EVIDENCE_HEADER}\n{}",
            evidence_row("PERF-D05-G11", "D05", "STORY-P0-01-01"),
        );
        let error = validate_guardrail_evidence(&fixture, &evidence_contracts())
            .expect_err("a domain outside the contract must fail");
        assert!(error.contains("STORY-P0-01-01"), "{error}");
        assert!(error.contains("D05"), "{error}");
    }

    #[test]
    fn guardrail_evidence_rejects_a_domain_disagreeing_with_its_own_id() {
        let fixture = format!(
            "{GUARDRAIL_EVIDENCE_HEADER}\n{}",
            evidence_row("PERF-D01-G11", "D05", "STORY-P0-02-01"),
        );
        let error = validate_guardrail_evidence(&fixture, &evidence_contracts())
            .expect_err("a mismatched domain must fail");
        assert!(error.contains("D01"), "{error}");
    }

    #[test]
    fn guardrail_evidence_rejects_an_unknown_story() {
        let fixture = format!(
            "{GUARDRAIL_EVIDENCE_HEADER}\n{}",
            evidence_row("PERF-D01-G11", "D01", "STORY-P9-99-99"),
        );
        let error = validate_guardrail_evidence(&fixture, &evidence_contracts())
            .expect_err("an unmapped Story must fail");
        assert!(error.contains("STORY-P9-99-99"), "{error}");
    }

    #[test]
    fn guardrail_evidence_rejects_a_malformed_guardrail_id() {
        let fixture = format!(
            "{GUARDRAIL_EVIDENCE_HEADER}\n{}",
            evidence_row("PERF-D01-G99", "D01", "STORY-P0-01-01"),
        );
        validate_guardrail_evidence(&fixture, &evidence_contracts())
            .expect_err("G99 is not a guardrail");
    }

    #[test]
    fn guardrail_evidence_rejects_duplicates() {
        let row = evidence_row("PERF-D01-G11", "D01", "STORY-P0-01-01");
        let fixture = format!("{GUARDRAIL_EVIDENCE_HEADER}\n{row}{row}");
        let error = validate_guardrail_evidence(&fixture, &evidence_contracts())
            .expect_err("a duplicate pair must fail");
        assert!(error.contains("duplicate"), "{error}");
    }

    #[test]
    fn guardrail_evidence_rejects_a_wrong_header() {
        let fixture = "guardrail_id\tdomain\n";
        validate_guardrail_evidence(fixture, &evidence_contracts())
            .expect_err("a changed header must fail");
    }

    // `TEST-P0-01-05-A` clause 3: the property every `PERF-Dnn-G11` row rests on.
    #[test]
    fn the_shipped_crates_contain_no_heap() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("xtask manifest lives at os/src/xtask")
            .to_path_buf();
        validate_no_heap(&repo_root).expect("no shipped crate may allocate");
    }

    fn loose_end_fixture() -> String {
        let mut fixture = String::from(LOOSE_END_HEADER);
        fixture.push('\n');
        fixture.push_str("LE-01\tsummary\torigin\tpath\towned\tclosed\traised\tclosed-here\n");
        fixture.push_str("LE-02\tsummary\torigin\tpath\tunowned\topen\traised\t-\n");
        fixture
    }

    #[test]
    fn loose_end_fixture_is_accepted() {
        let index = validate_loose_ends(&loose_end_fixture()).expect("fixture must validate");
        assert_eq!(index.ids.len(), 2);
        assert_eq!(index.open_count, 1);
    }

    #[test]
    fn loose_end_gap_is_rejected() {
        // The failure mode the register exists to prevent: an id carried in prose
        // that never made it into the machine-readable list.
        let fixture = loose_end_fixture().replace("LE-02\tsummary", "LE-03\tsummary");
        let error = validate_loose_ends(&fixture).expect_err("a gap must be rejected");
        assert!(error.contains("contiguously"), "unexpected error: {error}");
    }

    #[test]
    fn loose_end_closed_without_evidence_is_rejected() {
        let fixture = loose_end_fixture().replace(
            "LE-01\tsummary\torigin\tpath\towned\tclosed\traised\tclosed-here",
            "LE-01\tsummary\torigin\tpath\towned\tclosed\traised\t-",
        );
        let error =
            validate_loose_ends(&fixture).expect_err("a closed row needs a closing handover");
        assert!(error.contains("records no `closed_in`"), "unexpected error: {error}");
    }

    #[test]
    fn loose_end_open_claiming_closure_is_rejected() {
        let fixture = loose_end_fixture()
            .replace("unowned\topen\traised\t-", "unowned\topen\traised\tclosed-here");
        let error = validate_loose_ends(&fixture).expect_err("an open row cannot record closure");
        assert!(error.contains("is open but records"), "unexpected error: {error}");
    }

    #[test]
    fn loose_end_unknown_vocabulary_is_rejected() {
        let ownership = loose_end_fixture().replace("\towned\t", "\tmaybe\t");
        assert!(validate_loose_ends(&ownership).is_err(), "unknown ownership must be rejected");

        let state = loose_end_fixture().replace("\tclosed\t", "\tfixed\t");
        assert!(validate_loose_ends(&state).is_err(), "unknown state must be rejected");
    }

    #[test]
    fn loose_end_tokens_are_extracted_without_false_positives() {
        assert_eq!(loose_end_tokens("closes LE-20 and LE-22."), vec!["LE-20", "LE-22"]);
        assert_eq!(loose_end_tokens("`LE-19(b)` is open"), vec!["LE-19"]);
        assert!(loose_end_tokens("SAMPLE-01 and TITLE-02").is_empty());
        assert!(loose_end_tokens("LE-1 is too short").is_empty());
    }

    #[test]
    fn status_lines_in_every_committed_shape_parse() {
        // One case per shape found across the committed Epic/Feature/Story set.
        let cases = [
            ("Status: **Verified**", "Verified"),
            ("Status: **Verified** (locally; CI run pending)", "Verified"),
            ("Status: **Verified — 4/4 Stories Verified**", "Verified"),
            ("Status: **Complete — 3/3 Stories Verified**", "Complete"),
            ("Status: **Specified, not yet started**", "Specified"),
            ("Status: **Specified — no Story started**", "Specified"),
            ("Status: **In progress — acceptance criteria hardened**", "In progress"),
            (
                "Status: **Functionally Verified (Tier 0 + Host), 2026-07-27**",
                "Functionally Verified",
            ),
            ("Status: **Functionally complete (2026-07-27) — all 25**", "Functionally complete"),
            ("Status: **Verified (Tier 0 + Host) 2026-07-28**", "Verified"),
        ];
        for (line, expected) in cases {
            let (state, _) = parse_status_line(line).unwrap_or_else(|error| {
                panic!("`{line}` must parse: {error}");
            });
            assert_eq!(state, expected, "wrong state for `{line}`");
        }
    }

    #[test]
    fn a_status_outside_the_vocabulary_is_rejected() {
        let error = parse_status_line("Status: **Nearly done — trust me**")
            .expect_err("an invented state must be rejected");
        assert!(error.contains("must open with one of"), "unexpected error: {error}");
    }

    #[test]
    fn a_state_word_must_be_terminated() {
        // `Complete` must not match `Completely`, or the vocabulary would admit
        // any word that happens to start with a valid state.
        assert!(parse_status_line("Status: **Completely rewritten**").is_err());
        assert!(parse_status_line("Status: **Verifiable soon**").is_err());
    }

    #[test]
    fn an_unbolded_status_is_rejected() {
        assert!(parse_status_line("Status: Verified").is_err());
        assert!(parse_status_line("State: **Verified**").is_err());
    }

    #[test]
    fn functionally_verified_is_not_truncated_to_verified() {
        let (state, _) = parse_status_line("Status: **Functionally Verified (Host), 2026-07-27**")
            .expect("must parse");
        assert_eq!(state, "Functionally Verified");
    }

    #[test]
    fn every_committed_artifact_has_a_parseable_status() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("xtask manifest lives at os/src/xtask")
            .to_path_buf();
        let statuses =
            validate_status_headers(&repo_root).expect("every committed Status: must parse");
        assert!(statuses.iter().any(|status| status.id.starts_with("EPIC-")));
        assert!(statuses.iter().any(|status| status.id.starts_with("FEAT-")));
        assert!(statuses.iter().any(|status| status.id.starts_with("STORY-")));
        for status in &statuses {
            assert!(
                STATUS_STATES.contains(&status.state.as_str()),
                "`{}` has out-of-vocabulary state `{}`",
                status.id,
                status.state
            );
        }
    }

    #[test]
    fn committed_loose_end_references_all_resolve() {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("xtask manifest lives at os/src/xtask")
            .to_path_buf();
        let contents =
            fs::read_to_string(repo_root.join("goals").join("assurance").join("loose-ends.tsv"))
                .expect("committed register must be readable");
        let index = validate_loose_ends(&contents).expect("committed register must be valid");
        let references = validate_loose_end_references(&repo_root, &index.ids)
            .expect("every committed LE-* token must resolve");
        assert!(references > 0, "the scan must actually find tokens");
    }
}
