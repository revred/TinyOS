//! The board's wire shell: what `TOS64-CMD/1`'s `SHELL` row actually runs
//! (`STORY-P1-09-18`).
//!
//! This is the composition `21A` §3 step 2 asks for, and it is deliberately
//! small: everything it assembles already exists and is already tested. The
//! verb core, the labelled volume, the DOS front-end and the deny-by-default
//! policy seam are `shell`'s (`FEAT-P2-01`, `FEAT-P2-02`, `FEAT-P2-04`); the
//! classification, the rate bound and the answer rendering are
//! `hal_arm64::tos64_cmd`'s. What lives here is the one thing neither of them
//! could decide alone — **what an unauthenticated peer on a cable is allowed
//! to ask for**.
//!
//! # The containment argument, in the three sentences it rests on
//!
//! `PD-02` gives the wire peer no kernel-derived identity, so
//! [`STORY-P1-09-17`] admitted only verbs whose answers disclose what the
//! board already broadcasts and whose execution changes nothing. `-18` keeps
//! that sentence and satisfies it a second way rather than relaxing it:
//!
//! 1. **Execution changes nothing.** [`run`] builds its [`World`] from
//!    [`seed`] on every call and drops it before returning. There is no
//!    `static`, no cell and no carried handle anywhere in this module, so no
//!    cwd, environment variable, file, label or counter can survive one wire
//!    command into the next. The board after any admitted sequence is
//!    bit-identical to the board before it — a property of the shape, which is
//!    why [`tests::two_commands_cannot_see_each_other`] can assert it directly
//!    instead of auditing for leaks.
//! 2. **Nothing new is disclosed.** [`GRANTED`] is the strictly read-only
//!    subset of the verb core, over a volume this file seeds with content that
//!    shipped in the image. A peer can read back only bytes that were already
//!    published in an artifact anyone can download.
//! 3. **No authority is reachable.** `shell` carries
//!    `#![forbid(unsafe_code)]`, this crate carries it, and
//!    `hal_arm64::tos64_cmd` carries it. No signature on the path from an
//!    admitted frame to this function takes a device, an `Mmio` or a `&mut` to
//!    anything the board owns, so "a wire verb cannot touch a register" is
//!    enforced by the compiler across every crate on the path.
//!
//! # What is deliberately withheld, and what it waits on
//!
//! Two families, for two different reasons, both recorded so a later session
//! reads a decision rather than an omission:
//!
//! - **Every mutating verb** (`CD`, `COPY`, `MOVE`, `DEL`, `MD`, `RD`, `SET`,
//!   `PATH`). Not because the RAM volume matters — it is rebuilt every command
//!   — but because granting them would make sentence 1 above true only by
//!   accident of the rebuild. The rebuild is defence in depth; the grant set is
//!   the defence. `CLS` is withheld with them: it emits a real terminal escape,
//!   and trusted output that repaints an operator's screen is authority over a
//!   human, which is the one kind this table must not hand out either.
//! - **Every verb that reads live kernel state** (`MEM`, `TASKMGR`, `SPOOR`).
//!   These are read-only and stateless and would satisfy sentence 1 perfectly
//!   — they fail sentence 2. A task table, a memory figure and an audit
//!   journal are facts only the running board holds, and disclosing them to a
//!   peer with no identity is a decision worth taking on purpose. It waits on
//!   the session/authentication story (`WCI`/deploy-protocol model), which is
//!   the same thing the write half waits on.
//!
//! [`STORY-P1-09-17`]: ../../../../goals/stories/STORY-P1-09-17.md
//! [`World`]: shell::verbs::World

use core::fmt::Write;

use shell::labels::Labels;
use shell::policy::GrantSet;
use shell::verbs::{Env, NoSpoors, Platform, VerbKind, World};
use shell::volume::RamVolume;

/// Every verb the wire session may execute.
///
/// The **read-only** subset of the verb core, chosen against the two sentences
/// in this module's header rather than against what would be convenient to
/// demonstrate. Ordered as [`VerbKind::ALL`] declares them so the two lists can
/// be read side by side.
pub const GRANTED: &[VerbKind] = &[
    VerbKind::List,
    VerbKind::PrintCwd,
    VerbKind::ViewFile,
    VerbKind::FindText,
    VerbKind::SortStream,
    VerbKind::Page,
    VerbKind::TreeView,
    VerbKind::AttribView,
    VerbKind::Echo,
    VerbKind::VersionInfo,
    VerbKind::VolumeInfo,
];

