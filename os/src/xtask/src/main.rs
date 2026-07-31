//! `xtask` — the single, cross-platform home for build/test/QEMU-launch/deploy
//! commands, so no contributor has to remember the `cargo build --target
//! ... -Z build-std` incantation by hand and CI invokes the exact same
//! command a local developer would (`G-DX-3`).
//!
//! Application-level host tooling: `#![forbid(unsafe_code)]` per its `std`
//! classification in `docs/mvp-delivery-strategy.md#crate-map`.

#![forbid(unsafe_code)]

mod assurance;
mod bound_provenance;
mod dashboard;
mod external_isolation;
mod gate;
mod governance;
mod performance_catalogue;
mod pi5;
mod probe_pe;
mod shell_parity;
mod spine_files;
mod timing;
mod txe;

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Process exit codes `xtask` itself returns, distinguishing "the thing
/// under test failed" from "the harness couldn't even run the test" — the
/// distinction `STORY-P0-01-03`'s acceptance criteria require.
///
/// `STORY-P1-07-05` extends the scheme with the two outcomes only hardware
/// can produce: a board with no `isa-debug-exit` port can also *say nothing*
/// (the common bring-up case) or *speak and stop without a verdict*, and
/// `TEST-P1-07-05-A` clause 3 requires each to exit differently. `0`/`1`/`2`
/// keep exactly their Tier 0 meanings.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum XtaskExit {
    KernelBootSucceeded = 0,
    KernelBootFailed = 1,
    HarnessError = 2,
    /// Not one byte arrived from the board before the deadline.
    BoardSilent = 3,
    /// The board spoke but no trustworthy verdict arrived.
    BoardSpokeWithoutVerdict = 4,
}

/// A fixture that emits a `TOS64-MEAS/2` envelope, identified by what has
/// to be built to run it.
///
/// Previously this was just a Cargo feature name, because every measurable
/// fixture lived in `kernel`'s own binary. `STORY-P1-03-03`'s D04
/// same-space-vs-cross-space measurement cannot: it needs `exec`'s address
/// spaces, and `exec` depends on `kernel`, so it is a separate binary in a
/// separate package — the same constraint that put every other integration
/// fixture there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MeasurableTarget {
    package: &'static str,
    binary: &'static str,
    feature: Option<&'static str>,
}

/// Every `--fixture=` value the `measure` subcommand accepts.
///
/// A *separate namespace* from [`FIXTURES`], which `qemu-x86_64` accepts.
/// `dispatch` and `dispatch-measure` are the two names for the same binary,
/// one per subcommand. `list-fixtures` prints both sets for this reason.
const MEASURABLE_FIXTURES: &[&str] =
    &["measure", "measure-regression", "pool-bench", "dispatch", "pe-measure", "actuation"];

/// Maps a `--fixture=` value to what must be built, or `None` if that
/// fixture emits no measurement envelope.
fn measurable_fixture(name: &str) -> Option<MeasurableTarget> {
    match name {
        "measure" => Some(MeasurableTarget {
            package: "kernel",
            binary: "kernel",
            feature: Some("fixture-measure"),
        }),
        "measure-regression" => Some(MeasurableTarget {
            package: "kernel",
            binary: "kernel",
            feature: Some("fixture-measure-regression"),
        }),
        "pool-bench" => Some(MeasurableTarget {
            package: "kernel",
            binary: "kernel",
            feature: Some("fixture-pool-bench"),
        }),
        "dispatch" => Some(MeasurableTarget {
            package: "exec",
            binary: "dispatch-measure-fixture",
            feature: None,
        }),
        "pe-measure" => {
            Some(MeasurableTarget { package: "exec", binary: "pe-measure-fixture", feature: None })
        }
        // `STORY-P1-06-01`: the same binary `qemu-x86_64 --fixture=actuation`
        // boots, named in both namespaces because it is both a behavioural
        // fixture and a measured one. `actuation-overrun` is deliberately
        // absent: a run that trips to its safe state emits no envelope at all,
        // and offering it here would be offering a measurement that cannot
        // exist.
        "actuation" => Some(MeasurableTarget {
            package: "kernel",
            binary: "kernel",
            feature: Some("fixture-actuation"),
        }),
        _ => None,
    }
}

/// One bootable Tier 0 QEMU fixture.
///
/// This table is the single source of truth for what `--fixture=` accepts. It
/// exists because the set was previously discoverable only by reading this
/// file's match arms or grepping `.github/workflows/ci.yml`, which meant an
/// agent could not enumerate the test harness without reverse-engineering it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fixture {
    /// `--fixture=` value, or `""` for the default no-fixture boot.
    name: &'static str,
    package: &'static str,
    binary: &'static str,
    feature: Option<&'static str>,
    /// Whether a *passing* run exits non-zero. Three fixtures document a
    /// distinguishable failure as their pass condition.
    expects_failure: bool,
    /// The Test document that owns this fixture's pass condition.
    owning_test: &'static str,
    summary: &'static str,
}

