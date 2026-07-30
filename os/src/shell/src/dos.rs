//! The DOS flavour front-end (`FEAT-P2-04`, `STORY-P2-04-01`).
//!
//! One thin parser over the canonical core: command words matched
//! case-insensitively against the register's DOS bindings, `/switches` per the
//! 4.0-verified switch tables plus the recorded deliberate extensions (`/Y`,
//! `MOVE`), `%VAR%` expansion in a single bounded pass. Total over arbitrary
//! bytes: every input parses to either a request or a register-shape refusal —
//! never a panic, never a bypass (the parser holds no authority; `verbs::execute`
//! authorises whatever this produces).

use core::fmt::{self, Write};

use crate::capacities::MAX_LINE;
use crate::verbs::{execute, Request, World};

/// A parse-level refusal, rendered in the 4.0 PARSE-block shapes.
enum ParseError {
    /// `Invalid switch`.
    BadSwitch,
    /// `Required parameter missing`.
    Missing,
    /// `Too many parameters`.
    TooMany,
    /// Over-length line.
    LineTooLong,
}

fn parse_error(sink: &mut dyn Write, error: ParseError) -> fmt::Result {
    let text = match error {
        ParseError::BadSwitch => "Invalid switch",
        ParseError::Missing => "Required parameter missing",
        ParseError::TooMany => "Too many parameters",
        ParseError::LineTooLong => "Line too long",
    };
    writeln!(sink, "{text}")
}

/// Expand `%VAR%` against the session environment in one bounded pass
/// (no recursive expansion). Undefined variables expand to nothing — 4.0's
/// behaviour. Returns the expanded length, or `None` if it would overflow.
fn expand<'a>(world: &World<'_>, line: &str, buffer: &'a mut [u8; MAX_LINE]) -> Option<&'a str> {
    let mut out = 0usize;
    let mut rest = line;
    loop {
        match rest.find('%') {
            None => {
                let bytes = rest.as_bytes();
                if out + bytes.len() > MAX_LINE {
                    return None;
                }
                buffer[out..out + bytes.len()].copy_from_slice(bytes);
                out += bytes.len();
                return core::str::from_utf8(&buffer[..out]).ok();
            }
            Some(start) => {
                let bytes = &rest.as_bytes()[..start];
                if out + bytes.len() > MAX_LINE {
                    return None;
                }
                buffer[out..out + bytes.len()].copy_from_slice(bytes);
                out += bytes.len();
                let after = &rest[start + 1..];
                match after.find('%') {
                    None => {
                        // A lone `%` stays literal.
                        if out + 1 > MAX_LINE {
                            return None;
                        }
                        buffer[out] = b'%';
                        out += 1;
                        rest = after;
                    }
                    Some(end) => {
                        let name = &after[..end];
                        if let Some(value) = world.env.get(name) {
                            let bytes = value.as_bytes();
                            if out + bytes.len() > MAX_LINE {
                                return None;
                            }
                            buffer[out..out + bytes.len()].copy_from_slice(bytes);
                            out += bytes.len();
                        }
                        rest = &after[end + 1..];
                    }
                }
            }
        }
    }
}

struct Tokens<'a> {
    words: [&'a str; 8],
    count: usize,
    switches: [&'a str; 4],
    switch_count: usize,
    overflow: bool,
}

fn tokenize(line: &str) -> Tokens<'_> {
    let mut tokens =
        Tokens { words: [""; 8], count: 0, switches: [""; 4], switch_count: 0, overflow: false };
    for token in line.split_whitespace() {
        if let Some(switch) = token.strip_prefix('/') {
            if tokens.switch_count < 4 {
                tokens.switches[tokens.switch_count] = switch;
                tokens.switch_count += 1;
            } else {
                tokens.overflow = true;
            }
        } else if tokens.count < 8 {
            tokens.words[tokens.count] = token;
            tokens.count += 1;
        } else {
            tokens.overflow = true;
        }
    }
    tokens
}

fn switches_allowed(tokens: &Tokens<'_>, allowed: &[&str]) -> bool {
    tokens.switches[..tokens.switch_count]
        .iter()
        .all(|s| allowed.iter().any(|a| a.eq_ignore_ascii_case(s)))
}

fn has_switch(tokens: &Tokens<'_>, switch: &str) -> bool {
    tokens.switches[..tokens.switch_count].iter().any(|s| s.eq_ignore_ascii_case(switch))
}

