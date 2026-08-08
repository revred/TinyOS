//! `TOS64-CMD/1` — the admitted verb: the moment a received frame first means
//! something (`STORY-P1-09-17`, `TEST-P1-09-17-A`).
//!
//! [`crate::gem_receive`] made the board reachable and forbade the reach from
//! meaning anything — *"no value from a frame selects a branch, an address, an
//! offset or a size anywhere in the image"*. That absence was the whole of
//! `STORY-P1-09-16`'s containment argument, and its own text said the argument
//! expires the moment a frame is allowed to mean something, and must then be
//! **re-made rather than cited**. `STORY-P1-09-17` is that re-making; this
//! module is what it governs.
//!
//! What the re-made argument rests on, each of which is a test below:
//!
//! 1. **The envelope is fixed-width and the classifier reads fixed offsets.**
//!    No length inside the frame is believed, no value from the frame is used
//!    as an offset or an address, and the payload is required to be *exactly*
//!    [`COMMAND_PAYLOAD_BYTES`] — which is chosen so the whole frame is the
//!    Ethernet minimum and a sending NIC's padding cannot exist.
//! 2. **One input-derived selection exists and it is bounded by the table.**
//!    The verb id chooses a row of [`VERB_TABLE`] through [`resolve`], which
//!    walks the table's own length. Every one of the 65,536 possible ids is
//!    exercised by a test; nothing indexes.
//! 3. **The table denies by default and its rows own nothing.** The wire peer
//!    has no kernel-derived identity (`PD-02`), so a verb that changed state
//!    or disclosed anything new would be authority granted to an
//!    unauthenticated caller. `-17` satisfied that with two rows that execute
//!    nothing at all, and recorded that a third row would be a charter re-read
//!    rather than an addition.
//!
//!    **`STORY-P1-09-18` performed that re-read and added the third row —
//!    `SHELL` — without weakening the sentence.** It is satisfied a second
//!    way, by construction rather than by abstinence:
//!
//!    - *Execution changes nothing*, in the strongest sense available: the
//!      runner builds its `World` fresh from a `const` seed for **every**
//!      command and drops it when the answer is rendered. No cwd, no
//!      environment, no file, no counter survives one wire command into the
//!      next, so the board after any admitted sequence is bit-identical to
//!      the board before it. Statelessness is a property of the shape, not a
//!      discipline someone has to keep.
//!    - *Discloses nothing new*: the grant set is the **read-only** subset of
//!      `TINYCMD`'s verb core over a volume the board image itself seeded, so
//!      the only bytes a peer can read back are bytes that shipped in a
//!      published image. Every mutating verb is denied, and so is every verb
//!      that reads live kernel state (`MEM`, `TASKMGR`, `SPOOR`) — those are a
//!      separate decision and they wait on one.
//!    - *Owns nothing*: this module still executes nothing. It classifies,
//!      reports the line through [`CommandChannel::pending_line`], and renders
//!      what the caller hands back. The runner behind the seam is `shell`,
//!      which carries `#![forbid(unsafe_code)]` exactly as this module does,
//!      so "a row cannot reach a register" is still enforced by the compiler
//!      — now across two crates instead of one.
//! 4. **Every refusal is spoken, by name, on the wire.** A refused command
//!    that vanishes is indistinguishable from a dead board, which is the
//!    diagnosis failure `LE-80`'s family keeps producing.
//! 5. **The answer rate is bounded and fails safe.** At most one line leaves
//!    the board per park beat; excess is counted and refused as over-rate.
//!    An unauthenticated, broadcast-capable peer must not be able to make the
//!    board an amplifier (`SEC-20`).
//!
//! The module forbids `unsafe`, and that is load-bearing rather than tidy:
//! every register on this board is reached through an `unsafe` volatile
//! access, so a verb handler in here **cannot** write one. `TEST-P1-09-17-A`
//! clause 2's "a row that gains authority is a red test" is enforced by the
//! compiler, and the signatures below carry no device, no `Mmio` and no
//! `&mut` to anything the board owns.
#![forbid(unsafe_code)]

use crate::gem_receive::ENVELOPE_PREFIX;

/// The four octets that follow the `TOS64-` envelope tag and mark a payload as
/// a command rather than one of the board's own outbound text lines.
///
/// ASCII on purpose: a capture read by eye should say what it is looking at
/// without a decoder, and the board's own frames are all human-readable.
pub const COMMAND_MAGIC: &[u8; 4] = b"CMD1";

/// Width of the fixed argument field, octets. Compared and echoed at fixed
/// width; never dereferenced, never a length, never an offset.
///
/// **128 since 2026-08-08, and the number is derived from the runner rather
/// than from the wire.** `shell::capacities::MAX_LINE` is 128 and
/// `shell::dos` refuses a command line longer than it, so 128 is the widest
/// line the thing on the far side of this field can accept — a wider argument
/// field could only carry lines the shell would reject, and a narrower one
/// (this was 30) hands a 128-capable runner a keyhole. The composition root
/// holds the two constants against each other at compile time, because
/// `hal-arm64` cannot depend on `shell` without a cycle; see
/// `pi5_image::wire_shell`.
///
/// 30 was never chosen for the argument. It was whatever remained of a
/// 46-octet envelope after a 16-octet header, and the envelope's width was
/// itself over-constrained — see [`COMMAND_PAYLOAD_BYTES`].
pub const ARGUMENT_BYTES: usize = 128;

/// The command payload's exact length, octets: the 16-octet header plus
/// [`ARGUMENT_BYTES`].
///
/// # The padding argument, with the quantifier it always needed
///
/// Every Ethernet NIC pads a short frame **up to** 60 octets below any
/// software that could be told not to, so an envelope whose whole frame is
/// *under* the minimum arrives carrying padding no receiver can distinguish
/// from a wrong-width field. That is what makes "exactly this many octets" a
/// refusal rather than a hope, and it is unchanged.
///
/// What changed on 2026-08-08 is the quantifier. This constant was 46 —
/// chosen so `14 + 46` is **exactly** [`crate::gem::MINIMUM_FRAME_LEN`] — but
/// padding immunity requires the frame to be **at least** the minimum, never
/// equal to it. A NIC pads up; it never pads a 158-octet frame. So every
/// width from 46 upward carries the identical guarantee, and fixing the
/// envelope at the floor bought nothing while costing the argument field 98
/// octets. The board already transmits far wider frames on exactly this
/// reasoning — [`crate::gem::text_frame`] pads *up to* the minimum and
/// [`crate::gem::TEXT_FRAME_CAPACITY`] is `14 + 256`.
///
/// The refusal is untouched by the widening: [`classify`] still refuses any
/// payload that is not this width, to the octet, and
/// [`ADMITTED_CAPACITY`]'s headroom still makes
/// [`CommandRefusal::Oversize`] reachable *and* able to name how far over.
///
/// `LE-122`'s row said the width "should not move to 50 **to paper over
/// this**" — declining to disguise a defect. Moving it deliberately, for a
/// reason stated where the old reason lived, is the opposite act.
pub const COMMAND_PAYLOAD_BYTES: usize = HEADER_BYTES + ARGUMENT_BYTES;

/// The fixed header ahead of the argument: prefix, magic, verb, sequence.
/// Named so [`COMMAND_PAYLOAD_BYTES`] is arithmetic over the field layout
/// rather than a literal that has to be re-derived when a field moves.
pub const HEADER_BYTES: usize = 16;

/// How many octets the board copies out of the receive region before handing
/// the descriptor back — one more than the envelope, on purpose.
///
/// The extra octets are what keep [`CommandRefusal::Oversize`] reachable. A
/// buffer sized exactly to the envelope would truncate every over-long frame
/// into a perfectly well-formed command, so the refusal would exist in this
/// file and be unreachable on the board: the classifier would answer a frame
/// it is specified to refuse, and no test on the wire would ever say so.
///
/// **Sixteen spare rather than one, since 2026-08-08 (`LE-122`).** One spare
/// octet made the refusal reachable but made its *width* unknowable: the
/// first real console exchange reported `lastlen=47` for a 46-octet envelope,
/// and 47 was simply the cap, so the board could say "too wide" and not by
/// how much. A copy-out bound that saturates at one past the limit measures
/// nothing beyond it, and this project's own rule is that a refusal names
/// what it refused. Sixteen is enough to carry any plausible framing surplus
/// — an FCS is four — while staying far inside the region's own bound.
pub const ADMITTED_CAPACITY: usize = COMMAND_PAYLOAD_BYTES + 16;

