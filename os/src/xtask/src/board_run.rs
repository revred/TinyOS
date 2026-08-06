//! The tenth stage, and the nine it composes (`LE-95`).
//!
//! # Why this subcommand exists
//!
//! As of 2026-08-06 the board evidence loop was built end to end with exactly
//! one gap. Nine stages worked, were tested, and were each hard-won: build the
//! image, verify the staged digest, serve it, read the log live, wait for a
//! board event, capture the envelope, transmit to the board, parse the
//! envelope, gate the numbers. **The tenth was a human hand on a mains plug.**
//!
//! Because a session cannot power-cycle the board, every session ended at the
//! last thing a laptop can do — so three consecutive handovers opened with a
//! board item nobody could take, and release-gate evidence did not move for
//! forty-eight hours while every local gate stayed green. That is not a
//! discipline failure and a stronger instruction does not fix it: the
//! instruction was addressed to someone who could not execute it.
//!
//! `tos64-power` is the tenth stage. This module is the composition, so the
//! loop is **one command** rather than nine remembered in the right order under
//! the time pressure of a powered board.
//!
//! # The plan is a value, and that is the whole design
//!
//! Same reason as [`crate::boot_images`] and [`crate::guest_images`], and this
//! time it carries more weight than coverage: **the ordering constraints here
//! are safety properties on a mains path, and a safety property expressed as
//! the shape of an imperative function is a safety property nobody can test.**
//!
//! So [`plan`] is a pure function returning [`Step`]s, and the invariants below
//! are host tests rather than careful code:
//!
//! - **The run always ends with the board ON.** [`Step::EnsurePowerOn`] is the
//!   last step of every plan, unconditionally. `off` is the single state a
//!   later session cannot recover from without a hand on the plug, which is the
//!   exact stall this whole subcommand exists to remove — a tool that can end
//!   a run dark has made the problem worse rather than better.
//! - **Nothing rebuilds or re-verifies while a server is serving.** The digest
//!   check precedes the netboot start. `LE-87` cost three power cycles to a
//!   stale image served by a forgotten process, and it was found only because
//!   a metric was missing *by name*.
//! - **The server exists before power reaches the board.** A Pi 5 that DHCPs
//!   into silence retries, and the retry window is not the window the capture
//!   was sized against.
//! - **The watch is armed after the cycle, and only then is anything parsed.**
//! - **The server's own address is named, never discovered** (`LE-97`). This
//!   one follows from the second and third together: the server starts before
//!   power moves, so at that moment the board is off and the bench NIC has no
//!   link — discovery is guaranteed to find nothing on exactly this path.
//!   [`BoardRun::server`] is therefore not an `Option`, `--server=` is required,
//!   and [`server_address`] refuses a bad one before any plan runs. The
//!   interactive tool fell back to `0.0.0.0` in this state on 2026-08-06 and
//!   only a human reading the printed line stopped a boot being diagnosed as a
//!   board fault; on this path there is no human reading.
//!
//! # What this module does NOT claim
//!
//! It has never been run against a plug, because at the time of writing the
//! project does not own one — that purchase is `LE-95`'s owner decision and it
//! is the one item here that cannot be discharged by a session. The plan and
//! its invariants are tested; the executor is a thin process spawner, and it is
//! deliberately thin so that the untested part is as small as it can be. When
//! the relay arrives, the first real run is the executor's only unproven claim.

use std::path::{Path, PathBuf};
use std::process::Command;

/// One stage of the loop, as a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Read the staged image and hold it against the digest the operator
    /// stated. Before anything is served, and before power moves.
    VerifyDigest { image: PathBuf, expected: String },
    /// Start `tos64-netboot` on the staged root, answering exactly one MAC,
    /// **from a named server address** (`LE-97`).
    StartNetboot { mac: String, root: PathBuf, server: String },
    /// `tos64-power cycle` — the stage that did not exist.
    PowerCycle { off_ms: u32, on_wait: u32 },
    /// `ti64dink --until <condition> --timeout <n>`, optionally writing the
    /// harvested envelope where `parse-meas` can read it.
    Watch { until: String, timeout: u32, text: Option<PathBuf> },
    /// `xtask parse-meas` over the captured envelope.
    ParseMeas { capture: PathBuf },
    /// Clause 1 of `LE-95`, as a step rather than as a habit: whatever
    /// happened above, the board is left ON.
    EnsurePowerOn,
}

