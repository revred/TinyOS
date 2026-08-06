//! `check-spine-files` — the instrument `CONCURRENT_SESSIONS` rule 8 names.
//!
//! `LE-36`. Rule 8 asks a session that hand-edits a machine-checked shared file
//! to validate it **before its next tool call**, and the reason the rule needed
//! an instrument is that the full spine check is not fast enough to feel free:
//! it walks every markdown document, resolves every cross-file reference, and
//! scans every shipped crate for allocator use. A session under time pressure
//! skips it, which is how a seven-field row sat in an eight-field file long
//! enough to turn a *different* session's tree red for a reason they could not
//! diagnose.
//!
//! **What this checks, and the correction that shaped it.** `LE-36` originally
//! asked for a field-count guard. A second incident the same day showed that to
//! be under-specified: two sessions each wrote an `LE-43` row, the register went
//! red with `duplicate id LE-43`, and **both rows were well-formed at eight
//! fields** — so a field counter ran, passed, and was right to pass. A duplicate
//! id is a different defect class from a consumed separator. This command
//! therefore checks header agreement, field count, **and key uniqueness**, plus
//! id contiguity where the register demands it.
//!
//! **What it deliberately does not check**: nothing that requires reading a
//! second file. No cross-file id resolution, no markdown walk, no crate scan.
//! That is what keeps it fast, and it is also what makes it a strict subset of
//! `check-assurance-spine` rather than a second opinion that could disagree
//! with it.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// One hand-edited spine TSV and the columns whose combination must be unique.
struct SpineFile {
    /// Repository-relative path.
    path: &'static str,
    /// Column indices forming the row key. A single-column key is the usual
    /// case; `guardrail-evidence` is keyed on `(guardrail, story)` and the
    /// class-communication matrix on `(source, target)`, because in those files
    /// a repeated first column is correct rather than a defect.
    key_columns: &'static [usize],
    /// Prefix that every id in column 0 must carry and count contiguously from
    /// `01`, or `None` where ids are not numbered.
    contiguous_prefix: Option<&'static str>,
}

/// Every TSV a session edits by hand.
///
/// The list is explicit rather than a directory walk: a new register should
/// arrive here by someone deciding it belongs, and a file silently appearing in
/// a directory is exactly the drift `LE-29` describes in a different catalogue.
const SPINE_FILES: [SpineFile; 15] = [
    SpineFile {
        path: "goals/assurance/feature-contracts.tsv",
        key_columns: &[0],
        contiguous_prefix: None,
    },
    SpineFile {
        path: "goals/assurance/story-contracts.tsv",
        key_columns: &[0],
        contiguous_prefix: None,
    },
    SpineFile {
        path: "goals/assurance/loose-ends.tsv",
        key_columns: &[0],
        contiguous_prefix: Some("LE-"),
    },
    SpineFile {
        path: "goals/assurance/guardrail-evidence.tsv",
        key_columns: &[0, 2],
        contiguous_prefix: None,
    },
    SpineFile {
        path: "goals/assurance/open-debt.tsv",
        key_columns: &[0, 1],
        contiguous_prefix: None,
    },
    SpineFile {
        path: "goals/assurance/qualified-platforms.tsv",
        key_columns: &[0],
        contiguous_prefix: None,
    },
    SpineFile {
        path: "goals/security/containment-classes.tsv",
        key_columns: &[0],
        contiguous_prefix: None,
    },
    SpineFile {
        path: "goals/security/containment-tests.tsv",
        key_columns: &[0],
        contiguous_prefix: Some("BND-"),
    },
    SpineFile { path: "goals/security/controls.tsv", key_columns: &[0], contiguous_prefix: None },
    SpineFile {
        path: "goals/security/protection-domain-contracts.tsv",
        key_columns: &[0],
        contiguous_prefix: Some("PD-"),
    },
    SpineFile {
        path: "goals/security/code-admission-gates.tsv",
        key_columns: &[0],
        contiguous_prefix: Some("RCG-"),
    },
    SpineFile {
        path: "goals/security/class-communication-matrix.tsv",
        key_columns: &[0, 1],
        contiguous_prefix: None,
    },
    SpineFile {
        path: "goals/context/application-platforms.tsv",
        key_columns: &[0],
        contiguous_prefix: None,
    },
    SpineFile {
        path: "goals/context/landing-zones.tsv",
        key_columns: &[0],
        contiguous_prefix: None,
    },
    SpineFile {
        path: "goals/performance/catalogue.tsv",
        key_columns: &[0],
        contiguous_prefix: None,
    },
];

/// What the fast check looked at, so a caller can print evidence of coverage
/// rather than a bare "ok".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpineFileSummary {
    /// Files validated.
    pub file_count: usize,
    /// Data rows validated across all of them.
    pub row_count: usize,
}

