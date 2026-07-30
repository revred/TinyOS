//! The tab/session model (17G §1): one window, many tabs, each tab its own TINYCMD
//! session run **in-process on the host** — `shell::verbs::World` plus
//! `shell::dos::run_line`, the same crate the QEMU fixture boots, so a tab renders
//! byte-identically to the target (the `fmt::Write` sink property `FEAT-P2-01` bought).
//!
//! The per-tab session boundary is `EPIC-P2` §6.1 at host level: every tab owns its
//! `World` — env, cwd, volume view and policy — so `SET X=1` in one tab is invisible in
//! another *by construction*, and the tests prove it rather than assert it.
//!
//! Capacities doctrine: tab identities are a fixed enumeration ([`TAB_LABELS`]), not a
//! namespace — the signed manifest enumerates exactly these labels, and the registry
//! refuses the tab beyond the last slot as a typed refusal, never a panic.

use shell::batch;
use shell::dos;
use shell::labels::Labels;
use shell::parity;
use shell::policy::GrantSet;
use shell::verbs::{Env, VerbKind, World};
use shell::volume::RamVolume;

/// Maximum concurrent tabs — one reviewable constant, mirrored by the manifest's
/// `tab_labels` enumeration.
pub const MAX_TABS: usize = 6;

/// The enumerated tab webview labels. The signed manifest lists exactly these.
pub const TAB_LABELS: [&str; MAX_TABS] = ["tab-1", "tab-2", "tab-3", "tab-4", "tab-5", "tab-6"];

/// Per-tab session identities, carried into every audited denial the shell records.
pub const SESSION_IDS: [&str; MAX_TABS] = ["TAB-1", "TAB-2", "TAB-3", "TAB-4", "TAB-5", "TAB-6"];

/// Every canonical verb, granted to an interactive DOS tab. Nothing withheld and no
/// supervisor scope: killing an RT-critical task still refuses with an audit, which is
/// the demo's point, not an obstacle.
const TAB_GRANTED: &[VerbKind] = &[
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
];

/// The interactive tab policy: the full canonical verb set, no supervisor scope.
static TAB_POLICY: GrantSet =
    GrantSet { granted: TAB_GRANTED, withheld: None, supervisor: false };

/// What kind of session a tab hosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TabKind {
    /// An interactive host-run TINYCMD (DOS front-end) session.
    Dos,
    /// The target-parity tab: runs the whole MS-DOS parity suite.
    Parity,
}

impl TabKind {
    /// The flavour string the reserved region shows for a focused tab.
    pub fn flavour(self) -> &'static str {
        match self {
            TabKind::Dos => "DOS",
            TabKind::Parity => "TARGET-PARITY",
        }
    }
}

/// A typed registry refusal — every failure is a named reason, never a panic.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub enum TabError {
    /// All [`MAX_TABS`] slots are taken.
    CapacityExhausted,
    /// No tab carries the given label.
    NoSuchTab,
    /// The verb needs a DOS session but the tab is not one (or vice versa).
    WrongKind,
}

impl std::fmt::Display for TabError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            TabError::CapacityExhausted => "tab capacity exhausted: all slots are in use",
            TabError::NoSuchTab => "no tab carries this label",
            TabError::WrongKind => "the verb does not match this tab's session kind",
        };
        f.write_str(text)
    }
}

/// The serializable identity card of one tab — what `read_console` lists.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TabInfo {
    /// The tab's webview label (enumerated in the signed manifest).
    pub label: String,
    /// The session identity the shell audits denials under.
    pub session: String,
    /// What the tab hosts.
    pub kind: TabKind,
}

/// One tab: identity plus, for DOS tabs, the owned session.
pub struct Tab {
    label: &'static str,
    session: &'static str,
    kind: TabKind,
    /// `Some` for DOS tabs; the parity tab owns no interactive world.
    dos: Option<DosSession>,
}

/// The host-run TINYCMD session a DOS tab owns.
struct DosSession {
    world: World<'static>,
    transcript: String,
}

impl Tab {
    /// The identity card.
    pub fn info(&self) -> TabInfo {
        TabInfo { label: self.label.into(), session: self.session.into(), kind: self.kind }
    }