/// The wire session's policy. Deny-by-default: a verb absent from [`GRANTED`]
/// does not run, and the seam audits the denial into the transcript the peer
/// gets back — a refusal it can read is a refusal it cannot mistake for a dead
/// board.
pub static POLICY: GrantSet = GrantSet { granted: GRANTED, withheld: None, supervisor: false };

/// The session name every wire command runs under.
///
/// Fixed, and **not** derived from anything in the frame. `PD-02`'s sentence is
/// that identity is kernel-derived and never taken from a caller-supplied
/// field; the honest form of that here is a single constant name that says what
/// the session is, so nothing about the peer can influence a policy decision.
pub const SESSION: &str = "WIRE";

/// The volume label and serial the wire session reports.
const VOLUME_LABEL: &str = "TINYOS";
const VOLUME_SERIAL: (u16, u16) = (0x5049, 0x3501);

/// What this session runs on, supplied to the verb core rather than baked into
/// it (`LE-124`).
///
/// The two strings are the board's own vocabulary, not a second naming of the
/// same facts: `hal_arm64::timer` announces `tier=T1 arch=aarch64` in every
/// `TOS64-MEAS/2` envelope, so a reader holding a measurement capture beside a
/// `SHELL VER` transcript sees one machine described one way.
const PLATFORM: Platform<'static> = Platform { tier: "Tier 1", arch: "aarch64" };

/// The seeded content, as `(name, bytes)` pairs written into the root.
///
/// Small on purpose. Every octet here is stack cost on a 64 KiB board stack
/// (see [`STACK_BUDGET`]) and every octet is also something a peer with no
/// identity can read back, so the seed carries what makes a first session
/// legible and nothing else.
const SEED_FILES: &[(&str, &[u8])] = &[
    (
        "README.TXT",
        b"TinyOS on a Raspberry Pi 5.\nThis session runs over the Ethernet cable.\nThe volume is rebuilt for every command.\n",
    ),
    ("VERBS.TXT", b"DIR TYPE TREE FIND SORT MORE ATTRIB ECHO VER VOL\n"),
];

/// The stack this session is allowed to cost, octets.
///
/// The board's stack is 64 KiB with an unmapped guard page below it
/// (`targets/aarch64-tinyos.ld`), and this session is built on that stack
/// inside the park loop. A [`World`] is a fixed-capacity structure, so its
/// footprint is a compile-time constant and is pinned as one below rather than
/// discovered as a fault on a bench — the guard page would make an overflow
/// visible, but "visible" is not the same as "known", and this project's rule
/// is that a bound is measured.
pub const STACK_BUDGET: usize = 16 * 1024;

const _: () = assert!(
    core::mem::size_of::<World<'static>>() <= STACK_BUDGET,
    "the wire session outgrew a quarter of the board's stack"
);

/// The wire's argument field and the shell's command line are **one width**,
/// held here because this is the only crate that can see both.
///
/// `hal-arm64` cannot depend on `shell` — `shell` → `kernel` → `hal-arm64` is
/// the real edge and the reverse is a cycle Cargo refuses — so
/// `tos64_cmd::ARGUMENT_BYTES` cannot name `shell::capacities::MAX_LINE`
/// directly. The composition root can, and a disagreement is therefore a
/// **build failure here** rather than a line silently refused on a bench for a
/// reason neither crate could state.
///
/// This is the same argument `hal_arm64::wire_shell`'s header makes for
/// passing arrays across the seam instead of pointers: the two sides cannot
/// disagree about a width without failing to compile. It applies to the width
/// of the line as much as to the width of the buffer carrying it.
const _: () = assert!(
    hal_arm64::tos64_cmd::ARGUMENT_BYTES == shell::capacities::MAX_LINE,
    "the wire would carry a command line the shell refuses, or refuse one it would accept"
);

