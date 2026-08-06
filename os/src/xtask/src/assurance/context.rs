//! The holistic-context registers: the 19 application/platform destinations and
//! the nine whole-system landing zones.
//!
//! `agent.md` rule 10 — every product destination stays joined across all four
//! planes (goal, performance, security, class) before implementation. These two
//! files are where that join is recorded, and these validators are what stop a
//! new runtime or protocol being added on one plane only.

use super::*;

pub(super) fn validate_application_platforms(
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

pub(super) fn validate_landing_zones(
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
