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
//! 3. **The table denies by default and its rows own nothing.** Two rows,
//!    both answer-only, both assembled from data the board already broadcasts.
//!    The wire peer has no kernel-derived identity (`PD-02`), so a verb that
//!    changed state or disclosed anything new would be authority granted to an
//!    unauthenticated caller — a third row is a charter re-read, not an
//!    addition.
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
pub const ARGUMENT_BYTES: usize = 30;

/// The command payload's exact length, octets.
///
/// Chosen so that `14 + COMMAND_PAYLOAD_BYTES` is exactly
/// [`crate::gem::MINIMUM_FRAME_LEN`]. That is not arithmetic tidiness: every
/// Ethernet NIC pads a short frame to 60 octets below any software that could
/// be told not to, so a shorter envelope would arrive carrying padding that no
/// receiver can distinguish from a wrong-width field. Making the envelope the
/// minimum frame is what lets "exactly this many octets" be a refusal rather
/// than a hope.
pub const COMMAND_PAYLOAD_BYTES: usize = 46;

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
/// the sequence heard, and a status text the *caller* supplies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    /// The answer names the sequence heard. Discloses nothing the board did
    /// not just receive.
    Ping,
    /// The answer replays the transcript's own boot verdict line — already
    /// public on the wire every transcript cycle.
    Status,
}

impl Verb {
    /// The wire id. Never zero: an all-zero payload is the single most likely
    /// accident on a wire and must resolve to `UnknownVerb`, not to a row.
    pub const fn id(self) -> u16 {
        match self {
            Verb::Ping => 1,
            Verb::Status => 2,
        }
    }

    /// The name a capture and the operator's console both print.
    pub const fn name(self) -> &'static str {
        match self {
            Verb::Ping => "PING",
            Verb::Status => "STATUS",
        }
    }
}

/// The whole table. Two rows, both read-only, both answer-only.
///
/// `PD-02`: the peer on this cable has no kernel-derived identity, so the only
/// verbs that may exist are ones whose answers disclose what the board already
/// broadcasts and whose execution changes nothing. A third row waits on a
/// session/authentication story, not on this table growing.
pub const VERB_TABLE: [Verb; 2] = [Verb::Ping, Verb::Status];

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

/// A well-formed command: which row, and the sequence to name back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    /// The row the id resolved to.
    pub verb: Verb,
    /// The sequence the sender chose. Echoed; never interpreted.
    pub sequence: u32,
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
    Ok(Command { verb, sequence })
}

/// One line the board owes the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Spoken {
    /// A row of the table answered.
    Answer {
        /// Which row.
        verb: Verb,
        /// The sequence heard.
        sequence: u32,
    },
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
pub const ANSWER_CAPACITY: usize = 128;

