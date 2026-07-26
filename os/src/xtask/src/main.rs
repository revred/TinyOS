//! `xtask` — the single, cross-platform home for build/test/QEMU-launch/deploy
//! commands, so no contributor has to remember the `cargo build --target
//! ... -Z build-std` incantation by hand and CI invokes the exact same
//! command a local developer would (`G-DX-3`).
//!
//! Application-level host tooling: `#![forbid(unsafe_code)]` per its `std`
//! classification in `docs/mvp-delivery-strategy.md#crate-map`.

#![forbid(unsafe_code)]

mod governance;
mod performance_catalogue;

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

/// QEMU's own process exit code when the guest writes to the isa-debug-exit
/// port: `(value << 1) | 1`. See `kernel/src/qemu_exit.rs`.
const QEMU_EXIT_SUCCESS: i32 = (0x10 << 1) | 1; // 33
const QEMU_EXIT_FAILURE: i32 = (0x11 << 1) | 1; // 35

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        eprintln!(
            "usage: cargo run -p xtask -- <qemu-x86_64|check-crate-sizes|check-performance-catalogue|governance-fixture-test> [options]"
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
                Some("broken-boot") => ("kernel", "kernel", Some("fixture-broken-boot")),
                Some("context-switch") => ("kernel", "kernel", Some("fixture-context-switch")),
                Some("address-space") => ("exec", "exec-fixture", None),
                Some("win32-shim") => ("exec", "win32-shim-fixture", None),
                Some(other) => {
                    eprintln!("xtask: unknown --fixture value '{other}'");
                    return ExitCode::from(XtaskExit::HarnessError as u8);
                }
            };
            match qemu_x86_64(package, binary, feature) {
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
) -> Result<XtaskExit, String> {
    let os_root = os_root()?;
    let target_spec = os_root.join("targets").join("x86_64-tinyos.json");

    let mut build = Command::new("cargo");
    build
        .current_dir(&os_root)
        .arg("build")
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

    let kernel_elf = os_root.join("target").join("x86_64-tinyos").join("debug").join(binary);
    if !kernel_elf.exists() {
        return Err(format!(
            "expected {binary} binary at {} but it does not exist",
            kernel_elf.display()
        ));
    }

    let qemu_binary = find_qemu()?;
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
        .arg("none")
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
