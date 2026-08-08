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
///
/// **`shell` joined on 2026-08-08 (`21A` §4, `LE-123`), and it is here for a
/// different reason than the other three.** They are board crates that a
/// regression could stop compiling. `shell` is the crate that *never* compiled
/// for this target — its library is arch-neutral by construction, but its
/// manifest named `hal-x86_64` as a crate-wide dependency for the benefit of
/// its x86_64 fixture binary, so the board target dragged an x86 HAL it never
/// calls into. That is why the usable OS lived on one architecture and the
/// board ran instruments on another. Clippy for the target both compiles and
/// lints, so one row here is the whole gate.
///
/// This entry is written **before** the manifest that satisfies it, on purpose:
/// the dependency is exactly the kind someone re-adds while fixing an unrelated
/// fixture, and a crate that compiles for the board today and silently stops
/// tomorrow is the failure this module exists to catch.
pub const CLIPPY_TARGETS: &[ClippyTarget] = &[
    ClippyTarget { package: "hal-arm64", lib_only: false },
    ClippyTarget { package: "kernel", lib_only: true },
    ClippyTarget { package: "pi5-image", lib_only: false },
    ClippyTarget { package: "shell", lib_only: true },
];

/// Packages whose HOST lint must be checked one at a time (`LE-77`).
///
/// `cargo clippy --workspace --all-targets` looks like it covers everything and
/// on this bench it does not: `kernel`'s `[[bin]]` names `hal_x86_64` items that
/// are `cfg(not(windows))`, so on a Windows host that target fails to compile,
/// cargo stops, and **every package after it goes unlinted**. An `unused_import`
/// in `hal-arm64` reached CI that way — the one place it could still be caught.
///
/// Linting per package means one crate's failure reports itself instead of
/// hiding the next crate's. `kernel` is scoped to `--lib` for the same reason
/// `CLIPPY_TARGETS` scopes it: its bin is the x86_64 Tier 0 guest, and on a
/// Windows host it cannot build at all.
pub const HOST_LINT_TARGETS: &[HostLintTarget] = &[
    HostLintTarget { package: "hal", bin_only: false },
    HostLintTarget { package: "hal-arm64", bin_only: false },
    HostLintTarget { package: "hal-x86_64", bin_only: false },
    HostLintTarget { package: "kernel", bin_only: false },
    HostLintTarget { package: "exec", bin_only: false },
    HostLintTarget { package: "shell", bin_only: false },
    HostLintTarget { package: "motion", bin_only: false },
    // `STORY-P1-09-18` gave this crate its first unit tests — the wire shell's
    // grant set, its seed and its stack budget — and it was not in this list,
    // so nothing local linted them and a `clippy::assertions_on_constants`
    // reached CI past a green local gate set (run 31256380768). `bin_only`
    // because it genuinely has no library: on a non-AArch64 host it is an
    // inert `main` stub, which builds everywhere, so `--all-targets` is safe
    // here in a way it is not for the `cfg(not(windows))` fixture bins above.
    HostLintTarget { package: "pi5-image", bin_only: true },
    HostLintTarget { package: "xtask", bin_only: true },
];

