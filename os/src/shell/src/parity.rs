//! The parity harness (`TEST-P2-07-01-A`): one seeded world, one `.TCB`, one expected
//! transcript — shared verbatim by the host golden test and the QEMU fixture, so the two
//! can only drift by failing.

use crate::labels::Labels;
use crate::policy::GrantSet;
use crate::verbs::{Env, TaskInfo, VerbKind, World};
use crate::volume::RamVolume;

/// Every verb the parity session is granted — the MVP set minus [`WITHHELD`].
pub const GRANTED: &[VerbKind] = &[
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
    VerbKind::VersionInfo,
    VerbKind::VolumeInfo,
    VerbKind::MemInfo,
    VerbKind::TaskList,
];

/// Deliberately withheld (STORY-P2-07-01 acceptance 3): the batch tries it, the seam
/// denies it, the transcript shows it, the batch continues.
pub const WITHHELD: VerbKind = VerbKind::ClearScreen;

/// The parity policy.
pub static POLICY: GrantSet =
    GrantSet { granted: GRANTED, withheld: Some(WITHHELD), supervisor: false };

/// The injected task table (three deterministic rows).
pub static TASKS: &[TaskInfo] = &[
    TaskInfo { name: "RT-CTRL", priority: 0, state: "ready" },
    TaskInfo { name: "SPOOR", priority: 3, state: "waiting" },
    TaskInfo { name: "IDLE", priority: 9, state: "ready" },
];

/// The parity `.TCB` — the owner's "MS-DOS test as a batch file". Exercises the
/// STORY-P2-07-01 acceptance-1 verb list, including one unknown command and the
/// withheld verb.
pub const SCRIPT: &str = "\
@ECHO OFF
VER
VOL
ECHO
ECHO Hello from TINYCMD
SET FLAVOR=DOS
SET FLAVOR
PATH \\;
PATH
MD DOCS
CD DOCS
CD
COPY \\README.TXT NOTES.TXT
ATTRIB NOTES.TXT
REN NOTES.TXT KEEP.TXT
ATTRIB KEEP.TXT
CD ..
DIR
TYPE README.TXT
FIND /C \"TinyOS\" README.TXT
FIND /N \"soul\" README.TXT
SORT /R LIST.TXT
TREE /A /F
MEM
TASKMGR
CLS
DEL /Y DOCS\\KEEP.TXT
RD DOCS
DIR
WHATNOW
ECHO %FLAVOR% batch complete
";

/// Build the seeded parity world. Deterministic by construction: fixed serial, fixed
/// seed files, fixed task table, fixed timestamps in the renderer.
pub fn world() -> World<'static> {
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
    World {
        volume,
        env: Env::new(),
        cwd: 0,
        echo: true,
        policy: &POLICY,
        session: "PARITY",
        tasks: TASKS,
        denials: 0,
    }
}

/// The stats the run must produce: exactly one denial (the withheld verb), no
/// truncation. Executed-line count is asserted against the script itself.
pub fn expected_denials() -> u32 {
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::batch;

    /// P1 — the golden transcript (STORY-P2-07-01 acceptance 1/2/4, host half).
    /// Byte-compares the batch run against the committed golden file; on divergence
    /// prints the actual transcript so the reviewer diffs it against the golden in the
    /// tree. The QEMU fixture runs the *same* world and script on the target.
    #[test]
    fn p1_transcript_matches_golden() {
        let golden = include_str!("../golden/parity-smoke.golden.txt");
        let mut world = world();
        let mut transcript = String::new();
        let stats = batch::run(&mut world, SCRIPT, &mut transcript).unwrap();
        assert_eq!(stats.denials, expected_denials(), "exactly the withheld verb denies");
        assert!(!stats.truncated);
        if transcript != golden {
            let divergence = transcript
                .lines()
                .zip(golden.lines())
                .position(|(a, b)| a != b)
                .unwrap_or_else(|| transcript.lines().count().min(golden.lines().count()));
            panic!(
                "transcript diverges from golden at line {divergence}.\n--- actual transcript ---\n{transcript}\n--- end ---"
            );
        }
    }

    /// The golden recorder — deliberately `#[ignore]`d, the LE-23 division of labour:
    /// running it is an act somebody chooses (`cargo test -p shell -- --ignored
    /// regenerate_golden`), and the artifact it rewrites is reviewed as a diff and
    /// committed by hand. CI never runs it.
    #[test]
    #[ignore = "rewrites golden/parity-smoke.golden.txt; run deliberately and review the diff"]
    fn regenerate_golden() {
        let mut world = world();
        let mut transcript = String::new();
        batch::run(&mut world, SCRIPT, &mut transcript).unwrap();
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/golden/parity-smoke.golden.txt");
        std::fs::write(path, &transcript).expect("write golden");
    }

    /// P2 — determinism: two runs are byte-identical (acceptance 4, host half).
    #[test]
    fn p2_two_runs_identical() {
        let run = || {
            let mut w = world();
            let mut out = String::new();
            batch::run(&mut w, SCRIPT, &mut out).unwrap();
            out
        };
        assert_eq!(run(), run());
    }
}
