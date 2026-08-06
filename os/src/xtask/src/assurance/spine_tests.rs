//! The assurance spine's integration suite.
//!
//! These tests cross module boundaries on purpose, which is why they are not
//! filed beside any one validator: most of them build a *complete* catalogue
//! fixture, break exactly one thing, and assert the named refusal — and the
//! rest hold the committed tree itself to a floor. The unit tests for an
//! individual register live next to that register.
//!
//! Two of them deserve naming. `committed_assurance_spine_catalogues_are_exact`
//! is the pin: it fails when a canonical population changes, so growing the
//! spine is a deliberate edit rather than a drift. And
//! `committed_assurance_spine_population_never_shrinks` is the ratchet — the
//! spine may grow, and may not quietly lose documents.

use super::*;
use std::path::PathBuf;

fn containment_fixture() -> String {
    let mut fixture = String::from(CONTAINMENT_HEADER);
    fixture.push('\n');
    for number in 0..CONTAINMENT_CLASS_COUNT {
        fixture
            .push_str(&format!("C{number}\tname\tpurpose\tauthority\tinput\tfailure\tevidence\n"));
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
    let error =
        validate_containment_classes(&fixture).expect_err("missing containment class must fail");
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
    // Two Stories select `D05` on purpose: the register's unit is the
    // `(guardrail, story)` pair, so one gate carrying two rows is the
    // normal case rather than the exceptional one, and no test of the
    // gate-versus-row distinction can be written without it.
    let fixture = format!(
        "{CONTRACT_HEADER}\n\
             STORY-P0-01-01\tFEAT-P0-01\tD01\tSEC-19\tC0\tbaseline-debt\trationale\n\
             STORY-P0-02-01\tFEAT-P0-02\tD05\tSEC-19\tC0\tbaseline-debt\trationale\n\
             STORY-P0-02-03\tFEAT-P0-02\tD05\tSEC-19\tC0\tbaseline-debt\trationale\n"
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

fn refused_row(guardrail: &str, domain: &str, story: &str) -> String {
    format!("{guardrail}\t{domain}\t{story}\trefused\tpath\t2026-07-28\tnote\n")
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
    let error =
        validate_open_debt(&debt_register(rows), &debt_contracts(), &debt_readiness(), &evidenced)
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

/// `LE-84`/`LE-85`: the register could say *measured* and could say
/// nothing, and had no way to say **measured, read against the target, and
/// refused**. `STORY-P1-06-01` measured `PERF-D03-G20`, found 55%
/// run-to-run p99 CV, and declined the filing in Report prose — where no
/// gate can see it, and where the next session executing "file what
/// already exists" would either re-file it or re-derive the refusal.
///
/// A `refused` row records the reading. It must **not** count toward the
/// published figure: the numerator is gates carrying evidence, and a
/// refusal is the opposite of evidence. Counting it would be `LE-83`'s
/// defect wearing the opposite sign.
#[test]
fn a_refused_row_records_the_reading_without_counting_as_evidence() {
    let fixture = format!(
        "{GUARDRAIL_EVIDENCE_HEADER}\n{}{}",
        evidence_row("PERF-D01-G11", "D01", "STORY-P0-01-01"),
        refused_row("PERF-D05-G11", "D05", "STORY-P0-02-01"),
    );
    let evidence = validate_guardrail_evidence(&fixture, &evidence_contracts())
        .expect("a refused row is a legal row");
    assert_eq!(evidence.count, 1, "only the structural row is evidence");
    assert!(evidence.refused_gates.contains("PERF-D05-G11"));
    assert!(
        !evidence.refused_gates.contains("PERF-D01-G11"),
        "an evidenced gate is not a refused one"
    );
}

/// The two states are not exclusive per *gate*, only per row: one Story may
/// refuse a gate its own measurement could not close while another Story's
/// evidence stands. The gate is evidenced, and the refusal stays visible.
#[test]
fn a_gate_carrying_both_a_refusal_and_evidence_counts_once_as_evidenced() {
    let fixture = format!(
        "{GUARDRAIL_EVIDENCE_HEADER}\n{}{}",
        evidence_row("PERF-D05-G11", "D05", "STORY-P0-02-01"),
        refused_row("PERF-D05-G11", "D05", "STORY-P0-02-03"),
    );
    let evidence =
        validate_guardrail_evidence(&fixture, &evidence_contracts()).expect("both rows are legal");
    assert_eq!(evidence.count, 1);
    assert!(evidence.refused_gates.contains("PERF-D05-G11"));
}

/// The vocabulary is closed. It was open until 2026-08-05 — the column
/// accepted any string — which is why `refused` could not be relied on to
/// mean anything: a typo would have read as a novel evidence kind and
/// silently counted.
#[test]
fn guardrail_evidence_rejects_an_evidence_kind_outside_the_vocabulary() {
    let fixture = format!(
            "{GUARDRAIL_EVIDENCE_HEADER}\nPERF-D01-G11\tD01\tSTORY-P0-01-01\thearsay\tpath\t2026-07-28\tnote\n"
        );
    let error = validate_guardrail_evidence(&fixture, &evidence_contracts())
        .expect_err("an unknown evidence kind must fail");
    assert!(error.contains("hearsay"), "{error}");
    assert!(error.contains("structural"), "the error must name the vocabulary: {error}");
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

// ---- LE-51: citations must resolve to exactly one session document ----

#[test]
fn a_citation_names_a_dated_folder_and_a_slot() {
    assert_eq!(
        parse_citation("hand-2026-08-01/02B").expect("a plain citation parses"),
        Citation { folder: "hand-2026-08-01".to_string(), slot: "02B".to_string() }
    );
    assert_eq!(
        parse_citation("hand-2026-07-26/07").expect("a letterless slot parses"),
        Citation { folder: "hand-2026-07-26".to_string(), slot: "07".to_string() }
    );
}

/// Historical rows carry trailing prose. The prose is kept — rewriting
/// the register to suit its own gate would be the drift this project
/// keeps finding, not a fix for it.
#[test]
fn trailing_prose_after_the_citation_is_ignored_not_rejected() {
    assert_eq!(
        parse_citation("hand-2026-07-30/05A session, owner feedback")
            .expect("trailing prose is tolerated"),
        Citation { folder: "hand-2026-07-30".to_string(), slot: "05A".to_string() }
    );
}

#[test]
fn a_field_that_names_no_document_is_rejected() {
    assert!(parse_citation("hand-2026-08-01").is_err(), "a folder alone names no document");
    assert!(parse_citation("REPORT-2026-08-04-01").is_err(), "not a session citation");
    assert!(parse_citation("hand-2026-08-01/").is_err(), "an empty slot names nothing");
    assert!(parse_citation("hand-2026-08-01/AB").is_err(), "a slot needs its number");
    assert!(parse_citation("notes-2026-08-01/02A").is_err(), "not a hand- folder");
}

#[test]
fn a_slot_resolves_to_the_one_document_that_carries_it() {
    let files = vec![
        "02A-tinytile-architecture-document.md".to_string(),
        "02B-pushed-and-ci-fixed-le-64.md".to_string(),
        "index.html".to_string(),
    ];
    assert_eq!(
        resolve_slot(&files, "02B").expect("exactly one match"),
        "02B-pushed-and-ci-fixed-le-64.md"
    );
}

/// The defect `LE-51` was raised for: `LE-47` and `LE-48` both cited
/// `hand-2026-07-28/41A` while meaning two different documents.
#[test]
fn two_documents_in_one_slot_are_the_ambiguity_this_gate_exists_for() {
    let files = vec![
        "41A-the-dashboard-as-a-register.md".to_string(),
        "41A-something-else-entirely.md".to_string(),
    ];
    let error = resolve_slot(&files, "41A").expect_err("two matches must be refused");
    assert!(error.contains("exactly one"), "unexpected error: {error}");
}

#[test]
fn a_citation_with_no_document_behind_it_is_dangling() {
    let files = vec!["01A-only-this-one.md".to_string()];
    let error = resolve_slot(&files, "02A").expect_err("a dangling citation must fail");
    assert!(error.contains("no document"), "unexpected error: {error}");
}

/// `hand-2026-07-30/03A` really is two documents. Renaming a committed
/// handover to suit the gate would edit the historical record, so the
/// citation names the whole stem and resolves to exactly one.
#[test]
fn a_full_document_stem_disambiguates_a_shared_slot() {
    let files = vec![
        "03A-android-plan-and-the-spoor-gap.md".to_string(),
        "03A-deep-os-textbook-code-review.md".to_string(),
    ];
    assert!(resolve_slot(&files, "03A").is_err(), "the short slot is genuinely ambiguous");
    assert_eq!(
        resolve_slot(&files, "03A-android-plan-and-the-spoor-gap")
            .expect("the full stem names one document"),
        "03A-android-plan-and-the-spoor-gap.md"
    );
    assert!(
        parse_citation("hand-2026-07-30/03A-android-plan-and-the-spoor-gap").is_ok(),
        "the long form must parse"
    );
}

/// A letterless slot must not swallow its lettered siblings, or `07`
/// would silently resolve to `07A` and the citation would be wrong
/// while the gate stayed green.
#[test]
fn a_letterless_slot_does_not_match_a_lettered_document() {
    let files = vec!["07A-lettered.md".to_string()];
    assert!(resolve_slot(&files, "07").is_err(), "`07` must not match `07A-`");
}

/// The committed register must satisfy the gate it ships with.
#[test]
fn every_committed_citation_resolves_to_a_real_session_document() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("xtask manifest lives at os/src/xtask")
        .to_path_buf();
    let contents =
        fs::read_to_string(repo_root.join("goals").join("assurance").join("loose-ends.tsv"))
            .expect("the committed register must be readable");
    let index = validate_loose_ends(&contents).expect("committed register must be valid");
    validate_loose_end_citations(&repo_root, &index.citations)
        .expect("every committed citation must resolve");
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
    let error = validate_loose_ends(&fixture).expect_err("a closed row needs a closing handover");
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
        ("Status: **Functionally Verified (Tier 0 + Host), 2026-07-27**", "Functionally Verified"),
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
    let statuses = validate_status_headers(&repo_root).expect("every committed Status: must parse");
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
