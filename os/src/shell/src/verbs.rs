//! The canonical verb core (`FEAT-P2-01`, `STORY-P2-01-01`).
//!
//! Typed requests, executed against the [`crate::volume::RamVolume`] through the
//! deny-by-default [`crate::policy::VerbPolicy`] seam. Output shapes are bound to
//! `goals/context/terminal-gap.tsv`'s decided column — 4.0's message strings where
//! adopted, the recorded divergences where exceeded. No front-end syntax lives here.

use core::fmt::{self, Write};

use crate::capacities::{MAX_ENV, MAX_ENV_KEY, MAX_ENV_VAL, MAX_PATH, MAX_SORT_LINES};
use crate::labels::{Origin, Signer, Trust};
use crate::policy::VerbPolicy;
use crate::render::write_inert;
use crate::volume::{RamVolume, VolumeError};

/// Every canonical verb, for policy decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerbKind {
    /// `DIR` / `ls`.
    List,
    /// `CD` with an argument.
    ChangeDir,
    /// `CD` without an argument / `pwd`.
    PrintCwd,
    /// `COPY` / `cp`.
    Copy,
    /// `MOVE`, `REN` / `mv`.
    Move,
    /// `DEL`, `ERASE` / `rm`.
    Delete,
    /// `MD` / `mkdir`.
    MakeDir,
    /// `RD` / `rmdir`.
    RemoveDir,
    /// `TYPE` / `cat`.
    ViewFile,
    /// `FIND` / `grep`.
    FindText,
    /// `SORT` / `sort`.
    SortStream,
    /// `MORE` / `more` (non-paging batch form until `LE-55` is repaired).
    Page,
    /// `TREE` / `tree`.
    TreeView,
    /// `ATTRIB` / `ls -l`-column analogue.
    AttribView,
    /// `SET`, `PATH` / `env`, `export`.
    Env,
    /// `ECHO`.
    Echo,
    /// `CLS` / `clear`.
    ClearScreen,
    /// `VER` / `uname` subset.
    VersionInfo,
    /// `VOL` / `df` subset.
    VolumeInfo,
    /// `MEM` / `free` subset.
    MemInfo,
    /// `TASKMGR` list / `ps`.
    TaskList,
    /// `kill` analogue.
    TaskKill,
}

/// One task-table row, injected by the host (fixture or, later, the scheduler).
#[derive(Debug, Clone, Copy)]
pub struct TaskInfo {
    /// Task name.
    pub name: &'static str,
    /// Priority (0 = RT-critical).
    pub priority: u8,
    /// Scheduler state, rendered verbatim.
    pub state: &'static str,
}