/// One package linted for the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostLintTarget {
    /// Cargo package name.
    pub package: &'static str,
    /// This package has **no library target**, so `--lib` is an error rather
    /// than a narrowing. True only for `xtask`, which is a binary and whose
    /// binary builds on every host — unlike the fixture bins in `exec`,
    /// `shell` and `kernel`, which are `cfg(not(windows))` and are left to
    /// CI's Linux workspace run.
    pub bin_only: bool,
}

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

    /// The crate the masking hid must be in the per-package host list, or this
    /// gate would not have caught what CI caught.
    #[test]
    fn the_host_lint_list_covers_the_crate_the_workspace_run_never_reached() {
        let linted = |p| HOST_LINT_TARGETS.iter().any(|t| t.package == p);
        assert!(linted("hal-arm64"), "the crate whose unused import reached CI");
        assert!(linted("kernel"));
        assert!(linted("shell"));
    }

    /// Every workspace member this gate is responsible for is in the list.
    ///
    /// **Written after run 31256380768 went red on the runner and green here**
    /// (2026-08-08). `STORY-P1-09-18` put unit tests into `pi5-image` for the
    /// first time — the wire shell's grant set, its seeding and its stack
    /// budget — and `pi5-image` was **not in this list**, so nothing local
    /// linted a line of them. A `clippy::assertions_on_constants` in one of
    /// those tests reached CI past a fully green local gate set.
    ///
    /// That is `LE-72`'s lesson for the third time and it is always the same
    /// shape: **the defect is never in the compiler, it is in the list.** A
    /// crate that gains its first test is a crate that has just entered this
    /// gate's remit, and nothing said so. Enumerated against the workspace
    /// rather than hand-listed, so the fourth time cannot happen either.
    #[test]
    fn every_crate_this_gate_is_responsible_for_is_in_the_list() {
        // The workspace members whose host lint is this gate's job. `fdt-walk`
        // and `os` are absent deliberately and are named here so their absence
        // reads as a decision rather than as the same oversight again.
        const OWNED: &[&str] = &[
            "hal",
            "hal-arm64",
            "hal-x86_64",
            "kernel",
            "exec",
            "shell",
            "motion",
            "pi5-image",
            "xtask",
        ];
        for package in OWNED {
            assert!(
                HOST_LINT_TARGETS.iter().any(|target| target.package == *package),
                "{package} is not linted locally, so its warnings reach CI first"
            );
        }
        assert_eq!(HOST_LINT_TARGETS.len(), OWNED.len(), "a target was added without a decision");
    }

    /// `bin_only` is a fact about a crate, not a licence to widen a lint.
    ///
    /// It means exactly one thing — **this crate has no library target**, so
    /// `--lib` would be an error rather than a narrowing — and it is true of
    /// exactly the two crates that are a `main.rs` and nothing else. It was
    /// `xtask` alone until `pi5-image` joined the list on 2026-08-08.
    ///
    /// The distinction that matters, and the reason this test still exists
    /// after gaining a second member: `bin_only` selects `--all-targets`, and
    /// `--all-targets` is only safe where the binary **builds on this host**.
    /// Both of these do. The fixture bins in `exec`, `shell` and `kernel` do
    /// not — they name `hal_x86_64` items gated `cfg(not(windows))` — which is
    /// why those crates are `--lib --tests` and why marking one of them
    /// `bin_only` would make the whole gate fail permanently for reasons
    /// unrelated to the code under review (`LE-77`).
    #[test]
    fn bin_only_is_set_exactly_for_the_crates_that_have_no_library() {
        const NO_LIBRARY: [&str; 2] = ["pi5-image", "xtask"];
        for target in HOST_LINT_TARGETS {
            assert_eq!(
                target.bin_only,
                NO_LIBRARY.contains(&target.package),
                "{} — bin_only is a fact about the crate, not a lint preference",
                target.package
            );
        }
    }

    /// The narrowing rule, stated as a rule rather than as a list of one:
    /// `--lib` is legitimate exactly for a crate whose `[[bin]]` is an x86_64
    /// Tier 0 fixture and therefore not an AArch64 artifact at all. `kernel`
    /// and `shell` are both that shape; the other two are board crates whose
    /// binaries *are* the thing being gated. Narrowing anything else would be
    /// hiding a failure rather than scoping a lint.
    #[test]
    fn only_the_crates_whose_binary_is_an_x86_64_fixture_are_scoped_to_their_library() {
        const X86_FIXTURE_HOSTS: [&str; 2] = ["kernel", "shell"];
        for target in CLIPPY_TARGETS {
            assert_eq!(
                target.lib_only,
                X86_FIXTURE_HOSTS.contains(&target.package),
                "{} scopes clippy to its library",
                target.package
            );
        }
    }

    /// `21A` §4 / `LE-123` — the row this whole gate was extended for.
    ///
    /// The usable OS (`TINYCMD`'s verb core, the labelled volume, the DOS
    /// front-end, the `.TCB` runner) is `shell`. Until this entry existed
    /// nothing in the tree compiled it for the board, so "the shell is
    /// arch-neutral" was an argument from its source rather than a fact about
    /// a build — and the manifest disagreed with the argument for as long as
    /// nobody asked. Asserted here rather than left to the operator, because
    /// the whole lesson of `LE-72` is that coverage is a property of the list.
    #[test]
    fn the_shell_library_is_compiled_for_the_board() {
        let entry = CLIPPY_TARGETS
            .iter()
            .find(|target| target.package == "shell")
            .expect("shell --lib for the board target is step 1 of 21A §3");
        assert!(
            entry.lib_only,
            "shell's [[bin]] is the x86_64 Tier 0 parity fixture and is not a board artifact"
        );
    }

    /// The three crates the board image is actually built from stay covered.
    ///
    /// Pinned separately from the `shell` row so that adding a crate to this
    /// list can never be mistaken for a licence to drop one: `21A`'s step 1 is
    /// an addition to `LE-72`'s gate, not a replacement of it.
    #[test]
    fn adding_the_shell_did_not_displace_the_crates_the_image_links() {
        for package in ["hal-arm64", "kernel", "pi5-image"] {
            assert!(
                CLIPPY_TARGETS.iter().any(|target| target.package == package),
                "{package} left the board lint set"
            );
        }
    }
}
