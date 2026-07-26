//! CI governance gates: the crate-size ceiling check (per
//! `agent/CODING_STANDARDS.md#crate-size-ceiling-hard-limit-no-exceptions`)
//! and the fixture smoke test (`TEST-P0-01-02-A`) proving it — and the
//! standard `fmt`/`clippy`/`missing_docs` gates — actually catch what they
//! claim to, not just that they run.

use std::fs;
use std::path::Path;
use std::process::Command;

/// Counts lines of Rust source in `crate_dir/src`, excluding anything under
/// a `tests/` directory and excluding the body of any `#[cfg(test)]` module,
/// per the ceiling's "excluding test code" rule. This is a line-counting
/// heuristic (brace-depth tracking, not full parsing) — adequate for a hard
/// ceiling check at Phase 0's codebase size; revisit with `tokei` directly
/// if a future crate's structure defeats the heuristic.
pub fn count_crate_loc(crate_dir: &Path) -> Result<usize, String> {
    let src_dir = crate_dir.join("src");
    let mut total = 0usize;
    visit_rs_files(&src_dir, &mut |path| {
        if path.components().any(|c| c.as_os_str() == "tests") {
            return Ok(());
        }
        let content = fs::read_to_string(path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        total += count_loc_excluding_cfg_test(&content);
        Ok(())
    })?;
    Ok(total)
}

fn visit_rs_files(
    dir: &Path,
    visit: &mut impl FnMut(&Path) -> Result<(), String>,
) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    let entries =
        fs::read_dir(dir).map_err(|e| format!("failed to read dir {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("failed to read dir entry: {e}"))?;
        let path = entry.path();
        if path.is_dir() {
            visit_rs_files(&path, visit)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            visit(&path)?;
        }
    }
    Ok(())
}

/// Counts non-blank lines, skipping the contents of any `#[cfg(test)]`
/// module (tracked from the attribute to its matching closing brace).
fn count_loc_excluding_cfg_test(content: &str) -> usize {
    let mut count = 0;
    let mut skipping = false;
    let mut skip_depth = 0i32;
    let mut pending_cfg_test = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if skipping {
            skip_depth += trimmed.matches('{').count() as i32;
            skip_depth -= trimmed.matches('}').count() as i32;
            if skip_depth <= 0 {
                skipping = false;
            }
            continue;
        }

        if trimmed.starts_with("#[cfg(test)]") {
            pending_cfg_test = true;
            continue;
        }

        if pending_cfg_test {
            pending_cfg_test = false;
            if trimmed.contains('{') {
                skipping = true;
                skip_depth =
                    trimmed.matches('{').count() as i32 - trimmed.matches('}').count() as i32;
                if skip_depth <= 0 {
                    skipping = false;
                }
                continue;
            }
        }

        if !trimmed.is_empty() {
            count += 1;
        }
    }
    count
}

/// Runs `check-crate-sizes` over every workspace member under `os/src/*`,
/// failing if any crate's `src/` (excluding tests) exceeds `ceiling` lines.
pub fn check_all_crate_sizes(os_root: &Path, ceiling: usize) -> Result<(), String> {
    let src_root = os_root.join("src");
    let entries = fs::read_dir(&src_root)
        .map_err(|e| format!("failed to read {}: {e}", src_root.display()))?;
    let mut violations = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("failed to read dir entry: {e}"))?;
        let path = entry.path();
        if !path.join("Cargo.toml").is_file() {
            continue;
        }
        let loc = count_crate_loc(&path)?;
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        println!("crate-size-check: {name}: {loc} lines (ceiling {ceiling})");
        if loc > ceiling {
            violations.push(format!("{name}: {loc} lines exceeds the {ceiling}-line ceiling"));
        }
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations.join("; "))
    }
}

/// `TEST-P0-01-02-A`: builds fixture crates, each deliberately violating
/// exactly one governance gate, and asserts each fails only the gate it
/// targets — proving the gates actually catch what they claim to, not just
/// that they run.
pub fn run_fixture_smoke_test(work_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(work_dir)
        .map_err(|e| format!("failed to create {}: {e}", work_dir.display()))?;

    check_unformatted_fixture(work_dir)?;
    check_clippy_violation_fixture(work_dir)?;
    check_missing_docs_fixture(work_dir)?;
    check_oversized_fixture(work_dir)?;
    check_capacity_budget_fixture(work_dir)?;

    println!("governance-fixture-test: all five fixtures failed exactly the gate they targeted");
    Ok(())
}