/// A typed request — what both front-ends compile to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request<'a> {
    /// List a directory.
    List {
        /// Path, or the cwd.
        path: Option<&'a str>,
        /// `/W` wide mode.
        wide: bool,
    },
    /// Change directory.
    ChangeDir(&'a str),
    /// Print the cwd.
    PrintCwd,
    /// Copy `src` to `dst` (labels travel).
    Copy(&'a str, &'a str),
    /// Move/rename `src` to `dst`.
    Move(&'a str, &'a str),
    /// Delete a file. `assume_yes` is the explicit non-interactive flag the
    /// safety spec demands of scripts (`/Y`).
    Delete {
        /// Target path.
        path: &'a str,
        /// The `/Y` flag.
        assume_yes: bool,
    },
    /// Create a directory.
    MakeDir(&'a str),
    /// Remove an empty directory.
    RemoveDir(&'a str),
    /// Dump a file.
    ViewFile(&'a str),
    /// Find literal `pattern` in `path`.
    FindText {
        /// Literal pattern (no regex at MVP).
        pattern: &'a str,
        /// File to search.
        path: &'a str,
        /// `/V` invert.
        invert: bool,
        /// `/C` count only.
        count: bool,
        /// `/N` number lines.
        number: bool,
    },
    /// Sort a file's lines.
    SortStream {
        /// File whose lines are sorted.
        path: &'a str,
        /// `/R` reverse.
        reverse: bool,
    },
    /// Page a file (batch form: sequential dump).
    Page(&'a str),
    /// Tree view from the cwd.
    TreeView {
        /// `/A`: ASCII line art (the canonical Report form).
        ascii: bool,
        /// `/F`: include files.
        files: bool,
    },
    /// Show a file's DOS attributes and `G-SEC-5` labels.
    AttribView(Option<&'a str>),
    /// `SET` — dump, get, set or delete.
    EnvSet {
        /// `None` dumps the environment.
        key: Option<&'a str>,
        /// `Some("")` deletes; `None` with a key prints that key.
        value: Option<&'a str>,
    },
    /// `ECHO` — text, mode toggle, or state query.
    Echo {
        /// `Some(true)`=ON, `Some(false)`=OFF.
        mode: Option<bool>,
        /// Text to print.
        text: Option<&'a str>,
    },
    /// Clear the screen (emits `ESC[2J` — trusted output).
    ClearScreen,
    /// Version banner.
    VersionInfo,
    /// Volume header.
    VolumeInfo,
    /// Memory table.
    MemInfo,
    /// Task table.
    TaskList,
    /// Kill a task by name.
    TaskKill(&'a str),
    /// A command word no front-end recognises.
    Unknown,
}

impl Request<'_> {
    /// The policy-facing kind. `Unknown` maps to `None` (refused before policy).
    pub fn kind(&self) -> Option<VerbKind> {
        Some(match self {
            Request::List { .. } => VerbKind::List,
            Request::ChangeDir(_) => VerbKind::ChangeDir,
            Request::PrintCwd => VerbKind::PrintCwd,
            Request::Copy(..) => VerbKind::Copy,
            Request::Move(..) => VerbKind::Move,
            Request::Delete { .. } => VerbKind::Delete,
            Request::MakeDir(_) => VerbKind::MakeDir,
            Request::RemoveDir(_) => VerbKind::RemoveDir,
            Request::ViewFile(_) => VerbKind::ViewFile,
            Request::FindText { .. } => VerbKind::FindText,
            Request::SortStream { .. } => VerbKind::SortStream,
            Request::Page(_) => VerbKind::Page,
            Request::TreeView { .. } => VerbKind::TreeView,
            Request::AttribView(_) => VerbKind::AttribView,
            Request::EnvSet { .. } => VerbKind::Env,
            Request::Echo { .. } => VerbKind::Echo,
            Request::ClearScreen => VerbKind::ClearScreen,
            Request::VersionInfo => VerbKind::VersionInfo,
            Request::VolumeInfo => VerbKind::VolumeInfo,
            Request::MemInfo => VerbKind::MemInfo,
            Request::TaskList => VerbKind::TaskList,
            Request::TaskKill(_) => VerbKind::TaskKill,
            Request::Unknown => return None,
        })
    }
}

/// One stored environment pair: key bytes, key length, value bytes, value length.
type EnvSlot = ([u8; MAX_ENV_KEY], u8, [u8; MAX_ENV_VAL], u8);

/// The session environment is out of space or the pair is over-length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnvSpaceExhausted;

/// Fixed-capacity session environment.
pub struct Env {
    slots: [Option<EnvSlot>; MAX_ENV],
}

impl Default for Env {
    fn default() -> Self {
        Self::new()
    }
}

impl Env {
    /// Empty environment.
    pub const fn new() -> Self {
        Env { slots: [None; MAX_ENV] }
    }
    /// Look up `key` case-insensitively.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.slots.iter().flatten().find_map(|(k, kl, v, vl)| {
            let stored = core::str::from_utf8(&k[..*kl as usize]).ok()?;
            stored
                .eq_ignore_ascii_case(key)
                .then(|| core::str::from_utf8(&v[..*vl as usize]).unwrap_or(""))
        })
    }
    /// Set, replace or (empty value) delete.
    pub fn set(&mut self, key: &str, value: &str) -> Result<(), EnvSpaceExhausted> {
        if key.len() > MAX_ENV_KEY || value.len() > MAX_ENV_VAL {
            return Err(EnvSpaceExhausted);
        }
        let existing = self.slots.iter().position(|slot| {
            matches!(slot, Some((k, kl, ..))
                if core::str::from_utf8(&k[..*kl as usize]).unwrap_or("").eq_ignore_ascii_case(key))
        });
        if value.is_empty() {
            if let Some(slot) = existing {
                self.slots[slot] = None;
            }
            return Ok(());
        }
        let slot = existing
            .or_else(|| self.slots.iter().position(Option::is_none))
            .ok_or(EnvSpaceExhausted)?;
        let mut kb = [0u8; MAX_ENV_KEY];
        kb[..key.len()].copy_from_slice(key.as_bytes());
        let mut vb = [0u8; MAX_ENV_VAL];
        vb[..value.len()].copy_from_slice(value.as_bytes());
        self.slots[slot] = Some((kb, key.len() as u8, vb, value.len() as u8));
        Ok(())
    }
    /// Iterate `KEY=VALUE` pairs in slot order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.slots.iter().flatten().map(|(k, kl, v, vl)| {
            (
                core::str::from_utf8(&k[..*kl as usize]).unwrap_or("?"),
                core::str::from_utf8(&v[..*vl as usize]).unwrap_or("?"),
            )
        })
    }
}