/// The fixed offsets. Every read in this module goes through one of these
/// ranges and there is no other addressing of the payload anywhere.
pub mod field {
    use core::ops::Range;

    /// `TOS64-`, the envelope tag `crate::gem_receive::admit` already checked.
    pub const PREFIX: Range<usize> = 0..6;
    /// [`super::COMMAND_MAGIC`].
    pub const MAGIC: Range<usize> = 6..10;
    /// The verb id, big-endian.
    pub const VERB: Range<usize> = 10..12;
    /// The sequence the sender chose, big-endian. Echoed, never interpreted.
    pub const SEQUENCE: Range<usize> = 12..16;
    /// The fixed-width argument.
    pub const ARGUMENT: Range<usize> = 16..super::COMMAND_PAYLOAD_BYTES;
}

/// A row of the deny-by-default table.
///
/// A data enum with no behaviour and no handler, deliberately. There is
/// nowhere for a row to keep a capability, a register or a piece of state,
/// because the answer for every row is rendered by [`render`] from the verb,
/// the sequence heard, and the texts a *caller* supplies in [`AnswerText`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    /// The answer names the sequence heard. Discloses nothing the board did
    /// not just receive.
    Ping,
    /// The answer replays the transcript's own boot verdict line — already
    /// public on the wire every transcript cycle.
    Status,
    /// `STORY-P1-09-18`: the argument field is handed to `TINYCMD`'s verb core
    /// as one command line and the answer carries what it printed.
    ///
    /// This row is why `-17`'s absence argument expired. It is also the row
    /// that reaches the least: the runner behind it is stateless, holds no
    /// device, and is granted a read-only subset of the verb core over a
    /// volume the board seeded — see the module header's clause 3.
    Shell,
}

impl Verb {
    /// The wire id. Never zero: an all-zero payload is the single most likely
    /// accident on a wire and must resolve to `UnknownVerb`, not to a row.
    pub const fn id(self) -> u16 {
        match self {
            Verb::Ping => 1,
            Verb::Status => 2,
            Verb::Shell => 3,
        }
    }

    /// The name a capture and the operator's console both print.
    pub const fn name(self) -> &'static str {
        match self {
            Verb::Ping => "PING",
            Verb::Status => "STATUS",
            Verb::Shell => "SHELL",
        }
    }

    /// Whether this row's answer needs the caller to run something first.
    ///
    /// Exactly one row does, and saying so as a property of the row rather
    /// than as a `match` at the call site is what keeps the park loop from
    /// growing a second place that knows which verbs mean what.
    pub const fn needs_a_runner(self) -> bool {
        matches!(self, Verb::Shell)
    }
}

/// The whole table. Three rows: two answer-only, one that runs a stateless
/// read-only shell over a board-seeded volume.
///
/// `PD-02`: the peer on this cable has no kernel-derived identity, so a row
/// may exist only where its answer discloses what the board is willing to
/// broadcast and its execution leaves the board unchanged. `-17` satisfied
/// that by having its two rows execute nothing at all, and said a third row
/// would be a charter re-read rather than an addition.
///
/// **`STORY-P1-09-18` is that re-read, and it did not weaken the sentence —
/// it satisfied it a second way.** The `SHELL` row executes, but it executes
/// against a `World` built fresh from a `const` seed for every single command
/// and dropped when the answer is rendered. Nothing carries from one wire
/// command to the next, so *"execution changes nothing"* is true in the
/// strongest available sense: the board after any sequence of admitted
/// commands is bit-identical to the board before it. The grant set is the
/// read-only subset of the verb core, so the claim does not rest on the
/// freshness alone.
pub const VERB_TABLE: [Verb; 3] = [Verb::Ping, Verb::Status, Verb::Shell];

/// Resolves a verb id through the table, deny-by-default.
///
/// The one input-derived selection this Story introduces, and it is a walk of
/// the table's own length rather than an index — so it is exactly as wide as
/// the table and cannot be steered past it.
pub const fn resolve(id: u16) -> Option<Verb> {
    let mut at = 0;
    while at < VERB_TABLE.len() {
        let verb = VERB_TABLE[at];
        if verb.id() == id {
            return Some(verb);
        }
        at += 1;
    }
    None
}

/// Why a command was not answered. Each arm is spoken on the wire by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRefusal {
    /// The envelope tag or the command magic is not ours.
    WrongMagic,
    /// Shorter than the fixed envelope.
    Undersize,
    /// Longer than the fixed envelope.
    Oversize,
    /// A well-formed envelope naming a verb the table does not hold.
    UnknownVerb,
    /// The answer slot was already owed a line. Raised by
    /// [`CommandChannel`], never by [`classify`] — it is a property of the
    /// board's own bounded rate, not of the bytes.
    OverRate,
}

impl CommandRefusal {
    /// The wire name. This vocabulary is held to parity with Ti64Dink's by a
    /// test that reads this file (`LE-80`'s discipline, applied from day one
    /// rather than after the drift).
    pub const fn name(self) -> &'static str {
        match self {
            CommandRefusal::WrongMagic => "wrong-magic",
            CommandRefusal::Undersize => "undersize",
            CommandRefusal::Oversize => "oversize",
            CommandRefusal::UnknownVerb => "unknown-verb",
            CommandRefusal::OverRate => "over-rate",
        }
    }
}

/// A well-formed command: which row, the sequence to name back, and the fixed
/// argument field exactly as it arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    /// The row the id resolved to.
    pub verb: Verb,
    /// The sequence the sender chose. Echoed; never interpreted.
    pub sequence: u32,
    /// The argument field, copied at its fixed width.
    ///
    /// A **copy**, not a borrow, and a fixed-size array rather than a slice
    /// and a length. The classifier reads exactly [`ARGUMENT_BYTES`] octets
    /// from a fixed offset and there is nowhere for a length taken from the
    /// frame to be stored, let alone believed.
    pub argument: [u8; ARGUMENT_BYTES],
}

impl Command {
    /// A command with an empty argument — the two rows that have no use for
    /// one, and the shape most tests want.
    pub const fn bare(verb: Verb, sequence: u32) -> Self {
        Command { verb, sequence, argument: [0; ARGUMENT_BYTES] }
    }

    /// The command line this row hands its runner, or [`None`] for a row that
    /// has no runner.
    ///
    /// The line is the fixed field with its padding trimmed. Both fillers a
    /// sender can plausibly produce — spaces from a human-written console,
    /// NULs from a zeroed buffer — are padding, because a classifier that told
    /// them apart would make one command mean two things depending on who
    /// sent it. Trailing only: an argument is left-justified in its field by
    /// construction, and trimming from the front would let padding choose
    /// where a line starts.
    pub fn line(&self) -> Option<&[u8]> {
        if !self.verb.needs_a_runner() {
            return None;
        }
        let mut end = self.argument.len();
        while end > 0 && matches!(self.argument[end - 1], 0 | b' ') {
            end -= 1;
        }
        Some(&self.argument[..end])
    }
}

/// Classifies one admitted payload — total, over fixed offsets, with no
/// preconditions on the caller.
///
/// The prefix is re-checked here even though `crate::gem_receive::admit`
/// already compared it. A total function with a documented precondition is a
/// function whose safety depends on a caller nobody re-reads.
pub fn classify(payload: &[u8]) -> Result<Command, CommandRefusal> {
    if payload.len() < COMMAND_PAYLOAD_BYTES {
        return Err(CommandRefusal::Undersize);
    }
    if payload.len() > COMMAND_PAYLOAD_BYTES {
        return Err(CommandRefusal::Oversize);
    }
    if &payload[field::PREFIX] != ENVELOPE_PREFIX || &payload[field::MAGIC] != COMMAND_MAGIC {
        return Err(CommandRefusal::WrongMagic);
    }
    let id = u16::from_be_bytes([payload[field::VERB.start], payload[field::VERB.start + 1]]);
    let Some(verb) = resolve(id) else {
        return Err(CommandRefusal::UnknownVerb);
    };
    let sequence = u32::from_be_bytes([
        payload[field::SEQUENCE.start],
        payload[field::SEQUENCE.start + 1],
        payload[field::SEQUENCE.start + 2],
        payload[field::SEQUENCE.start + 3],
    ]);
    let mut argument = [0u8; ARGUMENT_BYTES];
    argument.copy_from_slice(&payload[field::ARGUMENT]);
    Ok(Command { verb, sequence, argument })
}

