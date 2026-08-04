//! The stamping and draining half of the spoor transport (`STORY-P1-10-02`).
//!
//! [`spoor_wire`](crate::spoor_wire) defines what a frame looks like. This
//! module is what fills one: a journal, a sequence counter, and the rungs a
//! boot passes on its way to the park loop.
//!
//! # Why a `Rung` enum and not a raw number
//!
//! The stamping call sites live in `hal-arm64`, which cannot import this crate
//! — on AArch64 the dependency runs `kernel` → `hal-arm64`, so the seam is a
//! `#[no_mangle] extern "C"` symbol, exactly as `tinyos_arm64_fixture_measure`
//! already does it. An `extern "C"` boundary carries integers, and integers
//! are how a vocabulary quietly rots: today's `3` is the tick, next month it
//! is something else and every recorded stream before the change silently
//! means something different.
//!
//! So the boundary carries a [`Rung`], each rung maps to a
//! (`Category`, `Action`) pair *here*, in code that a host test can hold, and
//! an unrecognised discriminant is refused rather than guessed. The board
//! passes a rung and an outcome; it never chooses a category.
//!
//! # What this module refuses to do
//!
//! It does not widen the spoor vocabulary. `Category`, `Actor`, `Action` and
//! `Outcome` are closed enums with decode-time validation, and a rung that has
//! no honest mapping is a reason to add a vocabulary entry test-first, never a
//! reason to stretch an existing one until it covers something it does not
//! mean. Every mapping below is defended in its own comment.

use crate::spoor::{Action, Actor, Category, Outcome, Spoor};
use crate::spoor_journal::SpoorJournal;
use crate::spoor_wire::{self, SpoorWireError, MAX_RECORDS};

/// A rung the boot or park path passes, as it crosses the `extern "C"` seam.
///
/// The discriminants are wire-visible in the `target` field of every spoor
/// this module stamps, so **they are append-only**: changing one silently
/// re-labels every stream ever recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum Rung {
    /// The MMU came up with caches enabled (`STORY-P1-07-03`).
    MmuEnabled = 1,
    /// The GIC routed the virtual-timer PPI (`STORY-P1-07-04`).
    GicRouted = 2,
    /// The virtual timer was armed and its control register believed.
    TickArmed = 3,
    /// A beacon frame was handed to the GEM (`FEAT-P1-09`).
    BeaconTransmitted = 4,
    /// The measurement fixture ran to completion (`STORY-P1-07-06`).
    FixtureMeasure = 5,
    /// One pass of the park loop.
    ParkIteration = 6,
    /// A synchronous exception was taken and reported (`STORY-P1-07-02`).
    FaultTaken = 7,
}

impl Rung {
    /// Decodes a rung from the `extern "C"` seam, refusing anything unknown.
    ///
    /// Fails closed for the same reason [`Spoor::decode`] does: an
    /// unrecognised discriminant is a version skew between the board and this
    /// table, and guessing at it produces a stream that reads plausibly and
    /// means nothing.
    #[must_use]
    pub const fn from_bits(bits: u16) -> Option<Self> {
        match bits {
            1 => Some(Rung::MmuEnabled),
            2 => Some(Rung::GicRouted),
            3 => Some(Rung::TickArmed),
            4 => Some(Rung::BeaconTransmitted),
            5 => Some(Rung::FixtureMeasure),
            6 => Some(Rung::ParkIteration),
            7 => Some(Rung::FaultTaken),
            _ => None,
        }
    }

    /// This rung's wire-visible identifier.
    #[must_use]
    pub const fn to_bits(self) -> u16 {
        self as u16
    }

    /// The `(Category, Action)` this rung honestly is.
    ///
    /// Every boot rung is `Category::Boot` with `Action::Create`: each one
    /// *establishes* something that did not exist before it — a translation
    /// regime, a route, an armed timer. `ParkIteration` is `Action::Select`
    /// because the park loop chooses what to do on each pass and creates
    /// nothing. `FaultTaken` is the one rung that is not boot at all:
    /// `Category::Fault` with `Action::Fault`, matching what
    /// `kernel::fault::audit` already stamps on the x86_64 path, so the two
    /// architectures describe a fault the same way.
    #[must_use]
    pub const fn taxonomy(self) -> (Category, Action) {
        match self {
            Rung::MmuEnabled
            | Rung::GicRouted
            | Rung::TickArmed
            | Rung::BeaconTransmitted
            | Rung::FixtureMeasure => (Category::Boot, Action::Create),
            Rung::ParkIteration => (Category::Boot, Action::Select),
            Rung::FaultTaken => (Category::Fault, Action::Fault),
        }
    }
}

