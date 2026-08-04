//! The AArch64 build gate a session actually runs (`LE-72`).
//!
//! `LE-72` was raised after two red pushes, and then a third arrived anyway.
//! That third one is the whole reason this module exists, so it is worth
//! writing down rather than summarising:
//!
//! - `cargo test`, `cargo fmt --check` and host clippy all pass on a tree
//!   whose AArch64 boot image does not link. None of them compiles `kernel`
//!   for the target.
//! - The gate adopted after the first two failures was
//!   `xtask pi5 --fixture=measure`. **CI does not run that.** CI builds
//!   `pi5-image` *featureless*, and `xtask pi5` cannot produce a featureless
//!   image at all — the subcommand requires `--fixture`.
//! - `--fixture=measure` pulls `kernel` into the link for its own reasons,
//!   which masked a missing `#[used]` reference perfectly. The featureless
//!   image is the one that caught it: an rlib nothing references is dropped
//!   from the link entirely, taking its `#[no_mangle]` definitions with it.
//!
//! So **"the AArch64 build" is plural**, and a gate that builds one of them is
//! a gate that will keep missing the others. This module builds every one:
//! the featureless image CI builds, and each registered fixture variant.
//!
//! # Why the plan is a value and not a loop
//!
//! The defect `LE-72` records is not a build failure — it is a *coverage*
//! failure, and coverage is a property of the list, not of the compiler. So
//! the list is a pure function this module's tests hold against
//! [`pi5::PI5_FIXTURES`]: a fixture registered and not built fails a host test
//! in milliseconds instead of a runner in minutes.

use crate::pi5;

/// One AArch64 build this gate performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootImageBuild {
    /// How the build is named in output — the `--fixture=` value, or
    /// `featureless`.
    pub label: &'static str,
    /// Cargo feature selecting this variant, or `None` for the image CI
    /// builds.
    pub feature: Option<&'static str>,
}

/// The label for the build with no fixture feature — the one CI performs and
/// the one `xtask pi5` structurally cannot.
pub const FEATURELESS: &str = "featureless";

/// Every AArch64 image variant, featureless first.
///
/// Featureless **first** deliberately: it is the variant a fixture build can
/// mask, so a session that reads only the first line of output reads the one
/// that catches the dropped-rlib class.
#[must_use]
pub fn build_plan() -> Vec<BootImageBuild> {
    let mut plan = vec![BootImageBuild { label: FEATURELESS, feature: None }];
    for fixture in pi5::PI5_FIXTURES {
        // A fixture with no feature *is* the featureless build; registering it
        // twice would double the runtime and prove nothing.
        if fixture.feature.is_none() {
            continue;
        }
        plan.push(BootImageBuild { label: fixture.name, feature: fixture.feature });
    }
    plan
}

/// One package clippy must lint **for the AArch64 target**, not just the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClippyTarget {
    /// Cargo package name.
    pub package: &'static str,
    /// Restrict to the library target.
    ///
    /// True only for `kernel`, and for a real reason rather than to quiet an
    /// error: `kernel`'s `[[bin]]` is the Tier 0 QEMU guest and names
    /// `hal_x86_64` types directly. It is not an AArch64 artifact and building
    /// it for this target is meaningless. `kernel`'s **library** is the thing
    /// the boot image links and the thing whose target-gating broke twice, so
    /// that is what gets linted.
    pub lib_only: bool,
}

/// Every package clippy lints for the board target.
///
/// `-p hal-arm64` alone was the local and CI habit, and it is exactly two
/// crates short: `kernel` is the crate whose target-gating broke the link
/// twice, and `pi5-image` is where the `#[used]` link seam lives. A lint gate
/// that cannot see the crate that failed is not a gate over it.
pub const CLIPPY_TARGETS: &[ClippyTarget] = &[
    ClippyTarget { package: "hal-arm64", lib_only: false },
    ClippyTarget { package: "kernel", lib_only: true },
    ClippyTarget { package: "pi5-image", lib_only: false },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The row `LE-72` was rewritten to demand: the featureless image is a
    /// first-class build here, because it is the one `xtask pi5` cannot make.
    #[test]
    fn the_featureless_image_ci_builds_is_in_the_plan() {
        let plan = build_plan();
        assert_eq!(plan[0], BootImageBuild { label: FEATURELESS, feature: None });
    }

    /// The coverage claim, held against the fixture register rather than
    /// against a second hand-maintained list — a fixture added tomorrow is
    /// built by this gate or fails this test today.
    #[test]
    fn every_registered_fixture_variant_is_built() {
        let plan = build_plan();
        for fixture in pi5::PI5_FIXTURES {
            let Some(feature) = fixture.feature else {
                continue;
            };
            assert!(
                plan.iter().any(|build| build.feature == Some(feature)),
                "fixture {} is registered but this gate never builds it",
                fixture.name
            );
        }
    }

    /// Building the same image twice costs a session time and proves nothing;
    /// the featureless build already covers a featureless fixture.
    #[test]
    fn no_variant_is_built_twice() {
        let plan = build_plan();
        for (index, build) in plan.iter().enumerate() {
            assert!(
                !plan[index + 1..].iter().any(|later| later.feature == build.feature),
                "{} duplicates an earlier build",
                build.label
            );
        }
    }

    /// `-p hal-arm64` was the habit and it is two crates short of the crate
    /// that actually broke. Pinned so narrowing it again is a test failure.
    #[test]
    fn clippy_sees_the_crate_that_broke_the_link_not_only_the_hal() {
        let linted = |package| CLIPPY_TARGETS.iter().any(|target| target.package == package);
        assert!(linted("kernel"), "kernel is the crate LE-72 was raised over");
        assert!(linted("pi5-image"), "and pi5-image holds the link seam");
        assert!(linted("hal-arm64"));
    }

    /// The one narrowing that is legitimate, pinned so it stays the only one:
    /// `kernel`'s bin is the x86_64 Tier 0 guest and is not an AArch64
    /// artifact. Widening `lib_only` to another crate would be hiding a
    /// failure rather than scoping a lint.
    #[test]
    fn only_the_kernel_library_is_linted_and_only_the_kernel() {
        for target in CLIPPY_TARGETS {
            assert_eq!(
                target.lib_only,
                target.package == "kernel",
                "{} scopes clippy to its library",
                target.package
            );
        }
    }
}