/// One line the board owes the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spoken {
    /// A row of the table answered.
    ///
    /// Carries the whole classified command rather than just the verb and the
    /// sequence, because the `SHELL` row's answer needs its argument handed to
    /// a runner before the line can be rendered. Keeping it in the one pending
    /// value is what stops the channel growing a second field that has to be
    /// held in step with this one — an invariant nobody re-reads is an
    /// invariant that breaks.
    Answer(Command),
    /// A refusal, named.
    Refused {
        /// Which refusal.
        refusal: CommandRefusal,
        /// The sequence heard, where the frame was readable enough to carry
        /// one; zero otherwise.
        sequence: u32,
        /// How many commands the bounded slot dropped. Meaningful only for
        /// [`CommandRefusal::OverRate`].
        dropped: u32,
    },
}

/// The longest answer line this module will build, octets.
///
/// **Raised from 128 to 256 by `STORY-P1-09-18`**, which is the whole of the
/// concession the `SHELL` row extracted from the wire format. It is bounded by
/// the text channel that carries it — [`crate::gem::TEXT_FRAME_CAPACITY`] is
/// `14 + 256`, so this is the largest line the existing frame builder can
/// carry and the const assertion below it is now tight rather than slack.
/// Nothing about the *rate* moved: the answer is still one line per park beat.
pub const ANSWER_CAPACITY: usize = 256;

/// The widest output this module will accept from a command runner, octets.
///
/// Deliberately **larger** than any single answer line can carry. The runner
/// is a shell and a shell's output is not bounded by what a frame holds; a
/// buffer sized to what fits would make the overflow invisible at the seam
/// that is supposed to measure it, which is the `ADMITTED_CAPACITY` lesson
/// (`LE-122`) one layer up. The excess is counted and named on the wire, never
/// silently dropped and never continued into a second frame.
pub const SHELL_OUTPUT_CAPACITY: usize = 256;

/// Everything a row's answer may say that this module did not compute itself.
///
/// One struct rather than a growing parameter list, and it is the module's
/// containment shape written as a type: **every byte an answer can disclose
/// arrives through here, from a caller.** There is no device, no register and
/// no state on this path, so a reviewer checking "what can a row leak?" reads
/// two fields instead of auditing a call graph.
#[derive(Debug, Clone, Copy)]
pub struct AnswerText<'a> {
    /// The boot verdict line the `STATUS` row replays — already public on the
    /// wire every transcript cycle.
    pub status: &'a [u8],
    /// What the caller's command runner printed for the `SHELL` row. Bounded
    /// by [`SHELL_OUTPUT_CAPACITY`] at the seam that produced it.
    pub output: &'a [u8],
}

/// The widest ` more=N` field, octets: the tag plus every digit of a `u32`.
///
/// Reserved before the output is written rather than appended after it, so the
/// field that reports what did not fit can never itself be the thing that does
/// not fit. An accounting field that a long output can push off the end is an
/// accounting field that is absent exactly when it matters.
const MORE_FIELD_MAX: usize = b" more=".len() + 10;

/// Renders one owed line. Pure: the verb, the sequence heard, and the texts the
/// caller supplies. No device, no state, nothing this module owns.
///
/// The two texts are treated **differently on purpose**, and the difference is
/// a decision rather than an inconsistency:
///
/// - A `status` that will not fit is dropped **whole** and named `none`. It is
///   a replay of a verdict, and a truncated verdict is a fabrication with a
///   plausible shape — worse than an absence, because nothing marks it.
/// - A shell `output` that will not fit is carried as a **prefix** with the
///   withheld octets counted in a ` more=` field. It is a stream, and a
///   labelled prefix is a true statement about the beginning of one. The label
///   is what separates the two cases: an unlabelled prefix would be the same
///   forgery the `status` rule refuses.
pub fn render(spoken: Spoken, text: AnswerText<'_>, out: &mut [u8]) -> usize {
    let mut writer = Writer::new(out);
    writer.put(b"TOS64-ANS/1 ");
    match spoken {
        Spoken::Answer(command) => {
            writer.put(b"verb=");
            writer.put(command.verb.name().as_bytes());
            writer.put(b" seq=");
            writer.put_u32(command.sequence);
            writer.put(b" ok=1");
            match command.verb {
                Verb::Ping => {}
                Verb::Status => {
                    writer.put(b" status=");
                    if text.status.is_empty() || writer.remaining() < text.status.len() + 1 {
                        writer.put(b"none");
                    } else {
                        writer.put(text.status);
                    }
                }
                Verb::Shell => {
                    writer.put(b" out=");
                    // The `+ 1` is the line terminator, which is owed
                    // unconditionally and is reserved with the same reasoning.
                    let room = writer.remaining().saturating_sub(MORE_FIELD_MAX + 1);
                    let carried = fitting_prefix(text.output, room);
                    if carried == 0 {
                        writer.put(b"none");
                    } else {
                        writer.put_escaped(&text.output[..carried]);
                    }
                    let withheld = text.output.len() - carried;
                    if withheld > 0 {
                        writer.put(b" more=");
                        writer.put_u32(withheld as u32);
                    }
                }
            }
        }
        Spoken::Refused { refusal, sequence, dropped } => {
            writer.put(b"refused=");
            writer.put(refusal.name().as_bytes());
            if refusal == CommandRefusal::OverRate {
                writer.put(b" dropped=");
                writer.put_u32(dropped);
            } else {
                writer.put(b" seq=");
                writer.put_u32(sequence);
            }
        }
    }
    writer.put(b"\n");
    writer.written()
}

/// The escaped width of one output octet on the wire.
///
/// Three classes and no others, so the function is total over `u8`:
///
/// - `\` and LF are **structure**, and are escaped reversibly — LF because the
///   answer is one line and a raw LF would end it early, `\` because it is the
///   escape character and an unescaped one would make the decoding ambiguous.
/// - Everything else outside printable ASCII becomes `?`. Lossy, and
///   deliberately so: this is the wire's own fence, standing behind the
///   shell's `write_inert` rendering of attacker-influenced names
///   (`EPIC-P2` §6.5 rule 3) rather than in place of it. The difference is
///   that this one is total over every octet a runner can return, whatever
///   produced it.
const fn escaped_width(byte: u8) -> usize {
    match byte {
        b'\n' | b'\\' => 2,
        _ => 1,
    }
}

/// How many octets of `output` can be carried in `room` escaped octets.
///
/// Counted before anything is written rather than discovered by overrunning a
/// buffer, because the answer must state how much it withheld and a writer
/// that has already silently dropped bytes cannot say.
fn fitting_prefix(output: &[u8], room: usize) -> usize {
    let mut used = 0;
    let mut carried = 0;
    while carried < output.len() {
        let width = escaped_width(output[carried]);
        if used + width > room {
            break;
        }
        used += width;
        carried += 1;
    }
    carried
}

/// The bounded answer slot: one line per park beat, and a saturating count of
/// what the bound turned away.
///
/// The rate bound is the containment for amplification (`SEC-20`), so it is a
/// structural property rather than a tuning: there is exactly one pending slot
/// and one counter, so no arrival pattern can make this channel owe more than
/// one line per beat or hold more than one line's worth of state.
#[derive(Debug, Clone, Copy)]
pub struct CommandChannel {
    pending: Option<Spoken>,
    dropped: u32,
    answered: u32,
    refused: u32,
    last: Option<Verb>,
}