/// Every Tier 0 fixture, in roadmap order.
const FIXTURES: &[Fixture] = &[
    Fixture {
        name: "",
        package: "kernel",
        binary: "kernel",
        feature: None,
        expects_failure: false,
        owning_test: "TEST-P0-01-01-A",
        summary: "Default boot: kernel reaches its halt state",
    },
    Fixture {
        name: "broken-boot",
        package: "kernel",
        binary: "kernel",
        feature: Some("fixture-broken-boot"),
        expects_failure: true,
        owning_test: "TEST-P0-01-03-A",
        summary: "Deliberately-broken boot exits with a distinguishable failure code",
    },
    Fixture {
        name: "context-switch",
        package: "kernel",
        binary: "kernel",
        feature: Some("fixture-context-switch"),
        expects_failure: false,
        owning_test: "TEST-P0-02-02-A",
        summary: "Cooperative context switch preserves callee-saved state",
    },
    Fixture {
        name: "pool-bench",
        package: "kernel",
        binary: "kernel",
        feature: Some("fixture-pool-bench"),
        expects_failure: false,
        owning_test: "TEST-P1-01-01-A",
        summary: "Fixed-capacity pool reports through the measurement harness",
    },
    Fixture {
        name: "idt-apic-timer",
        package: "kernel",
        binary: "kernel",
        feature: Some("fixture-idt-apic-timer"),
        expects_failure: false,
        owning_test: "TEST-P0-04-02-A",
        summary: "Local-APIC timer ticks at a bounded interval ratio",
    },
    Fixture {
        name: "idt-apic-unrouted",
        package: "kernel",
        binary: "kernel",
        feature: Some("fixture-idt-apic-unrouted"),
        expects_failure: true,
        owning_test: "TEST-P0-04-02-A",
        summary: "An unrouted vector reaches the fail-closed default handler",
    },
    Fixture {
        name: "pci-enumeration",
        package: "kernel",
        binary: "kernel",
        feature: Some("fixture-pci-enumeration"),
        expects_failure: false,
        owning_test: "TEST-P0-04-03-A",
        summary: "PCI enumeration builds a bounded device table",
    },
    // `address-space` (`TEST-P0-05-02-A`) boots a different binary package
    // (`exec`'s `exec-fixture`, not `kernel`) entirely — see `exec/Cargo.toml`'s
    // `[[bin]]` comment for why `exec`'s Tier 0 fixture can't just be another
    // `kernel` Cargo feature like `broken-boot`/`context-switch` are.
    Fixture {
        name: "address-space",
        package: "exec",
        binary: "exec-fixture",
        feature: None,
        expects_failure: false,
        owning_test: "TEST-P0-05-02-A",
        summary: "A process page table is constructed and validated",
    },
    Fixture {
        name: "win32-shim",
        package: "exec",
        binary: "win32-shim-fixture",
        feature: None,
        expects_failure: false,
        owning_test: "TEST-P0-05-03-A",
        summary: "Win32 shim bounds-checks buffers and gates calls through a policy",
    },
    Fixture {
        name: "blue-sharc",
        package: "exec",
        binary: "blue-sharc-fixture",
        feature: None,
        expects_failure: false,
        owning_test: "TEST-P0-05-04-A",
        summary: "A packed TXE container loads and its imports resolve",
    },
    Fixture {
        name: "blue-sharc-broken",
        package: "exec",
        binary: "blue-sharc-broken-fixture",
        feature: None,
        expects_failure: false,
        owning_test: "TEST-P0-05-04-A",
        summary: "A malformed TXE container is rejected rather than mapped",
    },
    Fixture {
        name: "shared-memory",
        package: "exec",
        binary: "shared-memory-fixture",
        feature: None,
        expects_failure: false,
        owning_test: "TEST-P0-07-02-A",
        summary: "Shared-memory grants check ownership, vacancy and escalation",
    },
    Fixture {
        name: "measure",
        package: "kernel",
        binary: "kernel",
        feature: Some("fixture-measure"),
        expects_failure: false,
        owning_test: "TEST-P1-01-01-A",
        summary: "Measurement harness emits a parseable TOS64-MEAS/2 envelope",
    },
    Fixture {
        name: "fault",
        package: "kernel",
        binary: "kernel",
        feature: Some("fixture-fault"),
        expects_failure: false,
        owning_test: "TEST-P1-02-01-A",
        summary: "Three real faults, each contained to one task",
    },
    Fixture {
        name: "double-fault",
        package: "kernel",
        binary: "kernel",
        feature: Some("fixture-double-fault"),
        expects_failure: false,
        owning_test: "TEST-P1-02-02-A",
        summary: "A fault inside the fault path lands on the IST stack",
    },
    Fixture {
        name: "address-space-switch",
        package: "exec",
        binary: "address-space-switch-fixture",
        feature: None,
        expects_failure: false,
        owning_test: "TEST-P1-03-01-A",
        summary: "Two real address spaces, switched by CR3, isolated by a real fault",
    },
    Fixture {
        name: "wx-seal",
        package: "exec",
        binary: "wx-seal-fixture",
        feature: None,
        expects_failure: false,
        owning_test: "TEST-P1-03-02-A",
        summary: "W^X both directions, shared kernel PDs, sealing and teardown",
    },
    Fixture {
        name: "first-task",
        package: "exec",
        binary: "first-task-fixture",
        feature: None,
        expects_failure: false,
        owning_test: "TEST-P1-03-02-A",
        summary: "The first real scheduled task, contained and audited",
    },
    Fixture {
        name: "dispatch-measure",
        package: "exec",
        binary: "dispatch-measure-fixture",
        feature: None,
        expects_failure: false,
        owning_test: "TEST-P1-03-03-A",
        summary: "D04 same-space vs cross-space dispatch cost",
    },
    Fixture {
        name: "pe-measure",
        package: "exec",
        binary: "pe-measure-fixture",
        feature: None,
        expects_failure: false,
        owning_test: "TEST-P0-01-06-A",
        summary: "D09 PE64 parse and import validation, accept and denial paths",
    },
    // The system image (`STORY-P1-03-03`): the real boot path that discovers
    // hardware, installs W^X address spaces, and schedules a real loaded task.
    // Lives in its own top-level package because it depends on *both* `kernel`
    // and `exec`, which `kernel`'s own binary can never do.
    Fixture {
        name: "os",
        package: "os",
        binary: "os",
        feature: None,
        expects_failure: false,
        owning_test: "TEST-P1-03-03-A",
        summary: "The shipping image boots, loads and runs a real task",
    },
    // `STORY-P1-04-03`: the same system image, embedding a real PE64 whose
    // `.text` is a two-byte self-jump. It is the only difference between this
    // build and the one above, which is what makes it evidence that the
    // *shipping* hook enforces rather than that a fixture can be made to.
    Fixture {
        name: "os-runaway",
        package: "os",
        binary: "os",
        feature: Some("fixture-os-runaway"),
        expects_failure: false,
        owning_test: "TEST-P1-04-03-A",
        summary: "The shipping image enforces its budget on a workload that never yields",
    },
    Fixture {
        name: "preempt",
        package: "kernel",
        binary: "kernel",
        feature: Some("fixture-preempt"),
        expects_failure: false,
        owning_test: "TEST-P1-04-01-A",
        summary: "A task that never yields is preempted, with its SSE state intact",
    },
    Fixture {
        name: "priority-inversion",
        package: "kernel",
        binary: "kernel",
        feature: Some("fixture-priority-inversion"),
        expects_failure: false,
        owning_test: "TEST-P1-04-01-A",
        summary: "Priority inversion avoided under real preemption",
    },
    Fixture {
        name: "wcet-restart",
        package: "kernel",
        binary: "kernel",
        feature: Some("fixture-wcet-restart"),
        expects_failure: false,
        owning_test: "TEST-P1-04-02-A",
        summary: "WCET overrun restarts the task from its entry point",
    },
    Fixture {
        name: "wcet-degrade",
        package: "kernel",
        binary: "kernel",
        feature: Some("fixture-wcet-degrade"),
        expects_failure: false,
        owning_test: "TEST-P1-04-02-A",
        summary: "WCET overrun degrades the task below a real competitor",
    },
    // `wcet-trip`'s correct outcome is exit 1: the system enters its declared
    // safe state, which at Tier 0 is a fail-closed stop. Its CI step expects
    // failure, as `broken-boot` and `idt-apic-unrouted` already do.
    Fixture {
        name: "wcet-trip",
        package: "kernel",
        binary: "kernel",
        feature: Some("fixture-wcet-trip"),
        expects_failure: true,
        owning_test: "TEST-P1-04-02-A",
        summary: "WCET overrun trips the system to its safe state",
    },
    // `STORY-P1-04-05`, closing `LE-50`: the *composed* scenario. A lock
    // holder overruns its budget while boosted by a blocked waiter, and the
    // evidence is which task the dispatcher selected — a medium task that is
    // `Ready` throughout makes no progress at all across a window containing
    // the degrade. Deliberately not folded into `priority-inversion`, whose
    // run is the evidence a Verified Story rests on.
    Fixture {
        name: "degrade-inheritance",
        package: "kernel",
        binary: "kernel",
        feature: Some("fixture-degrade-inheritance"),
        expects_failure: false,
        owning_test: "TEST-P1-04-05-A",
        summary: "A degrade taken while boosted keeps the boost, then lands on its floor",
    },
    // `STORY-P1-06-01` — `G-PA-1`'s flagship path. One RT task arms an
    // activation, computes a command and drives a real output boundary, with
    // the decision-to-actuation latency measured. Also in the *measurable*
    // namespace (`measure --fixture=actuation`), because it emits an envelope.
    Fixture {
        name: "actuation",
        package: "kernel",
        binary: "kernel",
        feature: Some("fixture-actuation"),
        expects_failure: false,
        owning_test: "TEST-P1-06-01-A",
        summary: "Decision to actuation, measured, with no ambient path to the output",
    },
    // `actuation-overrun`'s correct outcome is exit 1: the decision overruns
    // its declared budget, the system enters its declared safe state, and no
    // command reaches the line. Its CI step expects failure, as `broken-boot`,
    // `idt-apic-unrouted` and `wcet-trip` already do — and its `TOS64-RESULT/1`
    // line is what distinguishes a correct trip from a broken one, since the
    // exit code cannot.
    Fixture {
        name: "actuation-overrun",
        package: "kernel",
        binary: "kernel",
        feature: Some("fixture-actuation-overrun"),
        expects_failure: true,
        owning_test: "TEST-P1-06-01-A",
        summary: "A deliberate overrun trips the policy before any late command is emitted",
    },
    Fixture {
        name: "shell-batch",
        package: "shell",
        binary: "shell-batch-fixture",
        feature: None,
        expects_failure: false,
        owning_test: "TEST-P2-07-01-A",
        summary: "TINYCMD runs the parity .TCB against the labelled RAM volume over serial",
    },
];

/// Resolves a `--fixture=` value, treating absence as the default boot.
fn qemu_fixture(name: Option<&str>) -> Option<&'static Fixture> {
    let requested = name.unwrap_or_default();
    FIXTURES.iter().find(|fixture| fixture.name == requested)
}

/// One `xtask` subcommand, for the generated usage text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Subcommand {
    name: &'static str,
    summary: &'static str,
}

/// Every subcommand `main` dispatches. Kept beside the dispatch table so the
/// usage text cannot drift out of sync with what the binary actually accepts.
const SUBCOMMANDS: &[Subcommand] = &[
    Subcommand { name: "help", summary: "Print this text" },
    Subcommand { name: "qemu-x86_64", summary: "Boot a Tier 0 fixture under QEMU" },
    Subcommand {
        name: "pi5",
        summary: "Build the Pi 5 SD image, capture serial, and verdict a hardware run",
    },
    Subcommand { name: "list-fixtures", summary: "List every fixture and its owning Test" },
    Subcommand { name: "list-status", summary: "Emit Epic/Feature/Story state as TSV on stdout" },
    Subcommand { name: "measure", summary: "Run a measurable fixture and emit its envelope" },
    Subcommand {
        name: "check-timing-regression",
        summary: "Gate measured cycles against the committed baselines",
    },
    Subcommand {
        name: "check-assurance-spine",
        summary: "Validate contracts, loose ends and the Story/Feature join",
    },
    Subcommand {
        name: "check-shell-parity",
        summary: "Boot the shell-batch fixture and byte-compare its transcript to the golden",
    },
    Subcommand {
        name: "emit-dashboard",
        summary: "Print goals/index.html's generated stat-tile block from live spine data",
    },
    Subcommand {
        name: "check-spine-files",
        summary: "Fast: header, field count and id uniqueness on every hand-edited spine TSV",
    },
    Subcommand { name: "check-crate-sizes", summary: "Enforce the 20,000-LOC crate ceiling" },
    Subcommand { name: "check-image-size", summary: "Enforce the system-image size ceiling" },
    Subcommand {
        name: "check-performance-catalogue",
        summary: "Validate the 625-test performance catalogue",
    },
    Subcommand {
        name: "governance-fixture-test",
        summary: "Prove the governance fixtures catch what they claim",
    },
    Subcommand { name: "pack-txe", summary: "Pack a PE64 into a TXE container" },
    Subcommand { name: "make-probe-pe", summary: "Generate a probe PE64 test input" },
];

/// Renders the usage text from [`SUBCOMMANDS`].
fn usage() -> String {
    let mut text = String::from("usage: cargo run -p xtask -- <command> [options]\n\ncommands:\n");
    let width = SUBCOMMANDS.iter().map(|entry| entry.name.len()).max().unwrap_or(0);
    for entry in SUBCOMMANDS {
        text.push_str(&format!("  {:width$}  {}\n", entry.name, entry.summary));
    }
    text.push_str("\nrun `list-fixtures` for every --fixture= value.");
    text
}

