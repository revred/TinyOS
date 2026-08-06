//! Id grammar and list-membership validators.
//!
//! Every `STORY-Pn-NN-NN`, `FEAT-Pn-NN`, `Cnn`, `BND-nn`, `PD-nn`, `CAG-nn`,
//! `SEC-nn`, `Dnn`, `PERF-Dnn-Gnn`, `APP-nn`, `LZ-nn` and `G-**-n` in the spine
//! passes through here. Shape first, then membership in the canonical set —
//! kept apart so an error says *malformed* or says *unknown*, never both.

use super::*;

pub(super) fn validate_numbered_context_id(
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

pub(super) fn validate_complete_numbered_context(
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

pub(super) fn validate_context_horizon(
    value: &str,
    line_number: usize,
    kind: &str,
) -> Result<(), String> {
    if !matches!(value, "now" | "next" | "later" | "research") {
        return Err(format!("{kind} line {line_number}: unknown roadmap horizon `{value}`"));
    }
    Ok(())
}

pub(super) fn validate_goal_list(
    value: &str,
    line_number: usize,
) -> Result<BTreeSet<String>, String> {
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

pub(super) fn validate_domain_list(
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

pub(super) fn validate_application_list(
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

pub(super) fn validate_numbered_charter_id(
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

pub(super) fn validate_complete_numbered_charter(
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

pub(super) fn validate_boundary_list(
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

pub(super) fn validate_numbered_contract_list(
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

pub(super) fn validate_performance_guardrail_list(
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

pub(super) fn validate_feature_id(feature_id: &str, line_number: usize) -> Result<(), String> {
    let parts: Vec<&str> = feature_id.split('-').collect();
    if parts.len() != 3 || parts[0] != "FEAT" || !is_phase_id(parts[1]) || !is_two_digits(parts[2])
    {
        return Err(format!(
            "Feature contract line {line_number}: `{feature_id}` is not a valid FEAT-Pn-NN id"
        ));
    }
    Ok(())
}

pub(super) fn validate_containment_id(class: &str, line_number: usize) -> Result<(), String> {
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

pub(super) fn validate_boundary_test_id(test: &str, line_number: usize) -> Result<(), String> {
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

pub(super) fn validate_domain_id(domain: &str, line_number: usize) -> Result<(), String> {
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

pub(super) fn validate_performance_guardrail_id(
    guardrail: &str,
    line_number: usize,
) -> Result<(), String> {
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

pub(super) fn validate_security_id(control: &str, line_number: usize) -> Result<(), String> {
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

pub(super) fn validate_containment_list(
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

pub(super) fn validate_security_list(
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
