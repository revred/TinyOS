//! Spoors on the wire, unformatted (`STORY-P1-09-16`).
//!
//! A [`Spoor`](crate::spoor::Spoor) is 64 bits **for speed** — one packed
//! store, cheap enough to stamp from a real-time path. Rendering one into text
//! on the board would spend exactly what the packing saves, and would do it on
//! the hot path rather than on the laptop that has cycles to burn. So the wire
//! carries **raw records**: the board copies bytes, the host decodes them.
//!
//! The record format is not invented here. [`JOURNAL_MAGIC`] and the packed
//! `u64` layout are already what [`crate::spoor_journal`] declares as its
//! on-disk shape, so a host parser reads a captured frame and a journal file
//! with the same code.
//!
//! # Why this is a small thing to attack
//!
//! Every field below is **fixed width**. There is no length field, no options
//! region, no fragmentation and no reassembly — the classes of defect that
//! account for most of the history of IP stack vulnerabilities cannot be
//! expressed in this format. A frame is either exactly `HEADER_LEN + 8 * count`
//! bytes of payload or it is malformed, and malformed is a single early return.
//!
//! That is a statement about **surface**, not about security. This link has no
//! confidentiality and no authenticity: anyone on the cable can read every
//! record and can forge one. It is safe today because the board **only
//! transmits** — there is no inbound parser to attack. If a receive path is
//! ever enabled, this format's simplicity is a necessary condition and nowhere
//! near a sufficient one, and `LE-67` (GEM DMA with no IOMMU) is a hardware
//! exposure no protocol design can close.

use crate::spoor_journal::JOURNAL_MAGIC;

/// Bytes of frame header before the first record.
///
/// Twenty-four so records begin 8-byte aligned: the payload is a bulk copy of
/// packed `u64`s and the board should never be doing unaligned stores to build
/// it. Four of those bytes were padding written zero, held explicitly *"so a
/// future field does not have to move the records"*; `STORY-P1-10-04` spent
/// them on the boot epoch, and every stream captured before it still decodes
/// byte-for-byte because nothing after offset 24 moved.
pub const HEADER_LEN: usize = 24;

/// Records per frame.
///
/// Sized to fill a standard MTU rather than a diagnostic trickle. A spoor is
/// to a physical system what a token is to a language model — the uniform atom
/// its whole observable behaviour is made of — so this stream is continuous and
/// high-rate by nature, and the frame must not be what limits it. One transmit
/// carries 181 events.
///
/// The exact number is set by the *larger* of the two framings this payload
/// travels in, so one constant keeps both inside an MTU:
///
/// ```text
///   raw 0x88B5 : 14 (Ethernet)                  + 24 + 181*8 = 1486
///   IPv4/UDP   : 14 (Ethernet) + 20 (IP) + 8 (UDP) + 24 + 181*8 = 1514
/// ```
///
/// 1514 is exactly a maximum Ethernet frame before the FCS. That is why this is
/// 181 and not 184: the UDP wrapper (`crate::udp_wire`) exists so an ordinary
/// unprivileged socket can read the same records, and a payload sized only for
/// the raw framing would have forced *that* one to fragment.
///
/// It is also the constant that keeps fragmentation impossible in both: a
/// payload that cannot exceed the MTU is never handed to the MAC to split, and
/// neither protocol has reassembly because neither can ever need any.
pub const MAX_RECORDS: usize = 181;

/// The largest payload [`encode`] can produce.
pub const MAX_PAYLOAD: usize = HEADER_LEN + MAX_RECORDS * 8;

/// The `flags` bit marking a frame as a **re-announcement** of the boot
/// certificate rather than fresh stream (`STORY-P1-10-04`).
///
/// A retained frame carries records the stream already sent, with the sequence
/// numbers they were sent under. That is deliberate — it makes the
/// re-announcement verbatim rather than a summary — and it means a host
/// applying `seq + count` to it would compute a wild backwards jump. The flag
/// is on the frame because **a host cannot infer it**: a legitimately repeated
/// sequence is indistinguishable from a stream that restarted, and guessing
/// between the two is exactly the kind of inference §4's loss accounting
/// exists to avoid.
pub const FLAG_RETAINED: u16 = 0x0001;

