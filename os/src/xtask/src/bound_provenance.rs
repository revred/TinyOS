//! The gate behind `ADR 0004` and `ADR 0005` — `LE-33`, both conditions.
//!
//! Two decisions this project took correctly, and until now enforced only in
//! prose:
//!
//! 1. **`ADR 0004`**: a worst-case bound stated on x86_64 is a claim about the
//!    firmware, not about TinyOS, because SMIs are invisible, unmaskable and
//!    unattributable from the OS's exception level. A Tier 0 number is a claim
//!    about an emulator. Neither may be promoted into a `G04`-class bound.
//! 2. **`ADR 0005`**: ARM64 is not the real-time tier automatically — a
//!    *qualified* ARM64 platform is. A bound may be quoted only from a platform
//!    holding a current secure-world qualification record, and **the default
//!    for a platform with no record is "not qualified", never "presumed
//!    clean."**
//!
//! Both decisions were prose, and prose is weaker than a gate: a Report could
//! file a `PERF-Dnn-G04` row sourced from a QEMU x86_64 run and every check in
//! this repository stayed green. That is the failure mode `LE-33` registers and
//! this module closes.
//!
//! **What it does not do.** It reads the machine-readable spine — the guardrail
//! evidence register and the `TINYOS-BOUND/1` claim lines a Report must carry
//! to file a bound-class row. It does not read English. A Report may still
//! write "the worst case is 1.2 µs" in a sentence, and no lint in this project
//! parses sentences. The boundary is stated here rather than implied, because
//! `STORY-P0-01-07`'s whole subject is decisions that read as enforced and are
//! not.
//!
//! **On being shown to detect.** As of this module's introduction there is not
//! one bound-class row in the evidence register, so every check below is
//! vacuously satisfied against the committed tree and a green run proves
//! nothing about any of them. `ADR 0005`'s trap section is binding here as much
//! as on a `Q3` campaign: the tests at the bottom of this file are the positive
//! controls, and they are the reason to believe a zero.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// The claim-line sentinel a Report must carry to file a bound-class row.
///
/// Deliberately *not* `TINYOS-MEAS`: a measurement envelope is what a fixture
/// emitted, and a bound is what a human chose to promote it into. Conflating
/// the two is precisely the promotion `ADR 0004` forbids, so they do not share
/// a sentinel.
pub const BOUND_SENTINEL: &str = "TINYOS-BOUND/1";

/// The guardrail column that states a worst-case bound rather than an observed
/// statistic.
///
/// `G04` is *"observed maximum and WCET bound"* in all 25 domains — the only
/// column whose target sentence contains the words "declared bound". `G05`
/// (jitter) and `G03` (p99) describe distributions of what was seen; `G04`
/// alone asserts what cannot be exceeded, which is the assertion neither
/// x86_64 nor an unqualified platform can support.
pub const BOUND_CLASS_GUARDRAILS: [&str; 1] = ["G04"];

const PLATFORM_HEADER: &str =
    "platform_id\tarch\tdescription\tfirmware\tstate\tqualification_record\trecorded_in";
const PLATFORM_FIELD_COUNT: usize = 7;
const PLATFORM_UNSET: &str = "-";
const CLAIM_KEYS: [&str; 6] = ["guardrail", "value", "tier", "arch", "platform", "qualification"];

/// Architectures disqualified from carrying a bound at all, per `ADR 0004`
/// decision 1 as restated unmodified by `ADR 0005`.
const DISQUALIFIED_ARCHITECTURES: [&str; 1] = ["x86_64"];

/// Measurement tiers disqualified from carrying a bound. Tier 0 is an
/// emulator: `LE-42` is the worked example of a Tier 0 number being 17-39x its
/// own budget for reasons that say nothing about the code.
const DISQUALIFIED_TIERS: [&str; 1] = ["T0"];

/// One row of `goals/assurance/qualified-platforms.tsv`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformRow {
    /// Architecture this platform is, independent of what a claim asserts.
    pub arch: String,
    /// `qualified` or `unqualified`.
    pub state: String,
    /// The `REPORT-*` id holding the `Q1`-`Q4` record, or `-`.
    pub qualification_record: String,
}

/// Every measuring platform this project knows, and whether it is qualified.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlatformIndex {
    rows: BTreeMap<String, PlatformRow>,
}