/// Everything a session executes against.
pub struct World<'a> {
    /// The labelled volume.
    pub volume: RamVolume,
    /// Session environment.
    pub env: Env,
    /// Current directory index.
    pub cwd: u8,
    /// Batch echo state (4.0 default: ON).
    pub echo: bool,
    /// The ACI seam. `Sync` so a host-side tab owner can hold a `World<'static>`
    /// behind shared state (C5); every policy is plain data.
    pub policy: &'a (dyn VerbPolicy + Sync),
    /// Session identity (kernel-derived in destination; fixture-set at Tier 0).
    pub session: &'a str,
    /// Injected task table.
    pub tasks: &'a [TaskInfo],
    /// Denials observed (the fixture's in-guest assertion counter).
    pub denials: u32,
}

/// Fixed Tier 0 timestamp column (deterministic transcripts).
const STAMP: &str = "07-30-26  12:00p";

fn volume_error(sink: &mut dyn Write, error: VolumeError) -> fmt::Result {
    let text = match error {
        VolumeError::NotFound => "File not found",
        VolumeError::BadDirectory => "Invalid directory",
        VolumeError::Exists => "Directory already exists",
        VolumeError::Full | VolumeError::TooLarge => "Insufficient disk space",
        VolumeError::NotEmpty => "Invalid path, not directory,\nor directory not empty",
        VolumeError::BadPath | VolumeError::BadName => "Invalid path",
        VolumeError::Quarantined => "Refused: content is quarantined",
        VolumeError::ReadOnly => "Access denied ",
    };
    writeln!(sink, "{text}")
}

fn write_volume_header(sink: &mut dyn Write, volume: &RamVolume) -> fmt::Result {
    match volume.label() {
        Some(label) => writeln!(sink, " Volume in drive A is {label}")?,
        None => writeln!(sink, " Volume in drive A has no label")?,
    }
    let (hi, lo) = volume.serial;
    writeln!(sink, " Volume Serial Number is {hi:04X}-{lo:04X}")
}

fn write_cwd(world: &World<'_>, sink: &mut dyn Write) -> fmt::Result {
    let mut buffer = [0u8; MAX_PATH];
    let len = world.volume.dir_path(world.cwd, &mut buffer);
    write!(sink, "A:{}", core::str::from_utf8(&buffer[..len]).unwrap_or("\\"))
}

