//! Integrity validation for TinyOS's performance-and-security assurance spine.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

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

/// Validates both catalogues and the Story-level join relative to `repo_root`.
pub fn check_assurance_spine(repo_root: &Path) -> Result<AssuranceSummary, String> {
    crate::performance_catalogue::check_catalogue(repo_root)
        .map_err(|error| format!("performance catalogue: {error}"))?;

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

    Ok(AssuranceSummary {
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
    })
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

    #[test]
    fn committed_assurance_spine_is_complete() {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("xtask manifest lives at os/src/xtask")
            .to_path_buf();
        let summary =
            check_assurance_spine(&repo_root).expect("committed assurance spine must be valid");
        assert_eq!(summary.feature_count, 22);
        assert_eq!(summary.story_count, 47);
        assert_eq!(summary.containment_class_count, 5);
        assert_eq!(summary.boundary_test_count, 20);
        assert_eq!(summary.security_control_count, 20);
        assert_eq!(summary.protection_domain_contract_count, 14);
        assert_eq!(summary.code_admission_gate_count, 14);
        assert_eq!(summary.class_communication_pair_count, 25);
        assert_eq!(summary.application_platform_count, 19);
        assert_eq!(summary.landing_zone_count, 9);
        assert_eq!(summary.test_count, 33);
        assert_eq!(summary.report_count, 40);
        assert!(summary.selected_performance_contracts >= 625);
        assert!(summary.selected_application_performance_contracts >= 625);
    }
}