impl PlatformIndex {
    /// Looks a platform up. A platform absent from the register is *not*
    /// qualified — silence is not evidence (`ADR 0005` decision 3).
    pub fn get(&self, platform_id: &str) -> Option<&PlatformRow> {
        self.rows.get(platform_id)
    }

    /// How many platforms are registered.
    ///
    /// Named `count` rather than `len`: this is a register of declarations, not
    /// a collection, and nothing about it is meaningfully "empty".
    pub fn count(&self) -> usize {
        self.rows.len()
    }

    /// How many hold a qualification record. `ADR 0005` decision 3 states this
    /// is zero; the register exists so that the statement is a value rather
    /// than a sentence in a document.
    pub fn qualified_count(&self) -> usize {
        self.rows.values().filter(|row| row.state == "qualified").count()
    }
}

/// Validates `goals/assurance/qualified-platforms.tsv`.
///
/// `reports` is the set of `REPORT-*` ids that exist, so a `qualified` row
/// cannot cite a qualification record nobody wrote.
pub fn validate_platforms(
    contents: &str,
    reports: &BTreeSet<String>,
) -> Result<PlatformIndex, String> {
    const STATES: [&str; 2] = ["qualified", "unqualified"];
    const ARCHITECTURES: [&str; 2] = ["x86_64", "aarch64"];

    let mut lines = contents.lines();
    let header = lines
        .next()
        .ok_or_else(|| "qualified-platforms register is empty".to_string())?
        .trim_end_matches('\r');
    if header != PLATFORM_HEADER {
        return Err(format!(
            "unexpected qualified-platforms header; expected exactly `{PLATFORM_HEADER}`"
        ));
    }

    let mut rows = BTreeMap::new();
    for (zero_based_index, raw_line) in lines.enumerate() {
        let line_number = zero_based_index + 2;
        let fields =
            tsv_fields(raw_line, line_number, PLATFORM_FIELD_COUNT, "qualified-platforms")?;

        let platform_id = fields[0];
        let arch = fields[1];
        let state = fields[4];
        let record = fields[5];

        if !ARCHITECTURES.contains(&arch) {
            return Err(format!(
                "qualified-platforms line {line_number}: `{platform_id}` declares unknown arch \
                 `{arch}` (expected one of {})",
                ARCHITECTURES.join(", ")
            ));
        }
        if !STATES.contains(&state) {
            return Err(format!(
                "qualified-platforms line {line_number}: `{platform_id}` declares unknown state \
                 `{state}` (expected one of {})",
                STATES.join(", ")
            ));
        }

        // A qualification record that does not exist is worse than none: it
        // reads as evidence and cannot be read.
        match (state, record) {
            ("qualified", PLATFORM_UNSET) => {
                return Err(format!(
                    "qualified-platforms line {line_number}: `{platform_id}` is qualified but \
                     records no qualification record; ADR 0005 qualification is Q1-Q4 in a dated \
                     Report, never a state word on its own"
                ));
            }
            ("qualified", cited) if !reports.contains(cited) => {
                return Err(format!(
                    "qualified-platforms line {line_number}: `{platform_id}` cites qualification \
                     record `{cited}`, which is not a Report in goals/reports/"
                ));
            }
            ("unqualified", cited) if cited != PLATFORM_UNSET => {
                return Err(format!(
                    "qualified-platforms line {line_number}: `{platform_id}` is unqualified but \
                     cites qualification record `{cited}`"
                ));
            }
            _ => {}
        }

        if rows
            .insert(
                platform_id.to_string(),
                PlatformRow {
                    arch: arch.to_string(),
                    state: state.to_string(),
                    qualification_record: record.to_string(),
                },
            )
            .is_some()
        {
            return Err(format!(
                "qualified-platforms line {line_number}: duplicate platform `{platform_id}`"
            ));
        }
    }

    if rows.is_empty() {
        return Err("qualified-platforms register has no entries".to_string());
    }
    Ok(PlatformIndex { rows })
}

/// One `TINYOS-BOUND/1` line, parsed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundClaim {
    /// The `PERF-Dnn-Gnn` id this bound is filed against.
    pub guardrail: String,
    /// The bound itself, as written.
    pub value: String,
    /// Measurement tier it was sourced from.
    pub tier: String,
    /// Architecture it was sourced from.
    pub arch: String,
    /// Platform identity it was sourced from.
    pub platform: String,
    /// The qualification record backing it, or `none`.
    pub qualification: String,
}

