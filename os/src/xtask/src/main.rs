//! `xtask` — the single, cross-platform home for build/test/QEMU-launch/deploy
//! commands, so no contributor has to remember the `cargo build --target
//! ... -Z build-std` incantation by hand and CI invokes the exact same
//! command a local developer would (`G-DX-3`).
//!
//! Application-level host tooling: `#![forbid(unsafe_code)]` per its `std`
//! classification in `docs/mvp-delivery-strategy.md#crate-map`.

#![forbid(unsafe_code)]

mod assurance;
mod gate;
mod governance;
mod performance_catalogue;
mod probe_pe;
mod timing;
mod txe;

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

/// Process exit codes `xtask` itself returns, distinguishing "the thing
/// under test failed" from "the harness couldn't even run the test" — the
/// distinction `STORY-P0-01-03`'s acceptance criteria require.
#[repr(u8)]
enum XtaskExit {
    KernelBootSucceeded = 0,
    KernelBootFailed = 1,
    HarnessError = 2,
}

/// A fixture that emits a `TINYOS-MEAS/1` envelope, identified by what has
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
        _ => None,
    }
}

/// QEMU's own process exit code when the guest writes to the isa-debug-exit
/// port: `(value << 1) | 1`. See `kernel/src/qemu_exit.rs`.
const QEMU_EXIT_SUCCESS: i32 = (0x10 << 1) | 1; // 33
const QEMU_EXIT_FAILURE: i32 = (0x11 << 1) | 1; // 35

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        eprintln!(
            "usage: cargo run -p xtask -- <qemu-x86_64|measure|check-timing-regression|check-assurance-spine|check-crate-sizes|check-image-size|check-performance-catalogue|governance-fixture-test|pack-txe> [options]"
        );
        return ExitCode::from(XtaskExit::HarnessError as u8);
    };

    match command.as_str() {
        "qemu-x86_64" => {
            let fixture = args.find_map(|a| a.strip_prefix("--fixture=").map(str::to_string));
            // `address-space` (`TEST-P0-05-02-A`) boots a different binary
            // package (`exec`'s `exec-fixture`, not `kernel`) entirely —
            // see `exec/Cargo.toml`'s `[[bin]]` comment for why `exec`'s
            // Tier 0 fixture can't just be another `kernel` Cargo feature
            // like `broken-boot`/`context-switch` are.
            let (package, binary, feature) = match fixture.as_deref() {
                None => ("kernel", "kernel", None),
                // The system image (`STORY-P1-03-03`): the real boot path
                // that discovers hardware, installs W^X address spaces, and
                // schedules a real loaded task. Lives in its own top-level
                // package because it depends on *both* `kernel` and `exec`,
                // which `kernel`'s own binary can never do.
                Some("os") => ("os", "os", None),
                Some("broken-boot") => ("kernel", "kernel", Some("fixture-broken-boot")),
                Some("context-switch") => ("kernel", "kernel", Some("fixture-context-switch")),
                Some("address-space") => ("exec", "exec-fixture", None),
                Some("win32-shim") => ("exec", "win32-shim-fixture", None),
                Some("blue-sharc") => ("exec", "blue-sharc-fixture", None),
                Some("blue-sharc-broken") => ("exec", "blue-sharc-broken-fixture", None),
                Some("shared-memory") => ("exec", "shared-memory-fixture", None),
                Some("address-space-switch") => ("exec", "address-space-switch-fixture", None),
                Some("wx-seal") => ("exec", "wx-seal-fixture", None),
                Some("first-task") => ("exec", "first-task-fixture", None),
                Some("dispatch-measure") => ("exec", "dispatch-measure-fixture", None),
                Some("idt-apic-timer") => ("kernel", "kernel", Some("fixture-idt-apic-timer")),
                Some("idt-apic-unrouted") => {
                    ("kernel", "kernel", Some("fixture-idt-apic-unrouted"))
                }
                Some("pci-enumeration") => ("kernel", "kernel", Some("fixture-pci-enumeration")),
                Some("pool-bench") => ("kernel", "kernel", Some("fixture-pool-bench")),
                Some("measure") => ("kernel", "kernel", Some("fixture-measure")),
                Some("fault") => ("kernel", "kernel", Some("fixture-fault")),
                Some("double-fault") => ("kernel", "kernel", Some("fixture-double-fault")),
                Some("preempt") => ("kernel", "kernel", Some("fixture-preempt")),
                Some("priority-inversion") => {
                    ("kernel", "kernel", Some("fixture-priority-inversion"))
                }
                Some(other) => {
                    eprintln!("xtask: unknown --fixture value '{other}'");
                    return ExitCode::from(XtaskExit::HarnessError as u8);
                }
            };
            match qemu_x86_64(package, binary, feature, None, Profile::Dev) {
                Ok(code) => ExitCode::from(code as u8),
                Err(message) => {
                    eprintln!("xtask: {message}");
                    ExitCode::from(XtaskExit::HarnessError as u8)
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
                        "assurance-spine-check: {} Features, {} Stories, {} Tests, {} Reports, {} containment classes, {} boundary tests, {} security controls, {} Protection Domain contracts, {} code-admission gates, {} class communication pairs, {} application/platform targets, {} landing zones, {} selected Story/performance contracts, {} selected application/performance contracts",
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
                        summary.selected_application_performance_contracts
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
            let output = args.find_map(|a| a.strip_prefix("--output=").map(str::to_string));
            let Some(output) = output else {
                eprintln!("usage: cargo run -p xtask -- make-probe-pe --output=<path>");
                return ExitCode::from(XtaskExit::HarnessError as u8);
            };
            let image = probe_pe::build();
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
            XtaskExit::HarnessError => {
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
        gate::TIER0_TOLERANCE,
    )
    .map_err(|error| format!("{error}"))?;

    println!(
        "\ntiming gate: {runs} runs, {} profile, tolerance = max({}%, {} cycles) applied to the median across runs",
        profile.name(),
        gate::TIER0_TOLERANCE.relative_percent,
        gate::TIER0_TOLERANCE.absolute_cycles
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
        };
        println!(
            "  {:<52} {:<4} baseline={:<7} observed={:<7} limit={:<7} {}",
            comparison.key,
            comparison.statistic,
            comparison.baseline,
            comparison.observed,
            comparison.limit,
            verdict
        );
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
         measured on the Raspberry Pi 5 (loose end LE-09)."
    );

    if regressed > 0 {
        eprintln!("xtask: {regressed} gated statistic(s) regressed beyond tolerance");
        return Ok(XtaskExit::KernelBootFailed);
    }
    println!(
        "check-timing-regression: no regression across {} gated statistics",
        comparisons.len()
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