/// Every flag bit this version of the format defines.
///
/// Kept so a decoder can tell "a flag I do not implement" from "a bit that is
/// still reserved", rather than silently ignoring both.
pub const KNOWN_FLAGS: u16 = FLAG_RETAINED;

/// The epoch value meaning **not declared** (`STORY-P1-10-04`).
///
/// Reserved rather than merely unused: a board that never seeded its epoch and
/// a board running an image from before the field existed both emit zero, and
/// a host must be able to read that as an honest absence instead of as boot
/// number zero. A seeded stream never emits it.
pub const EPOCH_UNDECLARED: u32 = 0;

/// What a frame header says about itself.
///
/// A struct rather than a widening tuple: `seq` and `epoch` are both numbers a
/// host reasons about and a positional pair invites reading one as the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    /// Sequence number of the first record in this frame.
    pub seq: u64,
    /// Records the frame carries.
    pub count: usize,
    /// The boot this frame was emitted by, or [`EPOCH_UNDECLARED`].
    pub epoch: u32,
    /// Frame flags — see [`FLAG_RETAINED`].
    pub flags: u16,
}

impl FrameHeader {
    /// Whether this frame is a re-announcement rather than fresh stream.
    #[must_use]
    pub const fn is_retained(self) -> bool {
        self.flags & FLAG_RETAINED != 0
    }

    /// The sequence a host should expect next, or [`None`] when this frame
    /// says nothing about that.
    ///
    /// `None` for a retained frame, and that is the whole point of the return
    /// type: the arithmetic that would produce a phantom gap is **not
    /// reachable** for a frame that must not be counted, rather than merely
    /// discouraged in a comment a host decoder's author may not read.
    #[must_use]
    pub const fn expected_next(self) -> Option<u64> {
        if self.is_retained() {
            None
        } else {
            Some(self.seq + self.count as u64)
        }
    }
}

/// Why a frame could not be built. Both arms are caller errors, caught before
/// a byte is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpoorWireError {
    /// More than [`MAX_RECORDS`] were offered.
    TooManyRecords,
    /// The output buffer is smaller than the frame requires.
    BufferTooSmall,
}

/// Bytes a frame carrying `count` records occupies.
#[must_use]
pub const fn payload_len(count: usize) -> usize {
    HEADER_LEN + count * 8
}

