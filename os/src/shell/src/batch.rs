//! The `.TCB` batch runner (`FEAT-P2-07`, `STORY-P2-07-01`).
//!
//! Sequential execution through the DOS front-end with 4.0's echo discipline: `ECHO ON`
//! is the default, `@` suppresses one line, `REM` lines are skipped after echo, and the
//! echoed form is prompt + raw line — read from `TUCODE.ASM`, not folklore. Control flow
//! (`IF`/`GOTO`/`FOR`/`CALL`/`SHIFT`) is stated debt in the Story. A batch spends only
//! its session's authority: every line passes the same [`crate::verbs::execute`] policy
//! verdict as an interactive line, and a denied line refuses, audits and continues.

use core::fmt::{self, Write};

use crate::capacities::{MAX_BATCH_LINES, MAX_PATH};
use crate::dos::run_line;
use crate::verbs::World;

/// Outcome counters — the fixture's in-guest assertions read these.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BatchStats {
    /// Lines executed (excluding blanks/`REM`).
    pub executed: u32,
    /// Authority denials observed during the run.
    pub denials: u32,
    /// Lines refused for exceeding the batch line budget.
    pub truncated: bool,
}

/// Render the session prompt (`A:<cwd>>`) — the same prompt the echo discipline
/// prints before each batch line. Public so an interactive front-end (the 17G tab
/// host) echoes lines exactly as a `.TCB` run would render them.
pub fn prompt(world: &World<'_>, sink: &mut dyn Write) -> fmt::Result {
    let mut buffer = [0u8; MAX_PATH];
    let len = world.volume.dir_path(world.cwd, &mut buffer);
    write!(sink, "A:{}>", core::str::from_utf8(&buffer[..len]).unwrap_or("\\"))
}

/// Run `script` line by line. Bounded: at most [`MAX_BATCH_LINES`] lines execute; the
/// remainder is refused loudly (`truncated`), never silently dropped.
pub fn run(
    world: &mut World<'_>,
    script: &str,
    sink: &mut dyn Write,
) -> Result<BatchStats, fmt::Error> {
    let mut stats = BatchStats::default();
    let denials_before = world.denials;
    for (index, raw_line) in script.lines().enumerate() {
        if index >= MAX_BATCH_LINES {
            stats.truncated = true;
            writeln!(sink, "Batch line budget exceeded; remaining lines refused")?;
            break;
        }
        let line = raw_line.trim_end_matches('\r');
        let (suppressed, line) = match line.strip_prefix('@') {
            Some(rest) => (true, rest),
            None => (false, line),
        };
        if line.trim().is_empty() {
            continue;
        }
        if world.echo && !suppressed {
            writeln!(sink)?;
            prompt(world, sink)?;
            writeln!(sink, "{line}")?;
        }
        if line.trim_start().len() >= 4
            && line.trim_start()[..4.min(line.trim_start().len())].eq_ignore_ascii_case("REM ")
        {
            continue;
        }
        if line.trim().eq_ignore_ascii_case("REM") {
            continue;
        }
        run_line(world, line, sink)?;
        stats.executed += 1;
    }
    stats.denials = world.denials - denials_before;
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::GrantSet;
    use crate::verbs::{Env, VerbKind, World};
    use crate::volume::RamVolume;

    const GRANTS: &[VerbKind] = &[VerbKind::Echo, VerbKind::VersionInfo, VerbKind::PrintCwd];

    fn world(policy: &'static GrantSet) -> World<'static> {
        World {
            volume: RamVolume::new(Some("TINYOS"), (0x1234, 0xABCD)),
            env: Env::new(),
            cwd: 0,
            echo: true,
            policy,
            session: "BATCH",
            tasks: &[],
            spoors: &crate::verbs::NoSpoors,
            denials: 0,
        }
    }

    /// B1 — echo discipline: default ON echoes prompt+line, `@` suppresses one line,
    /// `ECHO OFF` suppresses the rest, `REM` executes nothing.
    #[test]
    fn b1_echo_discipline() {
        static POLICY: GrantSet = GrantSet { granted: GRANTS, withheld: None, supervisor: false };
        let mut w = world(&POLICY);
        let mut out = String::new();
        let stats = run(&mut w, "REM header\nVER\n@ECHO OFF\nVER\nECHO done", &mut out).unwrap();
        assert_eq!(stats.executed, 4, "REM executes nothing; VER, @ECHO OFF, VER, ECHO done do");
        assert!(out.contains("A:\\>VER"), "echo-on lines carry the prompt: {out}");
        assert!(!out.contains("A:\\>ECHO done"), "after ECHO OFF nothing echoes: {out}");
        assert!(out.contains("done\n"));
    }

    /// B2 — a denied verb inside a batch refuses that line, audits, and the batch
    /// continues (STORY-P2-07-01 acceptance 3).
    #[test]
    fn b2_denied_line_continues() {
        static POLICY: GrantSet =
            GrantSet { granted: GRANTS, withheld: Some(VerbKind::VersionInfo), supervisor: false };
        let mut w = world(&POLICY);
        let mut out = String::new();
        let stats = run(&mut w, "@ECHO OFF\nVER\nECHO still here", &mut out).unwrap();
        assert_eq!(stats.denials, 1);
        assert!(out.contains("Access denied"));
        assert!(out.contains("still here"), "batch continues after a denial: {out}");
    }

    /// B3 — the line budget refuses loudly, never silently.
    #[test]
    fn b3_line_budget_is_loud() {
        static POLICY: GrantSet = GrantSet { granted: GRANTS, withheld: None, supervisor: false };
        let mut w = world(&POLICY);
        let script = "ECHO x\n".repeat(MAX_BATCH_LINES + 5);
        let mut out = String::new();
        let stats = run(&mut w, &script, &mut out).unwrap();
        assert!(stats.truncated);
        assert!(out.contains("budget exceeded"));
        assert_eq!(stats.executed, MAX_BATCH_LINES as u32);
    }

    /// B4 — the public prompt renderer answers the same `A:<cwd>>` shape the echo
    /// discipline prints, so an interactive tab host echoes identically to a batch.
    #[test]
    fn b4_prompt_is_public_and_batch_shaped() {
        static POLICY: GrantSet = GrantSet { granted: GRANTS, withheld: None, supervisor: false };
        let w = world(&POLICY);
        let mut out = String::new();
        prompt(&w, &mut out).unwrap();
        assert_eq!(out, "A:\\>");
    }
}