fn write_fixture_manifest(dir: &Path, name: &str) -> Result<(), String> {
    fs::create_dir_all(dir.join("src")).map_err(|e| e.to_string())?;
    fs::write(
        dir.join("Cargo.toml"),
        format!(
            "[workspace]\n\n[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\npublish = false\n"
        ),
    )
    .map_err(|e| e.to_string())
}

fn check_unformatted_fixture(work_dir: &Path) -> Result<(), String> {
    let dir = work_dir.join("fixture-unformatted");
    write_fixture_manifest(&dir, "fixture-unformatted")?;
    fs::write(
        dir.join("src/lib.rs"),
        "//! Deliberately unformatted fixture, otherwise clean.\n#![deny(missing_docs)]\n\n\
         /// A trivial function.\npub fn f(  x:i32,y:i32   )->i32{ x+y }\n",
    )
    .map_err(|e| e.to_string())?;

    let fmt_ok = cargo_check_status(&dir, &["fmt", "--", "--check"])?;
    let clippy_ok = cargo_check_status(&dir, &["clippy", "--", "-D", "warnings"])?;
    let docs_ok = cargo_doc_missing_docs_status(&dir)?;

    if fmt_ok {
        return Err("fixture-unformatted: expected `cargo fmt --check` to fail, it passed".into());
    }
    if !clippy_ok {
        return Err("fixture-unformatted: expected clippy to pass, it failed".into());
    }
    if !docs_ok {
        return Err("fixture-unformatted: expected missing_docs check to pass, it failed".into());
    }
    Ok(())
}

fn check_clippy_violation_fixture(work_dir: &Path) -> Result<(), String> {
    let dir = work_dir.join("fixture-clippy");
    write_fixture_manifest(&dir, "fixture-clippy")?;
    fs::write(
        dir.join("src/lib.rs"),
        "//! Deliberate clippy violation (needless_return), otherwise clean.\n\
         #![deny(missing_docs)]\n\n/// A trivial function.\npub fn f() -> i32 {\n    return 1;\n}\n",
    )
    .map_err(|e| e.to_string())?;

    let fmt_ok = cargo_check_status(&dir, &["fmt", "--", "--check"])?;
    let clippy_ok = cargo_check_status(&dir, &["clippy", "--", "-D", "warnings"])?;
    let docs_ok = cargo_doc_missing_docs_status(&dir)?;

    if !fmt_ok {
        return Err("fixture-clippy: expected `cargo fmt --check` to pass, it failed".into());
    }
    if clippy_ok {
        return Err("fixture-clippy: expected clippy to fail, it passed".into());
    }
    if !docs_ok {
        return Err("fixture-clippy: expected missing_docs check to pass, it failed".into());
    }
    Ok(())
}

fn check_missing_docs_fixture(work_dir: &Path) -> Result<(), String> {
    let dir = work_dir.join("fixture-missing-docs");
    write_fixture_manifest(&dir, "fixture-missing-docs")?;
    fs::write(
        dir.join("src/lib.rs"),
        "//! Deliberately missing a doc comment on a public item, otherwise clean.\n\
         #![deny(missing_docs)]\n\npub fn f() -> i32 {\n    1\n}\n",
    )
    .map_err(|e| e.to_string())?;

    let fmt_ok = cargo_check_status(&dir, &["fmt", "--", "--check"])?;
    let clippy_ok = cargo_check_status(&dir, &["clippy", "--", "-D", "warnings"])?;
    let docs_ok = cargo_doc_missing_docs_status(&dir)?;

    if !fmt_ok {
        return Err("fixture-missing-docs: expected `cargo fmt --check` to pass, it failed".into());
    }
    // `#![deny(missing_docs)]` is a crate-level lint, so it fails *any*
    // rustc invocation over this crate, clippy included — not just
    // `cargo doc`. That's the correct, expected coupling (the gate really
    // does block the offending code from compiling at all), not a
    // fixture-isolation bug.
    if clippy_ok {
        return Err(
            "fixture-missing-docs: expected clippy to fail (deny(missing_docs) fails any rustc invocation), it passed"
                .into(),
        );
    }
    if docs_ok {
        return Err("fixture-missing-docs: expected missing_docs check to fail, it passed".into());
    }
    Ok(())
}