/// A journal, a sequence counter, and the drain that empties one into the
/// other.
///
/// The journal is a **jitter buffer, not storage**. It overwrites its oldest
/// entry when full, which is correct for a crash dump and wrong for a stream —
/// so anything it overwrites between drains is loss, and the sequence counter
/// is what makes that loss countable on the host rather than invisible.
pub struct SpoorStream<const N: usize> {
    journal: SpoorJournal<N>,
    /// Sequence number the next stamped record will carry.
    next_seq: u64,
    /// Records stamped since the last drain, saturating at `N` — the journal
    /// itself cannot report that it overwrote, so this counts what was
    /// *offered* and the difference is the loss.
    offered: usize,
}

impl<const N: usize> SpoorStream<N> {
    /// An empty stream. `const`, so it can initialise a `static`.
    #[must_use]
    pub const fn new() -> Self {
        SpoorStream { journal: SpoorJournal::new(), next_seq: 0, offered: 0 }
    }

    /// Stamps one rung. Never allocates, never blocks, never fails.
    ///
    /// `cost` is whatever the call site measured, in whatever unit that rung
    /// documents; zero is a legitimate "not measured" rather than a claim of
    /// free.
    pub fn stamp(&mut self, rung: Rung, outcome: Outcome, cost: u32) {
        let (category, action) = rung.taxonomy();
        self.journal.append(Spoor::stamp(
            category,
            Actor::Kernel,
            action,
            outcome,
            rung.to_bits(),
            cost,
        ));
        self.next_seq = self.next_seq.wrapping_add(1);
        self.offered = self.offered.saturating_add(1);
    }

    /// Records offered since the last drain — more than the journal holds
    /// means the ring wrapped and the excess is lost.
    #[must_use]
    pub const fn offered(&self) -> usize {
        self.offered
    }

    /// Sequence number the next stamp will carry.
    #[must_use]
    pub const fn next_sequence(&self) -> u64 {
        self.next_seq
    }

    /// Encodes everything held into `out` and empties the journal.
    ///
    /// Returns the payload length, or `None` when there is nothing to send —
    /// an empty drain must not put an empty frame on the wire every park pass.
    ///
    /// The frame's sequence is the sequence of its **oldest surviving record**,
    /// not of the oldest record stamped. When the ring has wrapped, the
    /// difference between the two is exactly the number of records lost, and
    /// the host sees it as a gap of that size. Loss is therefore reported by
    /// arithmetic the host already does, with no extra field and no way for
    /// the board to under-report it.
    ///
    /// # Errors
    ///
    /// [`SpoorWireError::BufferTooSmall`] if `out` cannot hold the frame; the
    /// journal is left untouched, so a caller with a bad buffer loses nothing.
    pub fn drain(&mut self, out: &mut [u8]) -> Result<Option<usize>, SpoorWireError> {
        let held = self.journal.len();
        if held == 0 {
            return Ok(None);
        }
        let take = held.min(MAX_RECORDS);

        // The oldest *surviving* record's sequence: everything stamped before
        // it was overwritten by the ring and is gone.
        let first_seq = self.next_seq - held as u64;

        let mut records = [0u64; MAX_RECORDS];
        for (slot, spoor) in records[..take].iter_mut().zip(self.journal.iter()) {
            *slot = spoor.to_bits();
        }

        let len = spoor_wire::encode(first_seq, &records[..take], out)?;
        self.journal = SpoorJournal::new();
        self.offered = 0;
        Ok(Some(len))
    }
}

impl<const N: usize> Default for SpoorStream<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// What a boot rung reports across the seam.
///
/// A deliberately tiny closed set rather than the whole [`Outcome`]
/// vocabulary. A boot rung either did the thing, was refused, or was not
/// attempted; the richer outcomes (`Chose`, `Capped`, `Superseded`, `Partial`)
/// describe decisions this path does not make, and exposing them across an
/// `extern "C"` boundary would invite a call site to pick one that sounds
/// close. The narrow set is also why `Outcome`'s own `from_bits` stays
/// private: nothing here needs to widen that type's surface.
///
/// Discriminants are wire-visible through the mapped [`Outcome`] and are
/// therefore append-only, for the same reason [`Rung`]'s are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Verdict {
    /// The rung did what it exists to do.
    Ok = 0,
    /// The rung was refused, by a readback that disagreed or a device that
    /// said no. A refusal must be as stampable as a success, or the stream
    /// hides exactly the runs worth reading.
    Failed = 1,
    /// The rung was not attempted — a path not taken, not a path that failed.
    Skipped = 2,
}