/// Where the plug is and how it speaks.
///
/// Kept OUT of [`BoardRun`] and out of the plan on purpose: which relay is on
/// the supply is a property of the bench, not of the run, and the ordering
/// invariants the plan exists to carry say nothing about it. Threading it
/// through the steps would put a bench detail inside the values the safety
/// tests read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlugConfig {
    pub url: String,
    pub dialect: String,
    pub entity: Option<String>,
}

impl PlugConfig {
    /// The arguments every `tos64-power` invocation begins with.
    fn args(&self) -> Vec<String> {
        let mut args =
            vec!["--plug".into(), self.url.clone(), "--dialect".into(), self.dialect.clone()];
        if let Some(entity) = &self.entity {
            args.push("--entity".into());
            args.push(entity.clone());
        }
        args
    }
}

/// Everything a run needs, validated at the edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoardRun {
    pub image: PathBuf,
    pub expected_digest: Option<String>,
    pub mac: String,
    pub root: PathBuf,
    /// The bench interface's own address, named rather than discovered
    /// (`LE-97`). Not an `Option`: see [`Step::StartNetboot`] and the test that
    /// explains why this path cannot discover it.
    pub server: String,
    pub off_ms: u32,
    pub on_wait: u32,
    pub until: String,
    pub timeout: u32,
    pub text: Option<PathBuf>,
}

/// The ordered plan for one board run.
///
/// Total, pure, and the only place the ordering lives. Every safety property
/// this subcommand claims is a property of this list, which is why it is a
/// value a test can read rather than the control flow of [`execute`].
#[must_use]
pub fn plan(run: &BoardRun) -> Vec<Step> {
    let mut steps = Vec::new();

    // Before the server: a digest checked while a transfer is in flight is a
    // digest of whatever the file is halfway through becoming.
    if let Some(expected) = &run.expected_digest {
        steps.push(Step::VerifyDigest { image: run.image.clone(), expected: expected.clone() });
    }

    steps.push(Step::StartNetboot {
        mac: run.mac.clone(),
        root: run.root.clone(),
        server: run.server.clone(),
    });
    steps.push(Step::PowerCycle { off_ms: run.off_ms, on_wait: run.on_wait });
    steps.push(Step::Watch {
        until: run.until.clone(),
        timeout: run.timeout,
        text: run.text.clone(),
    });

    if let Some(text) = &run.text {
        steps.push(Step::ParseMeas { capture: text.clone() });
    }

    // Unconditional, and last. Not "on the error path" — on EVERY path, so
    // there is no path to forget.
    steps.push(Step::EnsurePowerOn);
    steps
}

/// Validates a `--server=` value, or says why it cannot be served from
/// (`LE-97`).
///
/// Pure, and checked at the edge **before the plan runs**, because the cost of
/// a bad value here is a wasted power cycle: `tos64-netboot` would refuse to
/// start, `board-run` would cycle the board's mains anyway, and the board would
/// DHCP into silence. Catching it on the laptop costs nothing.
///
/// # Why four octets are checked explicitly
///
/// The far side of this seam is C#, and `.NET`'s `IPAddress.Parse` accepts the
/// historical shorthand forms: `169.254.113` parses **successfully** as
/// 169.254.0.113. A truncated address therefore does not fail — it silently
/// becomes a different, valid, wrong address, which is `LE-97`'s own failure
/// shape one layer down. Both programs check the rule; neither trusts the
/// other, because nothing links them (the same reason
/// [`POWER_EXIT_LEFT_OFF`] is duplicated by value).
pub fn server_address(text: &str) -> Result<String, String> {
    let refuse = |why: &str| {
        Err(format!(
            "`{text}` is not a usable server address: {why}. Pass --server=<ip> with the bench \
             interface's own address — the board is unpowered when the server starts, so its \
             link is down and the address cannot be discovered (LE-97)"
        ))
    };

    let octets: Vec<&str> = text.split('.').collect();
    if octets.len() != 4 {
        return refuse(
            "an IPv4 address is four dotted octets, and a shortened one parses as a DIFFERENT \
             valid address rather than failing",
        );
    }
    let mut parsed = [0u8; 4];
    for (slot, octet) in parsed.iter_mut().zip(&octets) {
        if octet.is_empty() || !octet.bytes().all(|b| b.is_ascii_digit()) {
            return refuse("every octet must be plain decimal digits");
        }
        // A leading zero is an octal ambiguity and `.NET`'s own parser — the
        // one on the other side of this seam — rejects it. Accepting it here
        // would say yes to a value `tos64-netboot` then refuses, which is the
        // one direction of disagreement that costs a mains cycle.
        if octet.len() > 1 && octet.starts_with('0') {
            return refuse("a leading zero in an octet is an octal ambiguity");
        }
        match octet.parse::<u8>() {
            Ok(value) => *slot = value,
            Err(_) => return refuse("every octet must be 0-255"),
        }
    }

    // Routable unicast is deliberately still allowed — a bench on a real subnet
    // is a real case and nothing here knows the topology, so refusing it would
    // be the guessing `LE-97` is about. What is refused is the category the
    // message below names: addresses no client could fetch FROM.
    match parsed[0] {
        _ if parsed == [0, 0, 0, 0] => refuse(
            "0.0.0.0 is a bind wildcard, not somewhere a client can fetch from; a board handed \
             siaddr=0.0.0.0 fails to fetch and looks like a board fault",
        ),
        0 => refuse("0.0.0.0/8 is \"this network\" and is not a source any client can reach"),
        127 => refuse("127.0.0.0/8 is loopback — the board would be told to fetch from itself"),
        224..=239 => refuse("224.0.0.0/4 is multicast, and a server address is one host"),
        240..=255 => refuse(
            "240.0.0.0/4 is reserved and 255.255.255.255 is the limited broadcast; neither is a \
             host a client can fetch from",
        ),
        _ => Ok(text.to_string()),
    }
}