/// `TREE`, always in the `/A` ASCII form — `EPIC-P2` §6.5 rule 2 makes ASCII the
/// canonical Report form; the graphics variant is the tab host's later concern.
/// Subdirectory names and indices both come from volume slot order, so position
/// `n` of the directory-typed listing entries names subdir `n`.
fn tree_walk(
    world: &World<'_>,
    sink: &mut dyn Write,
    dir: u8,
    files: bool,
    depth: usize,
) -> fmt::Result {
    let count = world.volume.subdirs(dir).count();
    for (position, sub) in world.volume.subdirs(dir).enumerate() {
        for _ in 0..depth {
            write!(sink, "|   ")?;
        }
        let branch = if position + 1 == count { "\\---" } else { "+---" };
        let label = world
            .volume
            .list(dir)
            .filter(|entry| entry.size.is_none())
            .nth(position)
            .map(|entry| entry.name)
            .unwrap_or("?");
        write!(sink, "{branch}")?;
        write_inert(sink, label)?;
        writeln!(sink)?;
        if files {
            for entry in world.volume.list(sub) {
                if entry.size.is_some() {
                    for _ in 0..depth + 1 {
                        write!(sink, "|   ")?;
                    }
                    write_inert(sink, entry.name)?;
                    writeln!(sink)?;
                }
            }
        }
        tree_walk(world, sink, sub, files, depth + 1)?;
    }
    Ok(())
}