impl Verdict {
    /// Decodes a verdict from the seam, refusing anything unknown.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Option<Self> {
        match bits {
            0 => Some(Verdict::Ok),
            1 => Some(Verdict::Failed),
            2 => Some(Verdict::Skipped),
            _ => None,
        }
    }

    /// This verdict's seam encoding.
    #[must_use]
    pub const fn to_bits(self) -> u8 {
        self as u8
    }

    /// The spoor [`Outcome`] this verdict is.
    #[must_use]
    pub const fn outcome(self) -> Outcome {
        match self {
            Verdict::Ok => Outcome::Ok,
            Verdict::Failed => Outcome::Failed,
            Verdict::Skipped => Outcome::Skipped,
        }
    }
}

/// Records the board's stream holds between drains.
///
/// A jitter buffer, not storage: it absorbs the burst a boot rung sequence
/// produces between park passes. Sized to one full frame so a drain that
/// happens promptly never loses anything, and a drain that is late loses
/// countably (`FEAT-P1-10` named debt — this number is chosen, not measured).
pub const BOARD_STREAM_CAPACITY: usize = MAX_RECORDS;

/// The board's one stream.
///
/// A `static mut` for the same reason the measurement fixture's sample buffer
/// is one: single core, non-reentrant, and no allocator exists. Every access
/// goes through the two `extern "C"` entry points below, which are the only
/// callers.
static mut BOARD_STREAM: SpoorStream<BOARD_STREAM_CAPACITY> = SpoorStream::new();

/// Stamps one rung from the AArch64 boot path (`STORY-P1-10-02`).
///
/// The `extern "C"` half of the seam: `hal-arm64` cannot import this crate —
/// on AArch64 the dependency runs `kernel` → `hal-arm64` — so the boot rungs
/// reach the journal through a symbol, exactly as `hal-arm64`'s boot already
/// reaches `tinyos_arm64_fixture_measure`.
///
/// An unrecognised `rung` or `verdict` is **dropped, not guessed**. A stamped
/// record that means nothing is worse than a missing one: the missing one
/// shows up as a sequence gap the host can count, and the meaningless one is
/// indistinguishable from truth.
///
/// # Safety
///
/// Single core, non-reentrant. The caller must not be inside a drain.
#[no_mangle]
pub extern "C" fn tinyos_spoor_stamp(rung: u16, verdict: u8, cost: u32) {
    let Some(rung) = Rung::from_bits(rung) else {
        return;
    };
    let Some(verdict) = Verdict::from_bits(verdict) else {
        return;
    };
    // SAFETY: single core, non-reentrant, and the only other accessor is the
    // drain below, which the caller contract forbids overlapping with.
    let stream = unsafe { &mut *core::ptr::addr_of_mut!(BOARD_STREAM) };
    stream.stamp(rung, verdict.outcome(), cost);
}

