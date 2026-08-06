//! The Security Charter's own catalogues: the five containment classes, the 20
//! cross-class boundary tests, the 20 security controls, the 14 Protection
//! Domain contracts, the 14 code-admission gates and the complete 25-pair
//! class-communication matrix — plus the coverage check that holds the charter
//! document to all of them.
//!
//! These are the registers `SECURITY_CHARTER.md` is the prose of. Every
//! validator here refuses a *gap* as loudly as it refuses a malformed row,
//! because a charter contract with no row behind it reads as satisfied.

use super::*;

pub(super) fn validate_security_charter_document(contents: &str) -> Result<(), String> {
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

pub(super) fn validate_containment_classes(contents: &str) -> Result<BTreeSet<String>, String> {
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

pub(super) fn validate_boundary_tests(
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

pub(super) fn validate_security_controls(
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

pub(super) fn validate_protection_domain_contracts(
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

pub(super) fn validate_code_admission_gates(
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

pub(super) fn validate_class_communication_matrix(
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

pub(super) fn validate_charter_coverage(
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