/// Renders one owed line. Pure: verb, sequence, and a status text the caller
/// supplies. No device, no state, nothing this module owns.
///
/// A `status` that will not fit is dropped **whole** and named `none`. A
/// truncated replay of a verdict line is a fabrication with a plausible shape,
/// which is worse than an absence because nothing marks it.
pub fn render(spoken: Spoken, status: &[u8], out: &mut [u8]) -> usize {
    let mut writer = Writer::new(out);
    writer.put(b"TOS64-ANS/1 ");
    match spoken {
        Spoken::Answer { verb, sequence } => {
            writer.put(b"verb=");
            writer.put(verb.name().as_bytes());
            writer.put(b" seq=");
            writer.put_u32(sequence);
            writer.put(b" ok=1");
            if verb == Verb::Status {
                writer.put(b" status=");
                if status.is_empty() || writer.remaining() < status.len() + 1 {
                    writer.put(b"none");
                } else {
                    writer.put(status);
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
            Ok(Command { verb, sequence }) => Spoken::Answer { verb, sequence },
            Err(refusal) => Spoken::Refused { refusal, sequence: sequence_of(payload), dropped: 0 },
        });
    }

    /// The answer slot. Renders at most one line per call, or [`None`] when
    /// nothing is owed — an empty slot transmits nothing rather than filling
    /// the wire with silence that looks like data.
    ///
    /// Counters move here and not in [`offer`](Self::offer), so the canvas row
    /// says what actually left the board.
    pub fn take(&mut self, status: &[u8], out: &mut [u8]) -> Option<usize> {
        if let Some(spoken) = self.pending.take() {
            match spoken {
                Spoken::Answer { verb, .. } => {
                    self.answered = self.answered.saturating_add(1);
                    self.last = Some(verb);
                }
                Spoken::Refused { .. } => self.refused = self.refused.saturating_add(1),
            }
            return Some(render(spoken, status, out));
        }
        if self.dropped > 0 {
            let dropped = self.dropped;
            self.dropped = 0;
            self.refused = self.refused.saturating_add(1);
            return Some(render(
                Spoken::Refused { refusal: CommandRefusal::OverRate, sequence: 0, dropped },
                status,
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
        let mut out = [0u8; ANSWER_CAPACITY];
        let len = render(spoken, status, &mut out);
        String::from_utf8(out[..len].to_vec()).expect("ASCII")
    }

    // --- clause 1: the classifier is total over fixed offsets ----------------

    #[test]
    fn the_layout_is_fixed_and_exactly_one_ethernet_minimum_frame() {
        // The command frame is deliberately the Ethernet minimum: 14 header
        // octets plus this payload. A shorter payload would be padded by the
        // sending NIC and the padding would be indistinguishable from an
        // oversize field, which is how a fixed-width envelope stops being one.
        assert_eq!(14 + COMMAND_PAYLOAD_BYTES, crate::gem::MINIMUM_FRAME_LEN);
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
        assert_eq!(
            classify(&payload(Verb::Ping.id(), 7)),
            Ok(Command { verb: Verb::Ping, sequence: 7 })
        );
        assert_eq!(
            classify(&payload(Verb::Status.id(), 0xDEAD_BEEF)),
            Ok(Command { verb: Verb::Status, sequence: 0xDEAD_BEEF })
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
        assert!(ADMITTED_CAPACITY > COMMAND_PAYLOAD_BYTES, "the refusal must stay reachable");
        assert!(
            ADMITTED_CAPACITY >= COMMAND_PAYLOAD_BYTES + 4,
            "and wide enough to measure a framing surplus — an FCS is four"
        );
        let good = payload(Verb::Ping.id(), 1);
        let mut over = [0u8; ADMITTED_CAPACITY];
        over[..COMMAND_PAYLOAD_BYTES].copy_from_slice(&good);
        assert_eq!(classify(&over), Err(CommandRefusal::Oversize));
        // Every width past the envelope is refused, not just the first.
        for width in COMMAND_PAYLOAD_BYTES + 1..=ADMITTED_CAPACITY {
            assert_eq!(classify(&over[..width]), Err(CommandRefusal::Oversize), "at {width}");
        }
        assert_eq!(
            classify(&over[..COMMAND_PAYLOAD_BYTES]),
            Ok(Command { verb: Verb::Ping, sequence: 1 })
        );
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
    fn no_byte_of_the_argument_field_steers_anything() {
        // The argument is carried at a fixed width and is never an offset, a
        // length or an address. Filling it across its range may not move the
        // verdict by one bit.
        for fill in [0x00u8, 0x01, 0x7F, 0x80, 0xFF] {
            let mut frame = payload(Verb::Ping.id(), 5);
            for byte in frame[field::ARGUMENT].iter_mut() {
                *byte = fill;
            }
            assert_eq!(
                classify(&frame),
                Ok(Command { verb: Verb::Ping, sequence: 5 }),
                "argument fill {fill:#04x} changed the verdict"
            );
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

    #[test]
    fn the_table_holds_exactly_two_answer_only_rows() {
        assert_eq!(
            VERB_TABLE.len(),
            2,
            "PD-02: an unauthenticated peer earns two rows and no more"
        );
        assert_eq!(VERB_TABLE, [Verb::Ping, Verb::Status]);
        assert_eq!(Verb::Ping.id(), 1);
        assert_eq!(Verb::Status.id(), 2);
        assert_eq!(Verb::Ping.name(), "PING");
        assert_eq!(Verb::Status.name(), "STATUS");
        // Ids are distinct, and zero is not a verb — so an all-zero payload,
        // the single most likely accident on a wire, is `UnknownVerb`.
        assert_ne!(Verb::Ping.id(), Verb::Status.id());
        for verb in VERB_TABLE {
            assert_ne!(verb.id(), 0);
            assert_eq!(resolve(verb.id()), Some(verb));
        }
    }

    #[test]
    fn every_row_answers_only_from_what_the_board_already_broadcasts() {
        // The rendering of every row is a pure function of (verb, sequence,
        // caller-supplied status text). There is no device, no register and no
        // state in any signature on this path — a row cannot reach authority it
        // is never handed. The enumeration is over the table itself, so a third
        // row added without an answer of the same shape fails here.
        let status = b"TOS64-RESULT/1 fixture=none ok=true";
        for verb in VERB_TABLE {
            let once = rendered(Spoken::Answer { verb, sequence: 11 }, status);
            let twice = rendered(Spoken::Answer { verb, sequence: 11 }, status);
            assert_eq!(once, twice, "an answer that is not a pure function of its inputs");
            assert!(once.starts_with("TOS64-ANS/1 "), "{once}");
            assert!(once.contains(verb.name()));
            match verb {
                // The sequence heard, named back. That is the whole of `M1`.
                Verb::Ping => {
                    assert_eq!(once, "TOS64-ANS/1 verb=PING seq=11 ok=1\n");
                    assert!(!once.contains("TOS64-RESULT"), "PING discloses nothing but the echo");
                }
                // The transcript's own verdict line, already public on the wire
                // every cycle — replayed, never composed.
                Verb::Status => {
                    assert_eq!(
                        once,
                        "TOS64-ANS/1 verb=STATUS seq=11 ok=1 status=TOS64-RESULT/1 fixture=none ok=true\n"
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
        let line = rendered(Spoken::Answer { verb: Verb::Status, sequence: 1 }, &long);
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
        let len = channel.take(b"", &mut out).expect("one answer");
        assert_eq!(&out[..len], b"TOS64-ANS/1 verb=PING seq=1 ok=1\n");
        assert_eq!(channel.answered(), 1);

        // The next beat speaks the over-rate refusal, with its count.
        let len = channel.take(b"", &mut out).expect("the over-rate refusal");
        assert_eq!(&out[..len], b"TOS64-ANS/1 refused=over-rate dropped=4\n");
        assert_eq!(channel.refused(), 1);

        // And then silence: nothing is owed, so nothing is transmitted.
        assert_eq!(channel.take(b"", &mut out), None, "an empty slot transmits nothing");
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
                if channel.take(b"", &mut out).is_some() {
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
        let len = channel.take(b"", &mut out).expect("the refusal");
        assert_eq!(&out[..len], b"TOS64-ANS/1 refused=unknown-verb seq=3\n");
        let len = channel.take(b"", &mut out).expect("the over-rate");
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
        channel.take(b"verdict", &mut out).expect("the answer");
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
        assert_eq!(channel.take(b"", &mut out), None, "an untouched channel owes nothing");
        channel.offer(&payload(Verb::Ping.id(), 1));
        assert!(channel.take(b"", &mut out).is_some());
        assert_eq!(channel.take(b"", &mut out), None);
    }

    #[test]
    fn the_answer_never_exceeds_the_line_the_text_channel_can_carry() {
        let mut out = [0u8; ANSWER_CAPACITY];
        for spoken in [
            Spoken::Answer { verb: Verb::Ping, sequence: u32::MAX },
            Spoken::Answer { verb: Verb::Status, sequence: u32::MAX },
            Spoken::Refused {
                refusal: CommandRefusal::OverRate,
                sequence: u32::MAX,
                dropped: u32::MAX,
            },
        ] {
            let len = render(spoken, &[b'S'; 64], &mut out);
            assert!(len <= ANSWER_CAPACITY, "{len} exceeds the answer capacity");
            const { assert!(ANSWER_CAPACITY + 14 <= crate::gem::TEXT_FRAME_CAPACITY) };
        }
    }
}
