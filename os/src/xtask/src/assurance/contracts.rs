//! The Feature and Story contract registers — `agent.md` rule 8's spine.
//!
//! A Feature declares implementation/subject containment classes, hostile
//! inputs, authority posture and `BND-*` tests; a Story selects performance
//! domains, security controls and containment classes. The join between them
//! is what makes "no Feature or Story bypasses the assurance spine" a gate
//! rather than an aspiration.

use super::*;

pub(super) fn validate_feature_contracts(
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
pub(super) fn validate_story_contracts(
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

pub(super) fn feature_for_story(story_id: &str, line_number: usize) -> Result<String, String> {
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