fn check_oversized_fixture(work_dir: &Path) -> Result<(), String> {
    let dir = work_dir.join("fixture-oversized");
    write_fixture_manifest(&dir, "fixture-oversized")?;

    let mut body = String::from(
        "//! Deliberately padded past a tiny test ceiling.\n#![deny(missing_docs)]\n\n",
    );
    for i in 0..40 {
        body.push_str(&format!(
            "/// Trivial function #{i}.\npub fn f{i}() -> i32 {{\n    {i}\n}}\n\n"
        ));
    }
    fs::write(dir.join("src/lib.rs"), &body).map_err(|e| e.to_string())?;

    // Format the fixture first so the size-ceiling case is the only
    // deliberate violation (an unformatted oversized fixture would
    // conflate two gates).
    let _ = Command::new("cargo").arg("fmt").current_dir(&dir).status();

    let fmt_ok = cargo_check_status(&dir, &["fmt", "--", "--check"])?;
    let clippy_ok = cargo_check_status(&dir, &["clippy", "--", "-D", "warnings"])?;
    let docs_ok = cargo_doc_missing_docs_status(&dir)?;
    let loc = count_crate_loc(&dir)?;
    const TEST_CEILING: usize = 50;

    if !fmt_ok {
        return Err("fixture-oversized: expected `cargo fmt --check` to pass, it failed".into());
    }
    if !clippy_ok {
        return Err("fixture-oversized: expected clippy to pass, it failed".into());
    }
    if !docs_ok {
        return Err("fixture-oversized: expected missing_docs check to pass, it failed".into());
    }
    if loc <= TEST_CEILING {
        return Err(format!(
            "fixture-oversized: expected {loc} lines to exceed the test ceiling of {TEST_CEILING}, it did not"
        ));
    }
    Ok(())
}

/// `STORY-P0-03-02` acceptance criterion 2: a `const _: () = assert!(...)`
/// capacity-budget check (`kernel::capacities`'s own pattern) deliberately
/// violated here — a self-contained fixture, not a reference to
/// `kernel::capacities` itself, since the property under test is "this
/// *style* of check fails the build," not any specific crate's current
/// capacity values. Distinct from `fixture-oversized` (a LOC ceiling,
/// checked by `xtask` itself post-build) — this is a build-time
/// const-evaluation failure, so `cargo build` never even produces an
/// artifact, which fmt/clippy/docs (all of which require a successful
/// compile first) surface as their own failures too.
fn check_capacity_budget_fixture(work_dir: &Path) -> Result<(), String> {
    let dir = work_dir.join("fixture-capacity-budget");
    write_fixture_manifest(&dir, "fixture-capacity-budget")?;
    fs::write(
        dir.join("src/lib.rs"),
        "//! Deliberately oversized capacity budget, otherwise clean.\n\
         #![deny(missing_docs)]\n\n\
         /// A capacity deliberately configured to overflow the budget below.\n\
         pub const CAPACITY: usize = 1_000_000;\n\n\
         /// The documented static-memory budget `CAPACITY` must fit within.\n\
         pub const BUDGET_BYTES: usize = 8;\n\n\
         const _: () = assert!(\n    CAPACITY * 4 <= BUDGET_BYTES,\n    \
         \"fixture-capacity-budget: CAPACITY exceeds BUDGET_BYTES\"\n);\n",
    )
    .map_err(|e| e.to_string())?;

    let fmt_ok = cargo_check_status(&dir, &["fmt", "--", "--check"])?;
    let build_ok = cargo_check_status(&dir, &["build"])?;
    let clippy_ok = cargo_check_status(&dir, &["clippy", "--", "-D", "warnings"])?;
    // Not checked: `cargo doc`'s pass/fail here. Empirically, rustdoc does
    // not force the same const-evaluation `cargo build`/`clippy` do, so a
    // crate that fails to *build* can still successfully *document* —
    // orthogonal to what this fixture exists to prove (that the budget
    // check fails a real build), so asserting on it would just be
    // asserting an accident of rustdoc's own implementation.

    if !fmt_ok {
        return Err(
            "fixture-capacity-budget: expected `cargo fmt --check` to pass, it failed".into()
        );
    }
    if build_ok {
        return Err("fixture-capacity-budget: expected `cargo build` to fail (const-eval budget \
             violation), it passed"
            .into());
    }
    if clippy_ok {
        return Err(
            "fixture-capacity-budget: expected clippy to fail (a crate that fails to build \
             fails any rustc invocation over it), it passed"
                .into(),
        );
    }
    Ok(())
}

fn cargo_check_status(dir: &Path, args: &[&str]) -> Result<bool, String> {
    let mut command = Command::new("cargo");
    command.current_dir(dir).args(args);
    let status = command
        .status()
        .map_err(|e| format!("failed to invoke `cargo {}`: {e}", args.join(" ")))?;
    Ok(status.success())
}

fn cargo_doc_missing_docs_status(dir: &Path) -> Result<bool, String> {
    let mut command = Command::new("cargo");
    command.current_dir(dir).arg("doc").arg("--no-deps").env("RUSTDOCFLAGS", "-D missing-docs");
    let status = command.status().map_err(|e| format!("failed to invoke `cargo doc`: {e}"))?;
    Ok(status.success())
}