/// QEMU's own process exit code when the guest writes to the isa-debug-exit
/// port: `(value << 1) | 1`. See `kernel/src/qemu_exit.rs`.
const QEMU_EXIT_SUCCESS: i32 = (0x10 << 1) | 1; // 33
const QEMU_EXIT_FAILURE: i32 = (0x11 << 1) | 1; // 35

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        eprintln!("{}", usage());
        return ExitCode::from(XtaskExit::HarnessError as u8);
    };

    match command.as_str() {
        "help" | "--help" | "-h" => {
            println!("{}", usage());
            ExitCode::SUCCESS
        }
        // Emitted rather than committed: a generated file checked into the tree
        // is one more thing that can drift from the documents it summarises,
        // which is the problem this replaces.
        "list-status" => {
            let result = os_root().and_then(|root| {
                let repo_root = root.parent().ok_or_else(|| {
                    format!("could not resolve repository root from {}", root.display())
                })?;
                assurance::artifact_statuses(repo_root)
            });
            match result {
                Ok(statuses) => {
                    println!("id\tstate\tdetail");
                    for status in statuses {
                        println!("{}\t{}\t{}", status.id, status.state, status.detail);
                    }
                    ExitCode::SUCCESS
                }
                Err(message) => {
                    eprintln!("xtask: status headers invalid: {message}");
                    ExitCode::FAILURE
                }
            }
        }
        "list-fixtures" => {
            println!("qemu-x86_64 --fixture= (boot a Tier 0 fixture under QEMU)\n");
            println!("{:<22} {:<7} {:<30} {:<8} TEST", "FIXTURE", "PACKAGE", "BINARY", "EXIT");
            for fixture in FIXTURES {
                let name = if fixture.name.is_empty() { "(none)" } else { fixture.name };
                let exit = if fixture.expects_failure { "failure" } else { "success" };
                println!(
                    "{:<22} {:<7} {:<30} {:<8} {}",
                    name, fixture.package, fixture.binary, exit, fixture.owning_test
                );
                println!("{:<22} {}", "", fixture.summary);
            }
            // A separate namespace, and one that has caught people out: CI runs
            // `measure --fixture=dispatch`, which `qemu-x86_64` does not accept.
            println!("\nmeasure --fixture= (run a measurable fixture; separate namespace)\n");
            println!("{:<22} {:<7} BINARY", "FIXTURE", "PACKAGE");
            for name in MEASURABLE_FIXTURES {
                let target = measurable_fixture(name).expect("listed fixture must resolve");
                println!("{:<22} {:<7} {}", name, target.package, target.binary);
            }
            // A third namespace: Tier 1 hardware, registered here per
            // `STORY-P0-01-04`'s rule even though (FEAT-P1-07 §7.4, decision b)
            // no CI step will ever run it — runs are manual and land in Reports.
            println!("\npi5 --fixture= (Tier 1 hardware run path; manual, never CI)\n");
            println!("{:<22} {:<9} {:<10} TEST", "FIXTURE", "PACKAGE", "BINARY");
            for fixture in pi5::PI5_FIXTURES {
                println!(
                    "{:<22} {:<9} {:<10} {}",
                    fixture.name,
                    pi5::IMAGE_PACKAGE,
                    pi5::IMAGE_BINARY,
                    fixture.owning_test
                );
                println!("{:<22} {}", "", fixture.summary);
            }
            ExitCode::SUCCESS
        }
        "qemu-x86_64" => {
            // Collected once rather than consumed by successive `find_map`s:
            // `args` is a by-value iterator, so a second search would only
            // ever see what the first one left behind.
            let rest: Vec<String> = args.collect();
            let fixture =
                rest.iter().find_map(|a| a.strip_prefix("--fixture=").map(str::to_string));
            // `STORY-P1-04-02`: fixtures otherwise run with `-serial none`
            // and report a single pass/fail bit through isa-debug-exit, which
            // is all CI needs but leaves a fixture's own diagnostic lines
            // unreadable — so no Test document could quote a capture without
            // hand-driving QEMU. Opt-in, so CI's steps are unchanged.
            let serial_capture =
                rest.iter().find_map(|a| a.strip_prefix("--serial-capture=").map(PathBuf::from));
            let Some(selected) = qemu_fixture(fixture.as_deref()) else {
                let name = fixture.as_deref().unwrap_or_default();
                eprintln!("xtask: unknown --fixture value '{name}'");
                eprintln!("xtask: run `cargo run -p xtask -- list-fixtures` for every fixture");
                return ExitCode::from(XtaskExit::HarnessError as u8);
            };
            let (package, binary, feature) = (selected.package, selected.binary, selected.feature);
            match qemu_x86_64(package, binary, feature, serial_capture.as_deref(), Profile::Dev) {
                Ok(code) => ExitCode::from(code as u8),
                Err(message) => {
                    eprintln!("xtask: {message}");
                    ExitCode::from(XtaskExit::HarnessError as u8)
                }
            }
        }
        // `STORY-P1-07-05`: the Tier 1 hardware run path. One command builds
        // the placeable image, prints where it goes, captures the debug UART
        // and exits on the Tier 0 scheme extended with the two outcomes only
        // hardware can produce (silence, spoke-without-verdict).
        "pi5" => {
            let rest: Vec<String> = args.collect();
            let flag = |prefix: &str| {
                rest.iter().find_map(|arg| arg.strip_prefix(prefix).map(str::to_string))
            };
            let Some(fixture_name) = flag("--fixture=") else {
                eprintln!("xtask: pi5 needs --fixture=<name>");
                eprintln!("xtask: run `cargo run -p xtask -- list-fixtures` for every fixture");
                return ExitCode::from(XtaskExit::HarnessError as u8);
            };
            let Some(fixture) = pi5::pi5_fixture(&fixture_name) else {
                eprintln!("xtask: unknown pi5 --fixture value '{fixture_name}'");
                eprintln!("xtask: run `cargo run -p xtask -- list-fixtures` for every fixture");
                return ExitCode::from(XtaskExit::HarnessError as u8);
            };
            let baud = flag("--baud=").and_then(|v| v.parse::<u32>().ok()).unwrap_or(115_200);
            let timeout_secs =
                flag("--timeout-secs=").and_then(|v| v.parse::<u64>().ok()).unwrap_or(90);
            let quiet_secs =
                flag("--quiet-secs=").and_then(|v| v.parse::<u64>().ok()).unwrap_or(10);
            let board_revision = flag("--board-rev=").unwrap_or_else(|| "unrecorded".to_string());
            let firmware_version = flag("--firmware=").unwrap_or_else(|| "unrecorded".to_string());
            match pi5_run(
                fixture,
                flag("--port=").as_deref(),
                baud,
                timeout_secs,
                quiet_secs,
                &board_revision,
                &firmware_version,
            ) {
                Ok(exit) => ExitCode::from(exit as u8),
                Err(message) => {
                    eprintln!("xtask: {message}");
                    ExitCode::from(XtaskExit::HarnessError as u8)
                }
            }
        }
        // `TEST-P2-07-01-A`: two independent signals, exactly `timing.rs`'s
        // discipline — the fixture's own isa-debug-exit verdict (its in-guest
        // assertions) AND the byte-compared transcript must both hold. A
        // matching transcript with a failing exit, or vice versa, is a harness
        // error, never a pass.
        "check-shell-parity" => {
            let result = os_root().and_then(|root| {
                let capture = env::temp_dir().join("shell-parity-capture.txt");
                let verdict = qemu_x86_64(
                    "shell",
                    "shell-batch-fixture",
                    None,
                    Some(&capture),
                    Profile::Dev,
                )?;
                if verdict != XtaskExit::KernelBootSucceeded {
                    return Err(format!(
                        "shell-batch fixture reported in-guest assertion failure (exit {})",
                        verdict as u8
                    ));
                }
                let actual = std::fs::read_to_string(&capture)
                    .map_err(|e| format!("cannot read serial capture: {e}"))?;
                // `LE-56`'s third signal: split the capture at the spoor
                // trailer — the transcript before it stays byte-sacred for
                // the golden comparison; the trailer itself must be present,
                // well-formed and self-corroborating (missing/malformed is a
                // FAIL, never a skip).
                let (transcript, trailer) = shell_parity::split_capture(&actual)?;
                let golden_path =
                    root.join("src").join("shell").join("golden").join("parity-smoke.golden.txt");
                let golden = std::fs::read_to_string(&golden_path)
                    .map_err(|e| format!("cannot read golden transcript: {e}"))?;
                let lines = shell_parity::compare_transcript(&transcript, &golden)?;
                let corroborated = trailer.corroborated()?;
                Ok((lines, corroborated))
            });
            match result {
                Ok((lines, corroborated)) => {
                    println!(
                        "shell-parity: transcript matches golden ({lines} lines), the fixture's in-guest assertions passed, and the spoor journal corroborates the denial count (TOS64-SPOOR/1 len={corroborated} denials={corroborated})"
                    );
                    ExitCode::SUCCESS
                }
                Err(message) => {
                    eprintln!("xtask: shell parity failed: {message}");
                    ExitCode::FAILURE
                }
            }
        }
        "check-crate-sizes" => {
            let ceiling = args
                .find_map(|a| a.strip_prefix("--ceiling=").map(str::to_string))
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(20_000);
            match os_root().and_then(|root| governance::check_all_crate_sizes(&root, ceiling)) {
                Ok(()) => ExitCode::SUCCESS,
                Err(message) => {
                    eprintln!("xtask: crate-size ceiling violated: {message}");
                    ExitCode::FAILURE
                }
            }
        }
        "check-performance-catalogue" => {
            let result = os_root().and_then(|root| {
                let repo_root = root.parent().ok_or_else(|| {
                    format!("could not resolve repository root from {}", root.display())
                })?;
                performance_catalogue::check_catalogue(repo_root)
            });
            match result {
                Ok(summary) => {
                    println!(
                        "performance-catalogue-check: {} tests present (25 domains x 25 guardrails)",
                        summary.test_count
                    );
                    ExitCode::SUCCESS
                }
                Err(message) => {
                    eprintln!("xtask: performance catalogue invalid: {message}");
                    ExitCode::FAILURE
                }
            }
        }
        // `LE-30`. Prints the block; it deliberately does *not* write the file.
        // A command that rewrites the page a reader meets first should be one
        // someone chose to apply, and the diff is the review.
        "emit-dashboard" => {
            let result = os_root().and_then(|root| {
                let repo_root = root.parent().ok_or_else(|| {
                    format!("could not resolve repository root from {}", root.display())
                })?;
                assurance::dashboard_facts(repo_root)
            });
            match result {
                Ok(facts) => {
                    println!("{}", dashboard::emit_stat_row(&facts));
                    ExitCode::SUCCESS
                }
                Err(message) => {
                    eprintln!("xtask: could not read the spine: {message}");
                    ExitCode::FAILURE
                }
            }
        }
        // `LE-36`. Deliberately its own subcommand rather than a flag on
        // `check-assurance-spine`: rule 8 asks for something a session will
        // actually run between two edits, and a flag on a slow command is a
        // slow command.
        "check-spine-files" => {
            let result = os_root().and_then(|root| {
                let repo_root = root.parent().ok_or_else(|| {
                    format!("could not resolve repository root from {}", root.display())
                })?;
                spine_files::check_spine_files(repo_root)
            });
            match result {
                Ok(summary) => {
                    println!(
                        "spine-files-check: {} files, {} rows — headers agree, no consumed \
                         separators, no duplicate keys, ids contiguous",
                        summary.file_count, summary.row_count
                    );
                    ExitCode::SUCCESS
                }
                Err(message) => {
                    eprintln!("xtask: spine file invalid: {message}");
                    ExitCode::FAILURE
                }
            }
        }
        "check-assurance-spine" => {
            let result = os_root().and_then(|root| {
                let repo_root = root.parent().ok_or_else(|| {
                    format!("could not resolve repository root from {}", root.display())
                })?;
                assurance::check_assurance_spine(repo_root)
            });
            match result {
                Ok(summary) => {
                    println!(
                        "assurance-spine-check: {} Features, {} Stories, {} Tests, {} Reports, {} containment classes, {} boundary tests, {} security controls, {} Protection Domain contracts, {} code-admission gates, {} class communication pairs, {} application/platform targets, {} landing zones, {} selected Story/performance contracts, {} selected application/performance contracts, {} loose ends ({} open), {} status headers, {} release gates with evidence, {} open-debt selections, {} platforms ({} qualified), {} bound claims checked, {} Feature/Story status rows agree, {} dashboard badges agree, {} manifests isolated from external/",
                        summary.feature_count,
                        summary.story_count,
                        summary.test_count,
                        summary.report_count,
                        summary.containment_class_count,
                        summary.boundary_test_count,
                        summary.security_control_count,
                        summary.protection_domain_contract_count,
                        summary.code_admission_gate_count,
                        summary.class_communication_pair_count,
                        summary.application_platform_count,
                        summary.landing_zone_count,
                        summary.selected_performance_contracts,
                        summary.selected_application_performance_contracts,
                        summary.loose_end_count,
                        summary.open_loose_end_count,
                        summary.status_header_count,
                        summary.guardrail_evidence_count,
                        summary.open_debt_count,
                        summary.platform_count,
                        summary.qualified_platform_count,
                        summary.bound_claim_count,
                        summary.feature_story_row_count,
                        summary.dashboard_badge_count,
                        summary.external_manifest_count
                    );
                    ExitCode::SUCCESS
                }
                Err(message) => {
                    eprintln!("xtask: assurance spine invalid: {message}");
                    ExitCode::FAILURE
                }
            }
        }
        "pack-txe" => {
            let rest: Vec<String> = args.collect();
            let input = rest.iter().find_map(|a| a.strip_prefix("--input=").map(str::to_string));
            let output = rest.iter().find_map(|a| a.strip_prefix("--output=").map(str::to_string));
            let (Some(input), Some(output)) = (input, output) else {
                eprintln!("usage: cargo run -p xtask -- pack-txe --input=<path> --output=<path>");
                return ExitCode::from(XtaskExit::HarnessError as u8);
            };
            match pack_txe(&input, &output) {
                Ok(()) => ExitCode::SUCCESS,
                Err(message) => {
                    eprintln!("xtask: {message}");
                    ExitCode::FAILURE
                }
            }
        }
        "make-probe-pe" => {
            let rest: Vec<String> = args.collect();
            let output = rest.iter().find_map(|a| a.strip_prefix("--output=").map(str::to_string));
            let Some(output) = output else {
                eprintln!("usage: cargo run -p xtask -- make-probe-pe [--runaway] --output=<path>");
                return ExitCode::from(XtaskExit::HarnessError as u8);
            };
            // `STORY-P1-04-03`: the same image with a self-jump for `.text`.
            // A workload that never yields is what the shipping image's WCET
            // enforcement has to be proven against, and there is no Windows
            // toolchain here to compile one — see `probe_pe::build_runaway`.
            let runaway = rest.iter().any(|a| a == "--runaway");
            let image = if runaway { probe_pe::build_runaway() } else { probe_pe::build() };
            match std::fs::write(&output, &image) {
                Ok(()) => {
                    println!("make-probe-pe: wrote {output} ({} bytes)", image.len());
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    eprintln!("xtask: could not write {output}: {error}");
                    ExitCode::FAILURE
                }
            }
        }
        "check-image-size" => {
            let ceiling = args
                .find_map(|a| a.strip_prefix("--ceiling=").map(str::to_string))
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(8 * 1024 * 1024);
            match check_image_size(ceiling) {
                Ok(size) => {
                    println!(
                        "check-image-size: system image (os) {size} bytes (ceiling {ceiling})"
                    );
                    ExitCode::SUCCESS
                }
                Err(message) => {
                    eprintln!("xtask: G-DX-8 image-size ceiling violated: {message}");
                    ExitCode::FAILURE
                }
            }
        }
        "measure" => {
            let rest: Vec<String> = args.collect();
            let runs = rest
                .iter()
                .find_map(|a| a.strip_prefix("--runs=").map(str::to_string))
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(3);
            let keep = rest.iter().find_map(|a| a.strip_prefix("--out=").map(PathBuf::from));
            let fixture = rest
                .iter()
                .find_map(|a| a.strip_prefix("--fixture=").map(str::to_string))
                .unwrap_or_else(|| "measure".to_string());
            let target = match measurable_fixture(&fixture) {
                Some(target) => target,
                None => {
                    eprintln!(
                        "xtask: --fixture={fixture} reports no measurement envelope; measurable fixtures are `measure`, `pool-bench` and `dispatch`"
                    );
                    return ExitCode::from(XtaskExit::HarnessError as u8);
                }
            };
            let profile =
                match rest.iter().find_map(|a| a.strip_prefix("--profile=").map(str::to_string)) {
                    Some(value) => match Profile::parse(&value) {
                        Ok(profile) => profile,
                        Err(message) => {
                            eprintln!("xtask: {message}");
                            return ExitCode::from(XtaskExit::HarnessError as u8);
                        }
                    },
                    None => Profile::Dev,
                };
            match measure(target, runs, keep.as_deref(), profile) {
                Ok(code) => ExitCode::from(code as u8),
                Err(message) => {
                    eprintln!("xtask: {message}");
                    ExitCode::from(XtaskExit::HarnessError as u8)
                }
            }
        }
        "check-timing-regression" => {
            let rest: Vec<String> = args.collect();
            let runs = rest
                .iter()
                .find_map(|a| a.strip_prefix("--runs=").map(str::to_string))
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(gate::MINIMUM_RUNS);
            let baseline =
                rest.iter().find_map(|a| a.strip_prefix("--baseline=").map(PathBuf::from));
            let update = rest.iter().any(|a| a == "--update-baseline");
            let date = rest.iter().find_map(|a| a.strip_prefix("--date=").map(str::to_string));
            // `--inject-regression` builds the fixture with a deliberately
            // slowed measured phase, so "prove the gate can fail" is a command
            // anyone can re-run rather than a one-time screenshot — the
            // discipline `fixture-broken-boot` established for boot.
            let fixture = if rest.iter().any(|a| a == "--inject-regression") {
                "measure-regression"
            } else {
                "measure"
            };
            let target = measurable_fixture(fixture).expect("both spellings are measurable");
            match check_timing_regression(runs, baseline, update, date, target) {
                Ok(code) => ExitCode::from(code as u8),
                Err(message) => {
                    eprintln!("xtask: {message}");
                    ExitCode::from(XtaskExit::HarnessError as u8)
                }
            }
        }
        "governance-fixture-test" => {
            let work_dir = env::temp_dir().join("tinyos-governance-fixtures");
            match governance::run_fixture_smoke_test(&work_dir) {
                Ok(()) => ExitCode::SUCCESS,
                Err(message) => {
                    eprintln!("xtask: {message}");
                    ExitCode::FAILURE
                }
            }
        }
        other => {
            eprintln!("xtask: unknown command '{other}'");
            eprintln!("{}", usage());
            ExitCode::from(XtaskExit::HarnessError as u8)
        }
    }
}