impl Default for CommandChannel {
    fn default() -> Self {
        CommandChannel::new()
    }
}

impl CommandChannel {
    /// A channel that owes nothing and has heard nothing.
    pub const fn new() -> Self {
        CommandChannel { pending: None, dropped: 0, answered: 0, refused: 0, last: None }
    }

    /// Offers one admitted payload. Decides; **transmits nothing**.
    ///
    /// The only producer of bytes on this path is [`take`](Self::take), which
    /// the park loop calls once per beat. Splitting the two is what makes "no
    /// path transmits in response to a frame outside the bounded answer slot"
    /// a shape rather than a promise.
    ///
    /// A command arriving while the slot is full is not lost silently: it is
    /// counted, and the count is spoken as an over-rate refusal on the wire.
    ///
    /// **An owed confession outranks a new command.** While `dropped` is
    /// non-zero the channel takes nothing new, so the over-rate refusal always
    /// reaches its slot. Without that precedence a sustained flood refills the
    /// pending slot on every beat and the count is never spoken — the drops
    /// vanish, which is precisely the "a refused command that vanishes is
    /// indistinguishable from a dead board" failure this Story forbids, and it
    /// is a failure a flood would produce and a single command never would.
    pub fn offer(&mut self, payload: &[u8]) {
        if self.pending.is_some() || self.dropped > 0 {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        self.pending = Some(match classify(payload) {
            Ok(command) => Spoken::Answer(command),
            Err(refusal) => Spoken::Refused { refusal, sequence: sequence_of(payload), dropped: 0 },
        });
    }

    /// The command line the pending answer needs run, if any.
    ///
    /// The whole of the runner seam, and it is deliberately this small: the
    /// channel **reports** and the caller **runs**. This module classifies and
    /// renders and does neither, which is why it can still carry
    /// `#![forbid(unsafe_code)]` and why no signature in it takes a device, an
    /// `Mmio` or a `&mut` to anything the board owns — `-17`'s clause 2, held
    /// unchanged across a row that executes.
    ///
    /// Returns [`None`] for a refusal (nothing reached a row), for the two
    /// answer-only rows, and for an empty slot.
    pub fn pending_line(&self) -> Option<&[u8]> {
        match &self.pending {
            Some(Spoken::Answer(command)) => command.line(),
            _ => None,
        }
    }

    /// The answer slot. Renders at most one line per call, or [`None`] when
    /// nothing is owed — an empty slot transmits nothing rather than filling
    /// the wire with silence that looks like data.
    ///
    /// Counters move here and not in [`offer`](Self::offer), so the canvas row
    /// says what actually left the board.
    pub fn take(&mut self, text: AnswerText<'_>, out: &mut [u8]) -> Option<usize> {
        if let Some(spoken) = self.pending.take() {
            match spoken {
                Spoken::Answer(command) => {
                    self.answered = self.answered.saturating_add(1);
                    self.last = Some(command.verb);
                }
                Spoken::Refused { .. } => self.refused = self.refused.saturating_add(1),
            }
            return Some(render(spoken, text, out));
        }
        if self.dropped > 0 {
            let dropped = self.dropped;
            self.dropped = 0;
            self.refused = self.refused.saturating_add(1);
            return Some(render(
                Spoken::Refused { refusal: CommandRefusal::OverRate, sequence: 0, dropped },
                text,
                out,
            ));
        }
        None
    }

    /// Answers that have actually left the board.
    pub const fn answered(&self) -> u32 {
        self.answered
    }

    /// Refusals that have actually been spoken on the wire.
    pub const fn refused(&self) -> u32 {
        self.refused
    }

    /// The last row that answered, for the canvas.
    pub const fn last(&self) -> Option<Verb> {
        self.last
    }
}

/// The sequence field of a payload long enough to have one, else zero.
///
/// A refusal names the sequence it can read. A malformed frame that is too
/// short to carry one reports zero rather than a value read from somewhere
/// else — the one place a refusal path could be tempted to guess.
fn sequence_of(payload: &[u8]) -> u32 {
    if payload.len() < field::SEQUENCE.end {
        return 0;
    }
    u32::from_be_bytes([
        payload[field::SEQUENCE.start],
        payload[field::SEQUENCE.start + 1],
        payload[field::SEQUENCE.start + 2],
        payload[field::SEQUENCE.start + 3],
    ])
}

/// A bounded byte writer: everything past the caller's buffer is dropped, so
/// no rendering can overrun and none needs a length precondition.
struct Writer<'a> {
    out: &'a mut [u8],
    at: usize,
}

impl<'a> Writer<'a> {
    fn new(out: &'a mut [u8]) -> Self {
        Writer { out, at: 0 }
    }

    fn remaining(&self) -> usize {
        self.out.len().saturating_sub(self.at)
    }