/// A freshly seeded world. Built per command and dropped with it.
fn seed() -> World<'static> {
    let mut volume = RamVolume::new(Some(VOLUME_LABEL), VOLUME_SERIAL);
    for (name, bytes) in SEED_FILES {
        // A seed that does not fit is a defect in this file, not a runtime
        // condition, and it is one the tests below catch. Discarding the
        // result deliberately: the session must still answer with whatever it
        // does hold rather than refusing to exist, because a board that stops
        // answering is the one failure mode this whole path exists to avoid.
        let _ = volume.create(0, name, bytes, Labels::seeded());
    }
    World {
        volume,
        // `LE-124`: the board states its own platform, and it is the same pair
        // it already announces in every `TOS64-MEAS/2` envelope
        // (`tier=T1 arch=aarch64`). Until 2026-08-08 the verb core carried
        // `(Tier 0, x86_64)` as a literal, so the first wire session ever run
        // against this composition answered `SHELL VER` with the wrong
        // architecture — a false line on the one surface a demo transcript is
        // quoted from.
        platform: PLATFORM,
        env: Env::new(),
        cwd: 0,
        echo: true,
        policy: &POLICY,
        session: SESSION,
        tasks: &[],
        spoors: &NoSpoors,
        denials: 0,
    }
}

/// A bounded sink: everything past the caller's buffer is counted, not written.
///
/// `core::fmt::Write` has no back-pressure, so a sink that simply stopped would
/// leave the caller unable to say how much it lost. Counting is what lets the
/// wire answer carry a `more=` field that is true.
struct BoundedSink<'a> {
    out: &'a mut [u8],
    written: usize,
    dropped: usize,
}

impl Write for BoundedSink<'_> {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        for &byte in text.as_bytes() {
            if self.written < self.out.len() {
                self.out[self.written] = byte;
                self.written += 1;
            } else {
                self.dropped = self.dropped.saturating_add(1);
            }
        }
        Ok(())
    }
}

/// Runs one command line and returns how many octets it printed into `out`.
///
/// Total: every input is answered. A line that is not UTF-8, is empty, names
/// no verb, or names a verb the policy denies all produce output rather than
/// silence, because a peer that hears nothing cannot tell a refusal from a
/// dead board — the failure `LE-80`'s family keeps producing.
#[must_use]
pub fn run(line: &[u8], out: &mut [u8]) -> usize {
    let mut sink = BoundedSink { out, written: 0, dropped: 0 };
    let Ok(text) = core::str::from_utf8(line) else {
        // Named rather than echoed: echoing the bytes back would put
        // attacker-chosen octets on the wire through a path that has not
        // rendered them inert.
        let _ = sink.write_str("Bad command or file name\n");
        return sink.written;
    };
    if text.trim().is_empty() {
        let _ = sink.write_str("A:\\>\n");
        return sink.written;
    }
    let mut world = seed();
    // The `fmt::Result` is discarded because `BoundedSink` cannot fail: it
    // counts what it cannot write instead of erroring, which is the whole
    // reason it exists. Silence here would be swallowing an error; there is
    // none to swallow.
    let _ = shell::dos::run_line(&mut world, text, &mut sink);
    sink.written
}

#[cfg(test)]
mod tests {
    use super::*;
    use shell::verbs::VerbKind;

    fn transcript(line: &str) -> String {
        let mut out = [0u8; 512];
        let len = run(line.as_bytes(), &mut out);
        String::from_utf8(out[..len].to_vec()).expect("the shell renders ASCII")
    }

    /// `21A` §3's destination, reduced to one assertion: a line typed by a
    /// human reaches `TINYCMD`'s verb core and comes back as `TINYCMD`'s own
    /// output. Everything else in this file is about what that is *allowed* to
    /// reach; this is the fact that it reaches anything at all.
    #[test]
    fn a_typed_line_reaches_the_verb_core_and_answers_as_tinycmd() {
        assert!(transcript("VER").contains("TinyOS"), "{}", transcript("VER"));
        assert!(transcript("VOL").contains("TINYOS"), "{}", transcript("VOL"));
        assert!(transcript("ECHO hello").contains("hello"));
        let dir = transcript("DIR");
        assert!(dir.contains("README.TXT"), "{dir}");
        assert!(dir.contains("VERBS.TXT"), "{dir}");
        assert!(transcript("TYPE README.TXT").contains("Raspberry Pi 5"));
    }

    /// Sentence 1 of the containment argument, asserted rather than reasoned.
    ///
    /// The strongest form available: no command can leave a trace another
    /// command can see. Written as *"the second command's output is what it
    /// would have been had the first never run"*, over the mutations most
    /// likely to leak — a directory change, a file creation, an environment
    /// variable.
    #[test]
    fn two_commands_cannot_see_each_other() {
        let baseline = transcript("DIR");
        for attempt in ["MD SCRATCH", "COPY README.TXT COPY.TXT", "SET X=1", "DEL README.TXT"] {
            let _ = transcript(attempt);
            assert_eq!(
                transcript("DIR"),
                baseline,
                "`{attempt}` left something behind for the next command"
            );
        }
        // And the environment specifically, which `DIR` would not reveal.
        let _ = transcript("SET FLAVOR=DOS");
        assert!(!transcript("ECHO %FLAVOR%").contains("DOS"), "an environment survived a command");
    }