/// Which Cargo profile a Tier 0 fixture is built with.
///
/// Exists because of `LE-13`: every measurement `STORY-P1-01-01` reported ran
/// **dev-profile** binaries, so its absolute cycle counts were inflated by
/// missing optimization as well as by emulation, and a baseline recorded that
/// way would bake the dev profile into the gate forever. Baselines and the
/// regression gate use [`Profile::Release`]; the `qemu-x86_64` boot fixtures
/// keep using [`Profile::Dev`], which is what they have always been verified
/// against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Profile {
    Dev,
    Release,
}

impl Profile {
    /// The `--profile=` spelling, and the name written into a baseline row's
    /// `profile` column.
    fn name(self) -> &'static str {
        match self {
            Profile::Dev => "dev",
            Profile::Release => "release",
        }
    }

    /// Cargo's own target sub-directory for this profile.
    fn target_dir(self) -> &'static str {
        match self {
            Profile::Dev => "debug",
            Profile::Release => "release",
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "dev" | "debug" => Ok(Profile::Dev),
            "release" => Ok(Profile::Release),
            other => Err(format!("unknown --profile={other} (expected `dev` or `release`)")),
        }
    }
}

/// Builds `package`'s `binary` bin target against the custom x86_64 target
/// and boots it under QEMU, per `STORY-P0-01-01`/`STORY-P0-01-03`.
/// `fixture_feature`, when given, builds with that Cargo feature enabled
/// instead of the normal boot path — `fixture-broken-boot` for
/// `TEST-P0-01-03-A`'s deliberately-broken-boot smoke test,
/// `fixture-context-switch` for `TEST-P0-02-02-A`'s context-switch fixture.
/// `exec`'s `exec-fixture` binary (`TEST-P0-05-02-A`) needs no feature — the
/// whole binary exists only to be this fixture.
fn qemu_x86_64(
    package: &str,
    binary: &str,
    fixture_feature: Option<&str>,
    serial_capture: Option<&Path>,
    profile: Profile,
) -> Result<XtaskExit, String> {
    let os_root = os_root()?;
    let target_spec = os_root.join("targets").join("x86_64-tinyos.json");

    let mut build = Command::new("cargo");
    build
        .current_dir(&os_root)
        .arg("build")
        .args(if profile == Profile::Release { &["--release"][..] } else { &[][..] })
        .arg("-p")
        .arg(package)
        .arg("--target")
        .arg(&target_spec)
        .arg("-Z")
        .arg("json-target-spec")
        .arg("-Z")
        .arg("build-std=core,compiler_builtins")
        .arg("-Z")
        .arg("build-std-features=compiler-builtins-mem");
    if let Some(feature) = fixture_feature {
        build.arg("--features").arg(feature);
    }

    let build_status = build.status().map_err(|e| format!("failed to invoke cargo build: {e}"))?;
    if !build_status.success() {
        return Err(format!("{package} build failed"));
    }

    let kernel_elf =
        os_root.join("target").join("x86_64-tinyos").join(profile.target_dir()).join(binary);
    if !kernel_elf.exists() {
        return Err(format!(
            "expected {binary} binary at {} but it does not exist",
            kernel_elf.display()
        ));
    }

    let qemu_binary = find_qemu()?;
    // A measurement fixture's evidence leaves the guest over COM1, so it needs
    // `-serial file:PATH`; every other fixture reports a single pass/fail bit
    // through isa-debug-exit and gets `-serial none`, unchanged — capturing
    // unconditionally would make every boot fixture write a host file it never
    // uses.
    let serial_argument = match serial_capture {
        Some(path) => format!("file:{}", path.display()),
        None => "none".to_string(),
    };
    let mut child = Command::new(qemu_binary)
        .arg("-kernel")
        .arg(&kernel_elf)
        .arg("-machine")
        .arg("q35")
        .arg("-m")
        .arg("128M")
        .arg("-display")
        .arg("none")
        .arg("-serial")
        .arg(&serial_argument)
        .arg("-monitor")
        .arg("none")
        .arg("-device")
        .arg("isa-debug-exit,iobase=0xf4,iosize=0x04")
        .arg("-no-reboot")
        .stdin(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to invoke QEMU: {e}"))?;

    // Bounded startup time budget (`TEST-P0-01-01-A`): a kernel that never
    // reaches the isa-debug-exit port is a boot failure, not a hang — poll
    // rather than block indefinitely on `wait()`.
    const BOOT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
    let deadline = std::time::Instant::now() + BOOT_TIMEOUT;
    let exit_status = loop {
        if let Some(status) =
            child.try_wait().map_err(|e| format!("failed to poll QEMU process: {e}"))?
        {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "kernel did not reach the isa-debug-exit port within the {BOOT_TIMEOUT:?} boot time budget"
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    };

    match exit_status.code() {
        Some(QEMU_EXIT_SUCCESS) => Ok(XtaskExit::KernelBootSucceeded),
        Some(QEMU_EXIT_FAILURE) => Ok(XtaskExit::KernelBootFailed),
        Some(other) => Err(format!(
            "QEMU exited with unexpected code {other} (neither the success nor failure isa-debug-exit code) — harness or QEMU configuration problem, not a kernel-boot result"
        )),
        None => Err("QEMU process terminated by signal, not a normal exit".to_string()),
    }
}

/// `STORY-P1-07-05`: builds the bootable AArch64 image, flattens it to a
/// placeable `kernel8.img`, prints the SD-card placement, and — when `--port`
/// is given — captures the debug UART and verdicts the run.
///
/// Everything here is I/O and process glue; every *decision* (flattening,
/// bounded capture, outcome classification, exit mapping, the run record) is
/// a pure, host-tested function in [`pi5`]. Exit-code discipline matches
/// `qemu-x86_64`'s own, extended: 0 pass, 1 the board's own verdict said
/// failure, 2 this harness could not run, 3 silence, 4 spoke-without-verdict.
fn pi5_run(
    fixture: &pi5::Pi5Fixture,
    port: Option<&str>,
    baud: u32,
    timeout_secs: u64,
    quiet_secs: u64,
    board_revision: &str,
    firmware_version: &str,
) -> Result<XtaskExit, String> {
    let os_root = os_root()?;
    let target_spec = os_root.join("targets").join("aarch64-tinyos.json");

    let mut build = Command::new("cargo");
    build
        .current_dir(&os_root)
        .arg("build")
        .arg("-p")
        .arg(pi5::IMAGE_PACKAGE)
        .arg("--target")
        .arg(&target_spec)
        // `-Z json-target-spec` is required by the pinned toolchain, and
        // `build-std=core` alone leaves memcpy/memcmp undefined at link time
        // (divergence record, build recipe).
        .arg("-Z")
        .arg("json-target-spec")
        .arg("-Z")
        .arg("build-std=core,compiler_builtins")
        .arg("-Z")
        .arg("build-std-features=compiler-builtins-mem");
    if let Some(feature) = fixture.feature {
        build.arg("--features").arg(feature);
    }
    let build_status = build.status().map_err(|e| format!("failed to invoke cargo build: {e}"))?;
    if !build_status.success() {
        return Err(format!("{} build failed", pi5::IMAGE_PACKAGE));
    }

    let elf_path =
        os_root.join("target").join("aarch64-tinyos").join("debug").join(pi5::IMAGE_BINARY);
    let elf = std::fs::read(&elf_path)
        .map_err(|e| format!("cannot read built ELF {}: {e}", elf_path.display()))?;
    let image = pi5::flatten_elf(&elf)?;
    let image_sha256 = pi5::sha256_hex(&image.bytes);

    let out_dir = os_root.join("target").join("pi5");
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| format!("cannot create {}: {e}", out_dir.display()))?;
    let image_path = out_dir.join("kernel8.img");
    std::fs::write(&image_path, &image.bytes)
        .map_err(|e| format!("cannot write {}: {e}", image_path.display()))?;

    println!(
        "pi5: built {} ({} bytes, entry {:#x} = first byte of the file)",
        image_path.display(),
        image.bytes.len(),
        image.entry
    );
    println!();
    print!("{}", pi5::placement_instructions(image.bytes.len(), &image_sha256));

    let Some(port) = port else {
        println!();
        println!("no --port given: image built and placement printed. To capture a run:");
        println!(
            "  cargo run -p xtask -- pi5 --fixture={} --port=COM3   (or /dev/ttyUSB0)",
            fixture.name
        );
        return Ok(XtaskExit::KernelBootSucceeded);
    };

    let serial = open_serial(port, baud)?;
    let policy = pi5::CapturePolicy {
        max_bytes: pi5::CapturePolicy::BRING_UP.max_bytes,
        overall_ms: timeout_secs.saturating_mul(1000),
        quiet_ms: quiet_secs.saturating_mul(1000),
    };
    println!();
    println!(
        "pi5: capturing {port} at {baud} baud — power-cycle the board now \
         (window {timeout_secs}s, quiet window {quiet_secs}s)"
    );
    let mut source = pi5::ChannelChunks::spawn(serial, std::time::Duration::from_millis(100));
    let clock = pi5::SystemClock::new();
    let (captured, end) = pi5::capture(&mut source, &clock, &policy);
    let outcome = pi5::classify(&captured);

    // Attribution (`TEST-P1-07-05-A` clause 7): the capture and its record
    // land together, named by time and fixture, so a Report quoting the bytes
    // can name the invocation that produced them.
    let timestamp_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let run_dir = out_dir.join("runs").join(format!("{timestamp_unix}-{}", fixture.name));
    std::fs::create_dir_all(&run_dir)
        .map_err(|e| format!("cannot create {}: {e}", run_dir.display()))?;
    let capture_path = run_dir.join("capture.log");
    std::fs::write(&capture_path, &captured)
        .map_err(|e| format!("cannot write {}: {e}", capture_path.display()))?;
    let commit = git_head(&os_root);
    let capture_sha256 = pi5::sha256_hex(&captured);
    let record = pi5::RunRecord {
        commit: &commit,
        fixture: fixture.name,
        port,
        baud,
        board_revision,
        firmware_version,
        image_sha256: &image_sha256,
        image_bytes: image.bytes.len(),
        capture_sha256: &capture_sha256,
        capture_bytes: captured.len(),
        capture_end: end,
        outcome: &outcome,
        timestamp_unix,
    };
    let record_path = run_dir.join("record.tsv");
    std::fs::write(&record_path, pi5::render_run_record(&record))
        .map_err(|e| format!("cannot write {}: {e}", record_path.display()))?;

    // Print what was seen, always — clause 3: each outcome "prints what it
    // saw", and on a bring-up the transcript IS the diagnostic.
    println!();
    println!("pi5: capture ended ({}) after {} bytes", end.as_str(), captured.len());
    println!("----------------------------------------------------------------");
    print!("{}", String::from_utf8_lossy(&captured));
    println!("----------------------------------------------------------------");
    match &outcome {
        pi5::Pi5Outcome::Pass { fixture } => {
            println!("pi5: PASS — the board's own `{fixture}` verdict is ok=true");
        }
        pi5::Pi5Outcome::ReportedFailure { fixture } => {
            println!("pi5: REPORTED FAILURE — the board's own `{fixture}` verdict is ok=false");
        }
        pi5::Pi5Outcome::Silence => {
            println!(
                "pi5: SILENCE — not one byte arrived. Check in this order: adapter \
                 loopback, connector muxing (SWD vs UART), config.txt has os_check=0, \
                 THEN suspect the code (divergence record §§1-4)"
            );
        }
        pi5::Pi5Outcome::SpokeWithoutVerdict { detail } => {
            println!(
                "pi5: SPOKE WITHOUT VERDICT — bytes arrived but no trustworthy verdict \
                 did ({detail})"
            );
        }
    }
    println!("pi5: retained {} and {}", capture_path.display(), record_path.display());
    Ok(pi5::outcome_exit(&outcome))
}

/// Best-effort `git rev-parse HEAD` — "unrecorded" rather than an error, so a
/// capture taken outside a git checkout still gets a record that says so.
fn git_head(os_root: &Path) -> String {
    Command::new("git")
        .current_dir(os_root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|stdout| stdout.trim().to_string())
        .filter(|commit| !commit.is_empty())
        .unwrap_or_else(|| "unrecorded".to_string())
}

/// Opens the serial device after configuring its line settings through the
/// platform's own tool (`mode` / `stty`) — the one seam `TEST-P1-07-05-A`
/// clause 6 leaves untested, kept exactly as thin as it reads.
fn open_serial(port: &str, baud: u32) -> Result<std::fs::File, String> {
    #[cfg(windows)]
    {
        let configure =
            format!("mode {port}: BAUD={baud} PARITY=n DATA=8 STOP=1 to=off xon=off octs=off");
        let status = Command::new("cmd")
            .args(["/C", &configure])
            .status()
            .map_err(|e| format!("failed to invoke `mode`: {e}"))?;
        if !status.success() {
            return Err(format!("`{configure}` failed — is {port} present and free?"));
        }
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(format!(r"\\.\{port}"))
            .map_err(|e| format!("cannot open {port}: {e}"))
    }
    #[cfg(not(windows))]
    {
        let status = Command::new("stty")
            .args([
                "-F",
                port,
                &baud.to_string(),
                "raw",
                "cs8",
                "-cstopb",
                "-parenb",
                "-echo",
                "clocal",
                "-crtscts",
            ])
            .status()
            .map_err(|e| format!("failed to invoke `stty`: {e}"))?;
        if !status.success() {
            return Err(format!("stty could not configure {port}"));
        }
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(port)
            .map_err(|e| format!("cannot open {port}: {e}"))
    }
}

/// Runs the Tier 0 measurement fixture `runs` times, parsing each run's
/// captured COM1 stream into structured percentile records and reporting
/// per-metric run-to-run variance — `STORY-P1-01-01`'s acceptance criteria 2
/// and 3.
///
/// Exit-code discipline, matching `qemu-x86_64`'s own: the fixture's own
/// self-consistency failure is [`XtaskExit::KernelBootFailed`] (1), while a
/// stream this harness cannot trust — malformed, truncated, unparseable, or
/// disagreeing between runs — is a [`XtaskExit::HarnessError`] (2). It is
/// never a pass, and never a quietly smaller set of samples: that fail-closed
/// rule is `FEAT-P1-01`'s containment contract (`BND-15`/`BND-16`/`BND-17`).
///
/// `keep_dir`, when given, retains each run's raw capture there, so a Report
/// can cite the actual bytes the numbers were parsed from rather than a
/// summary of them.
/// Every run's parsed evidence, plus whether every fixture said it passed.
struct MeasuredRuns {
    envelopes: Vec<timing::Envelope>,
    fixture_ok: bool,
}

/// Builds the fixture once, boots it `runs` times, and parses each run's
/// captured COM1 stream into an [`timing::Envelope`] plus its UART-borne
/// verdict.
///
/// Shared by `measure` (which reports) and `check-timing-regression` (which
/// gates), so the gate can never be looking at evidence gathered differently
/// from the evidence a developer sees.
fn run_measurements(
    target: MeasurableTarget,
    runs: usize,
    keep_dir: Option<&Path>,
    profile: Profile,
) -> Result<MeasuredRuns, String> {
    if runs == 0 {
        return Err("measure needs at least one run (--runs=N, default 3)".to_string());
    }
    let capture_dir = match keep_dir {
        Some(dir) => {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("could not create {}: {e}", dir.display()))?;
            dir.to_path_buf()
        }
        None => env::temp_dir().join("tinyos-measure"),
    };
    if keep_dir.is_none() {
        std::fs::create_dir_all(&capture_dir)
            .map_err(|e| format!("could not create {}: {e}", capture_dir.display()))?;
    }

    let mut envelopes = Vec::with_capacity(runs);
    let mut fixture_failed = false;
    for run in 1..=runs {
        let capture = capture_dir.join(format!("measure-run-{run}.log"));
        // Remove any previous capture first: parsing a stale file from an
        // earlier run would be exactly the silent-wrong-evidence failure this
        // command exists to prevent.
        let _ = std::fs::remove_file(&capture);
        let outcome = qemu_x86_64(
            target.package,
            target.binary,
            target.feature,
            Some(capture.as_path()),
            profile,
        )?;
        let text = std::fs::read_to_string(&capture).map_err(|e| {
            format!("run {run} produced no readable serial capture at {}: {e}", capture.display())
        })?;
        let envelope = timing::parse_stream(&text)
            .map_err(|error| format!("run {run} ({}): {error}", capture.display()))?;
        // The UART-borne verdict (`STORY-P1-01-02`, `LE-09` piece 4). On Tier 0
        // both signals exist, so they are cross-checked: that is what
        // establishes the UART bit is trustworthy *before* reaching a board
        // where it is the only bit there is.
        let verdict = timing::parse_result(&text)
            .map_err(|error| format!("run {run} ({}): {error}", capture.display()))?;
        let exit_ok = match outcome {
            XtaskExit::KernelBootSucceeded => true,
            XtaskExit::KernelBootFailed => false,
            // The board-only codes cannot come out of `qemu_x86_64` — they
            // exist for the `pi5` path, which has no isa-debug-exit port.
            XtaskExit::HarnessError
            | XtaskExit::BoardSilent
            | XtaskExit::BoardSpokeWithoutVerdict => {
                return Err(format!("run {run} could not be executed"));
            }
        };
        if verdict.ok != exit_ok {
            return Err(format!(
                "run {run} ({}): {}",
                capture.display(),
                timing::TimingError::ResultDisagreesWithExitCode { uart_ok: verdict.ok, exit_ok }
            ));
        }
        if !verdict.ok {
            eprintln!(
                "xtask measure: run {run}'s `{}` fixture reported a self-consistency failure (see {})",
                verdict.fixture,
                capture.display()
            );
            fixture_failed = true;
        }
        println!(
            "measure run {run}/{runs}: tier={} arch={} cycle_source={} overhead_cycles={} cycles_per_us={} metrics={}",
            envelope.tier,
            envelope.arch,
            envelope.cycle_source,
            envelope.overhead_cycles,
            envelope
                .cycles_per_us
                .map(|factor| factor.to_string())
                .unwrap_or_else(|| "unknown".to_string()),
            envelope.metrics.len()
        );
        for record in &envelope.metrics {
            println!(
                "  {:<8} {:<44} n={:<6} dropped={} p50={} p99={} p99.9={} max={} ({})",
                record.domain,
                record.metric,
                record.n,
                record.dropped,
                record.p50,
                record.p99,
                record.p99_9,
                record.max,
                record.unit
            );
        }
        envelopes.push(envelope);
    }

    Ok(MeasuredRuns { envelopes, fixture_ok: !fixture_failed })
}