/// Parses every `TINYOS-BOUND/1` line in one Report.
///
/// Strict, for [`crate::timing`]'s reason: a sentinel-bearing line is the claim
/// "this is a bound", and a claim is never parsed best-effort. A malformed one
/// is an error rather than a line that is skipped and therefore never checked.
pub fn parse_bound_claims(report: &str, report_id: &str) -> Result<Vec<BoundClaim>, String> {
    let mut claims = Vec::new();
    for (zero_based_index, line) in report.lines().enumerate() {
        let line = line.trim();
        if !line.starts_with(BOUND_SENTINEL) {
            continue;
        }
        let line_number = zero_based_index + 1;
        let mut values: BTreeMap<&str, &str> = BTreeMap::new();
        for token in line[BOUND_SENTINEL.len()..].split_whitespace() {
            let Some((key, value)) = token.split_once('=') else {
                return Err(format!(
                    "{report_id} line {line_number}: `{token}` is not a `key=value` pair"
                ));
            };
            if !CLAIM_KEYS.contains(&key) {
                return Err(format!(
                    "{report_id} line {line_number}: unknown key `{key}` (expected {})",
                    CLAIM_KEYS.join(", ")
                ));
            }
            if values.insert(key, value).is_some() {
                return Err(format!("{report_id} line {line_number}: repeated key `{key}`"));
            }
            if value.is_empty() {
                return Err(format!("{report_id} line {line_number}: `{key}` is empty"));
            }
        }
        for key in CLAIM_KEYS {
            if !values.contains_key(key) {
                return Err(format!("{report_id} line {line_number}: missing key `{key}`"));
            }
        }
        claims.push(BoundClaim {
            guardrail: values["guardrail"].to_string(),
            value: values["value"].to_string(),
            tier: values["tier"].to_string(),
            arch: values["arch"].to_string(),
            platform: values["platform"].to_string(),
            qualification: values["qualification"].to_string(),
        });
    }
    Ok(claims)
}

/// One bound-class row of the guardrail evidence register, as the caller
/// already parsed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundEvidenceRow {
    /// `PERF-Dnn-G04`.
    pub guardrail_id: String,
    /// The Story filing it.
    pub story_id: String,
    /// Repository-relative path of the Report holding the raw evidence.
    pub evidence_path: String,
}

/// Whether a guardrail id names a bound-class column.
pub fn is_bound_class(guardrail_id: &str) -> bool {
    guardrail_id
        .split('-')
        .nth(2)
        .is_some_and(|guardrail| BOUND_CLASS_GUARDRAILS.contains(&guardrail))
}

/// Refuses every bound-class evidence row whose provenance disqualifies it.
///
/// Returns how many bound claims were checked, so a caller can tell "nothing
/// was wrong" from "nothing was looked at" — the distinction `ADR 0005`'s trap
/// section exists to preserve.
pub fn check_bound_evidence(
    repo_root: &Path,
    rows: &[BoundEvidenceRow],
    platforms: &PlatformIndex,
) -> Result<usize, String> {
    let mut checked = 0;
    for row in rows.iter().filter(|row| is_bound_class(&row.guardrail_id)) {
        let path = repo_root.join(row.evidence_path.replace('/', std::path::MAIN_SEPARATOR_STR));
        let contents = fs::read_to_string(&path).map_err(|error| {
            format!(
                "guardrail-evidence: `{}` files bound-class `{}` against {}, which cannot be read: \
                 {error}",
                row.story_id,
                row.guardrail_id,
                row.evidence_path
            )
        })?;
        let claims = parse_bound_claims(&contents, &row.evidence_path)?;
        let claim =
            claims.iter().find(|claim| claim.guardrail == row.guardrail_id).ok_or_else(|| {
                format!(
                    "guardrail-evidence: `{}` files bound-class `{}` but {} carries no \
                     `{BOUND_SENTINEL} guardrail={}` line. A bound-class gate is closed by a \
                     provenance-carrying claim, never by a row on its own (LE-33)",
                    row.story_id, row.guardrail_id, row.evidence_path, row.guardrail_id
                )
            })?;
        check_claim(claim, &row.evidence_path, platforms)?;
        checked += 1;
    }
    Ok(checked)
}