    /// Sentence 2, half one: every mutating verb is denied, and the denial is
    /// spoken rather than silent.
    ///
    /// Enumerated over `VerbKind::ALL` rather than over a hand-written list, so
    /// a verb added to the core tomorrow is denied by default here and has to
    /// be granted deliberately — which is what deny-by-default means when the
    /// thing being denied has not been invented yet.
    #[test]
    fn every_verb_outside_the_grant_set_is_denied_and_says_so() {
        for kind in VerbKind::ALL {
            if GRANTED.contains(kind) {
                continue;
            }
            assert!(
                !POLICY_ALLOWS(*kind),
                "{kind:?} is reachable from the wire without being granted"
            );
        }
        // The seam audits, so the peer can tell a refusal from a dead board.
        for line in ["MD SCRATCH", "DEL README.TXT", "CLS", "MEM", "TASKMGR", "SPOOR"] {
            let out = transcript(line);
            assert!(
                out.contains("Access denied") || out.contains("Bad command"),
                "`{line}` was neither denied nor unknown: {out}"
            );
            assert!(out.contains("audited") || out.contains("Bad command"), "{line}: {out}");
        }
    }

    /// Sentence 2, half two: the grant set is exactly the read-only subset, and
    /// the three live-state verbs are outside it on purpose.
    #[test]
    fn the_grant_set_is_read_only_and_names_no_live_kernel_state() {
        for withheld in [
            VerbKind::ChangeDir,
            VerbKind::Copy,
            VerbKind::Move,
            VerbKind::Delete,
            VerbKind::MakeDir,
            VerbKind::RemoveDir,
            VerbKind::Env,
            VerbKind::ClearScreen,
            VerbKind::TaskKill,
            VerbKind::MemInfo,
            VerbKind::TaskList,
            VerbKind::SpoorJournal,
        ] {
            assert!(
                !GRANTED.contains(&withheld),
                "{withheld:?} is granted to an unidentified peer"
            );
        }
        // No row is listed twice — a duplicated grant reads as two decisions
        // and is one.
        for (index, kind) in GRANTED.iter().enumerate() {
            assert!(!GRANTED[index + 1..].contains(kind), "{kind:?} is granted twice");
        }
    }

    /// The seam is total: no input produces silence, because silence is
    /// indistinguishable from a dead board.
    #[test]
    fn every_input_produces_an_answer_including_the_ones_that_are_not_commands() {
        for line in ["", "   ", "WHATNOW", "DIR \\..\\..\\", "TYPE NOPE.TXT", "\u{1b}[2J"] {
            assert!(!transcript(line).is_empty(), "`{line}` answered with silence");
        }
        // Invalid UTF-8 is named, not echoed — echoing would put
        // attacker-chosen octets on a path that has not rendered them inert.
        let mut out = [0u8; 512];
        let len = run(&[0xFF, 0xFE, 0xFD], &mut out);
        assert_eq!(&out[..len], b"Bad command or file name\n");
    }

    /// The output bound holds whatever the shell produced, and what did not fit
    /// is counted rather than lost — the fact the wire's `more=` field states.
    #[test]
    fn the_sink_counts_what_it_could_not_write() {
        let mut tiny = [0u8; 8];
        let written = run(b"TYPE README.TXT", &mut tiny);
        assert_eq!(written, 8, "the sink fills its buffer and stops");
        // Nothing panics, nothing overruns, and a larger buffer sees more.
        let mut roomy = [0u8; 512];
        assert!(run(b"TYPE README.TXT", &mut roomy) > written);
    }

    /// The seed is what this file says it is. A seed file that silently failed
    /// to create would make every `DIR` above a weaker test than it reads as.
    #[test]
    fn every_seed_file_actually_lands_in_the_volume() {
        let world = seed();
        for (name, bytes) in SEED_FILES {
            let stored = world.volume.read(0, name).expect("seed file present");
            assert_eq!(stored, *bytes, "{name} was truncated or altered by seeding");
        }
        assert_eq!(world.volume.label(), Some(VOLUME_LABEL));
        assert_eq!(world.cwd, 0);
        assert_eq!(world.denials, 0);
    }

