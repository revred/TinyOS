//! The x86_64 build gate a session actually runs (`LE-92`).
//!
//! [`crate::boot_images`] exists because nothing local compiled anything for
//! AArch64 and three pushes went out green and came back red. This module
//! exists because **a gate written for one architecture after one incident does
//! not generalise itself**, and the second architecture kept the hole the first
//! one closed.
//!
//! Demonstrated 2026-08-06 on commit `fb3f36c`. `fixture_measure.rs` declared
//! `METRICS = 9` and left an eight-element `[None, None, …]` literal beside it;
//! CI's first QEMU job died on `E0308`. Every documented pre-push command was
//! green on that tree — including `check-boot-images`, which had just been
//! added for exactly this class of miss:
//!
//! - `cargo test -p kernel` builds the **host** test harness;
//! - `check-lints` lints the **host** target;
//! - `check-boot-images` builds **AArch64**;
//! - the only things that compile `src/kernel` for `targets/x86_64-tinyos.json`
//!   are `xtask measure` and its siblings, which need QEMU and run CI-side.
//!
//! So the whole Tier 0 fixture set — the set the project's timing evidence
//! comes from — was unbuilt by every gate a session is told to run.
//!
//! # Compilation only, and that is the point
//!
//! This gate does not boot anything. QEMU is what makes the Tier 0 fixtures
//! expensive and CI-side; the *compile* is the part that was missing locally
//! and it is cheap. A gate that needed QEMU would be a gate nobody runs, which
//! is how the hole stayed open.
//!
//! # Why the plan is a value and not a loop
//!
//! Same reason as [`crate::boot_images`], and the same trap: the defect is a
//! **coverage** failure, and coverage is a property of the list. So the plan is
//! a pure function of [`crate::FIXTURES`] — the register `list-fixtures`
//! already prints — and a host test holds it against that register. A fixture
//! added tomorrow is compiled by this gate, or fails a test today on a laptop.

use crate::Fixture;

/// One x86_64 guest binary this gate compiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuestImageBuild {
    /// How the build is named in output — the `--fixture=` value, or
    /// `default` for the no-feature boot.
    pub label: &'static str,
    /// Cargo package.
    pub package: &'static str,
    /// Cargo feature selecting this variant, or `None`.
    pub feature: Option<&'static str>,
}

/// The label for the fixture whose `--fixture=` value is the empty string.
pub const DEFAULT: &str = "default";

/// Every distinct x86_64 guest compilation, derived from the fixture register.
///
/// Deduplicated on `(package, feature)` rather than on fixture name: two
/// fixtures can share one binary (`exec`'s fixture binary needs no feature
/// because the whole binary exists to be that fixture), and compiling the same
/// artifact twice costs a session time and proves nothing.
#[must_use]
pub fn build_plan(fixtures: &'static [Fixture]) -> Vec<GuestImageBuild> {
    let mut plan: Vec<GuestImageBuild> = Vec::new();
    for fixture in fixtures {
        if plan
            .iter()
            .any(|built| built.package == fixture.package && built.feature == fixture.feature)
        {
            continue;
        }
        plan.push(GuestImageBuild {
            label: if fixture.name.is_empty() { DEFAULT } else { fixture.name },
            package: fixture.package,
            feature: fixture.feature,
        });
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::FIXTURES;

    /// The coverage claim, held against the fixture register rather than
    /// against a second hand-maintained list. This is the test that makes the
    /// gate a gate: a fixture registered and never compiled fails here in
    /// milliseconds instead of on a runner in minutes.
    #[test]
    fn every_registered_fixture_is_compiled_by_this_gate() {
        let plan = build_plan(FIXTURES);
        for fixture in FIXTURES {
            assert!(
                plan.iter()
                    .any(|build| build.package == fixture.package
                        && build.feature == fixture.feature),
                "fixture `{}` is registered but this gate never compiles it",
                if fixture.name.is_empty() { DEFAULT } else { fixture.name }
            );
        }
    }

    /// The crate `LE-92` was raised over. Pinned by name so a future narrowing
    /// of the register cannot quietly drop the one that failed.
    #[test]
    fn the_kernel_guest_the_type_error_reached_ci_through_is_in_the_plan() {
        let plan = build_plan(FIXTURES);
        assert!(
            plan.iter().any(|build| build.package == "kernel"),
            "kernel is the package whose Tier 0 fixture broke CI on fb3f36c"
        );
        assert!(
            plan.iter().any(|build| build.package == "kernel" && build.feature.is_none()),
            "including the default boot, which no feature selects"
        );
    }

    /// Compiling one artifact twice is time a session pays for nothing.
    #[test]
    fn no_artifact_is_compiled_twice() {
        let plan = build_plan(FIXTURES);
        for (index, build) in plan.iter().enumerate() {
            assert!(
                !plan[index + 1..]
                    .iter()
                    .any(|later| later.package == build.package && later.feature == build.feature),
                "{} duplicates an earlier compilation",
                build.label
            );
        }
    }

    /// Deduplication must collapse shared artifacts and **must not** collapse
    /// distinct ones. A dedup keyed on package alone would silently drop every
    /// feature variant of `kernel`, which is the entire Tier 0 fixture set —
    /// the gate would still pass, still print a plausible count, and cover
    /// almost nothing.
    #[test]
    fn deduplication_collapses_shared_artifacts_and_keeps_distinct_ones() {
        const SAMPLE: &[Fixture] = &[
            Fixture {
                name: "",
                package: "kernel",
                binary: "kernel",
                feature: None,
                expects_failure: false,
                owning_test: "TEST-P0-01-01-A",
                summary: "",
            },
            Fixture {
                name: "twin",
                package: "kernel",
                binary: "kernel",
                feature: None,
                expects_failure: false,
                owning_test: "TEST-P0-01-01-A",
                summary: "",
            },
            Fixture {
                name: "featured",
                package: "kernel",
                binary: "kernel",
                feature: Some("fixture-featured"),
                expects_failure: false,
                owning_test: "TEST-P0-01-01-A",
                summary: "",
            },
            Fixture {
                name: "other-package",
                package: "exec",
                binary: "exec-fixture",
                feature: None,
                expects_failure: false,
                owning_test: "TEST-P0-05-02-A",
                summary: "",
            },
        ];
        let plan = build_plan(SAMPLE);
        assert_eq!(plan.len(), 3, "the twin collapses; the feature and the package do not");
        assert_eq!(plan[0].label, DEFAULT, "the empty name renders as `default`");
        assert!(plan.iter().any(|b| b.feature == Some("fixture-featured")));
        assert!(plan.iter().any(|b| b.package == "exec"));
    }
}