/// Reports Tier 0 measurements and their run-to-run variance —
/// `STORY-P1-01-01`'s acceptance criteria 2 and 3.
fn measure(
    target: MeasurableTarget,
    runs: usize,
    keep_dir: Option<&Path>,
    profile: Profile,
) -> Result<XtaskExit, String> {
    let measured = run_measurements(target, runs, keep_dir, profile)?;
    let envelopes = measured.envelopes;

    if envelopes.len() >= 2 {
        let comparisons = timing::compare_runs(&envelopes).map_err(|error| format!("{error}"))?;
        println!("\nrun-to-run variance across {} runs:", envelopes.len());
        for comparison in comparisons {
            println!(
                "  {:<52} p99s={:?} p99_cv={:.2}% maxes={:?}",
                comparison.key, comparison.p99s, comparison.p99_cv_percent, comparison.maxes
            );
        }
    } else {
        println!(
            "\nrun-to-run variance: not computed — a single run cannot establish it (use --runs=3 or more)"
        );
    }
    println!(
        "\nTier 0 (QEMU/TCG) evidence only: these cycle counts calibrate the harness and the \
         regression mechanism, and are not hardware WCET evidence. Hardware-tier timing debt \
         stays open until measured on the Raspberry Pi 5 (loose end LE-09)."
    );

    if measured.fixture_ok {
        Ok(XtaskExit::KernelBootSucceeded)
    } else {
        Ok(XtaskExit::KernelBootFailed)
    }
}