/// The three refusals, in the order the two ADRs establish them.
fn check_claim(
    claim: &BoundClaim,
    report_id: &str,
    platforms: &PlatformIndex,
) -> Result<(), String> {
    if DISQUALIFIED_TIERS.contains(&claim.tier.as_str()) {
        return Err(format!(
            "{report_id}: `{}` is claimed as a bound from tier `{}`. A Tier 0 number is a \
             statement about an emulator (ADR 0004; LE-42 is the worked example)",
            claim.guardrail, claim.tier
        ));
    }
    if DISQUALIFIED_ARCHITECTURES.contains(&claim.arch.as_str()) {
        return Err(format!(
            "{report_id}: `{}` is claimed as a bound from arch `{}`. SMIs are invisible, \
             unmaskable and unattributable from the OS's exception level, so any worst case \
             stated on x86_64 is a claim about the firmware (ADR 0004 decision 1, restated \
             unmodified by ADR 0005)",
            claim.guardrail, claim.arch
        ));
    }

    let Some(platform) = platforms.get(&claim.platform) else {
        return Err(format!(
            "{report_id}: `{}` is claimed as a bound from platform `{}`, which is absent from \
             goals/assurance/qualified-platforms.tsv. The default for a platform with no record \
             is not qualified, never presumed clean (ADR 0005 decision 3)",
            claim.guardrail, claim.platform
        ));
    };

    // An unqualified platform cannot be laundered by writing a different
    // architecture beside it, so the claim's own arch must match the register.
    if platform.arch != claim.arch {
        return Err(format!(
            "{report_id}: `{}` claims arch `{}` for platform `{}`, which the register records as \
             `{}`",
            claim.guardrail, claim.arch, claim.platform, platform.arch
        ));
    }
    if platform.state != "qualified" {
        return Err(format!(
            "{report_id}: `{}` is claimed as a bound from platform `{}`, which holds no \
             secure-world qualification record. An ARM64 platform without one produces mechanism \
             evidence, which is real, useful, retained -- and not a bound (ADR 0005 decision 2)",
            claim.guardrail, claim.platform
        ));
    }
    if claim.qualification != platform.qualification_record {
        return Err(format!(
            "{report_id}: `{}` cites qualification record `{}` for platform `{}`, which the \
             register records as `{}`. A qualification record is void for any other firmware \
             version, so the citation must be the registered one",
            claim.guardrail, claim.qualification, claim.platform, platform.qualification_record
        ));
    }
    Ok(())
}