/// Parse and execute one DOS-syntax line against the world.
pub fn run_line(world: &mut World<'_>, raw: &str, sink: &mut dyn Write) -> fmt::Result {
    if raw.len() > MAX_LINE {
        return parse_error(sink, ParseError::LineTooLong);
    }
    let mut buffer = [0u8; MAX_LINE];
    let Some(line) = expand(world, raw, &mut buffer) else {
        return parse_error(sink, ParseError::LineTooLong);
    };
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }

    // `ECHO` and `SET` take their argument raw (case and spacing preserved), so they
    // are handled before whitespace tokenisation — exactly 4.0's special-casing.
    let (word, tail) = match line.find(char::is_whitespace) {
        Some(split) => (&line[..split], line[split..].trim_start()),
        None => (line, ""),
    };

    if word.eq_ignore_ascii_case("ECHO") {
        let request = if tail.is_empty() {
            Request::Echo { mode: None, text: None }
        } else if tail.eq_ignore_ascii_case("ON") {
            Request::Echo { mode: Some(true), text: None }
        } else if tail.eq_ignore_ascii_case("OFF") {
            Request::Echo { mode: Some(false), text: None }
        } else {
            Request::Echo { mode: None, text: Some(tail) }
        };
        return execute(world, &request, sink);
    }
    if word.eq_ignore_ascii_case("REM") {
        return Ok(());
    }
    if word.eq_ignore_ascii_case("SET") {
        let request = if tail.is_empty() {
            Request::EnvSet { key: None, value: None }
        } else {
            match tail.find('=') {
                None => Request::EnvSet { key: Some(tail), value: None },
                Some(split) => Request::EnvSet {
                    key: Some(tail[..split].trim()),
                    value: Some(tail[split + 1..].trim()),
                },
            }
        };
        return execute(world, &request, sink);
    }
    if word.eq_ignore_ascii_case("PATH") {
        let request = if tail.is_empty() {
            Request::EnvSet { key: Some("PATH"), value: None }
        } else if tail == ";" {
            Request::EnvSet { key: Some("PATH"), value: Some("") }
        } else {
            Request::EnvSet { key: Some("PATH"), value: Some(tail) }
        };
        return execute(world, &request, sink);
    }
    if word.eq_ignore_ascii_case("FIND") {
        // FIND [/V][/C][/N] "string" file — the string must be double-quoted (4.0 rule).
        let tokens = tokenize(tail);
        if !switches_allowed(&tokens, &["V", "C", "N"]) {
            write!(sink, "FIND: ")?;
            return parse_error(sink, ParseError::BadSwitch);
        }
        let Some(open) = tail.find('"') else {
            write!(sink, "FIND: ")?;
            return parse_error(sink, ParseError::Missing);
        };
        let Some(close) = tail[open + 1..].find('"') else {
            write!(sink, "FIND: ")?;
            return parse_error(sink, ParseError::Missing);
        };
        let pattern = &tail[open + 1..open + 1 + close];
        let path = tail[open + close + 2..].trim();
        if path.is_empty() {
            write!(sink, "FIND: ")?;
            return parse_error(sink, ParseError::Missing);
        }
        return execute(
            world,
            &Request::FindText {
                pattern,
                path,
                invert: has_switch(&tokens, "V"),
                count: has_switch(&tokens, "C"),
                number: has_switch(&tokens, "N"),
            },
            sink,
        );
    }

    let tokens = tokenize(tail);
    if tokens.overflow {
        return parse_error(sink, ParseError::TooMany);
    }
    let args = &tokens.words[..tokens.count];

    macro_rules! need {
        ($n:expr) => {{
            if args.len() < $n {
                return parse_error(sink, ParseError::Missing);
            }
            if args.len() > $n {
                return parse_error(sink, ParseError::TooMany);
            }
        }};
    }

    let request = if word.eq_ignore_ascii_case("DIR") {
        if !switches_allowed(&tokens, &["P", "W"]) {
            return parse_error(sink, ParseError::BadSwitch);
        }
        if args.len() > 1 {
            return parse_error(sink, ParseError::TooMany);
        }
        Request::List { path: args.first().copied(), wide: has_switch(&tokens, "W") }
    } else if word.eq_ignore_ascii_case("CD") || word.eq_ignore_ascii_case("CHDIR") {
        if args.is_empty() {
            Request::PrintCwd
        } else {
            need!(1);
            Request::ChangeDir(args[0])
        }
    } else if word.eq_ignore_ascii_case("COPY") {
        if !switches_allowed(&tokens, &["V"]) {
            return parse_error(sink, ParseError::BadSwitch);
        }
        need!(2);
        Request::Copy(args[0], args[1])
    } else if word.eq_ignore_ascii_case("MOVE")
        || word.eq_ignore_ascii_case("REN")
        || word.eq_ignore_ascii_case("RENAME")
    {
        need!(2);
        Request::Move(args[0], args[1])
    } else if word.eq_ignore_ascii_case("DEL") || word.eq_ignore_ascii_case("ERASE") {
        if !switches_allowed(&tokens, &["P", "Y"]) {
            return parse_error(sink, ParseError::BadSwitch);
        }
        need!(1);
        Request::Delete { path: args[0], assume_yes: has_switch(&tokens, "Y") }
    } else if word.eq_ignore_ascii_case("MD") || word.eq_ignore_ascii_case("MKDIR") {
        need!(1);
        Request::MakeDir(args[0])
    } else if word.eq_ignore_ascii_case("RD") || word.eq_ignore_ascii_case("RMDIR") {
        need!(1);
        Request::RemoveDir(args[0])
    } else if word.eq_ignore_ascii_case("TYPE") {
        need!(1);
        Request::ViewFile(args[0])
    } else if word.eq_ignore_ascii_case("MORE") {
        need!(1);
        Request::Page(args[0])
    } else if word.eq_ignore_ascii_case("SORT") {
        if !switches_allowed(&tokens, &["R"]) {
            write!(sink, "SORT: ")?;
            return parse_error(sink, ParseError::BadSwitch);
        }
        need!(1);
        Request::SortStream { path: args[0], reverse: has_switch(&tokens, "R") }
    } else if word.eq_ignore_ascii_case("TREE") {
        if !switches_allowed(&tokens, &["A", "F"]) {
            return parse_error(sink, ParseError::BadSwitch);
        }
        Request::TreeView { ascii: has_switch(&tokens, "A"), files: has_switch(&tokens, "F") }
    } else if word.eq_ignore_ascii_case("ATTRIB") {
        Request::AttribView(args.first().copied())
    } else if word.eq_ignore_ascii_case("CLS") {
        Request::ClearScreen
    } else if word.eq_ignore_ascii_case("VER") {
        Request::VersionInfo
    } else if word.eq_ignore_ascii_case("VOL") {
        Request::VolumeInfo
    } else if word.eq_ignore_ascii_case("MEM") {
        Request::MemInfo
    } else if word.eq_ignore_ascii_case("TASKMGR") || word.eq_ignore_ascii_case("TASKLIST") {
        Request::TaskList
    } else if word.eq_ignore_ascii_case("TASKKILL") {
        need!(1);
        Request::TaskKill(args[0])
    } else if word.eq_ignore_ascii_case("SPOOR") {
        if !args.is_empty() {
            return parse_error(sink, ParseError::TooMany);
        }
        Request::SpoorJournal
    } else {
        Request::Unknown
    };
    execute(world, &request, sink)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::labels::Labels;
    use crate::policy::GrantSet;
    use crate::verbs::{Env, TaskInfo, VerbKind, World};
    use crate::volume::RamVolume;

    const ALL: &[VerbKind] = &[
        VerbKind::List,
        VerbKind::ChangeDir,
        VerbKind::PrintCwd,
        VerbKind::Copy,
        VerbKind::Move,
        VerbKind::Delete,
        VerbKind::MakeDir,
        VerbKind::RemoveDir,
        VerbKind::ViewFile,
        VerbKind::FindText,
        VerbKind::SortStream,
        VerbKind::Page,
        VerbKind::TreeView,
        VerbKind::AttribView,
        VerbKind::Env,
        VerbKind::Echo,
        VerbKind::ClearScreen,
        VerbKind::VersionInfo,
        VerbKind::VolumeInfo,
        VerbKind::MemInfo,
        VerbKind::TaskList,
        VerbKind::TaskKill,
        VerbKind::SpoorJournal,
    ];
    const POLICY: GrantSet = GrantSet { granted: ALL, withheld: None, supervisor: false };

    fn world() -> World<'static> {
        let mut w = World {
            volume: RamVolume::new(Some("TINYOS"), (0x1234, 0xABCD)),
            env: Env::new(),
            cwd: 0,
            echo: true,
            policy: &POLICY,
            session: "TEST",
            tasks: &[TaskInfo {
                name: "IDLE",
                priority: 0,
                state: "ready",
                kill_authority: crate::verbs::KillAuthority::Ordinary,
            }],
            spoors: &crate::verbs::NoSpoors,
            denials: 0,
        };
        w.volume.create(0, "NOTES.TXT", b"alpha\nbeta\nalpha again", Labels::seeded()).unwrap();
        w
    }

    fn run(w: &mut World<'_>, line: &str) -> String {
        let mut out = String::new();
        run_line(w, line, &mut out).unwrap();
        out
    }

    /// D1 — the register's DOS bindings dispatch, case-insensitively
    /// (STORY-P2-04-01 acceptance 1).
    #[test]
    fn d1_bindings_dispatch() {
        let mut w = world();
        assert!(run(&mut w, "ver").contains("TinyOS Version"));
        assert!(run(&mut w, "TYPE NOTES.TXT").contains("alpha"));
        assert_eq!(run(&mut w, "cd"), "A:\\\n");
        run(&mut w, "MD DOCS");
        run(&mut w, "CHDIR DOCS");
        assert_eq!(run(&mut w, "CD"), "A:\\DOCS\n");
        assert_eq!(run(&mut w, "WHATNOW"), "Bad command or file name\n");
    }

    /// D1b — the `SPOOR` binding (`LE-56`, terminal-gap `verb:spoor-journal`)
    /// dispatches to the journal-dump verb, case-insensitively, and takes no
    /// parameters.
    #[test]
    fn d1b_spoor_binding_dispatches() {
        let mut w = world();
        assert!(run(&mut w, "SPOOR").starts_with("Spoor journal ("), "SPOOR dispatches");
        assert!(run(&mut w, "spoor").contains("No spoors journaled"), "case-insensitive");
        assert_eq!(run(&mut w, "SPOOR EXTRA"), "Too many parameters\n");
    }

    /// D2 — message-shape parity for refusals (STORY-P2-04-01 acceptance 2).
    #[test]
    fn d2_refusal_shapes() {
        let mut w = world();
        assert_eq!(run(&mut w, "DIR /X"), "Invalid switch\n");
        assert_eq!(run(&mut w, "COPY ONLYONE"), "Required parameter missing\n");
        assert_eq!(run(&mut w, "TYPE A B"), "Too many parameters\n");
        assert_eq!(run(&mut w, "TYPE MISSING.TXT"), "File not found\n");
        assert_eq!(run(&mut w, "CD NOWHERE"), "Invalid directory\n");
        assert!(run(&mut w, "DEL NOTES.TXT").contains("use /Y"), "script-safety rule");
    }

    /// D3 — totality: adversarial input never panics and never invents a verb
    /// (STORY-P2-04-01 acceptance 3).
    #[test]
    fn d3_adversarial_totality() {
        let mut w = world();
        for hostile in [
            "\"",
            "%",
            "%%",
            "%UNDEFINED%",
            "///",
            "DIR /P /P /P /P /P",
            "FIND unquoted NOTES.TXT",
            "FIND \"unclosed NOTES.TXT",
            "COPY \u{1b}[2J EVIL",
            "DEL",
            "     ",
            "\t\t",
        ] {
            let _ = run(&mut w, hostile); // must not panic
        }
        let long = "X".repeat(crate::capacities::MAX_LINE + 1);
        assert_eq!(run(&mut w, &long), "Line too long\n");
    }

    /// D4 — `%VAR%` expansion: bounded single pass, undefined expands to nothing,
    /// lone `%` literal (STORY-P2-04-01 acceptance 4).
    #[test]
    fn d4_percent_expansion() {
        let mut w = world();
        run(&mut w, "SET GREETING=hello");
        assert_eq!(run(&mut w, "ECHO %GREETING% world"), "hello world\n");
        assert_eq!(run(&mut w, "ECHO %UNDEFINED%x"), "x\n");
        assert_eq!(run(&mut w, "ECHO 100% done"), "100% done\n");
        // No recursion: a value containing %X% is not re-expanded.
        run(&mut w, "SET A=%GREETING%"); // expands at set time to "hello"
        run(&mut w, "SET B=literal");
        assert_eq!(run(&mut w, "ECHO %A%"), "hello\n");
    }

    /// D5 — FIND's 4.0 grammar: quoted pattern, /C /N /V.
    #[test]
    fn d5_find_grammar() {
        let mut w = world();
        let out = run(&mut w, "FIND /C \"alpha\" NOTES.TXT");
        assert_eq!(out, "---------- NOTES.TXT: 2\n");
        let out = run(&mut w, "FIND /N \"beta\" NOTES.TXT");
        assert!(out.contains("[2]beta"));
        let out = run(&mut w, "FIND /V \"alpha\" NOTES.TXT");
        assert!(out.contains("beta") && !out.contains("again"));
    }
}