/// Renders a plan for the operator before anything runs.
///
/// Printed rather than merely executed because a bench operator standing over a
/// board is entitled to know what is about to happen to its power before it
/// happens, and because a dry run is the only way to review this on a laptop
/// that has no relay attached.
#[must_use]
pub fn describe(steps: &[Step]) -> String {
    let mut text = String::new();
    for (index, step) in steps.iter().enumerate() {
        let line = match step {
            Step::VerifyDigest { image, expected } => {
                format!("verify {} is sha256 {expected}", image.display())
            }
            Step::StartNetboot { mac, root, server } => {
                format!("serve {} to {mac} over DHCP+TFTP from {server}", root.display())
            }
            Step::PowerCycle { off_ms, on_wait } => {
                format!("POWER: off for {off_ms} ms, on, settle {on_wait} s")
            }
            Step::Watch { until, timeout, text } => match text {
                Some(path) => {
                    format!("watch for {until} (≤{timeout} s), harvest to {}", path.display())
                }
                None => format!("watch for {until} (≤{timeout} s)"),
            },
            Step::ParseMeas { capture } => {
                format!("parse {} as a TOS64-MEAS envelope", capture.display())
            }
            Step::EnsurePowerOn => "POWER: leave the board ON, whatever happened above".to_string(),
        };
        text.push_str(&format!("  {}. {line}\n", index + 1));
    }
    text
}

/// How a run ended, in the same three-way shape the rest of `xtask` uses:
/// the thing under test failed, or the harness could not even run it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunOutcome {
    /// Every step completed and the board is confirmed on.
    Completed,
    /// A step failed. The board is still left on — the restore runs regardless.
    StepFailed(String),
    /// The loudest one: `tos64-power` exited 4. The board may be dark and the
    /// next session needs a hand on the plug.
    BoardMayBeOff,
}

/// `tos64-power`'s exit code for "the board may be off and I could not fix it".
///
/// Duplicated here from the C# tool by value, because the two are separate
/// programs and nothing links them. It is the one exit code whose meaning this
/// module acts on rather than reports, so it is named rather than compared to a
/// literal at the call site.
pub const POWER_EXIT_LEFT_OFF: i32 = 4;