fn tsv_fields<'a>(
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn reports() -> BTreeSet<String> {
        ["REPORT-2026-07-28-10".to_string(), "REPORT-2027-01-01-01".to_string()]
            .into_iter()
            .collect()
    }

    fn register(rows: &str) -> String {
        format!("{PLATFORM_HEADER}\n{rows}")
    }

    fn qualified_pi() -> PlatformIndex {
        validate_platforms(
            &register(
                "rpi5-bcm2712\taarch64\tPi 5\tfw-1.2\tqualified\tREPORT-2027-01-01-01\t2027-01-01\n",
            ),
            &reports(),
        )
        .expect("fixture register is valid")
    }

    // --- clause 1, the register ---------------------------------------------

    #[test]
    fn a_qualified_row_citing_no_record_is_refused() {
        let error = validate_platforms(
            &register("rpi5\taarch64\tPi 5\tfw\tqualified\t-\t2027-01-01\n"),
            &reports(),
        )
        .expect_err("qualified with no record must fail");
        assert!(error.contains("records no qualification record"), "{error}");
    }

    #[test]
    fn a_qualified_row_citing_a_nonexistent_report_is_refused() {
        let error = validate_platforms(
            &register("rpi5\taarch64\tPi 5\tfw\tqualified\tREPORT-1999-01-01-01\t2027-01-01\n"),
            &reports(),
        )
        .expect_err("qualified citing a missing Report must fail");
        assert!(error.contains("not a Report"), "{error}");
    }

    #[test]
    fn an_unqualified_row_citing_a_record_is_refused() {
        let error = validate_platforms(
            &register("rpi5\taarch64\tPi 5\tfw\tunqualified\tREPORT-2027-01-01-01\t2027-01-01\n"),
            &reports(),
        )
        .expect_err("unqualified citing a record must fail");
        assert!(error.contains("is unqualified but cites"), "{error}");
    }

    #[test]
    fn a_duplicate_platform_is_refused() {
        let error = validate_platforms(
            &register(
                "rpi5\taarch64\tPi 5\tfw\tunqualified\t-\t2027-01-01\n\
                 rpi5\taarch64\tPi 5 again\tfw\tunqualified\t-\t2027-01-01\n",
            ),
            &reports(),
        )
        .expect_err("duplicate platform must fail");
        assert!(error.contains("duplicate platform"), "{error}");
    }

    #[test]
    fn a_well_formed_register_parses_and_counts_its_qualified_rows() {
        let index = qualified_pi();
        assert_eq!(index.count(), 1);
        assert_eq!(index.qualified_count(), 1);
    }

    // --- clause 1, the claim line -------------------------------------------

    #[test]
    fn bound_class_is_g04_and_nothing_else() {
        assert!(is_bound_class("PERF-D04-G04"));
        assert!(!is_bound_class("PERF-D04-G03"));
        assert!(!is_bound_class("PERF-D04-G05"));
        assert!(!is_bound_class("PERF-D04-G11"));
    }

    #[test]
    fn a_claim_missing_a_key_is_an_error_not_a_skipped_line() {
        let report = format!(
            "{BOUND_SENTINEL} guardrail=PERF-D04-G04 value=1.2us tier=T1 arch=aarch64 \
             platform=rpi5-bcm2712\n"
        );
        let error = parse_bound_claims(&report, "R").expect_err("missing key must fail");
        assert!(error.contains("missing key `qualification`"), "{error}");
    }

    #[test]
    fn a_claim_with_an_unknown_key_is_refused() {
        let report = format!("{BOUND_SENTINEL} guardrail=PERF-D04-G04 wibble=1\n");
        let error = parse_bound_claims(&report, "R").expect_err("unknown key must fail");
        assert!(error.contains("unknown key `wibble`"), "{error}");
    }

    #[test]
    fn a_line_without_the_sentinel_is_ordinary_prose() {
        let claims = parse_bound_claims("The observed maximum was 1.2 us.\n", "R")
            .expect("prose is not a claim");
        assert!(claims.is_empty());
    }

    // --- clause 2, the positive controls ------------------------------------

    fn claim(tier: &str, arch: &str, platform: &str, qualification: &str) -> BoundClaim {
        BoundClaim {
            guardrail: "PERF-D04-G04".to_string(),
            value: "1.2us".to_string(),
            tier: tier.to_string(),
            arch: arch.to_string(),
            platform: platform.to_string(),
            qualification: qualification.to_string(),
        }
    }

    #[test]
    fn a_tier_0_bound_is_refused() {
        let error = check_claim(
            &claim("T0", "aarch64", "rpi5-bcm2712", "REPORT-2027-01-01-01"),
            "R",
            &qualified_pi(),
        )
        .expect_err("a Tier 0 bound must be refused");
        assert!(error.contains("emulator"), "{error}");
    }

    #[test]
    fn an_x86_64_bound_is_refused() {
        let index = validate_platforms(
            &register("box\tx86_64\tserver\tfw\tqualified\tREPORT-2027-01-01-01\t2027-01-01\n"),
            &reports(),
        )
        .expect("fixture register is valid");
        let error = check_claim(&claim("T2", "x86_64", "box", "REPORT-2027-01-01-01"), "R", &index)
            .expect_err("an x86_64 bound must be refused");
        assert!(error.contains("claim about the firmware"), "{error}");
    }

    #[test]
    fn a_bound_from_an_unqualified_arm64_platform_is_refused() {
        let index = validate_platforms(
            &register("rpi5-bcm2712\taarch64\tPi 5\tfw\tunqualified\t-\t2027-01-01\n"),
            &reports(),
        )
        .expect("fixture register is valid");
        let error = check_claim(&claim("T1", "aarch64", "rpi5-bcm2712", "none"), "R", &index)
            .expect_err("an unqualified platform must be refused");
        assert!(error.contains("holds no secure-world qualification record"), "{error}");
    }

    #[test]
    fn a_bound_from_an_unregistered_platform_is_refused() {
        let error =
            check_claim(&claim("T1", "aarch64", "some-board", "none"), "R", &qualified_pi())
                .expect_err("an unregistered platform must be refused");
        assert!(error.contains("absent from"), "{error}");
    }

    #[test]
    fn a_bound_cannot_launder_its_architecture_past_the_register() {
        let index = validate_platforms(
            &register("box\tx86_64\tserver\tfw\tqualified\tREPORT-2027-01-01-01\t2027-01-01\n"),
            &reports(),
        )
        .expect("fixture register is valid");
        let error =
            check_claim(&claim("T2", "aarch64", "box", "REPORT-2027-01-01-01"), "R", &index)
                .expect_err("a mismatched arch must be refused");
        assert!(error.contains("which the register records as `x86_64`"), "{error}");
    }

    #[test]
    fn a_bound_citing_the_wrong_qualification_record_is_refused() {
        let error = check_claim(
            &claim("T1", "aarch64", "rpi5-bcm2712", "REPORT-2026-07-28-10"),
            "R",
            &qualified_pi(),
        )
        .expect_err("a mismatched qualification record must be refused");
        assert!(error.contains("void for any other firmware version"), "{error}");
    }

    /// The acceptance case. Without it every test above would pass against a
    /// `check_claim` that refused unconditionally, which is a gate that detects
    /// nothing while appearing maximally strict.
    #[test]
    fn a_bound_from_a_qualified_arm64_platform_is_accepted() {
        check_claim(
            &claim("T1", "aarch64", "rpi5-bcm2712", "REPORT-2027-01-01-01"),
            "R",
            &qualified_pi(),
        )
        .expect("a qualified platform's bound is exactly what the ADR permits");
    }

    #[test]
    fn a_bound_class_row_whose_report_carries_no_claim_line_is_refused() {
        let repo_root = repo_root();
        let rows = [BoundEvidenceRow {
            guardrail_id: "PERF-D04-G04".to_string(),
            story_id: "STORY-P1-07-06".to_string(),
            // A real Report that carries no bound claim, which is every Report
            // in the tree today.
            evidence_path: "goals/reports/REPORT-2026-07-28-08.md".to_string(),
        }];
        let error = check_bound_evidence(&repo_root, &rows, &qualified_pi())
            .expect_err("a bound row with no claim line must be refused");
        assert!(error.contains("carries no"), "{error}");
    }

    #[test]
    fn a_non_bound_class_row_is_not_required_to_carry_a_claim_line() {
        let repo_root = repo_root();
        let rows = [BoundEvidenceRow {
            guardrail_id: "PERF-D04-G11".to_string(),
            story_id: "STORY-P0-02-02".to_string(),
            evidence_path: "goals/reports/REPORT-2026-07-28-08.md".to_string(),
        }];
        let checked = check_bound_evidence(&repo_root, &rows, &qualified_pi())
            .expect("a G11 row is not a bound");
        assert_eq!(checked, 0, "no bound was checked, and the count says so");
    }

    // --- the committed tree --------------------------------------------------

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("xtask manifest lives at os/src/xtask")
            .to_path_buf()
    }

    /// `ADR 0005` decision 3 states the count of qualified platforms is zero.
    /// This asserts it as a value, which is the whole reason the register
    /// exists — and it is the test that fails loudly the day someone marks a
    /// platform qualified without a Report to point at.
    #[test]
    fn no_platform_in_the_committed_register_is_qualified() {
        let repo_root = repo_root();
        let contents =
            fs::read_to_string(repo_root.join("goals/assurance/qualified-platforms.tsv"))
                .expect("committed register is readable");
        let mut report_ids = BTreeSet::new();
        for entry in fs::read_dir(repo_root.join("goals/reports")).expect("reports dir") {
            let entry = entry.expect("dir entry");
            if let Some(stem) = entry.path().file_stem().and_then(|stem| stem.to_str()) {
                if stem.starts_with("REPORT-") {
                    report_ids.insert(stem.to_string());
                }
            }
        }
        let index =
            validate_platforms(&contents, &report_ids).expect("committed register is valid");
        assert_eq!(index.qualified_count(), 0, "ADR 0005 decision 3");
        assert!(index.count() >= 5, "the register names the platforms that measure");
    }
}
