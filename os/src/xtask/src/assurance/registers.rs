//! The two evidence registers and the compiler-enforced property one of them
//! records: `guardrail-evidence.tsv`, `open-debt.tsv`, and the no-heap gate.
//!
//! Both registers are counts of *evidence*, never scores. A gate absent from
//! the evidence register is unevidenced, which is what it is; it is never
//! "passed", and no Story's assurance state is derived from either file.
//!
//! The pairing is deliberate. `LE-35`: selecting a performance domain pulls all
//! 25 of its guardrails into the selecting Story's contract, and where the
//! subsystem does not exist not one of them can be closed — so a selection is
//! either satisfiable or stated open debt, and the two registers are checked
//! against each other rather than separately.

use super::*;

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
pub(super) fn validate_guardrail_evidence(
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
    let mut evidenced_gates = BTreeSet::new();
    let mut refused_gates = BTreeSet::new();
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

        // The `evidence_kind` vocabulary is closed. It accepted any string
        // until 2026-08-05, which is why `refused` could not have been relied
        // on to mean anything: a typo would have read as a novel kind and
        // counted as evidence in silence.
        match fields[3] {
            EVIDENCE_KIND_REFUSED => {
                refused_gates.insert(guardrail_id.to_string());
            }
            kind if EVIDENCE_KINDS.contains(&kind) => {
                evidenced_gates.insert(guardrail_id.to_string());
            }
            kind => {
                return Err(format!(
                    "guardrail-evidence line {line_number}: `{kind}` is not an evidence kind; \
                     expected one of {}",
                    EVIDENCE_KINDS.join(", ")
                ));
            }
        }

        // Bound-class rows are handed to `bound_provenance`, which is where
        // `ADR 0004`'s and `ADR 0005`'s refusals live. Filtering here rather
        // than there keeps the register's parser in one place.
        //
        // A `refused` row is deliberately not among them: it asserts no bound,
        // so holding it to the provenance rules would refuse a row whose whole
        // content is that a bound was *not* claimed.
        if fields[3] != EVIDENCE_KIND_REFUSED && bound_provenance::is_bound_class(guardrail_id) {
            bound_rows.push(bound_provenance::BoundEvidenceRow {
                guardrail_id: guardrail_id.to_string(),
                story_id: story_id.to_string(),
                evidence_path: fields[4].to_string(),
            });
        }
    }

    // `seen` is the (guardrail, story) pairs the duplicate check needs; the
    // published count is the distinct gates those pairs cover, and only the
    // ones covered by a row that is actually evidence.
    Ok(GuardrailEvidenceIndex {
        count: evidenced_gates.len(),
        refused_gates,
        story_domain_pairs,
        bound_rows,
    })
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
pub(super) fn validate_open_debt(
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
pub(super) fn validate_open_debt_coverage(
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
pub(super) fn validate_no_heap(repo_root: &Path) -> Result<(), String> {
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

/// The published evidence figure counts **gates**, not rows.
///
/// Pinned against the committed register precisely because the two numbers
/// differ there: `PERF-D07-G11` carries three rows (`STORY-P0-03-01`,
/// `STORY-P1-10-02`, `STORY-P1-10-04`), all legitimate — each Story selects
/// `D07` and the compiler-enforced no-heap property holds for each. Three
/// honest rows, one gate. A row count would publish 22 where 20 gates have
/// evidence, and would keep climbing every time another Story selected an
/// already-evidenced domain.
#[cfg(test)]
mod evidence_counts_gates_tests {
    use super::*;

    /// `(rows, gates carrying evidence, gates carrying only a refusal)`.
    ///
    /// The third value exists because this test's independent recount has to
    /// use the *same* definition of "a gate with evidence" as the validator,
    /// or it is not an independent check of the count — it is a check that two
    /// pieces of code parse a file the same way. When `refused` joined the
    /// vocabulary on 2026-08-06 this helper silently disagreed and the test
    /// caught it, which is the behaviour wanted.
    fn committed_register() -> (usize, usize, usize) {
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("xtask manifest lives at os/src/xtask")
            .to_path_buf();
        let path = repo_root.join("goals").join("assurance").join("guardrail-evidence.tsv");
        let contents = fs::read_to_string(&path).expect("the register is committed");
        let mut rows = 0;
        let mut evidenced = BTreeSet::new();
        let mut refused = BTreeSet::new();
        for line in contents.lines().skip(1).filter(|line| !line.trim().is_empty()) {
            rows += 1;
            let fields: Vec<&str> = line.split('\t').collect();
            let id = fields.first().copied().unwrap_or_default().to_string();
            if fields.get(3).copied() == Some(EVIDENCE_KIND_REFUSED) {
                refused.insert(id);
            } else {
                evidenced.insert(id);
            }
        }
        let refused_only = refused.difference(&evidenced).count();
        (rows, evidenced.len(), refused_only)
    }

    /// A refusal is recorded and is not counted, and both halves matter.
    ///
    /// Counting it would be `LE-83`'s numerator defect with the opposite sign;
    /// *not recording it* is the state that let `STORY-P1-06-01`'s reasoned
    /// `PERF-D03-G20` refusal live only in Report prose for seven days, where
    /// `09A` §8 step 1 would have walked straight into re-filing it.
    #[test]
    fn the_committed_register_records_a_refusal_and_does_not_count_it() {
        let (_, evidenced, refused_only) = committed_register();
        assert!(
            refused_only > 0,
            "the register should carry at least the PERF-D03-G20 refusal; without one, the \
             exclusion below is untested against real data"
        );
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("xtask manifest lives at os/src/xtask")
            .to_path_buf();
        let summary = check_assurance_spine(&repo_root).expect("the committed spine is valid");
        assert_eq!(summary.guardrail_evidence_count, evidenced);
        assert_ne!(
            summary.guardrail_evidence_count,
            evidenced + refused_only,
            "a refused gate must not reach the published numerator"
        );
    }

    /// The condition that makes this test able to fail at all. If the register
    /// ever holds one row per gate, the assertion below passes for the wrong
    /// reason and this guard says so instead of going quietly green.
    #[test]
    fn the_committed_register_actually_distinguishes_the_two_counts() {
        let (rows, gates, _) = committed_register();
        assert!(
            rows > gates,
            "this test is only meaningful while some gate carries more than one row              (rows={rows}, gates={gates}); if that stops being true, pin a fixture instead"
        );
    }

    #[test]
    fn the_published_count_is_the_gate_count_not_the_row_count() {
        let (rows, gates, _) = committed_register();
        let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("xtask manifest lives at os/src/xtask")
            .to_path_buf();
        let summary = check_assurance_spine(&repo_root).expect("the committed spine is valid");
        assert_eq!(
            summary.guardrail_evidence_count, gates,
            "the figure the dashboard publishes must be gates with evidence"
        );
        assert_ne!(
            summary.guardrail_evidence_count,
            rows,
            "a row count would overstate coverage by {}",
            rows - gates
        );
    }
}