/// Runs a plan.
///
/// Deliberately thin, and deliberately the only untested part of this module:
/// every decision worth reviewing is in [`plan`], and what remains here is
/// spawning processes in an order that has already been checked.
pub fn execute(repo_root: &Path, steps: &[Step], plug: &PlugConfig, dry_run: bool) -> RunOutcome {
    let mut netboot = None;
    let mut failure: Option<String> = None;

    for step in steps {
        // `EnsurePowerOn` is never skipped, and that is the point of clause 1:
        // a failure stops the loop from doing more work but does not stop the
        // board getting its power back.
        if failure.is_some() && !matches!(step, Step::EnsurePowerOn) {
            continue;
        }

        if dry_run {
            if let Step::EnsurePowerOn = step {
                println!("board-run: dry run — nothing was switched, nothing was served");
            }
            continue;
        }

        match step {
            Step::VerifyDigest { image, expected } => match std::fs::read(image) {
                Ok(bytes) => {
                    let actual = crate::pi5::sha256_hex(&bytes);
                    if &actual == expected {
                        println!("  staged: {} bytes, sha256 {actual} — matches", bytes.len());
                    } else {
                        failure = Some(format!(
                            "the staged image is sha256 {actual}, not {expected}. \
                                 Nothing has been served and nothing has been switched."
                        ));
                    }
                }
                Err(error) => {
                    failure = Some(format!("cannot read {}: {error}", image.display()));
                }
            },
            // `--server` is passed through, and `LE-97` is why: this step runs
            // BEFORE `PowerCycle`, so the board is off, the bench NIC has no
            // link, and `tos64-netboot`'s own discovery is guaranteed to find
            // nothing on exactly this path. It used to answer `IPAddress.Any`
            // and serve `siaddr=0.0.0.0`; it now refuses to start. Either way
            // the automated path is the one where no human is reading the line,
            // so the address is named here rather than guessed there.
            Step::StartNetboot { mac, root, server } => {
                match tool(repo_root, "netboot")
                    .args(["--mac", mac, "--server", server, "--root"])
                    .arg(root)
                    .spawn()
                {
                    Ok(child) => {
                        println!(
                            "  tos64-netboot serving {} for {mac} from {server}",
                            root.display()
                        );
                        netboot = Some(child);
                    }
                    Err(error) => failure = Some(format!("could not start tos64-netboot: {error}")),
                }
            }
            Step::PowerCycle { off_ms, on_wait } => {
                let status = tool(repo_root, "power")
                    .args(plug.args())
                    .arg("--root")
                    .arg(root_of(steps))
                    .args([
                        "--off-ms",
                        &off_ms.to_string(),
                        "--on-wait",
                        &on_wait.to_string(),
                        "cycle",
                    ])
                    .status();
                match status {
                    Ok(code) if code.success() => {}
                    Ok(code) if code.code() == Some(POWER_EXIT_LEFT_OFF) => {
                        kill(netboot.take());
                        return RunOutcome::BoardMayBeOff;
                    }
                    Ok(code) => failure = Some(format!("tos64-power cycle exited {code}")),
                    Err(error) => failure = Some(format!("could not run tos64-power: {error}")),
                }
            }
            Step::Watch { until, timeout, text } => {
                let mut command = tool(repo_root, "ti64dink");
                command.args(["--until", until, "--timeout", &timeout.to_string()]);
                if let Some(path) = text {
                    command.arg("--text").arg(path);
                }
                match command.status() {
                    Ok(code) if code.success() => {}
                    Ok(_) => failure = Some(format!("the board did not reach {until} in time")),
                    Err(error) => failure = Some(format!("could not run ti64dink: {error}")),
                }
            }
            Step::ParseMeas { capture } => {
                let status =
                    Command::new(std::env::current_exe().unwrap_or_else(|_| "xtask".into()))
                        .arg("parse-meas")
                        .arg(capture)
                        .status();
                match status {
                    Ok(code) if code.success() => {}
                    Ok(code) => failure = Some(format!("parse-meas exited {code}")),
                    Err(error) => failure = Some(format!("could not run parse-meas: {error}")),
                }
            }
            Step::EnsurePowerOn => {
                kill(netboot.take());
                let status = tool(repo_root, "power").args(plug.args()).arg("on").status();
                match status {
                    Ok(code) if code.code() == Some(POWER_EXIT_LEFT_OFF) => {
                        return RunOutcome::BoardMayBeOff;
                    }
                    Ok(code) if !code.success() => {
                        eprintln!(
                            "board-run: the final power-on did not confirm ({code}). \
                             CHECK THE BOARD BEFORE LEAVING THE BENCH."
                        );
                    }
                    Err(error) => eprintln!("board-run: could not run tos64-power on: {error}"),
                    _ => {}
                }
            }
        }
    }

    match failure {
        Some(message) => RunOutcome::StepFailed(message),
        None => RunOutcome::Completed,
    }
}