/// Where the committed Tier 0 baseline lives, relative to the repository root.
const BASELINE_RELATIVE: [&str; 3] = ["goals", "performance", "baselines"];
/// The committed Tier 0 baseline's file name.
const BASELINE_FILE: &str = "tier0-x86_64.tsv";

/// `STORY-P1-01-02`'s gate: measure, compare against committed baselines, and
/// fail the build on a timing regression exactly as a functional failure does.
///
/// Exit-code discipline, matching every other `xtask` command: **0** pass,
/// **1** a timing regression (or a fixture whose own self-checks failed),
/// **2** a harness error — a missing or malformed baseline, an unparseable
/// stream, a metric on one side of the comparison and not the other. A missing
/// baseline is deliberately *not* a skip: "no baseline yet, pass" is how a
/// regression gate quietly becomes decoration.
///
/// Measurement is always **release-profile** (`LE-13`). A dev-profile number is
/// not a noisier version of the same thing; it is a different binary.
fn check_timing_regression(
    runs: usize,
    baseline_path: Option<PathBuf>,
    update: bool,
    date: Option<String>,
    target: MeasurableTarget,
) -> Result<XtaskExit, String> {
    let profile = Profile::Release;
    if runs < gate::MINIMUM_RUNS {
        return Err(format!(
            "check-timing-regression needs at least {} runs to take a median (--runs=N)",
            gate::MINIMUM_RUNS
        ));
    }
    let os_root = os_root()?;
    let repo_root = os_root
        .parent()
        .ok_or_else(|| format!("could not resolve repository root from {}", os_root.display()))?;
    let path = baseline_path.unwrap_or_else(|| {
        let mut path = repo_root.to_path_buf();
        for part in BASELINE_RELATIVE {
            path.push(part);
        }
        path.push(BASELINE_FILE);
        path
    });

    let measured = run_measurements(target, runs, None, profile)?;
    if !measured.fixture_ok {
        eprintln!(
            "xtask check-timing-regression: a fixture reported a self-consistency failure, so its numbers are not evidence"
        );
        return Ok(XtaskExit::KernelBootFailed);
    }

    if update {
        let recorded_on = date.ok_or_else(|| {
            "--update-baseline needs --date=YYYY-MM-DD: the recorded date is committed data, not a wall clock this tool reads"
                .to_string()
        })?;
        let rendered = gate::render_baseline(&measured.envelopes, profile.name(), &recorded_on)
            .map_err(|error| format!("{error}"))?;
        // Round-trip what is about to be written, so this tool can never emit
        // a baseline its own gate would reject.
        gate::parse_baseline(&rendered).map_err(|error| {
            format!("refusing to write a baseline this parser rejects: {error}")
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
        }
        std::fs::write(&path, &rendered)
            .map_err(|e| format!("could not write {}: {e}", path.display()))?;
        println!(
            "check-timing-regression: wrote {} ({runs} runs, {} profile)",
            path.display(),
            profile.name()
        );
        print!("{rendered}");
        return Ok(XtaskExit::KernelBootSucceeded);
    }

    let text = std::fs::read_to_string(&path).map_err(|e| {
        format!(
            "no committed baseline at {} ({e}) — a missing baseline is a gate failure, not a skip; record one with `--update-baseline --date=YYYY-MM-DD`",
            path.display()
        )
    })?;
    let baseline = gate::parse_baseline(&text)
        .map_err(|error| format!("{} is not a usable baseline: {error}", path.display()))?;
    let comparisons = gate::check_against_baseline(
        &baseline,
        &measured.envelopes,
        profile.name(),
        gate::TIER0_POLICY,
    )
    .map_err(|error| format!("{error}"))?;

    // The reference's absolute value is the direct measure of how fast this
    // runner was, and reading it is how anyone diagnoses this gate in future
    // (`TEST-P1-01-04-A` clause 6). It goes first, before any verdict, because
    // every ratio below is denominated in it.
    let reference_p50 = comparisons
        .iter()
        .find(|c| c.key == gate::REFERENCE_METRIC && c.statistic == "p50")
        .map(|c| c.observed_cycles)
        .unwrap_or(0);
    println!(
        "\ntiming gate: {runs} runs, {} profile. Gated quantity is each metric's same-run ratio to\n\
         `{}` (median of the per-run ratios), tolerance = max({}%, {} ppm).\n\
         This run's reference measured p50={reference_p50} cycles — that is how fast the machine was,\n\
         and a ratio gate is what keeps that number out of the verdict.",
        profile.name(),
        gate::REFERENCE_METRIC,
        gate::TIER0_POLICY.ratio.relative_percent,
        gate::TIER0_POLICY.ratio.absolute_floor,
    );
    let mut regressed = 0usize;
    for comparison in &comparisons {
        let verdict = match comparison.verdict {
            gate::Verdict::Pass => "ok",
            gate::Verdict::Regressed => {
                regressed += 1;
                "REGRESSED"
            }
            gate::Verdict::ImprovedBeyondTolerance => "improved (is the baseline stale?)",
            gate::Verdict::ReportedNotGated => "reported, NOT gated",
        };
        // Both numbers on every line: the one that decided the verdict, named
        // by its unit, and the other one labelled with what it is — so a
        // reader is never left guessing which number produced the verdict.
        // The reference is the one row whose cycles *are* the gated quantity,
        // so it must not also be captioned "not gated".
        let (unit, aside) = match comparison.quantity {
            gate::Quantity::RatioPpm => (
                "ppm",
                format!(
                    "[{} -> {} cycles, not gated]",
                    comparison.baseline_cycles, comparison.observed_cycles
                ),
            ),
            gate::Quantity::Cycles => {
                ("cyc", "[structural band on the reference, not a regression check]".to_string())
            }
        };
        println!(
            "  {:<52} {:<4} baseline={:<9} observed={:<9} limit={:<9} {unit}  {aside:<46}  {verdict}",
            comparison.key,
            comparison.statistic,
            comparison.baseline,
            comparison.observed,
            comparison.limit,
        );
    }

    // A metric that is measured but carries no verdict has to say why, on
    // every run, in the gate's own output — not only in a source comment
    // somebody would have to go looking for.
    if !gate::UNGATED_AT_TIER0.is_empty() {
        println!("\nmeasured and baselined, but deliberately carrying no verdict:");
        for (metric, reason) in gate::UNGATED_AT_TIER0 {
            println!("  {metric}\n    {reason}");
        }
    }

    // The tails are printed and explicitly *not* gated: Tier 0 run-to-run p99
    // variation was measured at 39–61% (`REPORT-2026-07-27-02`), so a tail
    // threshold would fail green code. Printing them unlabelled would be worse
    // than not printing them at all — a reader would assume they had passed
    // something.
    println!("\nreported but NOT gated (Tier 0 tail variance makes these unusable as thresholds):");
    for record in &measured.envelopes[0].metrics {
        println!(
            "  {:<52} p99={:<7} p99.9={:<7} max={:<7} (run 1 of {runs})",
            record.key(),
            record.p99,
            record.p99_9,
            record.max
        );
    }
    println!(
        "\nTier 0 (QEMU/TCG) evidence only: this gate detects regressions in the mechanism it \
         measures. It closes no PERF guardrail, and hardware-tier timing debt stays open until \
         measured on the Raspberry Pi 5 (loose end LE-09).\n\
         A uniform slowdown of everything, the reference included, passes by construction — that \
         is the price of a verdict that survives a busy runner, and LE-16 is restated in ratio \
         units rather than closed (STORY-P1-01-04)."
    );

    if regressed > 0 {
        eprintln!("xtask: {regressed} gated statistic(s) regressed beyond tolerance");
        return Ok(XtaskExit::KernelBootFailed);
    }
    // Count what was actually gated, not what was printed. Reporting 14 when
    // two of those statistics deliberately carry no verdict would overstate
    // the gate's coverage by exactly the amount this Story removed.
    let gated = comparisons
        .iter()
        .filter(|comparison| comparison.verdict != gate::Verdict::ReportedNotGated)
        .count();
    println!(
        "check-timing-regression: no regression across {gated} gated statistics ({} reported without a verdict)",
        comparisons.len() - gated
    );
    Ok(XtaskExit::KernelBootSucceeded)
}

/// Reads `input`, re-layouts it via [`txe::pack`] (`STORY-P0-08-01`), and
/// writes the result to `output` — the host-side half of producing a TXE
/// from a real PE build artifact.
fn pack_txe(input: &str, output: &str) -> Result<(), String> {
    let bytes = std::fs::read(input).map_err(|e| format!("failed to read {input}: {e}"))?;
    let packed = txe::pack(&bytes).map_err(|e| format!("failed to pack {input}: {e:?}"))?;
    std::fs::write(output, &packed).map_err(|e| format!("failed to write {output}: {e}"))?;
    println!("pack-txe: wrote {output} ({} bytes, from {} bytes)", packed.len(), bytes.len());
    Ok(())
}

/// Builds the **system image** — the `os` binary — against the custom
/// x86_64 target and checks its file size against `ceiling`: `G-DX-8`'s
/// whole-image (not per-crate) budget, applied to what actually ships.
///
/// **This moved from `kernel` to `os` in `STORY-P1-03-03`, and the move is
/// the point.** Until then `kernel`'s binary *was* the shipping image, and
/// its own doc comment noted that it "links no `exec` code" — true, and
/// exactly the gap that meant the shipping image had never loaded or
/// scheduled anything. `os` links `kernel` *and* `exec` *and* the embedded
/// workload it schedules, so this number now covers the loader, the address
/// spaces, the capability shim and the image being run. Measuring `kernel`
/// today would be measuring a library's test harness rather than the
/// product, and would quietly under-report the budget it exists to enforce.
///
/// Tier 0 fixture binaries (`exec-fixture`, `blue-sharc-fixture`,
/// `wx-seal-fixture`, ...) are still excluded: they exist only to drive
/// QEMU-harness tests and never ship — the same "test code doesn't count"
/// convention `check-crate-sizes` applies by excluding `#[cfg(test)]`
/// bodies. `blue-sharc.exe` in particular is an 8.3MiB third-party
/// application that lives in a fixture precisely so it is not part of this
/// measurement; the workload `os` embeds is the 16KiB capability probe.
fn check_image_size(ceiling: u64) -> Result<u64, String> {
    let os_root = os_root()?;
    let target_spec = os_root.join("targets").join("x86_64-tinyos.json");

    let build_status = Command::new("cargo")
        .current_dir(&os_root)
        .arg("build")
        .arg("--release")
        .arg("-p")
        .arg("os")
        .arg("--target")
        .arg(&target_spec)
        .arg("-Z")
        .arg("json-target-spec")
        .arg("-Z")
        .arg("build-std=core,compiler_builtins")
        .arg("-Z")
        .arg("build-std-features=compiler-builtins-mem")
        .status()
        .map_err(|e| format!("failed to invoke cargo build: {e}"))?;
    if !build_status.success() {
        return Err("os release build failed".to_string());
    }

    let image = os_root.join("target").join("x86_64-tinyos").join("release").join("os");
    let size = std::fs::metadata(&image)
        .map_err(|e| format!("could not stat {}: {e}", image.display()))?
        .len();
    if size > ceiling {
        return Err(format!("system image is {size} bytes, exceeding the {ceiling}-byte ceiling"));
    }
    Ok(size)
}

/// Locates `qemu-system-x86_64` on `PATH`, falling back to the default
/// Windows install location for the `SoftwareFreedomConservancy.QEMU`
/// winget package, since MSI/EXE installers on Windows don't always update
/// `PATH` for already-open shells.
fn find_qemu() -> Result<PathBuf, String> {
    let exe_name = if cfg!(windows) { "qemu-system-x86_64.exe" } else { "qemu-system-x86_64" };

    if let Ok(path_var) = env::var("PATH") {
        let separator = if cfg!(windows) { ';' } else { ':' };
        for dir in path_var.split(separator) {
            let candidate = Path::new(dir).join(exe_name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }

    if cfg!(windows) {
        let fallback = PathBuf::from(r"C:\Program Files\qemu").join(exe_name);
        if fallback.is_file() {
            return Ok(fallback);
        }
    }

    Err(format!(
        "could not find {exe_name} on PATH (or the default Windows install location); install QEMU and ensure it is reachable"
    ))
}

/// The `os/` workspace root, resolved relative to this crate's own
/// `CARGO_MANIFEST_DIR` so `xtask` works regardless of the caller's cwd.
fn os_root() -> Result<PathBuf, String> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest_dir)
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("could not resolve os/ root from {manifest_dir}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .expect("xtask manifest lives at os/src/xtask")
            .to_path_buf()
    }

    #[test]
    fn fixture_names_are_unique() {
        let mut seen = BTreeSet::new();
        for fixture in FIXTURES {
            assert!(seen.insert(fixture.name), "duplicate fixture `{}`", fixture.name);
        }
    }

    #[test]
    fn the_default_boot_resolves_without_a_fixture_flag() {
        let default = qemu_fixture(None).expect("the no-fixture boot must resolve");
        assert_eq!(default.package, "kernel");
        assert!(!default.expects_failure);
    }

    #[test]
    fn an_unknown_fixture_is_rejected() {
        assert!(qemu_fixture(Some("no-such-fixture")).is_none());
    }

    #[test]
    fn exactly_four_fixtures_document_failure_as_their_pass_condition() {
        // `broken-boot`, `idt-apic-unrouted`, `wcet-trip` and
        // `actuation-overrun`. Pinned so that adding a fifth is a deliberate
        // act: these are the ones whose CI steps invert the exit code, and an
        // inverted step is the one place a fixture that never ran at all is
        // indistinguishable from one that passed.
        //
        // `wcet-trip` closed that exit-code hole by asserting its own
        // `TOS64-RESULT/1` line as well as the exit code; `actuation-overrun`
        // (`STORY-P1-06-01`) is held to the same standard by its own CI step,
        // and its `run` emits a *failing* result line on every path that
        // reaches the safe state for the wrong reason.
        let failing: Vec<&str> =
            FIXTURES.iter().filter(|f| f.expects_failure).map(|f| f.name).collect();
        assert_eq!(
            failing,
            vec!["broken-boot", "idt-apic-unrouted", "wcet-trip", "actuation-overrun"]
        );
    }

    #[test]
    fn every_ci_fixture_value_exists_in_the_table() {
        // The drift guard. CI was the only place the full fixture set could be
        // read; if a step names a fixture this table does not, the table has
        // stopped being the source of truth it claims to be.
        let workflow = fs::read_to_string(repo_root().join(".github/workflows/ci.yml"))
            .expect("CI workflow must be readable");
        let mut checked = 0;
        for (index, _) in workflow.match_indices("--fixture=") {
            let rest = &workflow[index + "--fixture=".len()..];
            let name: String =
                rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
            // `--fixture=` is overloaded across two subcommands with two
            // separate namespaces, so a CI value may legitimately resolve in
            // either. Both are printed by `list-fixtures`.
            assert!(
                qemu_fixture(Some(&name)).is_some() || measurable_fixture(&name).is_some(),
                "CI runs `--fixture={name}` but neither fixture table has such an entry"
            );
            checked += 1;
        }
        assert!(checked > 0, "the scan must actually find CI fixture invocations");
    }

    // TEST-P0-01-04-A clause 4. A registry that names an owning Test which
    // does not exist reads as traceability and provides none — the same
    // failure class the assurance spine already rejects for Stories and
    // Features, applied to the table that claims to be the fixture set's
    // source of truth.
    #[test]
    fn every_fixture_declares_an_owning_test_that_exists() {
        let tests = repo_root().join("goals").join("tests");
        for fixture in FIXTURES {
            let path = tests.join(format!("{}.md", fixture.owning_test));
            assert!(
                path.is_file(),
                "fixture `{}` declares owning test `{}`, which is not a document under goals/tests/",
                fixture.name,
                fixture.owning_test
            );
        }
    }

    // TEST-P0-01-04-A clause 5. The existing drift guard checks CI -> table.
    // This is the reverse and more dangerous direction: a fixture that
    // exists, compiles, and is never run is an unverified fixture that looks
    // verified. LE-07 in a new place.
    #[test]
    fn every_fixture_in_the_table_is_run_by_ci() {
        let workflow = fs::read_to_string(repo_root().join(".github/workflows/ci.yml"))
            .expect("CI workflow must be readable");
        let mut invoked: BTreeSet<String> = BTreeSet::new();
        for (index, _) in workflow.match_indices("--fixture=") {
            let rest = &workflow[index + "--fixture=".len()..];
            invoked.insert(
                rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect(),
            );
        }
        // Three shapes count as "run", and conflating them would overstate
        // the gap:
        //   - `--fixture=<name>` on `qemu-x86_64` (the common case),
        //   - the bare `qemu-x86_64` / `measure` subcommands, which invoke
        //     their namespace's default with no flag at all,
        //   - a name that resolves in the *measurable* namespace, which CI
        //     drives through `measure` and `check-timing-regression`.
        // The last is why `--fixture=` is overloaded across two namespaces,
        // which is the condition the existing drift guard already documents.
        // Line-based rather than a substring search on `\n`: the workflow is
        // checked out with CRLF on Windows, so anchoring on a bare newline
        // silently matches nothing and reports every default-invoked fixture
        // as unrun.
        let invoked_bare =
            |suffix: &str| workflow.lines().any(|line| line.trim_end().ends_with(suffix));
        // Credit coverage by *build target*, not by name. `--fixture=` is
        // overloaded across two namespaces and the same binary can be spelled
        // differently in each — `dispatch` and `dispatch-measure` are one
        // fixture — so comparing names would report a fixture as unrun that
        // CI runs under its other spelling.
        type Target = (&'static str, &'static str, Option<&'static str>);
        let mut covered: BTreeSet<Target> = BTreeSet::new();
        if invoked_bare("xtask -- qemu-x86_64") {
            if let Some(f) = qemu_fixture(None) {
                covered.insert((f.package, f.binary, f.feature));
            }
        }
        if invoked_bare("xtask -- measure --runs=1") {
            if let Some(t) = measurable_fixture("measure") {
                covered.insert((t.package, t.binary, t.feature));
            }
        }
        for name in &invoked {
            if let Some(f) = qemu_fixture(Some(name)) {
                covered.insert((f.package, f.binary, f.feature));
            }
            if let Some(t) = measurable_fixture(name) {
                covered.insert((t.package, t.binary, t.feature));
            }
        }
        let missing: Vec<&str> = FIXTURES
            .iter()
            .filter(|f| !covered.contains(&(f.package, f.binary, f.feature)))
            .map(|f| f.name)
            .collect();
        assert!(
            missing.is_empty(),
            "these fixtures exist but no CI step runs them, so they are unverified while looking verified: {missing:?}"
        );
    }

    #[test]
    fn usage_names_every_subcommand() {
        let text = usage();
        for entry in SUBCOMMANDS {
            assert!(text.contains(entry.name), "usage omits `{}`", entry.name);
        }
    }
}