/// Writes a spoor frame payload into `out`, returning its length.
///
/// `seq` is the sequence number of the **first** record in this frame. The
/// link is unreliable broadcast with no retransmission and no acknowledgement,
/// so the host cannot assume it saw everything; `seq + count` is the next
/// sequence it should expect, and any gap is a measured count of records that
/// were dropped. A stream that cannot say how much it lost is a stream that
/// quietly lies about how much it saw.
///
/// The counter is 64-bit because this stream is meant to run continuously: a
/// 32-bit sequence wraps after ~4 billion records, which at even a modest
/// sustained rate is under an hour, and a wrapped counter turns drop accounting
/// from a measurement into a fiction. At 64 bits it cannot wrap in the life of
/// the hardware.
///
/// `epoch` identifies the boot that emitted the frame, so a listener joining
/// mid-stream can tell "continuing normally" from "joined after a reboot I
/// never saw" — a distinction a sequence number cannot express, because a
/// sequence is a position *within* a boot. See [`EPOCH_UNDECLARED`].
///
/// `flags` is the word this format reserved from the beginning; today
/// [`FLAG_RETAINED`] is its only defined bit.
///
/// # Errors
///
/// [`SpoorWireError::TooManyRecords`] above [`MAX_RECORDS`], or
/// [`SpoorWireError::BufferTooSmall`] if `out` cannot hold the frame. Nothing
/// is written in either case.
pub fn encode(
    seq: u64,
    epoch: u32,
    flags: u16,
    records: &[u64],
    out: &mut [u8],
) -> Result<usize, SpoorWireError> {
    if records.len() > MAX_RECORDS {
        return Err(SpoorWireError::TooManyRecords);
    }
    let len = payload_len(records.len());
    if out.len() < len {
        return Err(SpoorWireError::BufferTooSmall);
    }

    out[0..8].copy_from_slice(&JOURNAL_MAGIC);
    out[8..16].copy_from_slice(&seq.to_le_bytes());
    // `count` and `flags` are `u16` each; the count is bounded by MAX_RECORDS.
    // The four bytes at 20..24 were padding written zero, held so a future
    // field would not have to move the records. `STORY-P1-10-04` is that
    // field: the boot epoch lands in the reserved space and every record stays
    // exactly where every previously captured stream put it.
    let count = records.len() as u16;
    out[16..18].copy_from_slice(&count.to_le_bytes());
    out[18..20].copy_from_slice(&flags.to_le_bytes());
    out[20..24].copy_from_slice(&epoch.to_le_bytes());

    for (index, record) in records.iter().enumerate() {
        let at = HEADER_LEN + index * 8;
        out[at..at + 8].copy_from_slice(&record.to_le_bytes());
    }
    Ok(len)
}

/// What a frame header says it carries.
///
/// Host-side, and deliberately total — it validates rather than trusting, so
/// the same function serves a capture file and a hostile frame identically.
///
/// Neither `epoch` nor `flags` is validated, and neither can be: any 32-bit
/// value is a legal epoch and an unknown flag bit is a newer board, not a
/// malformed frame. Both are reported as read, and a decoder that cares
/// compares `flags & !KNOWN_FLAGS` itself rather than having this function
/// refuse a frame it merely does not fully understand.
///
/// # Errors
///
/// [`SpoorWireError::BufferTooSmall`] if the frame is shorter than its own
/// header or shorter than the records it claims, and
/// [`SpoorWireError::TooManyRecords`] if it claims more than [`MAX_RECORDS`].
/// A wrong magic is reported as [`SpoorWireError::BufferTooSmall`]'s sibling
/// case by returning `Err` rather than a partial decode.
pub fn decode_header(frame: &[u8]) -> Result<FrameHeader, SpoorWireError> {
    if frame.len() < HEADER_LEN || frame[0..8] != JOURNAL_MAGIC {
        return Err(SpoorWireError::BufferTooSmall);
    }
    let mut seq_bytes = [0u8; 8];
    seq_bytes.copy_from_slice(&frame[8..16]);
    let mut count_bytes = [0u8; 2];
    count_bytes.copy_from_slice(&frame[16..18]);
    let count = u16::from_le_bytes(count_bytes) as usize;
    if count > MAX_RECORDS {
        return Err(SpoorWireError::TooManyRecords);
    }
    if frame.len() < payload_len(count) {
        return Err(SpoorWireError::BufferTooSmall);
    }
    let mut flag_bytes = [0u8; 2];
    flag_bytes.copy_from_slice(&frame[18..20]);
    let mut epoch_bytes = [0u8; 4];
    epoch_bytes.copy_from_slice(&frame[20..24]);
    Ok(FrameHeader {
        seq: u64::from_le_bytes(seq_bytes),
        count,
        epoch: u32::from_le_bytes(epoch_bytes),
        flags: u16::from_le_bytes(flag_bytes),
    })
}