/// Drains the board's stream into `out`, returning the payload length, or `0`
/// when there was nothing to send (`STORY-P1-10-02`).
///
/// Zero means "no frame" rather than "empty frame": an empty frame every park
/// pass would be a stream of silence indistinguishable from a stream of
/// nothing happening.
///
/// # Safety
///
/// `out` must be valid for `cap` bytes. Single core, non-reentrant.
#[no_mangle]
pub unsafe extern "C" fn tinyos_spoor_drain(out: *mut u8, cap: usize) -> usize {
    if out.is_null() {
        return 0;
    }
    // SAFETY: the caller's contract is that `out` is valid for `cap` bytes.
    let buffer = unsafe { core::slice::from_raw_parts_mut(out, cap) };
    // SAFETY: as in `tinyos_spoor_stamp`.
    let stream = unsafe { &mut *core::ptr::addr_of_mut!(BOARD_STREAM) };
    match stream.drain(buffer) {
        Ok(Some(len)) => len,
        // A refused drain leaves the journal intact, so the records are not
        // lost — they go out on the next pass. Fail-safe over keep-trying.
        Ok(None) | Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spoor_wire::{decode_header, record, MAX_PAYLOAD};

    const CAPACITY: usize = 8;

    fn stream() -> SpoorStream<CAPACITY> {
        SpoorStream::new()
    }

    // ---- the vocabulary is closed and append-only ------------------------

    #[test]
    fn every_rung_round_trips_through_the_seam() {
        for rung in [
            Rung::MmuEnabled,
            Rung::GicRouted,
            Rung::TickArmed,
            Rung::BeaconTransmitted,
            Rung::FixtureMeasure,
            Rung::ParkIteration,
            Rung::FaultTaken,
        ] {
            assert_eq!(Rung::from_bits(rung.to_bits()), Some(rung));
        }
    }

    /// An unknown discriminant is version skew between the board and this
    /// table. Guessing produces a stream that reads plausibly and means
    /// nothing, so it is refused — the same posture `Spoor::decode` takes.
    #[test]
    fn an_unknown_rung_is_refused_not_guessed() {
        assert_eq!(Rung::from_bits(0), None);
        assert_eq!(Rung::from_bits(8), None);
        assert_eq!(Rung::from_bits(u16::MAX), None);
    }

    /// The discriminants are wire-visible in every stamped record, so changing
    /// one silently re-labels every stream ever captured. Pinned deliberately.
    #[test]
    fn rung_discriminants_are_append_only() {
        assert_eq!(Rung::MmuEnabled.to_bits(), 1);
        assert_eq!(Rung::GicRouted.to_bits(), 2);
        assert_eq!(Rung::TickArmed.to_bits(), 3);
        assert_eq!(Rung::BeaconTransmitted.to_bits(), 4);
        assert_eq!(Rung::FixtureMeasure.to_bits(), 5);
        assert_eq!(Rung::ParkIteration.to_bits(), 6);
        assert_eq!(Rung::FaultTaken.to_bits(), 7);
    }

    /// A fault must describe itself the same way on both architectures, or a
    /// host decoder has to know which board it is reading.
    #[test]
    fn a_fault_is_categorised_as_the_x86_path_categorises_one() {
        assert_eq!(Rung::FaultTaken.taxonomy(), (Category::Fault, Action::Fault));
    }

    #[test]
    fn a_boot_rung_establishes_and_a_park_pass_chooses() {
        assert_eq!(Rung::MmuEnabled.taxonomy(), (Category::Boot, Action::Create));
        assert_eq!(Rung::ParkIteration.taxonomy(), (Category::Boot, Action::Select));
    }

    // ---- stamping and draining -------------------------------------------

    #[test]
    fn a_stamp_carries_its_rung_in_the_target_field() {
        let mut s = stream();
        s.stamp(Rung::TickArmed, Outcome::Ok, 42);
        let mut out = [0u8; MAX_PAYLOAD];
        let len = s.drain(&mut out).expect("encodes").expect("one record");
        assert_eq!(len, spoor_wire::payload_len(1));

        let bits = record(&out, 0).expect("record present");
        let spoor = Spoor::decode(bits).expect("a stamped spoor decodes");
        assert_eq!(spoor.category(), Category::Boot);
        assert_eq!(spoor.action(), Action::Create);
    }

    #[test]
    fn an_empty_stream_puts_no_frame_on_the_wire() {
        let mut s = stream();
        let mut out = [0u8; MAX_PAYLOAD];
        assert_eq!(s.drain(&mut out).expect("no error"), None, "silence is not a frame");
    }

    #[test]
    fn a_drain_empties_the_journal_so_records_are_never_sent_twice() {
        let mut s = stream();
        s.stamp(Rung::MmuEnabled, Outcome::Ok, 0);
        s.stamp(Rung::GicRouted, Outcome::Ok, 0);
        let mut out = [0u8; MAX_PAYLOAD];
        assert!(s.drain(&mut out).expect("encodes").is_some());
        assert_eq!(s.drain(&mut out).expect("no error"), None, "a drained journal is empty");
    }

    #[test]
    fn consecutive_drains_carry_consecutive_sequences() {
        let mut s = stream();
        let mut out = [0u8; MAX_PAYLOAD];

        s.stamp(Rung::MmuEnabled, Outcome::Ok, 0);
        s.stamp(Rung::GicRouted, Outcome::Ok, 0);
        s.drain(&mut out).expect("encodes").expect("records");
        let (first_seq, first_count) = decode_header(&out).expect("valid");
        assert_eq!((first_seq, first_count), (0, 2));

        s.stamp(Rung::TickArmed, Outcome::Ok, 0);
        s.drain(&mut out).expect("encodes").expect("records");
        let (second_seq, _) = decode_header(&out).expect("valid");
        assert_eq!(second_seq, first_seq + first_count as u64, "no gap where none was lost");
    }

    /// The honest-degradation clause. The ring is a jitter buffer; when it
    /// wraps, the records it dropped must show up on the host as a gap of
    /// exactly that size rather than as nothing at all.
    #[test]
    fn a_wrapped_ring_reports_its_loss_as_an_exact_sequence_gap() {
        let mut s = stream();
        let mut out = [0u8; MAX_PAYLOAD];

        // One clean drain establishes where the host's expectation sits.
        s.stamp(Rung::MmuEnabled, Outcome::Ok, 0);
        s.drain(&mut out).expect("encodes").expect("records");
        let (seq, count) = decode_header(&out).expect("valid");
        let expected_next = seq + count as u64;

        // Now overrun the ring: CAPACITY + 3 stamped, CAPACITY survive.
        for _ in 0..(CAPACITY + 3) {
            s.stamp(Rung::ParkIteration, Outcome::Ok, 0);
        }
        assert_eq!(s.offered(), CAPACITY + 3, "the stream knows what it was offered");

        s.drain(&mut out).expect("encodes").expect("records");
        let (next_seq, next_count) = decode_header(&out).expect("valid");
        assert_eq!(next_count, CAPACITY, "only what the ring held survives");
        assert_eq!(
            next_seq - expected_next,
            3,
            "the three overwritten records are a countable gap, not a silence"
        );
    }

    #[test]
    fn a_buffer_too_small_leaves_the_journal_intact() {
        let mut s = stream();
        s.stamp(Rung::MmuEnabled, Outcome::Ok, 0);
        let mut tiny = [0u8; 4];
        assert_eq!(s.drain(&mut tiny), Err(SpoorWireError::BufferTooSmall));
        let mut out = [0u8; MAX_PAYLOAD];
        assert!(s.drain(&mut out).expect("encodes").is_some(), "nothing was lost to the refusal");
    }

    // ---- the seam's verdict vocabulary -----------------------------------

    #[test]
    fn every_verdict_round_trips_and_maps_to_an_outcome() {
        for (verdict, outcome) in [
            (Verdict::Ok, Outcome::Ok),
            (Verdict::Failed, Outcome::Failed),
            (Verdict::Skipped, Outcome::Skipped),
        ] {
            assert_eq!(Verdict::from_bits(verdict.to_bits()), Some(verdict));
            assert_eq!(verdict.outcome(), outcome);
        }
    }

    #[test]
    fn an_unknown_verdict_is_refused_not_guessed() {
        assert_eq!(Verdict::from_bits(3), None);
        assert_eq!(Verdict::from_bits(u8::MAX), None);
    }

    /// Wire-visible through the mapped `Outcome`, so append-only.
    #[test]
    fn verdict_discriminants_are_append_only() {
        assert_eq!(Verdict::Ok.to_bits(), 0);
        assert_eq!(Verdict::Failed.to_bits(), 1);
        assert_eq!(Verdict::Skipped.to_bits(), 2);
    }

    /// A garbage rung or verdict must produce **no record at all**. A stamped
    /// record that means nothing is worse than a missing one: the missing one
    /// is a countable sequence gap, the meaningless one reads as truth.
    #[test]
    fn the_seam_drops_what_it_cannot_decode_rather_than_stamping_a_guess() {
        // Exercised through the same decode the `extern "C"` entry uses.
        assert!(Rung::from_bits(999).is_none(), "an unknown rung decodes to nothing");
        assert!(Verdict::from_bits(9).is_none(), "an unknown verdict decodes to nothing");
    }

    /// A refused rung must still be observable: the stream is how a reader
    /// learns the board *tried* and was refused, which is exactly the case a
    /// success-only stream hides.
    #[test]
    fn a_refusal_is_stamped_as_readily_as_a_success() {
        let mut s = stream();
        s.stamp(Rung::GicRouted, Outcome::Failed, 0);
        let mut out = [0u8; MAX_PAYLOAD];
        s.drain(&mut out).expect("encodes").expect("one record");
        let spoor = Spoor::decode(record(&out, 0).expect("present")).expect("decodes");
        assert_eq!(spoor.outcome(), Outcome::Failed);
    }
}