    fn put(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if self.at >= self.out.len() {
                return;
            }
            self.out[self.at] = byte;
            self.at += 1;
        }
    }

    /// Writes `bytes` through [`escaped_width`]'s three classes.
    ///
    /// Total over every octet: there is no arm that passes a byte through
    /// unexamined, so no sequence a runner can produce reaches the wire able
    /// to end the line early or move an operator's cursor.
    fn put_escaped(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            match byte {
                b'\n' => self.put(b"\\n"),
                b'\\' => self.put(b"\\\\"),
                0x20..=0x7E => self.put(&[byte]),
                _ => self.put(b"?"),
            }
        }
    }

    fn put_u32(&mut self, value: u32) {
        let mut digits = [0u8; 10];
        let mut count = 0;
        let mut rest = value;
        loop {
            digits[count] = b'0' + (rest % 10) as u8;
            count += 1;
            rest /= 10;
            if rest == 0 {
                break;
            }
        }
        while count > 0 {
            count -= 1;
            self.put(&[digits[count]]);
        }
    }

    fn written(&self) -> usize {
        self.at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A well-formed command payload for `verb` carrying `sequence`.
    fn payload(verb_id: u16, sequence: u32) -> [u8; COMMAND_PAYLOAD_BYTES] {
        let mut out = [0u8; COMMAND_PAYLOAD_BYTES];
        out[field::PREFIX].copy_from_slice(crate::gem_receive::ENVELOPE_PREFIX);
        out[field::MAGIC].copy_from_slice(COMMAND_MAGIC);
        out[field::VERB].copy_from_slice(&verb_id.to_be_bytes());
        out[field::SEQUENCE].copy_from_slice(&sequence.to_be_bytes());
        out
    }

    fn rendered(spoken: Spoken, status: &[u8]) -> String {
        rendered_with(spoken, AnswerText { status, output: b"" })
    }

    fn rendered_with(spoken: Spoken, text: AnswerText<'_>) -> String {
        let mut out = [0u8; ANSWER_CAPACITY];
        let len = render(spoken, text, &mut out);
        String::from_utf8(out[..len].to_vec()).expect("ASCII")
    }

    /// The answer for a row that needs no argument — most of this suite.
    fn answer(verb: Verb, sequence: u32) -> Spoken {
        Spoken::Answer(Command::bare(verb, sequence))
    }

    // --- clause 1: the classifier is total over fixed offsets ----------------

    #[test]
    fn the_layout_is_fixed_and_the_frame_can_never_be_padded() {
        // The property is `>=`, not `==`, and the difference is the whole of
        // 2026-08-08's widening. A NIC pads a short frame UP to 60 octets; it
        // never pads a frame already at or above it. So any width from the
        // minimum upward is equally immune, and pinning `==` fixed the
        // argument field at 30 octets for a guarantee `>=` already gave.
        assert!(
            14 + COMMAND_PAYLOAD_BYTES >= crate::gem::MINIMUM_FRAME_LEN,
            "a frame under the Ethernet minimum arrives padded, and padding is \
             indistinguishable from a wrong-width field"
        );
        // Stated as its own assertion so the reason survives: the widening is
        // legitimate precisely because this margin is non-negative, and a
        // future narrowing that broke it would fail above rather than produce
        // a subtly paddable envelope.
        assert_eq!(COMMAND_PAYLOAD_BYTES, HEADER_BYTES + ARGUMENT_BYTES);
        assert_eq!(HEADER_BYTES, 16);
        assert_eq!(field::PREFIX, 0..6);
        assert_eq!(field::MAGIC, 6..10);
        assert_eq!(field::VERB, 10..12);
        assert_eq!(field::SEQUENCE, 12..16);
        assert_eq!(field::ARGUMENT, 16..COMMAND_PAYLOAD_BYTES);
        assert_eq!(field::ARGUMENT.len(), ARGUMENT_BYTES);
        // The mutation arm: the fields tile the payload exactly, with no gap
        // and no overlap. Widening any field by one octet, or adding one, moves
        // a later field's offset and fails here — the fixed-offset discipline
        // asserted rather than narrated.
        assert_eq!(
            field::PREFIX.len()
                + field::MAGIC.len()
                + field::VERB.len()
                + field::SEQUENCE.len()
                + field::ARGUMENT.len(),
            COMMAND_PAYLOAD_BYTES
        );
    }

    #[test]
    fn a_well_formed_ping_and_status_classify_to_their_verb_and_sequence() {
        assert_eq!(classify(&payload(Verb::Ping.id(), 7)), Ok(Command::bare(Verb::Ping, 7)));
        assert_eq!(
            classify(&payload(Verb::Status.id(), 0xDEAD_BEEF)),
            Ok(Command::bare(Verb::Status, 0xDEAD_BEEF))
        );
    }

    #[test]
    fn every_malformed_envelope_maps_to_its_own_named_refusal() {
        let good = payload(Verb::Ping.id(), 1);

        let mut wrong_magic = good;
        wrong_magic[field::MAGIC][0] = b'X';
        assert_eq!(classify(&wrong_magic), Err(CommandRefusal::WrongMagic));

        assert_eq!(classify(&good[..COMMAND_PAYLOAD_BYTES - 1]), Err(CommandRefusal::Undersize));
        assert_eq!(classify(&[]), Err(CommandRefusal::Undersize));

        let mut oversize = good.to_vec();
        oversize.push(0);
        assert_eq!(classify(&oversize), Err(CommandRefusal::Oversize));

        assert_eq!(classify(&payload(0, 1)), Err(CommandRefusal::UnknownVerb));
        assert_eq!(classify(&payload(u16::MAX, 1)), Err(CommandRefusal::UnknownVerb));

        // A payload that is not even an envelope: the prefix is `-16`'s to
        // check, and this classifier refuses it too rather than assuming the
        // caller already did — a total function has no preconditions.
        let mut not_ours = good;
        not_ours[field::PREFIX][0] = b'X';
        assert_eq!(classify(&not_ours), Err(CommandRefusal::WrongMagic));
    }

    #[test]
    fn the_copy_the_board_makes_keeps_the_oversize_refusal_reachable() {
        // The glue copies at most `ADMITTED_CAPACITY` octets out of the
        // receive region. Sized to the envelope exactly, an over-long frame
        // would arrive here truncated into a well-formed command and the
        // board would answer something it is specified to refuse.
        //
        // The headroom is asserted as a *floor* rather than an exact width
        // (`LE-122`): one spare octet keeps the refusal reachable but makes
        // its width unknowable, and the board must be able to report how far
        // over an over-long frame was. A count is a floor, never a total.
        // `const` blocks: these are claims about constants, so they belong to
        // compilation rather than to a test run — and plain `assert!` on a
        // constant is a clippy error under `-D warnings` on the runner.
        const { assert!(ADMITTED_CAPACITY > COMMAND_PAYLOAD_BYTES) };
        // Wide enough to measure a framing surplus — an FCS is four.
        const { assert!(ADMITTED_CAPACITY >= COMMAND_PAYLOAD_BYTES + 4) };
        let good = payload(Verb::Ping.id(), 1);
        let mut over = [0u8; ADMITTED_CAPACITY];
        over[..COMMAND_PAYLOAD_BYTES].copy_from_slice(&good);
        assert_eq!(classify(&over), Err(CommandRefusal::Oversize));
        // Every width past the envelope is refused, not just the first.
        for width in COMMAND_PAYLOAD_BYTES + 1..=ADMITTED_CAPACITY {
            assert_eq!(classify(&over[..width]), Err(CommandRefusal::Oversize), "at {width}");
        }
        assert_eq!(classify(&over[..COMMAND_PAYLOAD_BYTES]), Ok(Command::bare(Verb::Ping, 1)));
    }

    #[test]
    fn the_classifier_is_total_over_every_verb_id_and_the_lookup_is_bounded() {
        // The one input-derived selection this Story introduces, exhausted:
        // every 16-bit id resolves either to a row that exists or to
        // `UnknownVerb`. Nothing indexes, nothing wraps, nothing panics.
        let mut known = 0;
        for id in 0..=u16::MAX {
            match classify(&payload(id, 1)) {
                Ok(command) => {
                    known += 1;
                    assert_eq!(
                        command.verb.id(),
                        id,
                        "a row answered for an id that is not its own"
                    );
                    assert!(resolve(id).is_some());
                }
                Err(refusal) => {
                    assert_eq!(refusal, CommandRefusal::UnknownVerb);
                    assert!(resolve(id).is_none());
                }
            }
        }
        assert_eq!(known, VERB_TABLE.len(), "exactly the table's rows, and no others");
    }

    #[test]
    fn no_byte_of_the_argument_field_steers_the_classification_of_any_row() {
        // `-17` asserted this as *"no byte of the argument field steers
        // anything"*, comparing the whole classified command. `-18` gave one
        // row an argument that means something, so the sentence had to become
        // more precise rather than quietly weaker — the surviving claim is
        // about the **classification**: which row answered, and with which
        // sequence, is not a function of the argument for any row in the
        // table, including the one that reads it.
        //
        // What the argument may now steer — a command line handed to a
        // stateless runner that owns nothing — is asserted separately in
        // `the_argument_steers_the_shell_row_and_still_steers_nothing_for_the_other_two`,
        // which is where `-18`'s containment argument is pinned.
        for fill in [0x00u8, 0x01, 0x7F, 0x80, 0xFF] {
            for verb in VERB_TABLE {
                let mut frame = payload(verb.id(), 5);
                for byte in frame[field::ARGUMENT].iter_mut() {
                    *byte = fill;
                }
                let command = classify(&frame).expect("well formed");
                assert_eq!(
                    (command.verb, command.sequence),
                    (verb, 5),
                    "argument fill {fill:#04x} moved {verb:?}'s classification"
                );
            }
        }
    }

    #[test]
    fn classification_never_produces_a_refusal_only_the_channel_can_raise() {
        // Over-rate is a property of the answer slot, not of the bytes. A
        // classifier that could produce it would be a classifier whose verdict
        // depended on history.
        for id in [0u16, 1, 2, 3, u16::MAX] {
            for length in [0usize, 1, COMMAND_PAYLOAD_BYTES - 1, COMMAND_PAYLOAD_BYTES] {
                let frame = payload(id, 1);
                let slice = &frame[..length.min(frame.len())];
                if let Err(refusal) = classify(slice) {
                    assert_ne!(refusal, CommandRefusal::OverRate);
                }
            }
        }
    }

    // --- clause 2: the table denies by default -------------------------------

    /// The two original rows, unchanged by the widening.
    ///
    /// **This test used to assert `VERB_TABLE.len() == 2` with the reason
    /// "`PD-02`: an unauthenticated peer earns two rows and no more".** That
    /// clause was retired deliberately by `STORY-P1-09-18`, whose charter
    /// re-read is quoted in the module header and in the Story's own text —
    /// not weakened by a session that found it inconvenient. The count now
    /// lives in [`the_table_holds_exactly_three_rows_and_the_third_is_the_shell`],
    /// so it is still pinned; what this test keeps is the part `-18` did not
    /// touch, which is everything about `PING` and `STATUS`.
    #[test]
    fn the_two_original_rows_kept_their_ids_names_and_answer_only_shape() {
        assert_eq!(Verb::Ping.id(), 1);
        assert_eq!(Verb::Status.id(), 2);
        assert_eq!(Verb::Ping.name(), "PING");
        assert_eq!(Verb::Status.name(), "STATUS");
        assert!(!Verb::Ping.needs_a_runner(), "PING executes nothing");
        assert!(!Verb::Status.needs_a_runner(), "STATUS executes nothing");
        // Ids are distinct across the whole table, and zero is not a verb — so
        // an all-zero payload, the single most likely accident on a wire, is
        // `UnknownVerb`.
        for (index, verb) in VERB_TABLE.iter().enumerate() {
            assert_ne!(verb.id(), 0);
            assert_eq!(resolve(verb.id()), Some(*verb));
            for later in &VERB_TABLE[index + 1..] {
                assert_ne!(verb.id(), later.id(), "two rows share an id");
                assert_ne!(verb.name(), later.name(), "two rows share a wire name");
            }
        }
        // Exactly one row in the whole table reaches a runner. Asserted over
        // the table rather than over the one variant, so a fourth row that
        // quietly wanted one fails here.
        assert_eq!(VERB_TABLE.iter().filter(|verb| verb.needs_a_runner()).count(), 1);
    }

    #[test]
    fn every_row_answers_only_from_what_the_board_already_broadcasts() {
        // The rendering of every row is a pure function of (verb, sequence,
        // caller-supplied texts). There is no device, no register and no state
        // in any signature on this path — a row cannot reach authority it is
        // never handed. The enumeration is over the table itself, so a fourth
        // row added without an answer of the same shape fails here.
        let text = AnswerText {
            status: b"TOS64-RESULT/1 fixture=none ok=true",
            output: b"A:\\>DIR\n 2 File(s)\n",
        };
        for verb in VERB_TABLE {
            let once = rendered_with(answer(verb, 11), text);
            let twice = rendered_with(answer(verb, 11), text);
            assert_eq!(once, twice, "an answer that is not a pure function of its inputs");
            assert!(once.starts_with("TOS64-ANS/1 "), "{once}");
            assert!(once.contains(verb.name()));
            match verb {
                // The sequence heard, named back. That is the whole of `M1`.
                Verb::Ping => {
                    assert_eq!(once, "TOS64-ANS/1 verb=PING seq=11 ok=1\n");
                    assert!(!once.contains("TOS64-RESULT"), "PING discloses nothing but the echo");
                    assert!(!once.contains("DIR"), "PING discloses no runner output either");
                }
                // The transcript's own verdict line, already public on the wire
                // every cycle — replayed, never composed.
                Verb::Status => {
                    assert_eq!(
                        once,
                        "TOS64-ANS/1 verb=STATUS seq=11 ok=1 status=TOS64-RESULT/1 fixture=none ok=true\n"
                    );
                }
                // The runner's output, escaped onto one line — and only the
                // runner's output. `-18`'s row discloses what a read-only
                // shell over a board-seeded volume printed, and nothing the
                // module knows that the caller did not hand it.
                Verb::Shell => {
                    assert_eq!(
                        once,
                        "TOS64-ANS/1 verb=SHELL seq=11 ok=1 out=A:\\\\>DIR\\n 2 File(s)\\n\n"
                    );
                    assert!(
                        !once.contains("TOS64-RESULT"),
                        "the boot verdict belongs to STATUS and may not leak into SHELL"
                    );
                }
            }
        }
    }

    #[test]
    fn a_status_answer_carries_the_caller_s_text_verbatim_or_refuses_to_start() {
        // Truncation is the failure mode that would turn a replay into a
        // fabrication, so a status text that will not fit is dropped whole and
        // the answer says so, rather than being cut mid-field.
        let long = [b'S'; ANSWER_CAPACITY];
        let line = rendered(answer(Verb::Status, 1), &long);
        assert_eq!(line, "TOS64-ANS/1 verb=STATUS seq=1 ok=1 status=none\n");
        assert!(line.len() <= ANSWER_CAPACITY);
    }

    // --- clause 3: every refusal is spoken and distinct -----------------------

    #[test]
    fn each_refusal_has_its_own_wire_name_and_no_two_share_one() {
        let names: [&str; 5] = [
            CommandRefusal::WrongMagic.name(),
            CommandRefusal::Undersize.name(),
            CommandRefusal::Oversize.name(),
            CommandRefusal::UnknownVerb.name(),
            CommandRefusal::OverRate.name(),
        ];
        assert_eq!(names, ["wrong-magic", "undersize", "oversize", "unknown-verb", "over-rate"]);
        for (i, a) in names.iter().enumerate() {
            for (j, b) in names.iter().enumerate() {
                assert!(i == j || a != b, "two refusals share a wire name: {a}");
            }
        }
    }

    #[test]
    fn a_refusal_is_spoken_on_the_wire_and_names_the_frame_that_earned_it() {
        assert_eq!(
            rendered(
                Spoken::Refused { refusal: CommandRefusal::UnknownVerb, sequence: 9, dropped: 0 },
                b""
            ),
            "TOS64-ANS/1 refused=unknown-verb seq=9\n"
        );
        assert_eq!(
            rendered(
                Spoken::Refused { refusal: CommandRefusal::OverRate, sequence: 0, dropped: 4 },
                b""
            ),
            "TOS64-ANS/1 refused=over-rate dropped=4\n"
        );
    }

    // --- clause 4: the answer rate is bounded, and the bound fails safe -------

    #[test]
    fn one_answer_leaves_the_board_per_beat_and_the_excess_is_refused_as_over_rate() {
        let mut channel = CommandChannel::new();
        let mut out = [0u8; ANSWER_CAPACITY];

        // A flood: five well-formed PINGs offered inside one beat.
        for sequence in 1..=5u32 {
            channel.offer(&payload(Verb::Ping.id(), sequence));
        }
        // Exactly one answer leaves, and it is the first one heard — the
        // board answers what it heard first rather than what shouted last.
        let len =
            channel.take(AnswerText { status: b"", output: b"" }, &mut out).expect("one answer");
        assert_eq!(&out[..len], b"TOS64-ANS/1 verb=PING seq=1 ok=1\n");
        assert_eq!(channel.answered(), 1);

        // The next beat speaks the over-rate refusal, with its count.
        let len = channel
            .take(AnswerText { status: b"", output: b"" }, &mut out)
            .expect("the over-rate refusal");
        assert_eq!(&out[..len], b"TOS64-ANS/1 refused=over-rate dropped=4\n");
        assert_eq!(channel.refused(), 1);

        // And then silence: nothing is owed, so nothing is transmitted.
        assert_eq!(
            channel.take(AnswerText { status: b"", output: b"" }, &mut out),
            None,
            "an empty slot transmits nothing"
        );
    }

    #[test]
    fn a_flood_of_any_length_never_owes_more_than_one_line_per_beat() {
        // Ten thousand well-formed PINGs against a slot that opens once every
        // three offers: the board may never emit more lines than it had beats,
        // whatever the arrival pattern. That inequality is the amplification
        // bound, stated as arithmetic rather than as a rate constant.
        let mut channel = CommandChannel::new();
        let mut out = [0u8; ANSWER_CAPACITY];
        let mut beats = 0u32;
        let mut lines = 0u32;
        for sequence in 0..10_000u32 {
            channel.offer(&payload(Verb::Ping.id(), sequence));
            if sequence.is_multiple_of(3) {
                beats += 1;
                if channel.take(AnswerText { status: b"", output: b"" }, &mut out).is_some() {
                    lines += 1;
                }
            }
        }
        assert!(lines <= beats, "{lines} lines left the board across {beats} beats");
        assert_eq!(
            channel.answered() + channel.refused(),
            lines,
            "every counted line is a line that actually left, and every line is counted"
        );
        // And the flood is visible rather than absorbed: the board answered a
        // few and said so about the rest.
        assert!(channel.refused() > 0, "a flood that produced no over-rate refusal");
    }

    #[test]
    fn a_refused_command_still_gets_its_slot_and_a_valid_one_behind_it_is_not_lost_silently() {
        let mut channel = CommandChannel::new();
        let mut out = [0u8; ANSWER_CAPACITY];
        channel.offer(&payload(0, 3)); // unknown verb
        channel.offer(&payload(Verb::Ping.id(), 4)); // arrives with the slot full
        let len =
            channel.take(AnswerText { status: b"", output: b"" }, &mut out).expect("the refusal");
        assert_eq!(&out[..len], b"TOS64-ANS/1 refused=unknown-verb seq=3\n");
        let len =
            channel.take(AnswerText { status: b"", output: b"" }, &mut out).expect("the over-rate");
        assert_eq!(&out[..len], b"TOS64-ANS/1 refused=over-rate dropped=1\n");
        assert_eq!(channel.answered(), 0, "nothing was answered");
        assert_eq!(channel.refused(), 2, "both refusals were spoken, neither vanished");
    }

    #[test]
    fn the_counters_move_only_when_a_line_actually_leaves_the_board() {
        let mut channel = CommandChannel::new();
        let mut out = [0u8; ANSWER_CAPACITY];
        channel.offer(&payload(Verb::Status.id(), 2));
        assert_eq!(channel.answered(), 0, "decided is not sent");
        assert_eq!(channel.last(), None);
        channel.take(AnswerText { status: b"verdict", output: b"" }, &mut out).expect("the answer");
        assert_eq!(channel.answered(), 1);
        assert_eq!(channel.last(), Some(Verb::Status), "the canvas names the verb that answered");
    }

    #[test]
    fn nothing_is_transmitted_in_response_to_a_frame_outside_the_answer_slot() {
        // `offer` returns nothing to transmit; the only producer of bytes is
        // `take`, which the park loop calls once per beat. Stated as a test
        // because "no path transmits outside the bounded slot" is otherwise a
        // claim about code nobody re-reads.
        let mut channel = CommandChannel::new();
        let mut out = [0u8; ANSWER_CAPACITY];
        assert_eq!(
            channel.take(AnswerText { status: b"", output: b"" }, &mut out),
            None,
            "an untouched channel owes nothing"
        );
        channel.offer(&payload(Verb::Ping.id(), 1));
        assert!(channel.take(AnswerText { status: b"", output: b"" }, &mut out).is_some());
        assert_eq!(channel.take(AnswerText { status: b"", output: b"" }, &mut out), None);
    }

    #[test]
    fn the_answer_never_exceeds_the_line_the_text_channel_can_carry() {
        let mut out = [0u8; ANSWER_CAPACITY];
        for spoken in [
            Spoken::Answer(Command::bare(Verb::Ping, u32::MAX)),
            Spoken::Answer(Command::bare(Verb::Status, u32::MAX)),
            Spoken::Answer(Command::bare(Verb::Shell, u32::MAX)),
            Spoken::Refused {
                refusal: CommandRefusal::OverRate,
                sequence: u32::MAX,
                dropped: u32::MAX,
            },
        ] {
            let len = render(
                spoken,
                AnswerText { status: &[b'S'; 64], output: &[b'O'; SHELL_OUTPUT_CAPACITY] },
                &mut out,
            );
            assert!(len <= ANSWER_CAPACITY, "{len} exceeds the answer capacity");
            const { assert!(ANSWER_CAPACITY + 14 <= crate::gem::TEXT_FRAME_CAPACITY) };
        }
    }

    // --- STORY-P1-09-18: the third row, and the argument that now means -------

    /// The table grew, and it grew by exactly one row with exactly one id.
    ///
    /// `-17`'s own test asserted **two** rows and said a third was "a charter
    /// re-read, not an addition". The re-read happened
    /// ([`STORY-P1-09-18`](../../../goals/stories/STORY-P1-09-18.md)) and this
    /// is what it licensed: one row, reaching a runner that owns nothing.
    #[test]
    fn the_table_holds_exactly_three_rows_and_the_third_is_the_shell() {
        assert_eq!(VERB_TABLE.len(), 3);
        assert_eq!(VERB_TABLE, [Verb::Ping, Verb::Status, Verb::Shell]);
        assert_eq!(Verb::Shell.id(), 3);
        assert_eq!(Verb::Shell.name(), "SHELL");
        for verb in VERB_TABLE {
            assert_ne!(verb.id(), 0, "zero is never a verb");
            assert_eq!(resolve(verb.id()), Some(verb));
        }
        // Still exhaustive over the whole id space, and still exactly the
        // table's rows — the widening did not widen the selection.
        let mut known = 0;
        for id in 0..=u16::MAX {
            if resolve(id).is_some() {
                known += 1;
            }
        }
        assert_eq!(known, VERB_TABLE.len());
    }

    /// The containment sentence that changed, stated as two assertions.
    ///
    /// `-17` could say *no byte of the argument steers anything*. That is no
    /// longer true and pretending otherwise would be the drift this project
    /// keeps catching. What is true now, and is what `-18`'s argument rests
    /// on: the argument steers **only** the `SHELL` row, it is carried at a
    /// fixed width to a runner as **data**, and for the two original rows the
    /// old sentence still holds unweakened.
    #[test]
    fn the_argument_steers_the_shell_row_and_still_steers_nothing_for_the_other_two() {
        for fill in [0x00u8, 0x01, 0x7F, 0x80, 0xFF] {
            for verb in [Verb::Ping, Verb::Status] {
                let mut frame = payload(verb.id(), 5);
                for byte in frame[field::ARGUMENT].iter_mut() {
                    *byte = fill;
                }
                let command = classify(&frame).expect("well formed");
                assert_eq!(command.verb, verb);
                assert_eq!(command.sequence, 5);
                // The classifier carries the argument for every row — it is
                // one fixed-width copy, not a decision — but only the SHELL
                // row has anywhere to take it.
                assert_eq!(command.line(), None, "{verb:?} must not acquire a command line");
            }
        }
        // And the SHELL row: the same bytes now mean a line, and the line is
        // exactly the field, trimmed at a fixed width. No length inside the
        // frame is believed, because there is no length inside the frame.
        let mut frame = payload(Verb::Shell.id(), 6);
        frame[field::ARGUMENT][..3].copy_from_slice(b"VER");
        let command = classify(&frame).expect("well formed");
        assert_eq!(command.line(), Some(&b"VER"[..]));
    }

    /// A command line is trimmed at a fixed width, and every filler the wire
    /// might carry resolves to the same line.
    ///
    /// Space padding is what a human-written sender produces; NUL padding is
    /// what a zeroed buffer produces. Both are padding, neither is content,
    /// and a classifier that treated them differently would make the same
    /// command mean two things depending on who sent it.
    #[test]
    fn the_command_line_is_the_fixed_field_trimmed_and_never_a_length_from_the_frame() {
        for filler in [0x00u8, b' '] {
            let mut frame = payload(Verb::Shell.id(), 1);
            for byte in frame[field::ARGUMENT].iter_mut() {
                *byte = filler;
            }
            frame[field::ARGUMENT][..9].copy_from_slice(b"DIR A:\\ ~");
            frame[field::ARGUMENT][8] = filler; // the `~` was only a placeholder
            let command = classify(&frame).expect("well formed");
            assert_eq!(command.line(), Some(&b"DIR A:\\"[..]), "filler {filler:#04x}");
        }
        // A completely blank argument is an empty line, not a missing one:
        // the row resolved, so the answer is owed. An empty line is what the
        // shell itself refuses, and it refuses it as a shell rather than as a
        // classifier.
        let frame = payload(Verb::Shell.id(), 1);
        assert_eq!(classify(&frame).expect("well formed").line(), Some(&b""[..]));
        // The full field with no padding at all is still exactly the field —
        // the one case where a trim could run off the end.
        let mut full = payload(Verb::Shell.id(), 1);
        for byte in full[field::ARGUMENT].iter_mut() {
            *byte = b'X';
        }
        assert_eq!(
            classify(&full).expect("well formed").line().map(<[u8]>::len),
            Some(ARGUMENT_BYTES)
        );
    }

    /// The channel tells its caller what to run, and only for the row that has
    /// something to run.
    ///
    /// This is the whole of the seam: `offer` classifies, `pending_line`
    /// reports, the **caller** runs, `take` renders what the caller produced.
    /// The module still executes nothing, which is why it can still forbid
    /// `unsafe` and still take no device in any signature.
    #[test]
    fn the_channel_reports_the_line_to_run_and_never_runs_it() {
        let mut channel = CommandChannel::new();
        assert_eq!(channel.pending_line(), None, "an untouched channel owes no run");

        channel.offer(&payload(Verb::Ping.id(), 1));
        assert_eq!(channel.pending_line(), None, "PING has nothing to run");

        let mut channel = CommandChannel::new();
        let mut frame = payload(Verb::Shell.id(), 2);
        frame[field::ARGUMENT][..3].copy_from_slice(b"VOL");
        channel.offer(&frame);
        assert_eq!(channel.pending_line(), Some(&b"VOL"[..]));

        // A refusal has no line either: an unknown verb never reached a row.
        let mut channel = CommandChannel::new();
        channel.offer(&payload(0, 3));
        assert_eq!(channel.pending_line(), None);
    }

    /// The `SHELL` answer carries the caller's output and nothing the module
    /// invented.
    #[test]
    fn a_shell_answer_carries_the_callers_output_escaped_onto_one_line() {
        let line = rendered_with(
            Spoken::Answer(Command::bare(Verb::Shell, 4)),
            AnswerText { status: b"ignored", output: b"TinyOS 4.0\nA:\\>" },
        );
        assert_eq!(line, "TOS64-ANS/1 verb=SHELL seq=4 ok=1 out=TinyOS 4.0\\nA:\\\\>\n");
        // The status text belongs to STATUS and may not leak into SHELL, and
        // the shell output may not leak into STATUS. Two rows, two texts.
        let status = rendered_with(
            Spoken::Answer(Command::bare(Verb::Status, 4)),
            AnswerText { status: b"TOS64-RESULT/1 ok=true", output: b"SHOULD NOT APPEAR" },
        );
        assert!(!status.contains("SHOULD NOT APPEAR"), "{status}");
        assert!(status.contains("TOS64-RESULT/1 ok=true"), "{status}");
    }

    /// One frame in, one line out, whatever the shell produced.
    ///
    /// This is `SEC-20` restated for the widened row and it is the property a
    /// verb core most plausibly breaks: a `DIR` of a full volume is many lines
    /// of output, and many lines would be many frames, which is the
    /// amplification an unauthenticated broadcast-capable peer must not get.
    /// The answer is one line and the excess is **named**, never dropped
    /// silently and never continued into a second frame.
    #[test]
    fn a_large_shell_output_still_leaves_as_exactly_one_named_bounded_line() {
        let huge = [b'D'; SHELL_OUTPUT_CAPACITY];
        let mut out = [0u8; ANSWER_CAPACITY];
        let len = render(
            Spoken::Answer(Command::bare(Verb::Shell, 7)),
            AnswerText { status: b"", output: &huge },
            &mut out,
        );
        let line = core::str::from_utf8(&out[..len]).expect("ASCII");
        assert!(len <= ANSWER_CAPACITY, "{len}");
        assert_eq!(line.matches('\n').count(), 1, "exactly one line: {line}");
        assert!(line.ends_with('\n'));
        assert!(line.contains(" more="), "the withheld octets must be named: {line}");
        // And the count is the truth: prefix carried + withheld = produced.
        let carried = line
            .split("out=")
            .nth(1)
            .and_then(|rest| rest.split(" more=").next())
            .expect("an out= field")
            .len();
        let withheld: usize = line
            .rsplit(" more=")
            .next()
            .expect("a more= field")
            .trim_end()
            .parse()
            .expect("a count");
        assert_eq!(
            carried + withheld,
            SHELL_OUTPUT_CAPACITY,
            "the answer must account for every octet the shell produced"
        );
    }

    /// A labelled prefix is a measurement; an unlabelled one is a forgery.
    ///
    /// The deliberate divergence from `status`'s whole-drop rule, asserted so
    /// the divergence is a decision rather than an accident: a `STATUS` reply
    /// is a **replay of a verdict**, where a partial is a plausible lie with
    /// no marker, so it is dropped whole and named `none`. Shell output is a
    /// **stream**, where a prefix is a true statement about the beginning of
    /// it provided the answer says how much it did not carry.
    #[test]
    fn output_that_fits_carries_no_more_field_and_output_that_does_not_always_carries_one() {
        let fits = rendered_with(
            Spoken::Answer(Command::bare(Verb::Shell, 1)),
            AnswerText { status: b"", output: b"A:\\>" },
        );
        assert!(!fits.contains("more="), "nothing was withheld: {fits}");

        for length in 1..=SHELL_OUTPUT_CAPACITY {
            let output = vec![b'X'; length];
            let line = rendered_with(
                Spoken::Answer(Command::bare(Verb::Shell, 1)),
                AnswerText { status: b"", output: &output },
            );
            assert!(line.len() <= ANSWER_CAPACITY, "at {length}: {line}");
            assert_eq!(line.matches('\n').count(), 1, "at {length}");
            let carried = line
                .split("out=")
                .nth(1)
                .and_then(|rest| rest.split(" more=").next())
                .expect("out=")
                .trim_end()
                .len();
            let withheld: usize = match line.rsplit_once(" more=") {
                Some((_, count)) => count.trim_end().parse().expect("a count"),
                None => 0,
            };
            // Escaping only ever *grows* the carried text, so the carried
            // count is compared as a floor over source octets rather than as
            // an equality — what must hold exactly is the accounting.
            assert!(carried >= length - withheld, "at {length}: {line}");
            assert!(withheld <= length, "at {length}: withheld more than was produced");
        }
    }

    /// Nothing the shell can emit can move the operator's cursor or break the
    /// wire's own line framing.
    ///
    /// The shell already renders attacker-influenced names inert
    /// (`EPIC-P2` §6.5 rule 3), so this is the second fence and not the first
    /// — but it is a **total** function over bytes, which the first one is
    /// not: it runs over every octet the runner returns regardless of which
    /// side of the shell produced it.
    #[test]
    fn no_octet_of_shell_output_can_escape_the_one_line_it_is_carried_on() {
        let mut output = [0u8; 64];
        for (index, byte) in output.iter_mut().enumerate() {
            *byte = index as u8;
        }
        let line = rendered_with(
            Spoken::Answer(Command::bare(Verb::Shell, 1)),
            AnswerText { status: b"", output: &output },
        );
        assert_eq!(line.matches('\n').count(), 1, "{line:?}");
        for (index, byte) in line.bytes().enumerate() {
            if index + 1 == line.len() {
                assert_eq!(byte, b'\n', "the terminator");
                continue;
            }
            assert!(
                (0x20..0x7F).contains(&byte),
                "octet {byte:#04x} at {index} reached the wire unrendered: {line:?}"
            );
        }
        // Every one of the 256 octets, not just the first 64 — the escape is
        // total or it is not a fence.
        for byte in 0..=u8::MAX {
            let line = rendered_with(
                Spoken::Answer(Command::bare(Verb::Shell, 1)),
                AnswerText { status: b"", output: &[byte, byte, byte] },
            );
            assert_eq!(line.matches('\n').count(), 1, "octet {byte:#04x}: {line:?}");
        }
    }

    /// The rate bound is the same bound, with a row that does far more work
    /// behind it.
    #[test]
    fn the_shell_row_owes_no_more_lines_per_beat_than_a_ping_does() {
        let mut channel = CommandChannel::new();
        let mut out = [0u8; ANSWER_CAPACITY];
        let mut frame = payload(Verb::Shell.id(), 1);
        frame[field::ARGUMENT][..3].copy_from_slice(b"DIR");
        let mut beats = 0u32;
        let mut lines = 0u32;
        for sequence in 0..1_000u32 {
            frame[field::SEQUENCE].copy_from_slice(&sequence.to_be_bytes());
            channel.offer(&frame);
            if sequence.is_multiple_of(3) {
                beats += 1;
                if channel.take(AnswerText { status: b"", output: b"x" }, &mut out).is_some() {
                    lines += 1;
                }
            }
        }
        assert!(lines <= beats, "{lines} lines across {beats} beats");
        assert!(channel.refused() > 0, "a flood that produced no over-rate refusal");
    }
}