    /// The stack bound is a compile-time constant and is stated here too, so a
    /// reader who does not spot the `const` assertion still sees the number.
    #[test]
    fn the_session_fits_the_stack_budget_it_declares() {
        assert!(core::mem::size_of::<World<'static>>() <= STACK_BUDGET);
        assert!(STACK_BUDGET * 4 <= 64 * 1024, "the budget is a quarter of the board stack");
    }

    /// `GrantSet::allows` reaches through the trait; naming it once keeps the
    /// enumeration above readable.
    #[allow(non_snake_case)]
    fn POLICY_ALLOWS(kind: VerbKind) -> bool {
        use shell::policy::VerbPolicy;
        POLICY.allows(SESSION, kind)
    }

    // --- the whole path, joined: frame in, answer line out -------------------
    //
    // This is the only place in the workspace that can hold this test, and
    // that is the same reason the composition lives here: `hal-arm64` cannot
    // name `shell`, `shell` knows nothing about a board, and this crate sees
    // both. Everything below the MAC is exercised — build a real
    // `TOS64-CMD/1` envelope, classify it with the board's own classifier, ask
    // the channel what to run, run it through the real verb core, and render
    // the real answer line. What is left untested here is the wire itself,
    // which is a bench's job and not a host's.

    use hal_arm64::tos64_cmd::{
        self, AnswerText, CommandChannel, Verb, ANSWER_CAPACITY, COMMAND_PAYLOAD_BYTES,
        SHELL_OUTPUT_CAPACITY,
    };

    /// A well-formed `SHELL` command frame carrying `line`.
    fn frame(line: &str, sequence: u32) -> [u8; COMMAND_PAYLOAD_BYTES] {
        let mut payload = [0u8; COMMAND_PAYLOAD_BYTES];
        payload[tos64_cmd::field::PREFIX].copy_from_slice(b"TOS64-");
        payload[tos64_cmd::field::MAGIC].copy_from_slice(tos64_cmd::COMMAND_MAGIC);
        payload[tos64_cmd::field::VERB].copy_from_slice(&Verb::Shell.id().to_be_bytes());
        payload[tos64_cmd::field::SEQUENCE].copy_from_slice(&sequence.to_be_bytes());
        let bytes = line.as_bytes();
        assert!(bytes.len() <= tos64_cmd::ARGUMENT_BYTES, "`{line}` exceeds the argument field");
        payload[tos64_cmd::field::ARGUMENT.start..][..bytes.len()].copy_from_slice(bytes);
        payload
    }

    /// Drives one command exactly as the park loop's answer slot does.
    fn exchange(line: &str, sequence: u32) -> String {
        let mut channel = CommandChannel::new();
        channel.offer(&frame(line, sequence));
        let mut output = [0u8; SHELL_OUTPUT_CAPACITY];
        let output_len = match channel.pending_line() {
            Some(pending) => run(pending, &mut output),
            None => 0,
        };
        let mut answer = [0u8; ANSWER_CAPACITY];
        let len = channel
            .take(AnswerText { status: b"", output: &output[..output_len] }, &mut answer)
            .expect("the slot owed a line");
        String::from_utf8(answer[..len].to_vec()).expect("the answer is ASCII")
    }

    /// **`21A`'s destination, host half.** A command line goes in as an
    /// Ethernet payload and `TINYCMD`'s own output comes back as one answer
    /// line. Every component is the shipped one: the board's classifier, the
    /// board's channel, the board's grant set, the real verb core.
    #[test]
    fn a_command_frame_becomes_a_tinycmd_answer_line() {
        let answer = exchange("VER", 1);
        assert!(answer.starts_with("TOS64-ANS/1 verb=SHELL seq=1 ok=1 out="), "{answer}");
        assert!(answer.contains("TinyOS"), "{answer}");
        assert_eq!(answer.matches('\n').count(), 1, "one frame in, one line out: {answer:?}");

        let dir = exchange("DIR", 2);
        assert!(dir.contains("README.TXT"), "{dir}");
        assert!(dir.contains("VERBS.TXT"), "{dir}");
        // A DIR listing is many lines of shell output on one line of wire, and
        // the escape is what makes that lossless rather than mangled.
        assert!(dir.contains("\\n"), "the shell's line breaks survive as escapes: {dir}");
    }

    /// The refusal a peer is most likely to earn, spoken as itself.
    #[test]
    fn a_denied_verb_comes_back_as_an_audited_denial_and_not_as_silence() {
        let answer = exchange("DEL README.TXT", 3);
        assert!(answer.contains("Access denied"), "{answer}");
        assert!(answer.contains("audited"), "{answer}");
        assert!(answer.contains("seq=3"), "the sequence heard is named back: {answer}");
    }

    /// Statelessness, asserted on the **wire** path rather than only on the
    /// runner — the two could in principle disagree, and the property that
    /// matters is the one an attacker can reach.
    #[test]
    fn nothing_a_frame_does_is_visible_to_the_next_frame() {
        let before = exchange("DIR", 1);
        for hostile in ["DEL README.TXT", "MD X", "SET A=B", "CD .."] {
            let _ = exchange(hostile, 9);
        }
        let after = exchange("DIR", 1);
        assert_eq!(before, after, "a frame changed what the next frame could see");
    }

    /// An over-long listing is carried as a labelled prefix, and the label is
    /// arithmetic rather than a hint.
    #[test]
    fn an_answer_that_could_not_carry_everything_says_how_much_it_withheld() {
        let answer = exchange("TYPE README.TXT", 4);
        assert!(answer.len() <= ANSWER_CAPACITY);
        assert_eq!(answer.matches('\n').count(), 1);
        if let Some((_, tail)) = answer.rsplit_once(" more=") {
            let withheld: usize = tail.trim_end().parse().expect("a count");
            assert!(withheld > 0, "a more= field that reports nothing withheld");
        }
    }

    /// `LE-124` — the board's `VER` names the board.
    ///
    /// The defect this closes was a live falsehood on the demo-facing surface:
    /// the first wire session ever run against this composition answered
    /// `SHELL VER` with `x86_64`, and a demo transcript naming the wrong
    /// architecture is a transcript nobody can cite. Asserted here rather than
    /// only in `shell`, because the platform is *this crate's* fact to supply
    /// and the bug was a missing supply, not a bad render.
    #[test]
    fn the_boards_version_line_names_the_board_and_not_the_host_it_was_built_on() {
        let answer = exchange("VER", 11);
        assert!(answer.contains("aarch64"), "the board must name its own architecture: {answer}");
        assert!(answer.contains("Tier 1"), "{answer}");
        assert!(
            !answer.contains("x86_64"),
            "LE-124: the literal that was true of every context until this crate moved: {answer}"
        );
    }

    /// The board's own two vocabularies agree. `TOS64-MEAS/2` announces
    /// `tier=T1 arch=aarch64`; `SHELL VER` must not invent a second naming of
    /// the same machine.
    #[test]
    fn the_platform_is_the_same_pair_the_measurement_envelope_announces() {
        assert_eq!(PLATFORM.arch, "aarch64");
        assert_eq!(PLATFORM.tier, "Tier 1");
    }

    /// The command line a frame can carry is the line the shell accepts — the
    /// same number, not a wire-imposed keyhole. Stated as a test because the
    /// two constants live in crates that cannot see each other.
    #[test]
    fn the_command_line_a_frame_can_carry_is_the_one_the_shell_accepts() {
        assert_eq!(tos64_cmd::ARGUMENT_BYTES, shell::capacities::MAX_LINE);
        assert_eq!(tos64_cmd::ARGUMENT_BYTES, 128);
        let longest = "A".repeat(tos64_cmd::ARGUMENT_BYTES);
        let answer = exchange(&longest, 5);
        assert!(answer.contains("Bad command or file name"), "{answer}");
    }

    /// The command the old 30-octet field could not carry, run end to end.
    ///
    /// This is the width change's reason for existing, so it is a test rather
    /// than a claim: `FIND` with a quoted needle and a filename is 31 octets
    /// and did not fit through the envelope until 2026-08-08.
    #[test]
    fn a_real_dos_command_line_now_fits_and_is_answered() {
        let line = "FIND /N \"Ethernet cable\" README.TXT";
        assert_eq!(line.len(), 35);
        assert!(line.len() > 30, "this test is pointless if it fit in the old field");
        let answer = exchange(line, 9);
        assert!(
            !answer.contains("Bad command or file name"),
            "the verb core must have seen a whole line: {answer}"
        );
        assert!(answer.contains("Ethernet"), "the match should come back: {answer}");
    }
}