/// `dotnet run --project work/tools/<name>` — the tools are C# console apps
/// under `work/tools/` per the standing rule that bench tooling is C# and never
/// a script. Invoked through `dotnet run` rather than a built path so a bench
/// that has not built them today still works, at the cost of a few seconds.
fn tool(repo_root: &Path, name: &str) -> Command {
    let mut command = Command::new("dotnet");
    command
        .arg("run")
        .arg("--project")
        .arg(repo_root.join("work").join("tools").join(name))
        .arg("--");
    command
}

/// The served root, recovered from the plan so the power tool looks for the
/// transfer marker in the same directory the server writes it to.
fn root_of(steps: &[Step]) -> PathBuf {
    steps
        .iter()
        .find_map(|step| match step {
            Step::StartNetboot { root, .. } => Some(root.clone()),
            _ => None,
        })
        .unwrap_or_else(|| PathBuf::from("."))
}

fn kill(child: Option<std::process::Child>) {
    if let Some(mut child) = child {
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_run() -> BoardRun {
        BoardRun {
            image: PathBuf::from("target/pi5/kernel8.img"),
            expected_digest: Some("b6dbabae".into()),
            mac: "88:a2:9e:11:4e:cc".into(),
            root: PathBuf::from("target/pi5"),
            server: "169.254.113.248".into(),
            off_ms: 5000,
            on_wait: 20,
            until: "rung=DispatchRound".into(),
            timeout: 120,
            text: Some(PathBuf::from("env.txt")),
        }
    }

    /// `LE-97`, and the reason this is a REQUIREMENT here rather than a
    /// convenience: `plan` puts [`Step::StartNetboot`] before
    /// [`Step::PowerCycle`], which the test below already pins — so at the
    /// moment the server starts, **the board is off and the bench NIC has no
    /// link**, because link needs a powered far end. Discovery is therefore
    /// guaranteed to find nothing on exactly this path. The address cannot be
    /// discovered by construction, so it must be named.
    #[test]
    fn the_netboot_step_carries_the_server_address() {
        let steps = plan(&a_run());
        let served = steps
            .iter()
            .find_map(|step| match step {
                Step::StartNetboot { server, .. } => Some(server.clone()),
                _ => None,
            })
            .expect("a plan serves something");
        assert_eq!(served, "169.254.113.248");
    }

    /// A plan an operator reads before power moves must show the address, or
    /// the review this description exists for cannot catch a wrong one.
    #[test]
    fn the_description_names_the_server_address() {
        let text = describe(&plan(&a_run()));
        assert!(text.contains("169.254.113.248"), "the plan hid the server address: {text}");
    }

    /// The four-octet rule, and it is not pedantry. `.NET`'s `IPAddress.Parse`
    /// — which `tos64-netboot` uses on the other side of this seam — ACCEPTS
    /// the historical shorthand, so `169.254.113` parses successfully as
    /// 169.254.0.113. A truncated address does not fail; it becomes a
    /// different, valid, wrong one, and the tool then prints it in the same
    /// confident column it prints a right one. `netboot.tests` holds the C#
    /// half of this rule; this is the Rust half, and they are separate
    /// programs, so both are checked rather than one trusted.
    #[test]
    fn a_four_octet_address_is_accepted() {
        assert_eq!(server_address("169.254.113.248"), Ok("169.254.113.248".to_string()));
        assert_eq!(server_address("10.0.0.1"), Ok("10.0.0.1".to_string()));
    }

    /// The exact value that nearly poisoned the 2026-08-06 boot. Refused on the
    /// way IN as well as on the way out — a fix that covers only the path that
    /// produced it once is not a fix.
    #[test]
    fn the_unspecified_address_is_refused() {
        assert!(server_address("0.0.0.0").is_err());
    }

    /// Everything a wrong `--server=` can look like, refused before power moves
    /// rather than after a wasted cycle.
    #[test]
    fn a_malformed_address_is_refused() {
        for bad in ["169.254.113", "169.254", "169", "", "not-an-address", "169.254.113.248.9"] {
            assert!(server_address(bad).is_err(), "`{bad}` was accepted and must not be");
        }
    }

    /// An octet is a byte. `169.254.113.999` parses as three fine numbers and
    /// one that is not an address at all.
    #[test]
    fn an_out_of_range_octet_is_refused() {
        assert!(server_address("169.254.113.999").is_err());
        assert!(server_address("169.254.113.-1").is_err());
        assert!(server_address("169.254.113. 248").is_err());
    }

    /// The explicit path used to accept anything four octets long that was not
    /// `0.0.0.0`, while the C# discovery path filtered hard to 169.254/16.
    /// Routable unicast stays accepted — a bench on a real subnet is a real
    /// case — but loopback would tell the board to fetch from **itself**, and
    /// multicast and broadcast are the same category the refusal text already
    /// names: not somewhere a client can fetch from.
    #[test]
    fn an_address_no_client_could_fetch_from_is_refused() {
        for bad in [
            "127.0.0.1",
            "127.255.255.254",
            "224.0.0.1",
            "239.255.255.250",
            "240.0.0.1",
            "255.255.255.255",
            "0.1.2.3",
        ] {
            assert!(server_address(bad).is_err(), "`{bad}` was accepted and must not be");
        }
    }

    /// Routable unicast is deliberately still allowed: nothing here knows the
    /// bench's topology, and guessing it is the mistake `LE-97` is about.
    #[test]
    fn a_routable_unicast_address_is_still_accepted() {
        for good in ["10.0.0.1", "192.168.1.20", "8.8.8.8"] {
            assert!(server_address(good).is_ok(), "`{good}` must remain nameable");
        }
    }

    /// A leading zero is an octal ambiguity, and `.NET`'s own parser rejects it
    /// — so accepting it here would put the two sides of the seam out of step
    /// in the one direction that matters: this side saying yes to a value the
    /// server will then refuse, after power has moved.
    #[test]
    fn a_leading_zero_octet_is_refused() {
        assert!(server_address("169.254.0.01").is_err());
        assert!(server_address("169.254.0.0").is_ok(), "a bare zero octet is fine");
    }

    /// The mirror `08C` argued for and did not build. `ServerAddress.Choose`
    /// implements these same rules in C#, in a separate program that nothing
    /// links to this one, and `netboot.tests` reads the same file. The
    /// duplication is right across this seam; leaving it unasserted was not,
    /// which is what `TransferBeacon`/`TransferGuard` got a mirror test to
    /// avoid.
    #[test]
    fn every_shared_case_gets_the_verdict_the_mirror_file_states() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("..")
            .join("work")
            .join("tools")
            .join("netboot")
            .join("server-address-cases.tsv");
        let table = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("the shared case table is missing: {path:?}: {error}"));

        let mut checked = 0;
        for line in table.lines().skip(1) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            assert_eq!(fields.len(), 3, "not three fields: `{line}`");
            let (value, verdict, why) = (fields[0], fields[1], fields[2]);
            let accepted = server_address(value).is_ok();
            assert_eq!(
                accepted,
                verdict == "accept",
                "`{value}` should {verdict} ({why}) but server_address said {accepted}"
            );
            checked += 1;
        }
        // "Nothing was wrong" and "nothing was looked at" are different
        // results, and a mirror test that silently reads zero rows reports the
        // first while meaning the second.
        assert!(checked >= 20, "only {checked} shared cases were read");
    }

    /// A refusal with no next step gets worked around rather than fixed, and
    /// there is exactly one action that resolves every one of these.
    #[test]
    fn every_refusal_names_the_flag_that_resolves_it() {
        for bad in ["0.0.0.0", "169.254.113", "", "nonsense"] {
            let message = server_address(bad).expect_err("must refuse");
            assert!(message.contains("--server="), "`{bad}` refused without saying how: {message}");
        }
    }

    /// Clause 1 of `LE-95`, as the invariant it is. Asserted over every shape
    /// of run this planner can produce, not over one example — the plan is
    /// small enough that "every shape" is four booleans.
    #[test]
    fn every_plan_ends_with_the_board_on() {
        for digest in [None, Some("b6dbabae".to_string())] {
            for text in [None, Some(PathBuf::from("env.txt"))] {
                let run = BoardRun { expected_digest: digest.clone(), text, ..a_run() };
                let steps = plan(&run);
                assert_eq!(
                    steps.last(),
                    Some(&Step::EnsurePowerOn),
                    "a plan that does not end with the board on can end a session dark"
                );
                assert_eq!(
                    steps.iter().filter(|step| **step == Step::EnsurePowerOn).count(),
                    1,
                    "exactly one restore, and it is the last thing"
                );
            }
        }
    }

    /// The other half of clause 1: power never moves without a restore after
    /// it. Stated as a position comparison rather than as "the last step is
    /// EnsurePowerOn" so that a future step appended after the restore fails
    /// this test rather than passing the one above.
    #[test]
    fn a_cycle_is_always_followed_by_a_restore() {
        let steps = plan(&a_run());
        let cycle = steps.iter().position(|s| matches!(s, Step::PowerCycle { .. })).unwrap();
        let restore = steps.iter().position(|s| *s == Step::EnsurePowerOn).unwrap();
        assert!(cycle < restore, "the cycle must precede the restore, not follow it");
    }

    /// `LE-87`: a digest read while a server is serving is a digest of whatever
    /// the file is halfway through becoming, and the stale image that cost
    /// three power cycles was served by a process nobody remembered starting.
    #[test]
    fn the_digest_is_checked_before_anything_is_served() {
        let steps = plan(&a_run());
        let verify = steps.iter().position(|s| matches!(s, Step::VerifyDigest { .. })).unwrap();
        let serve = steps.iter().position(|s| matches!(s, Step::StartNetboot { .. })).unwrap();
        assert!(verify < serve);
    }

    /// A Pi 5 that DHCPs into silence retries, and the retry window is not the
    /// window the capture was sized against — so the server is up before power
    /// reaches the board, always.
    #[test]
    fn the_server_is_up_before_power_moves() {
        let steps = plan(&a_run());
        let serve = steps.iter().position(|s| matches!(s, Step::StartNetboot { .. })).unwrap();
        let cycle = steps.iter().position(|s| matches!(s, Step::PowerCycle { .. })).unwrap();
        assert!(serve < cycle);
    }

    #[test]
    fn the_watch_is_armed_after_the_board_is_powered() {
        let steps = plan(&a_run());
        let cycle = steps.iter().position(|s| matches!(s, Step::PowerCycle { .. })).unwrap();
        let watch = steps.iter().position(|s| matches!(s, Step::Watch { .. })).unwrap();
        assert!(cycle < watch);
    }

    /// Nothing is parsed that was not captured. A `parse-meas` over a stale
    /// file from a previous run is the same class of failure as a stale image:
    /// a complete, plausible, entirely wrong result.
    #[test]
    fn parsing_happens_only_when_something_was_harvested() {
        let with = plan(&a_run());
        assert!(with.iter().any(|s| matches!(s, Step::ParseMeas { .. })));

        let without = plan(&BoardRun { text: None, ..a_run() });
        assert!(
            !without.iter().any(|s| matches!(s, Step::ParseMeas { .. })),
            "a run that harvested nothing has nothing to parse"
        );

        let parse = with.iter().position(|s| matches!(s, Step::ParseMeas { .. })).unwrap();
        let watch = with.iter().position(|s| matches!(s, Step::Watch { .. })).unwrap();
        assert!(watch < parse);
    }

    /// A run with no stated digest still serves, cycles and restores. Stated
    /// because the digest is the one optional safety step, and an optional step
    /// that silently removes the mandatory ones would be worse than no plan.
    #[test]
    fn a_run_without_a_digest_keeps_every_other_stage() {
        let steps = plan(&BoardRun { expected_digest: None, ..a_run() });
        assert!(!steps.iter().any(|s| matches!(s, Step::VerifyDigest { .. })));
        assert!(steps.iter().any(|s| matches!(s, Step::StartNetboot { .. })));
        assert!(steps.iter().any(|s| matches!(s, Step::PowerCycle { .. })));
        assert_eq!(steps.last(), Some(&Step::EnsurePowerOn));
    }

    /// The description is what an operator reads before the board's power
    /// moves, so it names every step rather than summarising the run.
    #[test]
    fn the_description_names_every_step() {
        let steps = plan(&a_run());
        let text = describe(&steps);
        assert_eq!(text.lines().count(), steps.len());
        assert!(text.contains("POWER: off for 5000 ms"));
        assert!(text.contains("leave the board ON"));
        assert!(text.contains("b6dbabae"));
    }

    /// The marker the power tool reads lives beside the image the server
    /// serves, and this is the join between the two: if the plan ever grew a
    /// second root, the guard would look in the wrong directory and read
    /// "no transfer" while one was in flight.
    #[test]
    fn the_power_guard_looks_where_the_server_writes() {
        let steps = plan(&a_run());
        assert_eq!(root_of(&steps), PathBuf::from("target/pi5"));
    }
}