    /// The tab's rendered transcript (empty for the parity tab — its content is the
    /// suite state, not a session transcript).
    pub fn transcript(&self) -> &str {
        self.dos.as_ref().map(|d| d.transcript.as_str()).unwrap_or("")
    }

    /// The tab's kind.
    pub fn kind(&self) -> TabKind {
        self.kind
    }

    /// The tab's label.
    pub fn label(&self) -> &'static str {
        self.label
    }
}

/// The sample batch every DOS tab is seeded with: typing `SAMPLE.TCB` at the tab
/// prompt runs it through the real `.TCB` runner — the visible "batch file delivering
/// on expectations" demo. Deterministic; fits the volume's per-file byte capacity.
pub const SAMPLE_TCB: &str = "\
@ECHO OFF
ECHO == SAMPLE.TCB: TINYCMD batch demo ==
VER
SET DEMO=RUNNING
SET DEMO
MD WORK
COPY README.TXT WORK\\COPY.TXT
DIR
FIND /C \"TinyOS\" README.TXT
SORT /R LIST.TXT
ECHO %DEMO% batch complete
";

/// Seed one deterministic DOS world for a tab — same seed files the parity world uses,
/// so `DIR` in a fresh tab shows familiar, review-decided content, plus [`SAMPLE_TCB`].
fn seeded_world(session: &'static str) -> World<'static> {
    let mut volume = RamVolume::new(Some("TINYOS"), (0x1234, 0xABCD));
    volume
        .create(
            0,
            "README.TXT",
            b"TinyOS carries the soul of MS-DOS\ninto a real-time, labelled world.\nTinyOS is tested, not asserted.",
            Labels::seeded(),
        )
        .expect("seed README");
    volume
        .create(0, "LIST.TXT", b"delta\nalpha\ncharlie\nbravo", Labels::seeded())
        .expect("seed LIST");
    volume
        .create(0, "SAMPLE.TCB", SAMPLE_TCB.as_bytes(), Labels::seeded())
        .expect("seed SAMPLE.TCB");
    World {
        volume,
        env: Env::new(),
        cwd: 0,
        echo: true,
        policy: &TAB_POLICY,
        session,
        tasks: parity::TASKS,
        denials: 0,
    }
}

/// The registry: every open tab plus which one holds focus.
#[derive(Default)]
pub struct TabRegistry {
    tabs: Vec<Tab>,
    focused: Option<usize>,
}

impl TabRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Open a tab of `kind`. The new tab takes the next enumerated label, owns a fresh
    /// session (DOS tabs), and receives focus. Refuses beyond [`MAX_TABS`].
    pub fn open(&mut self, kind: TabKind) -> Result<TabInfo, TabError> {
        let index = self.tabs.len();
        if index >= MAX_TABS {
            return Err(TabError::CapacityExhausted);
        }
        let (label, session) = (TAB_LABELS[index], SESSION_IDS[index]);
        let dos = match kind {
            TabKind::Dos => {
                Some(DosSession { world: seeded_world(session), transcript: String::new() })
            }
            TabKind::Parity => None,
        };
        self.tabs.push(Tab { label, session, kind, dos });
        self.focused = Some(index);
        Ok(self.tabs[index].info())
    }

    /// Give `label` the focus.
    pub fn focus(&mut self, label: &str) -> Result<(), TabError> {
        match self.tabs.iter().position(|t| t.label == label) {
            Some(index) => {
                self.focused = Some(index);
                Ok(())
            }
            None => Err(TabError::NoSuchTab),
        }
    }

    /// The focused tab, if any.
    pub fn focused(&self) -> Option<&Tab> {
        self.focused.map(|i| &self.tabs[i])
    }

    /// Every open tab's identity card, in open order.
    pub fn infos(&self) -> Vec<TabInfo> {
        self.tabs.iter().map(Tab::info).collect()
    }

    /// Look a tab up by label.
    pub fn get(&self, label: &str) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.label == label)
    }

    /// Run one DOS-syntax line in `label`'s own session, echoing prompt + raw line into
    /// the transcript exactly as the `.TCB` echo discipline renders it, then the output.
    /// Only that tab's world changes — the session boundary the tests prove.
    pub fn run_line(&mut self, label: &str, line: &str) -> Result<(), TabError> {
        let tab = self
            .tabs
            .iter_mut()
            .find(|t| t.label == label)
            .ok_or(TabError::NoSuchTab)?;
        let dos = tab.dos.as_mut().ok_or(TabError::WrongKind)?;
        let _ = batch::prompt(&dos.world, &mut dos.transcript);
        dos.transcript.push_str(line);
        dos.transcript.push('\n');
        // A word ending `.TCB` that names a readable file runs as a batch — DOS's
        // batch-by-name, restricted to the explicit extension (stated simplification).
        // Anything else (including a missing batch) goes through the front-end, which
        // answers in the register's own shapes.
        let word = line.trim().split_whitespace().next().unwrap_or("");
        if word.len() >= 4 && word[word.len() - 4..].eq_ignore_ascii_case(".TCB") {
            let script = dos
                .world
                .volume
                .read(dos.world.cwd, word)
                .ok()
                .and_then(|bytes| std::str::from_utf8(bytes).ok())
                .map(String::from);
            if let Some(script) = script {
                let _ = batch::run(&mut dos.world, &script, &mut dos.transcript);
                return Ok(());
            }
        }
        let _ = dos::run_line(&mut dos.world, line, &mut dos.transcript);
        Ok(())
    }

    /// The one line of truth the host-owned reserved region paints: focused-tab identity
    /// and flavour. Composed here so the shape is testable without a window.
    pub fn reserved_line(&self) -> String {
        match self.focused() {
            Some(tab) => format!(
                "TINYOS HOST CONSOLE \u{2014} focused {} [{} session {}] \u{2014} {} tab(s) open",
                tab.label,
                tab.kind.flavour(),
                tab.session,
                self.tabs.len()
            ),
            None => "TINYOS HOST CONSOLE \u{2014} no tab focused".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T1 — three tabs open with distinct enumerated labels and session identities;
    /// focus follows the latest open.
    #[test]
    fn t1_tabs_open_with_distinct_sessions() {
        let mut reg = TabRegistry::new();
        let a = reg.open(TabKind::Dos).unwrap();
        let b = reg.open(TabKind::Dos).unwrap();
        let c = reg.open(TabKind::Parity).unwrap();
        assert_eq!((a.label.as_str(), a.session.as_str()), ("tab-1", "TAB-1"));
        assert_eq!((b.label.as_str(), b.session.as_str()), ("tab-2", "TAB-2"));
        assert_eq!((c.label.as_str(), c.kind), ("tab-3", TabKind::Parity));
        assert_eq!(reg.focused().unwrap().label(), "tab-3");
        assert_eq!(reg.infos().len(), 3);
    }

    /// T2 — the per-tab session boundary (§6.1 at host level): `SET` in tab 1 is
    /// visible there and *visibly absent* in tab 2 — the same words a real DOS answers.
    #[test]
    fn t2_env_is_per_tab() {
        let mut reg = TabRegistry::new();
        reg.open(TabKind::Dos).unwrap();
        reg.open(TabKind::Dos).unwrap();
        reg.run_line("tab-1", "SET GREET=HELLO-17G").unwrap();
        reg.run_line("tab-1", "SET GREET").unwrap();
        reg.run_line("tab-2", "SET GREET").unwrap();
        assert!(reg.get("tab-1").unwrap().transcript().contains("GREET=HELLO-17G"));
        assert!(
            reg.get("tab-2")
                .unwrap()
                .transcript()
                .contains("Environment variable GREET not defined"),
            "tab 2 must not see tab 1's environment: {}",
            reg.get("tab-2").unwrap().transcript()
        );
    }

    /// T3 — cwd and volume are per-tab too: `MD`+`CD` in tab 1 leaves tab 2 at root,
    /// and tab 2's `DIR` does not show tab 1's directory.
    #[test]
    fn t3_volume_and_cwd_are_per_tab() {
        let mut reg = TabRegistry::new();
        reg.open(TabKind::Dos).unwrap();
        reg.open(TabKind::Dos).unwrap();
        reg.run_line("tab-1", "MD PRIVATE").unwrap();
        reg.run_line("tab-1", "CD PRIVATE").unwrap();
        reg.run_line("tab-1", "CD").unwrap();
        reg.run_line("tab-2", "DIR").unwrap();
        assert!(reg.get("tab-1").unwrap().transcript().contains("A:\\PRIVATE"));
        assert!(!reg.get("tab-2").unwrap().transcript().contains("PRIVATE"));
    }

    /// T4 — the transcript echoes prompt + raw line, the batch echo shape.
    #[test]
    fn t4_transcript_echoes_prompt_and_line() {
        let mut reg = TabRegistry::new();
        reg.open(TabKind::Dos).unwrap();
        reg.run_line("tab-1", "DIR").unwrap();
        let transcript = reg.get("tab-1").unwrap().transcript();
        assert!(transcript.contains("A:\\>DIR"), "prompt+line echo missing: {transcript}");
        assert!(transcript.contains("Directory of A:\\"), "DIR output missing: {transcript}");
    }

    /// T5 — every refusal is typed: capacity, unknown label, wrong kind.
    #[test]
    fn t5_refusals_are_typed() {
        let mut reg = TabRegistry::new();
        for _ in 0..MAX_TABS {
            reg.open(TabKind::Dos).unwrap();
        }
        assert_eq!(reg.open(TabKind::Dos), Err(TabError::CapacityExhausted));
        assert_eq!(reg.run_line("tab-9", "DIR"), Err(TabError::NoSuchTab));
        let mut reg = TabRegistry::new();
        reg.open(TabKind::Parity).unwrap();
        assert_eq!(reg.run_line("tab-1", "DIR"), Err(TabError::WrongKind));
        assert_eq!(reg.focus("tab-7"), Err(TabError::NoSuchTab));
    }

    /// T7 — typing `SAMPLE.TCB` at the prompt runs the seeded batch through the real
    /// `.TCB` runner in this tab's own world: echo discipline, environment, volume
    /// writes and expansion all land in the transcript; a missing batch answers the
    /// register's own refusal.
    #[test]
    fn t7_sample_tcb_runs_by_name() {
        let mut reg = TabRegistry::new();
        reg.open(TabKind::Dos).unwrap();
        reg.run_line("tab-1", "SAMPLE.TCB").unwrap();
        let transcript = reg.get("tab-1").unwrap().transcript();
        assert!(transcript.contains("A:\\>SAMPLE.TCB"), "prompt echo: {transcript}");
        assert!(transcript.contains("== SAMPLE.TCB: TINYCMD batch demo =="));
        assert!(transcript.contains("DEMO=RUNNING"), "SET inside the batch: {transcript}");
        assert!(transcript.contains("RUNNING batch complete"), "%DEMO% expanded: {transcript}");
        assert!(transcript.contains("        1 File(s) copied"), "COPY ran: {transcript}");
        // The batch's volume writes persist in this tab's world…
        reg.run_line("tab-1", "DIR").unwrap();
        assert!(reg.get("tab-1").unwrap().transcript().contains("WORK          <DIR>"));
        // …and a second tab saw none of it.
        reg.open(TabKind::Dos).unwrap();
        reg.run_line("tab-2", "SET DEMO").unwrap();
        assert!(reg
            .get("tab-2")
            .unwrap()
            .transcript()
            .contains("Environment variable DEMO not defined"));
        reg.run_line("tab-1", "NOPE.TCB").unwrap();
        assert!(reg.get("tab-1").unwrap().transcript().contains("Bad command or file name"));
    }

    /// T6 — the reserved-region line names the focused tab's identity and flavour and
    /// follows focus; no tab content string ever feeds it.
    #[test]
    fn t6_reserved_line_names_identity_and_flavour() {
        let mut reg = TabRegistry::new();
        assert!(reg.reserved_line().contains("no tab focused"));
        reg.open(TabKind::Dos).unwrap();
        reg.open(TabKind::Parity).unwrap();
        assert!(reg.reserved_line().contains("tab-2"));
        assert!(reg.reserved_line().contains("TARGET-PARITY"));
        reg.focus("tab-1").unwrap();
        assert!(reg.reserved_line().contains("tab-1"));
        assert!(reg.reserved_line().contains("[DOS session TAB-1]"));
    }
}