/// Reads record `index` out of a validated frame.
///
/// The caller is expected to have called [`decode_header`] and to pass an
/// `index` below the count it reported.
///
/// # Errors
///
/// [`SpoorWireError::BufferTooSmall`] if the frame does not extend that far.
pub fn record(frame: &[u8], index: usize) -> Result<u64, SpoorWireError> {
    let at = HEADER_LEN + index * 8;
    if frame.len() < at + 8 {
        return Err(SpoorWireError::BufferTooSmall);
    }
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&frame[at..at + 8]);
    Ok(u64::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spoor::{Action, Actor, Category, Outcome, Spoor};

    fn sample() -> [u64; 3] {
        [
            Spoor::stamp(Category::Boot, Actor::Kernel, Action::Create, Outcome::Ok, 1, 10)
                .to_bits(),
            Spoor::stamp(Category::Fault, Actor::Kernel, Action::Fault, Outcome::Failed, 2, 20)
                .to_bits(),
            Spoor::stamp(Category::Memory, Actor::Kernel, Action::Select, Outcome::Chose, 3, 30)
                .to_bits(),
        ]
    }

    /// The epoch an ordinary drained frame carries in these tests. Any nonzero
    /// value serves; it is written out rather than hidden behind a helper so
    /// each assertion reads against a literal.
    const EPOCH: u32 = 0x0BAD_CAFE;

    #[test]
    fn the_frame_opens_with_the_journal_s_own_magic() {
        let mut out = [0u8; MAX_PAYLOAD];
        let len = encode(0, EPOCH, 0, &sample(), &mut out).expect("three records fit");
        assert_eq!(&out[0..8], &JOURNAL_MAGIC, "a capture and a journal file parse alike");
        assert_eq!(len, HEADER_LEN + 24);
    }

    /// The board must not format anything: what goes on the wire is the same
    /// packed `u64` the stamp produced, byte for byte.
    #[test]
    fn records_travel_as_raw_packed_bits() {
        let records = sample();
        let mut out = [0u8; MAX_PAYLOAD];
        encode(7, EPOCH, 0, &records, &mut out).expect("records fit");
        for (index, expected) in records.iter().enumerate() {
            assert_eq!(record(&out, index).expect("record present"), *expected);
        }
    }

    #[test]
    fn the_header_reports_the_sequence_and_count_it_was_given() {
        let mut out = [0u8; MAX_PAYLOAD];
        encode(0xDEAD_BEEF_0BAD_F00D, EPOCH, 0, &sample(), &mut out).expect("records fit");
        let header = decode_header(&out).expect("valid header");
        assert_eq!((header.seq, header.count), (0xDEAD_BEEF_0BAD_F00D, 3));
    }

    // ---- the boot epoch and the retained flag (`STORY-P1-10-04`) ----------

    /// `TEST-P1-10-04-A` clause 1: every frame self-identifies, so a listener
    /// joining at any point can tell which boot it is reading.
    #[test]
    fn a_frame_carries_the_boot_epoch_it_was_emitted_by() {
        let mut out = [0u8; MAX_PAYLOAD];
        encode(25_138, 0x1234_5678, 0, &sample(), &mut out).expect("records fit");
        assert_eq!(decode_header(&out).expect("valid").epoch, 0x1234_5678);
    }

    /// The field lands in the space the format reserved for exactly this, so a
    /// stream captured before the epoch existed still decodes record-for-record.
    /// Pinned by offset, because "we had room" is only true until someone moves
    /// something.
    #[test]
    fn the_epoch_occupies_the_reserved_padding_and_moves_no_record() {
        let records = sample();
        let mut without = [0u8; MAX_PAYLOAD];
        let mut with = [0u8; MAX_PAYLOAD];
        let plain = encode(9, EPOCH_UNDECLARED, 0, &records, &mut without).expect("fits");
        let tagged = encode(9, 0xFFFF_FFFF, FLAG_RETAINED, &records, &mut with).expect("fits");

        assert_eq!(plain, tagged, "the frame is not one byte longer for carrying an epoch");
        assert_eq!(without[0..18], with[0..18], "magic, sequence and count are untouched");
        assert_eq!(&with[18..20], &FLAG_RETAINED.to_le_bytes(), "flags at 18..20");
        assert_eq!(&with[20..24], &0xFFFF_FFFFu32.to_le_bytes(), "epoch at 20..24");
        assert_eq!(
            without[HEADER_LEN..plain],
            with[HEADER_LEN..tagged],
            "every record sits where it always sat"
        );
    }

    /// Zero is reserved for *not declared* — an unseeded board or an image
    /// older than the field. A host must read that as an absence rather than
    /// as boot number zero.
    #[test]
    fn an_undeclared_epoch_is_zero_and_says_so() {
        let mut out = [0u8; MAX_PAYLOAD];
        encode(0, EPOCH_UNDECLARED, 0, &sample(), &mut out).expect("fits");
        assert_eq!(decode_header(&out).expect("valid").epoch, 0);
        assert_eq!(EPOCH_UNDECLARED, 0, "the reserved value is pinned, not incidental");
    }

    /// `TEST-P1-10-04-A` clause 5. The flag exists to stop a host counting a
    /// re-announcement as stream, so the test runs the host's own arithmetic
    /// rather than merely asserting the bit.
    #[test]
    fn a_retained_frame_is_marked_and_says_nothing_about_what_comes_next() {
        let mut drained = [0u8; MAX_PAYLOAD];
        encode(400, EPOCH, 0, &sample(), &mut drained).expect("fits");
        let mut retained = [0u8; MAX_PAYLOAD];
        encode(0, EPOCH, FLAG_RETAINED, &sample(), &mut retained).expect("fits");

        let fresh = decode_header(&drained).expect("valid");
        let announced = decode_header(&retained).expect("valid");

        assert!(!fresh.is_retained(), "a drained frame is stream");
        assert!(announced.is_retained(), "an announced frame is not");
        assert_eq!(fresh.expected_next(), Some(403), "stream advances the expectation");
        assert_eq!(
            announced.expected_next(),
            None,
            "a retained frame must not be able to produce a phantom gap"
        );
    }

    /// An unknown flag bit is a newer board, not a malformed frame. Refusing
    /// one would make every future flag a flag day.
    #[test]
    fn an_unknown_flag_bit_is_reported_rather_than_refused() {
        let mut out = [0u8; MAX_PAYLOAD];
        encode(1, EPOCH, 0x8000, &sample(), &mut out).expect("fits");
        let header = decode_header(&out).expect("a frame with a newer flag still decodes");
        assert_eq!(
            header.flags & !KNOWN_FLAGS,
            0x8000,
            "the decoder can name what it did not know"
        );
        assert!(!header.is_retained(), "and does not mistake it for a flag it does know");
    }

    /// A 32-bit counter wraps in under an hour on a continuously streaming
    /// system, and a wrapped counter makes drop accounting a fiction rather
    /// than a measurement.
    #[test]
    fn the_sequence_survives_past_the_point_a_32_bit_counter_would_wrap() {
        let beyond_u32 = u64::from(u32::MAX) + 1_000;
        let mut out = [0u8; MAX_PAYLOAD];
        encode(beyond_u32, EPOCH, 0, &sample(), &mut out).expect("records fit");
        assert_eq!(decode_header(&out).expect("valid").seq, beyond_u32);
    }

    /// One transmit must carry a stream, not a sample of one.
    #[test]
    fn a_full_frame_fills_a_standard_mtu() {
        let full = [0u64; MAX_RECORDS];
        let mut out = [0u8; MAX_PAYLOAD];
        let len = encode(0, EPOCH, 0, &full, &mut out).expect("a full frame is legal");
        assert_eq!(len, MAX_PAYLOAD);
        assert!(len + 14 <= 1514, "frame stays inside a standard MTU: {len}");
        assert!(len + 14 > 1000, "and is not sized for a trickle: {len}");
    }

    /// The link is unreliable broadcast. A host that cannot count what it lost
    /// would report a partial stream as a complete one.
    #[test]
    fn a_gap_between_frames_is_a_measurable_number_of_lost_records() {
        let mut first = [0u8; MAX_PAYLOAD];
        encode(100, EPOCH, 0, &sample(), &mut first).expect("fits");
        let mut later = [0u8; MAX_PAYLOAD];
        encode(109, EPOCH, 0, &sample(), &mut later).expect("fits");

        let expected_next =
            decode_header(&first).expect("valid").expected_next().expect("stream, not retained");
        let later_seq = decode_header(&later).expect("valid").seq;
        assert_eq!(later_seq - expected_next, 6, "six records were dropped, and it is countable");
    }

    #[test]
    fn an_empty_frame_is_a_header_and_nothing_else() {
        let mut out = [0u8; MAX_PAYLOAD];
        let len = encode(5, EPOCH, 0, &[], &mut out).expect("an empty frame is legal");
        assert_eq!(len, HEADER_LEN);
        let header = decode_header(&out).expect("valid");
        assert_eq!((header.seq, header.count), (5, 0));
    }

    #[test]
    fn more_records_than_a_frame_holds_are_refused_not_truncated() {
        let too_many = [0u64; MAX_RECORDS + 1];
        let mut out = [0u8; MAX_PAYLOAD * 2];
        assert_eq!(encode(0, EPOCH, 0, &too_many, &mut out), Err(SpoorWireError::TooManyRecords));
    }

    #[test]
    fn a_buffer_too_small_is_refused_before_a_byte_is_written() {
        let mut out = [0u8; HEADER_LEN + 8];
        assert_eq!(encode(0, EPOCH, 0, &sample(), &mut out), Err(SpoorWireError::BufferTooSmall));
        assert!(out.iter().all(|byte| *byte == 0), "nothing is written on refusal");
    }

    // ---- decoding a frame that is not ours -------------------------------

    /// Every rejection below is one early return over fixed-width fields.
    /// There is no length field to lie about, no options to walk and no
    /// fragment to reassemble, which is the whole argument for this shape.
    #[test]
    fn a_frame_that_is_not_ours_is_refused_rather_than_parsed() {
        assert!(decode_header(&[]).is_err(), "an empty frame");
        assert!(decode_header(&[0u8; HEADER_LEN]).is_err(), "a zeroed frame has the wrong magic");
        let mut wrong = [0u8; MAX_PAYLOAD];
        wrong[0..8].copy_from_slice(b"NOTSPOOR");
        assert!(decode_header(&wrong).is_err(), "a wrong magic is refused");
    }

    /// The one field an attacker could inflate, bounded by a constant rather
    /// than by the buffer it would be used to index.
    #[test]
    fn a_count_larger_than_the_format_allows_is_refused() {
        let mut lying = [0u8; MAX_PAYLOAD];
        lying[0..8].copy_from_slice(&JOURNAL_MAGIC);
        lying[16..18].copy_from_slice(&(MAX_RECORDS as u16 + 1).to_le_bytes());
        assert_eq!(decode_header(&lying), Err(SpoorWireError::TooManyRecords));
    }

    /// A count that fits the format but not the frame: the truncation case a
    /// length-trusting parser would read past.
    #[test]
    fn a_count_the_frame_is_too_short_to_carry_is_refused() {
        let mut truncated = [0u8; HEADER_LEN + 8];
        truncated[0..8].copy_from_slice(&JOURNAL_MAGIC);
        truncated[16..18].copy_from_slice(&4u16.to_le_bytes());
        assert_eq!(decode_header(&truncated), Err(SpoorWireError::BufferTooSmall));
    }

    #[test]
    fn a_record_read_past_the_end_is_refused() {
        let mut out = [0u8; MAX_PAYLOAD];
        encode(0, EPOCH, 0, &sample(), &mut out).expect("fits");
        assert!(record(&out[..HEADER_LEN + 16], 2).is_err(), "the third record is not there");
    }
}
