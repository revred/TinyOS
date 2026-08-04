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
/// Twenty-four rather than the twenty the fields need, so records begin
/// 8-byte aligned: the payload is a bulk copy of packed `u64`s and the board
/// should never be doing unaligned stores to build it.
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
/// # Errors
///
/// [`SpoorWireError::TooManyRecords`] above [`MAX_RECORDS`], or
/// [`SpoorWireError::BufferTooSmall`] if `out` cannot hold the frame. Nothing
/// is written in either case.
pub fn encode(seq: u64, records: &[u64], out: &mut [u8]) -> Result<usize, SpoorWireError> {
    if records.len() > MAX_RECORDS {
        return Err(SpoorWireError::TooManyRecords);
    }
    let len = payload_len(records.len());
    if out.len() < len {
        return Err(SpoorWireError::BufferTooSmall);
    }

    out[0..8].copy_from_slice(&JOURNAL_MAGIC);
    out[8..16].copy_from_slice(&seq.to_le_bytes());
    // `count` and `flags` are `u16` each: the count is bounded by MAX_RECORDS
    // and the flags word is reserved, written zero, and read by nobody yet.
    // It exists so a future field does not have to move the records.
    let count = records.len() as u16;
    out[16..18].copy_from_slice(&count.to_le_bytes());
    out[18..20].copy_from_slice(&0u16.to_le_bytes());
    out[20..24].copy_from_slice(&0u32.to_le_bytes());

    for (index, record) in records.iter().enumerate() {
        let at = HEADER_LEN + index * 8;
        out[at..at + 8].copy_from_slice(&record.to_le_bytes());
    }
    Ok(len)
}

/// What a frame header says it carries: `(seq, count)`.
///
/// Host-side, and deliberately total — it validates rather than trusting, so
/// the same function serves a capture file and a hostile frame identically.
///
/// # Errors
///
/// [`SpoorWireError::BufferTooSmall`] if the frame is shorter than its own
/// header or shorter than the records it claims, and
/// [`SpoorWireError::TooManyRecords`] if it claims more than [`MAX_RECORDS`].
/// A wrong magic is reported as [`SpoorWireError::BufferTooSmall`]'s sibling
/// case by returning `Err` rather than a partial decode.
pub fn decode_header(frame: &[u8]) -> Result<(u64, usize), SpoorWireError> {
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
    Ok((u64::from_le_bytes(seq_bytes), count))
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

    #[test]
    fn the_frame_opens_with_the_journal_s_own_magic() {
        let mut out = [0u8; MAX_PAYLOAD];
        let len = encode(0, &sample(), &mut out).expect("three records fit");
        assert_eq!(&out[0..8], &JOURNAL_MAGIC, "a capture and a journal file parse alike");
        assert_eq!(len, HEADER_LEN + 24);
    }

    /// The board must not format anything: what goes on the wire is the same
    /// packed `u64` the stamp produced, byte for byte.
    #[test]
    fn records_travel_as_raw_packed_bits() {
        let records = sample();
        let mut out = [0u8; MAX_PAYLOAD];
        encode(7, &records, &mut out).expect("records fit");
        for (index, expected) in records.iter().enumerate() {
            assert_eq!(record(&out, index).expect("record present"), *expected);
        }
    }

    #[test]
    fn the_header_reports_the_sequence_and_count_it_was_given() {
        let mut out = [0u8; MAX_PAYLOAD];
        encode(0xDEAD_BEEF_0BAD_F00D, &sample(), &mut out).expect("records fit");
        assert_eq!(decode_header(&out).expect("valid header"), (0xDEAD_BEEF_0BAD_F00D, 3));
    }

    /// A 32-bit counter wraps in under an hour on a continuously streaming
    /// system, and a wrapped counter makes drop accounting a fiction rather
    /// than a measurement.
    #[test]
    fn the_sequence_survives_past_the_point_a_32_bit_counter_would_wrap() {
        let beyond_u32 = u64::from(u32::MAX) + 1_000;
        let mut out = [0u8; MAX_PAYLOAD];
        encode(beyond_u32, &sample(), &mut out).expect("records fit");
        assert_eq!(decode_header(&out).expect("valid").0, beyond_u32);
    }

    /// One transmit must carry a stream, not a sample of one.
    #[test]
    fn a_full_frame_fills_a_standard_mtu() {
        let full = [0u64; MAX_RECORDS];
        let mut out = [0u8; MAX_PAYLOAD];
        let len = encode(0, &full, &mut out).expect("a full frame is legal");
        assert_eq!(len, MAX_PAYLOAD);
        assert!(len + 14 <= 1514, "frame stays inside a standard MTU: {len}");
        assert!(len + 14 > 1000, "and is not sized for a trickle: {len}");
    }

    /// The link is unreliable broadcast. A host that cannot count what it lost
    /// would report a partial stream as a complete one.
    #[test]
    fn a_gap_between_frames_is_a_measurable_number_of_lost_records() {
        let mut first = [0u8; MAX_PAYLOAD];
        encode(100, &sample(), &mut first).expect("fits");
        let mut later = [0u8; MAX_PAYLOAD];
        encode(109, &sample(), &mut later).expect("fits");

        let (first_seq, first_count) = decode_header(&first).expect("valid");
        let (later_seq, _) = decode_header(&later).expect("valid");
        let expected_next = first_seq + first_count as u64;
        assert_eq!(later_seq - expected_next, 6, "six records were dropped, and it is countable");
    }

    #[test]
    fn an_empty_frame_is_a_header_and_nothing_else() {
        let mut out = [0u8; MAX_PAYLOAD];
        let len = encode(5, &[], &mut out).expect("an empty frame is legal");
        assert_eq!(len, HEADER_LEN);
        assert_eq!(decode_header(&out).expect("valid"), (5, 0));
    }

    #[test]
    fn more_records_than_a_frame_holds_are_refused_not_truncated() {
        let too_many = [0u64; MAX_RECORDS + 1];
        let mut out = [0u8; MAX_PAYLOAD * 2];
        assert_eq!(encode(0, &too_many, &mut out), Err(SpoorWireError::TooManyRecords));
    }

    #[test]
    fn a_buffer_too_small_is_refused_before_a_byte_is_written() {
        let mut out = [0u8; HEADER_LEN + 8];
        assert_eq!(encode(0, &sample(), &mut out), Err(SpoorWireError::BufferTooSmall));
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
        encode(0, &sample(), &mut out).expect("fits");
        assert!(record(&out[..HEADER_LEN + 16], 2).is_err(), "the third record is not there");
    }
}