/// Execute one authorised request. The **only** path to a verb's effect: the policy
/// verdict happens here, denials are audited to the sink, and `Unknown` answers the
/// 4.0 shape before policy is even consulted (there is no verb to authorise).
pub fn execute(world: &mut World<'_>, request: &Request<'_>, sink: &mut dyn Write) -> fmt::Result {
    let Some(kind) = request.kind() else {
        return writeln!(sink, "Bad command or file name");
    };
    if !world.policy.allows(world.session, kind) {
        world.denials += 1;
        return writeln!(
            sink,
            "Access denied: verb {kind:?} is not granted to session {} [audited]",
            world.session
        );
    }
    match *request {
        Request::List { path, wide } => {
            let dir = match path {
                Some(p) => match world.volume.resolve_dir(world.cwd, p) {
                    Ok(d) => d,
                    Err(e) => return volume_error(sink, e),
                },
                None => world.cwd,
            };
            writeln!(sink)?;
            write_volume_header(sink, &world.volume)?;
            writeln!(sink)?;
            let mut buffer = [0u8; MAX_PATH];
            let len = world.volume.dir_path(dir, &mut buffer);
            writeln!(
                sink,
                " Directory of A:{}",
                core::str::from_utf8(&buffer[..len]).unwrap_or("\\")
            )?;
            writeln!(sink)?;
            let mut file_count = 0u32;
            for entry in world.volume.list(dir) {
                if wide {
                    write_inert(sink, entry.name)?;
                    write!(sink, "\t")?;
                    if entry.size.is_some() {
                        file_count += 1;
                    }
                    continue;
                }
                let padding = 14usize.saturating_sub(entry.name.len());
                write_inert(sink, entry.name)?;
                for _ in 0..padding {
                    write!(sink, " ")?;
                }
                match entry.size {
                    None => writeln!(sink, "<DIR>          {STAMP}")?,
                    Some(size) => {
                        file_count += 1;
                        writeln!(sink, "{size:>10} {STAMP}")?;
                    }
                }
            }
            if wide {
                writeln!(sink)?;
            }
            writeln!(sink, "{file_count:>8} File(s) {:>10} bytes free", world.volume.free_bytes())
        }
        Request::ChangeDir(path) => match world.volume.resolve_dir(world.cwd, path) {
            Ok(dir) => {
                world.cwd = dir;
                Ok(())
            }
            Err(_) => writeln!(sink, "Invalid directory"),
        },
        Request::PrintCwd => {
            write_cwd(world, sink)?;
            writeln!(sink)
        }
        Request::Copy(src, dst) => match world.volume.copy(world.cwd, src, dst) {
            Ok(()) => writeln!(sink, "        1 File(s) copied"),
            Err(e) => volume_error(sink, e),
        },
        Request::Move(src, dst) => match world.volume.rename(world.cwd, src, dst) {
            Ok(()) => Ok(()),
            Err(VolumeError::NotFound) => {
                writeln!(sink, "Duplicate file name or file not found")
            }
            Err(VolumeError::Exists) => {
                writeln!(sink, "Duplicate file name or file not found")
            }
            Err(e) => volume_error(sink, e),
        },
        Request::Delete { path, assume_yes } => {
            if !assume_yes {
                // The Safety rule: scripts must opt out of confirmation explicitly, and
                // interactive confirmation has no transport yet (`LE-55`).
                return writeln!(
                    sink,
                    "Confirmation required and no interactive session exists; use /Y"
                );
            }
            match world.volume.delete(world.cwd, path) {
                Ok(()) => Ok(()),
                Err(e) => volume_error(sink, e),
            }
        }
        Request::MakeDir(path) => match world.volume.mkdir(world.cwd, path) {
            Ok(()) => Ok(()),
            Err(VolumeError::Exists) => writeln!(sink, "Directory already exists"),
            Err(VolumeError::Full) => writeln!(sink, "Unable to create directory"),
            Err(e) => volume_error(sink, e),
        },
        Request::RemoveDir(path) => match world.volume.rmdir(world.cwd, path) {
            Ok(()) => Ok(()),
            Err(_) => {
                writeln!(sink, "Invalid path, not directory,")?;
                writeln!(sink, "or directory not empty")
            }
        },
        Request::ViewFile(path) | Request::Page(path) => match world.volume.read(world.cwd, path) {
            Ok(bytes) => {
                let text = core::str::from_utf8(bytes).unwrap_or("?binary?");
                for line in text.lines() {
                    write_inert(sink, line)?;
                    writeln!(sink)?;
                }
                Ok(())
            }
            Err(e) => volume_error(sink, e),
        },
        Request::FindText { pattern, path, invert, count, number } => {
            match world.volume.read(world.cwd, path) {
                Err(e) => {
                    write!(sink, "FIND: ")?;
                    volume_error(sink, e)
                }
                Ok(bytes) => {
                    let text = core::str::from_utf8(bytes).unwrap_or("");
                    write!(sink, "---------- ")?;
                    write_inert(sink, path)?;
                    let mut hits = 0u32;
                    if count {
                        for line in text.lines() {
                            if line.contains(pattern) != invert {
                                hits += 1;
                            }
                        }
                        return writeln!(sink, ": {hits}");
                    }
                    writeln!(sink)?;
                    for (index, line) in text.lines().enumerate() {
                        if line.contains(pattern) != invert {
                            if number {
                                write!(sink, "[{}]", index + 1)?;
                            }
                            write_inert(sink, line)?;
                            writeln!(sink)?;
                        }
                    }
                    Ok(())
                }
            }
        }
        Request::SortStream { path, reverse } => match world.volume.read(world.cwd, path) {
            Err(e) => {
                write!(sink, "SORT: ")?;
                volume_error(sink, e)
            }
            Ok(bytes) => {
                let text = core::str::from_utf8(bytes).unwrap_or("");
                let mut lines: [&str; MAX_SORT_LINES] = [""; MAX_SORT_LINES];
                let mut used = 0;
                for line in text.lines() {
                    if used == MAX_SORT_LINES {
                        return writeln!(sink, "SORT: Insufficient memory");
                    }
                    lines[used] = line;
                    used += 1;
                }
                lines[..used].sort_unstable();
                if reverse {
                    lines[..used].reverse();
                }
                for line in &lines[..used] {
                    write_inert(sink, line)?;
                    writeln!(sink)?;
                }
                Ok(())
            }
        },
        Request::TreeView { ascii: _, files } => {
            match world.volume.label() {
                Some(label) => writeln!(sink, "Directory PATH listing for Volume {label}")?,
                None => writeln!(sink, "Directory PATH listing")?,
            }
            let (hi, lo) = world.volume.serial;
            writeln!(sink, "Volume Serial Number is {hi:04X}-{lo:04X}")?;
            write_cwd(world, sink)?;
            writeln!(sink)?;
            if world.volume.subdirs(world.cwd).next().is_none() {
                writeln!(sink, "No sub-directories exist")?;
                writeln!(sink)?;
                return Ok(());
            }
            tree_walk(world, sink, world.cwd, files, 0)
        }
        Request::AttribView(path) => match path {
            None => writeln!(sink, "Required parameter missing"),
            Some(p) => match world.volume.stat(world.cwd, p) {
                Err(e) => volume_error(sink, e),
                Ok(labels) => {
                    let archive = if labels.derivation != 0 { 'A' } else { ' ' };
                    let read_only = if labels.read_only { 'R' } else { ' ' };
                    let quarantine = if labels.quarantine { 'Q' } else { ' ' };
                    write!(sink, " {archive}   {read_only}{quarantine}  ")?;
                    write!(
                        sink,
                        "[origin={} trust={}] ",
                        match labels.origin {
                            Origin::Local => "local",
                            Origin::Seeded => "seeded",
                            Origin::External => "external",
                        },
                        match labels.trust {
                            Trust::Untrusted => "untrusted",
                            Trust::Operator => "operator",
                            Trust::System => "system",
                        }
                    )?;
                    let _ = Signer::Unsigned;
                    write_inert(sink, p)?;
                    writeln!(sink)
                }
            },
        },
        Request::EnvSet { key, value } => match (key, value) {
            (None, _) => {
                for (k, v) in world.env.iter() {
                    write_inert(sink, k)?;
                    write!(sink, "=")?;
                    write_inert(sink, v)?;
                    writeln!(sink)?;
                }
                Ok(())
            }
            (Some(k), Some(v)) => match world.env.set(k, v) {
                Ok(()) => Ok(()),
                Err(EnvSpaceExhausted) => writeln!(sink, "Out of environment space"),
            },
            (Some(k), None) => match world.env.get(k) {
                Some(v) => {
                    write_inert(sink, k)?;
                    write!(sink, "=")?;
                    write_inert(sink, v)?;
                    writeln!(sink)
                }
                None => writeln!(sink, "Environment variable {k} not defined"),
            },
        },
        Request::Echo { mode, text } => match (mode, text) {
            (Some(state), _) => {
                world.echo = state;
                Ok(())
            }
            (None, Some(t)) => {
                write_inert(sink, t)?;
                writeln!(sink)
            }
            (None, None) => {
                writeln!(sink, "ECHO is {}", if world.echo { "on" } else { "off" })
            }
        },
        Request::ClearScreen => write!(sink, "\u{1b}[2J"),
        Request::VersionInfo => {
            writeln!(sink)?;
            writeln!(sink, "TinyOS Version 0.2.0 (Tier 0, x86_64)")?;
            writeln!(sink)
        }
        Request::VolumeInfo => {
            writeln!(sink)?;
            write_volume_header(sink, &world.volume)
        }
        Request::MemInfo => {
            writeln!(sink, "  Address     Name          Size       Type")?;
            writeln!(sink, "  -------     ----          ----       ----")?;
            let total = (crate::capacities::MAX_FILES * crate::capacities::MAX_DATA) as u32;
            let free = world.volume.free_bytes();
            writeln!(sink, "  000000      VOLUME        {total:>6}     Static Pool")?;
            writeln!(sink)?;
            writeln!(sink, "{total:>10} bytes total memory")?;
            writeln!(sink, "{free:>10} bytes available")
        }
        Request::TaskList => {
            writeln!(sink, "  TASK          PRI  STATE")?;
            for task in world.tasks {
                write!(sink, "  ")?;
                write_inert(sink, task.name)?;
                for _ in 0..14usize.saturating_sub(task.name.len()) {
                    write!(sink, " ")?;
                }
                writeln!(sink, "{:>3}  {}", task.priority, task.state)?;
            }
            Ok(())
        }
        Request::TaskKill(name) => {
            let Some(task) = world.tasks.iter().find(|t| t.name.eq_ignore_ascii_case(name)) else {
                return writeln!(sink, "File not found");
            };
            if task.priority == 0 && !world.policy.supervisor(world.session) {
                world.denials += 1;
                return writeln!(
                    sink,
                    "Access denied: task {} is RT-critical and session {} lacks supervisor scope [audited]",
                    task.name, world.session
                );
            }
            writeln!(sink, "Task {} signalled", task.name)
        }
        Request::Unknown => unreachable!("handled before policy"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{DenyAll, GrantSet};

    const ALL: &[VerbKind] =
        &[VerbKind::List, VerbKind::PrintCwd, VerbKind::Echo, VerbKind::TaskKill];

    fn world<'a>(policy: &'a (dyn VerbPolicy + Sync)) -> World<'a> {
        World {
            volume: RamVolume::new(Some("TINYOS"), (0x1234, 0xABCD)),
            env: Env::new(),
            cwd: 0,
            echo: true,
            policy,
            session: "TEST",
            tasks: &[
                TaskInfo { name: "RT-CTRL", priority: 0, state: "ready" },
                TaskInfo { name: "IDLE", priority: 9, state: "ready" },
            ],
            denials: 0,
        }
    }

    /// C5 — the multi-tab host (17G) holds one `World<'static>` per tab behind shared
    /// state on the host side; that only holds if a static world is `Send`.
    #[test]
    fn c5_world_static_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<World<'static>>();
    }

    /// C1 — deny-by-default: with no policy nothing runs, and the denial is audited
    /// (STORY-P2-01-01 acceptance 2).
    #[test]
    fn c1_deny_all_denies_and_audits() {
        let mut w = world(&DenyAll);
        let mut out = String::new();
        execute(&mut w, &Request::PrintCwd, &mut out).unwrap();
        assert!(out.contains("Access denied"));
        assert!(out.contains("[audited]"));
        assert!(out.contains("TEST"), "audit must carry session identity");
        assert_eq!(w.denials, 1);
    }

    /// C2 — a granted verb runs; an ungranted one refuses; Unknown answers the 4.0 shape.
    #[test]
    fn c2_grants_are_exact_and_unknown_is_dos_shaped() {
        let policy = GrantSet { granted: ALL, withheld: None, supervisor: false };
        let mut w = world(&policy);
        let mut out = String::new();
        execute(&mut w, &Request::PrintCwd, &mut out).unwrap();
        assert_eq!(out, "A:\\\n");
        out.clear();
        execute(&mut w, &Request::VersionInfo, &mut out).unwrap();
        assert!(out.contains("Access denied"));
        out.clear();
        execute(&mut w, &Request::Unknown, &mut out).unwrap();
        assert_eq!(out, "Bad command or file name\n");
    }

    /// C3 — RT-critical task-kill needs supervisor scope, and the refusal is audited.
    #[test]
    fn c3_rt_critical_kill_needs_supervisor() {
        let operator = GrantSet { granted: ALL, withheld: None, supervisor: false };
        let mut w = world(&operator);
        let mut out = String::new();
        execute(&mut w, &Request::TaskKill("RT-CTRL"), &mut out).unwrap();
        assert!(out.contains("RT-critical"));
        assert_eq!(w.denials, 1);

        let supervisor = GrantSet { granted: ALL, withheld: None, supervisor: true };
        let mut w = world(&supervisor);
        let mut out = String::new();
        execute(&mut w, &Request::TaskKill("rt-ctrl"), &mut out).unwrap();
        assert_eq!(out, "Task RT-CTRL signalled\n");
    }

    /// C4 — deterministic output: two identical worlds render byte-identical DIR
    /// (STORY-P2-01-01 acceptance 3), and hostile filenames render inert in it.
    #[test]
    fn c4_dir_is_deterministic_and_inert() {
        let policy = GrantSet {
            granted: &[VerbKind::List, VerbKind::MakeDir],
            withheld: None,
            supervisor: false,
        };
        let render = || {
            let mut w = world(&policy);
            w.volume.create(0, "EVIL\u{1b}[2J.TXT", b"x", crate::labels::Labels::seeded()).unwrap();
            let mut out = String::new();
            execute(&mut w, &Request::List { path: None, wide: false }, &mut out).unwrap();
            out
        };
        let first = render();
        assert_eq!(first, render());
        assert!(first.contains("EVIL?[2J.TXT"), "filename escape must be inert: {first}");
        assert!(first.contains(" Volume in drive A is TINYOS"));
        assert!(first.contains(" File(s)"));
    }
}