/// Validates every hand-edited spine TSV relative to `repo_root`.
pub fn check_spine_files(repo_root: &Path) -> Result<SpineFileSummary, String> {
    let mut row_count = 0;
    for file in &SPINE_FILES {
        let path = repo_root.join(file.path.replace('/', std::path::MAIN_SEPARATOR_STR));
        let contents = fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", file.path))?;
        row_count += validate_one(&contents, file)?;
    }
    Ok(SpineFileSummary { file_count: SPINE_FILES.len(), row_count })
}

fn validate_one(contents: &str, file: &SpineFile) -> Result<usize, String> {
    let label = file.path;
    let mut lines = contents.lines();
    let header =
        lines.next().ok_or_else(|| format!("{label}: file is empty"))?.trim_end_matches('\r');
    let expected_fields = header.split('\t').count();
    if expected_fields < 2 {
        return Err(format!("{label}: header is not tab-separated"));
    }

    let mut keys = BTreeSet::new();
    let mut rows = 0;
    for (zero_based_index, raw_line) in lines.enumerate() {
        let line_number = zero_based_index + 2;
        let line = raw_line.trim_end_matches('\r');
        if line.is_empty() {
            return Err(format!("{label} line {line_number}: blank rows are not allowed"));
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() != expected_fields {
            return Err(format!(
                "{label} line {line_number}: expected {expected_fields} tab-separated fields \
                 (the header's count), found {}. A consumed tab is the usual cause",
                fields.len()
            ));
        }
        if let Some(position) = fields.iter().position(|field| field.trim().is_empty()) {
            return Err(format!("{label} line {line_number}: field {} is empty", position + 1));
        }

        let key: Vec<&str> = file.key_columns.iter().map(|&column| fields[column]).collect();
        if !keys.insert(key.clone()) {
            return Err(format!(
                "{label} line {line_number}: duplicate key `{}`. This is the defect class a \
                 field count cannot catch — both duplicate LE-43 rows were well-formed",
                key.join(" / ")
            ));
        }
        rows += 1;

        if let Some(prefix) = file.contiguous_prefix {
            let id = fields[0];
            let Some(number) = id.strip_prefix(prefix).and_then(|n| n.parse::<usize>().ok()) else {
                return Err(format!("{label} line {line_number}: `{id}` is not a `{prefix}NN` id"));
            };
            if number != rows {
                return Err(format!(
                    "{label} line {line_number}: `{id}` is out of order or leaves a gap; ids must \
                     run contiguously from `{prefix}01`"
                ));
            }
        }
    }

    if rows == 0 {
        return Err(format!("{label}: header only, no rows"));
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("xtask manifest lives at os/src/xtask")
            .to_path_buf()
    }

    const LOOSE_ENDS: SpineFile =
        SpineFile { path: "fixture", key_columns: &[0], contiguous_prefix: Some("LE-") };

    fn loose_end_fixture() -> String {
        "le_id\tsummary\tstate\n\
         LE-01\tfirst\topen\n\
         LE-02\tsecond\topen\n"
            .to_string()
    }

    // The defect that started rule 8: a consumed tab.
    #[test]
    fn a_row_missing_a_separator_is_refused() {
        let fixture = loose_end_fixture().replace("LE-02\tsecond", "LE-02 second");
        let error = validate_one(&fixture, &LOOSE_ENDS).expect_err("a 2-field row must fail");
        assert!(error.contains("expected 3 tab-separated fields"), "{error}");
    }

    // The defect that showed a field count to be insufficient: a duplicate id
    // in two rows that are both perfectly well-formed.
    #[test]
    fn a_duplicate_id_in_well_formed_rows_is_refused() {
        let fixture = format!("{}LE-02\tsecond again\topen\n", loose_end_fixture());
        let error = validate_one(&fixture, &LOOSE_ENDS).expect_err("a duplicate id must fail");
        assert!(error.contains("duplicate key `LE-02`"), "{error}");
    }

    #[test]
    fn a_gap_in_the_id_sequence_is_refused() {
        let fixture = loose_end_fixture().replace("LE-02", "LE-04");
        let error = validate_one(&fixture, &LOOSE_ENDS).expect_err("a gap must fail");
        assert!(error.contains("leaves a gap"), "{error}");
    }

    #[test]
    fn an_empty_field_is_refused() {
        let fixture = loose_end_fixture().replace("\tsecond\t", "\t\t");
        let error = validate_one(&fixture, &LOOSE_ENDS).expect_err("an empty field must fail");
        assert!(error.contains("field 2 is empty"), "{error}");
    }

    #[test]
    fn a_composite_key_permits_a_repeated_first_column() {
        const PAIRED: SpineFile =
            SpineFile { path: "fixture", key_columns: &[0, 1], contiguous_prefix: None };
        let fixture = "source\ttarget\tdecision\n\
                       C0\tC1\tallow\n\
                       C0\tC2\tdeny\n";
        assert_eq!(validate_one(fixture, &PAIRED).expect("a matrix repeats its source"), 2);
    }

    #[test]
    fn the_committed_tree_passes() {
        let summary = check_spine_files(&repo_root()).expect("committed spine files are valid");
        assert_eq!(summary.file_count, SPINE_FILES.len());
        assert!(summary.row_count > 600, "the catalogue alone is 625 rows");
    }

    /// `TEST-P0-01-07-A` clause 4's subset property, checked structurally: every
    /// file the fast check reads must be named in the source of the full check.
    /// The fast check can then never pass on a file the full one does not also
    /// examine, which is what makes "run the fast one" safe advice.
    /// Every source file the above test treats as "the full check".
    ///
    /// `assurance` was one 4,400-line file until 2026-08-06 and is now a
    /// directory, which is why [`every_assurance_module_is_part_of_the_full_check`]
    /// exists beside this list: a subset test whose corpus silently stops
    /// covering a module would keep passing while guaranteeing less.
    const FULL_CHECK_SOURCES: [(&str, &str); 13] = [
        ("performance_catalogue.rs", include_str!("performance_catalogue.rs")),
        ("bound_provenance.rs", include_str!("bound_provenance.rs")),
        ("assurance/mod.rs", include_str!("assurance/mod.rs")),
        ("assurance/common.rs", include_str!("assurance/common.rs")),
        ("assurance/context.rs", include_str!("assurance/context.rs")),
        ("assurance/contracts.rs", include_str!("assurance/contracts.rs")),
        ("assurance/documents.rs", include_str!("assurance/documents.rs")),
        ("assurance/ids.rs", include_str!("assurance/ids.rs")),
        ("assurance/loose_ends.rs", include_str!("assurance/loose_ends.rs")),
        ("assurance/registers.rs", include_str!("assurance/registers.rs")),
        ("assurance/release_status.rs", include_str!("assurance/release_status.rs")),
        ("assurance/security_spine.rs", include_str!("assurance/security_spine.rs")),
        ("assurance/status.rs", include_str!("assurance/status.rs")),
    ];

    /// Assurance modules deliberately outside [`FULL_CHECK_SOURCES`].
    ///
    /// Test sources are excluded because including them would make the subset
    /// test below pass for the wrong reason: a spine file named only in a test
    /// fixture would satisfy "the full check reads this file" while no
    /// validator touched it.
    const TEST_ONLY_ASSURANCE_MODULES: [&str; 1] = ["spine_tests.rs"];

    /// The guard the directory split made necessary.
    ///
    /// `include_str!` needs literal paths, so the corpus above is hand-kept.
    /// This reads the directory the compiler read and fails if the two
    /// disagree — so adding `assurance/whatever.rs` and forgetting this list
    /// breaks a test rather than quietly shrinking what the subset test
    /// proves. Same shape as `LE-80`: a hand-kept mirror of a real set, with
    /// nothing checking the mirror. It caught `spine_tests.rs` on its first
    /// run, which is the only reason that file is classified rather than
    /// silently absent.
    #[test]
    fn every_assurance_module_is_part_of_the_full_check() {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/assurance");
        let mut on_disk: Vec<String> = std::fs::read_dir(&directory)
            .expect("the assurance module directory exists")
            .map(|entry| entry.expect("readable entry").file_name().to_string_lossy().into_owned())
            .filter(|name| {
                name.ends_with(".rs") && !TEST_ONLY_ASSURANCE_MODULES.contains(&name.as_str())
            })
            .collect();
        on_disk.sort();
        let mut listed: Vec<String> = FULL_CHECK_SOURCES
            .iter()
            .filter_map(|(name, _)| name.strip_prefix("assurance/").map(str::to_string))
            .collect();
        listed.sort();
        assert_eq!(
            on_disk, listed,
            "FULL_CHECK_SOURCES must name every assurance module; the subset test below is only \
             as strong as this corpus"
        );
    }

    #[test]
    fn every_fast_checked_file_is_also_read_by_the_full_spine_check() {
        let full_check: String =
            FULL_CHECK_SOURCES.iter().map(|(_, source)| *source).collect::<Vec<_>>().join("\n");
        let full_check = full_check.as_str();
        for file in &SPINE_FILES {
            let basename = file.path.rsplit('/').next().expect("path has a basename");
            let stem = basename.trim_end_matches(".tsv");
            assert!(
                full_check.contains(stem),
                "{} is validated by check-spine-files but named nowhere in the full spine check",
                file.path
            );
        }
    }
}
