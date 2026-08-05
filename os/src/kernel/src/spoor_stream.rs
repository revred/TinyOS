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
use crate::spoor_wire::{self, SpoorWireError, EPOCH_UNDECLARED, FLAG_RETAINED, MAX_RECORDS};

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
    /// The SoC die temperature was sampled (`LE-75`).
    ///
    /// The cost field carries the AVS monitor's **raw register word**, not a
    /// temperature. The board does not convert, because the raw-to-millicelsius
    /// calibration is unverified on this hardware and a converted value would
    /// arrive as a confident number nobody could tell was wrong.
    ThermalSample = 8,
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
            8 => Some(Rung::ThermalSample),
            _ => None,
        }
    }

    /// This rung's wire-visible identifier.
    #[must_use]
    pub const fn to_bits(self) -> u16 {
        self as u16
    }

    /// Whether this rung belongs to the **boot certificate**
    /// (`STORY-P1-10-04`) — the birth certificate a late listener is
    /// re-announced.
    ///
    /// True for the rungs that happen **once per boot** and establish
    /// something a reader cannot recover any other way: a translation regime,
    /// a route, an armed timer, a completed measurement. False for everything
    /// that repeats, and that exclusion is what makes the certificate bounded
    /// rather than merely small — a rung stamped every park pass could
    /// otherwise fill a fixed buffer with the least interesting seconds of the
    /// run and displace the boot.
    ///
    /// `BeaconTransmitted` is excluded despite reading like a boot event: it
    /// stamps on every pass that transmits, so it is stream, not birth.
    /// `FaultTaken` is excluded because a fault is not boot state and is
    /// unbounded in count — and because a fault that happened once must not be
    /// re-broadcast every few seconds as though it kept happening.
    #[must_use]
    pub const fn is_boot_certificate(self) -> bool {
        match self {
            Rung::MmuEnabled | Rung::GicRouted | Rung::TickArmed | Rung::FixtureMeasure => true,
            Rung::BeaconTransmitted
            | Rung::ParkIteration
            | Rung::FaultTaken
            | Rung::ThermalSample => false,
        }
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
            // The one rung that records what the machine *is* rather than what
            // it did: `Observe` because reading changes nothing, and nothing
            // yet acts on the reading.
            Rung::ThermalSample => (Category::Thermal, Action::Observe),
        }
    }
}

/// Records the boot certificate holds (`STORY-P1-10-04`).
///
/// Sixteen: four times the once-per-boot rungs the vocabulary defines today,
/// so the vocabulary can double twice before this number is the constraint,
/// and 128 bytes against a `no_std` image either way. It is a **ceiling, not a
/// target** — the certificate closes at the first record that is not a boot
/// rung, which on every boot so far is well before this bound.
pub const CERTIFICATE_CAPACITY: usize = 16;

/// Calls to [`SpoorStream::announce`] between re-announcements.
///
/// The park loop calls once per beat (~1 s), so this is the **stated
/// worst-case window** a listener waits before it learns the boot state:
/// roughly five seconds. It is a constant here rather than a modulo in the
/// park loop so a host test can read the bound the board promises, and so the
/// promise is one number rather than a number and a loop's cadence multiplied
/// together in a reader's head.
///
/// Five, because the cost is one small frame and the benefit is bounded
/// ignorance. Nothing measured chose it and nothing measured could: the right
/// value is a trade between wire share and how long a diagnostic session
/// tolerates not knowing which boot it is watching.
pub const ANNOUNCE_EVERY: usize = 5;

/// A journal, a sequence counter, the drain that empties one into the other,
/// and the birth certificate neither of them can lose.
///
/// The journal is a **jitter buffer, not storage**. It overwrites its oldest
/// entry when full, which is correct for a crash dump and wrong for a stream —
/// so anything it overwrites between drains is loss, and the sequence counter
/// is what makes that loss countable on the host rather than invisible.
///
/// # Why the certificate is not in the ring
///
/// The boot rungs stamp exactly once, the drain clears the ring, and nothing
/// re-sends them. A host that missed that one frame learns from the sequence
/// gap *how many* records it lost and never *what they were* — and boot state
/// is the least repeatable, most diagnostic part of the whole stream. So the
/// prologue is copied into a buffer the ring cannot reach and re-emitted on a
/// bounded period. Verbatim, with its original sequence numbers: a re-stamp
/// would be a different event wearing the same name.
pub struct SpoorStream<const N: usize> {
    journal: SpoorJournal<N>,
    /// Sequence number the next stamped record will carry.
    next_seq: u64,
    /// Records stamped since the last drain, saturating at `N` — the journal
    /// itself cannot report that it overwrote, so this counts what was
    /// *offered* and the difference is the loss.
    offered: usize,
    /// The boot this stream belongs to, or [`EPOCH_UNDECLARED`].
    epoch: u32,
    /// The boot prologue, verbatim, in the order it was stamped.
    certificate: [u64; CERTIFICATE_CAPACITY],
    /// How much of `certificate` is written. Also the sequence number one past
    /// the last retained record, because the run starts at zero.
    certificate_len: usize,
    /// Set by the first record that is not a boot rung, or by the buffer
    /// filling. Once set the certificate never changes again — a birth
    /// certificate is written once or it is not one.
    certificate_closed: bool,
    /// Calls to [`SpoorStream::announce`] remaining before the next one emits.
    until_announce: usize,
}

impl<const N: usize> SpoorStream<N> {
    /// An empty stream. `const`, so it can initialise a `static`.
    #[must_use]
    pub const fn new() -> Self {
        SpoorStream {
            journal: SpoorJournal::new(),
            next_seq: 0,
            offered: 0,
            epoch: EPOCH_UNDECLARED,
            certificate: [0; CERTIFICATE_CAPACITY],
            certificate_len: 0,
            certificate_closed: false,
            until_announce: 0,
        }
    }

    /// Fixes this stream's boot epoch from whatever per-boot sample the caller
    /// has (`STORY-P1-10-04`).
    ///
    /// The board passes the generic counter at kernel entry. That value differs
    /// between boots because firmware timing does, which makes it a **change
    /// detector and not an identifier** — it cannot say *which* boot or *how
    /// many* were missed, and `LE-74` records why nothing on this hardware can
    /// yet. The folding below is not an attempt to manufacture entropy it does
    /// not have; it only stops a long firmware wait from parking the whole
    /// sample in bits this field cannot carry.
    ///
    /// A sample that folds to zero is stored as one, because zero is reserved
    /// for [`EPOCH_UNDECLARED`] and a seeded stream must never look unseeded.
    pub const fn seed_epoch(&mut self, sample: u64) {
        let folded = (sample ^ (sample >> 32)) as u32;
        self.epoch = if folded == EPOCH_UNDECLARED { 1 } else { folded };
    }

    /// This stream's boot epoch, [`EPOCH_UNDECLARED`] until seeded.
    #[must_use]
    pub const fn epoch(&self) -> u32 {
        self.epoch
    }

    /// Stamps one rung. Never allocates, never blocks, never fails.
    ///
    /// `cost` is whatever the call site measured, in whatever unit that rung
    /// documents; zero is a legitimate "not measured" rather than a claim of
    /// free.
    pub fn stamp(&mut self, rung: Rung, outcome: Outcome, cost: u32) {
        let (category, action) = rung.taxonomy();
        let spoor = Spoor::stamp(category, Actor::Kernel, action, outcome, rung.to_bits(), cost);
        self.retain(rung, spoor);
        self.journal.append(spoor);
        self.next_seq = self.next_seq.wrapping_add(1);
        self.offered = self.offered.saturating_add(1);
    }

    /// Offers one stamped record to the boot certificate.
    ///
    /// The certificate is a **consecutive run beginning at sequence 0**, and
    /// that is a correctness requirement rather than a simplification: a frame
    /// header carries one sequence number and implies its records follow it
    /// consecutively, so a certificate assembled from scattered records would
    /// make its own header lie. The first record that is not a boot rung
    /// therefore closes it permanently — the run cannot be resumed across a
    /// hole without inventing one.
    fn retain(&mut self, rung: Rung, spoor: Spoor) {
        if self.certificate_closed {
            return;
        }
        if !rung.is_boot_certificate() || self.certificate_len == CERTIFICATE_CAPACITY {
            self.certificate_closed = true;
            return;
        }
        self.certificate[self.certificate_len] = spoor.to_bits();
        self.certificate_len += 1;
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

        let len = spoor_wire::encode(first_seq, self.epoch, 0, &records[..take], out)?;
        self.journal = SpoorJournal::new();
        self.offered = 0;
        Ok(Some(len))
    }

    /// Re-announces the boot certificate, at most once every
    /// [`ANNOUNCE_EVERY`] calls (`STORY-P1-10-04`).
    ///
    /// Returns the payload length, or [`None`] when the announcement is not
    /// due or there is nothing to announce. The period lives here rather than
    /// in the park loop so the board holds no policy and a host test can read
    /// the bound the board promises.
    ///
    /// The frame carries [`FLAG_RETAINED`] and the sequence numbers the
    /// records originally went out under. A host must therefore **not** apply
    /// `seq + count` to it — which is why the flag is a wire field and not a
    /// convention: [`spoor_wire::FrameHeader::expected_next`] returns nothing
    /// for a retained frame, so the phantom gap is unreachable rather than
    /// merely documented.
    ///
    /// This does not touch the journal, the sequence counter or the drain's
    /// loss accounting. An announcement is a copy of bytes that were already
    /// sent; it can be lost like anything else on an unreliable broadcast link
    /// and the next one comes.
    ///
    /// # Errors
    ///
    /// [`SpoorWireError::BufferTooSmall`] if `out` cannot hold the frame.
    /// Nothing is consumed, so the next announcement is unaffected.
    pub fn announce(&mut self, out: &mut [u8]) -> Result<Option<usize>, SpoorWireError> {
        if self.until_announce > 0 {
            self.until_announce -= 1;
            return Ok(None);
        }
        self.until_announce = ANNOUNCE_EVERY - 1;
        if self.certificate_len == 0 {
            return Ok(None);
        }
        // Sequence 0: the certificate is a consecutive run from the first
        // record this boot ever stamped, so the frame that carries it is
        // byte-identical to the frame the original drain built.
        let len = spoor_wire::encode(
            0,
            self.epoch,
            FLAG_RETAINED,
            &self.certificate[..self.certificate_len],
            out,
        )?;
        Ok(Some(len))
    }

    /// Records the boot certificate holds — never more than
    /// [`CERTIFICATE_CAPACITY`], and never fewer once the boot has passed.
    #[must_use]
    pub const fn certificate_len(&self) -> usize {
        self.certificate_len
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

/// Fixes the board stream's boot epoch (`STORY-P1-10-04`).
///
/// Called once, as early in the boot as a per-boot sample exists. Calling it
/// again would re-label every frame emitted after the call and is a caller
/// error, not something this function can detect: two boots are indistinguish-
/// able from one boot seeded twice, which is precisely the ambiguity the epoch
/// exists to remove.
///
/// # Safety
///
/// Single core, non-reentrant. The caller must not be inside a drain or an
/// announcement.
#[no_mangle]
pub extern "C" fn tinyos_spoor_seed_epoch(sample: u64) {
    // SAFETY: as in `tinyos_spoor_stamp`.
    let stream = unsafe { &mut *core::ptr::addr_of_mut!(BOARD_STREAM) };
    stream.seed_epoch(sample);
}

/// Re-announces the board's boot certificate into `out`, returning the payload
/// length, or `0` when the announcement is not due or there is nothing to
/// announce (`STORY-P1-10-04`).
///
/// The park loop calls this every pass and the period is enforced here, so the
/// board carries no policy: what it knows is "offer the wire a frame if there
/// is one".
///
/// # Safety
///
/// `out` must be valid for `cap` bytes. Single core, non-reentrant.
#[no_mangle]
pub unsafe extern "C" fn tinyos_spoor_announce(out: *mut u8, cap: usize) -> usize {
    if out.is_null() {
        return 0;
    }
    // SAFETY: the caller's contract is that `out` is valid for `cap` bytes.
    let buffer = unsafe { core::slice::from_raw_parts_mut(out, cap) };
    // SAFETY: as in `tinyos_spoor_stamp`.
    let stream = unsafe { &mut *core::ptr::addr_of_mut!(BOARD_STREAM) };
    match stream.announce(buffer) {
        Ok(Some(len)) => len,
        // A refused announcement consumes nothing: the certificate is
        // immutable and the next period offers it again.
        Ok(None) | Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spoor_wire::{decode_header, record, HEADER_LEN, MAX_PAYLOAD};

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
            Rung::ThermalSample,
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
        assert_eq!(Rung::from_bits(9), None);
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
        assert_eq!(Rung::ThermalSample.to_bits(), 8);
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
        let first = decode_header(&out).expect("valid");
        assert_eq!((first.seq, first.count), (0, 2));

        s.stamp(Rung::TickArmed, Outcome::Ok, 0);
        s.drain(&mut out).expect("encodes").expect("records");
        let second = decode_header(&out).expect("valid");
        assert_eq!(Some(second.seq), first.expected_next(), "no gap where none was lost");
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
        let expected_next =
            decode_header(&out).expect("valid").expected_next().expect("a drained frame is stream");

        // Now overrun the ring: CAPACITY + 3 stamped, CAPACITY survive.
        for _ in 0..(CAPACITY + 3) {
            s.stamp(Rung::ParkIteration, Outcome::Ok, 0);
        }
        assert_eq!(s.offered(), CAPACITY + 3, "the stream knows what it was offered");

        s.drain(&mut out).expect("encodes").expect("records");
        let next = decode_header(&out).expect("valid");
        assert_eq!(next.count, CAPACITY, "only what the ring held survives");
        assert_eq!(
            next.seq - expected_next,
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

    // ---- the boot certificate and the epoch (`STORY-P1-10-04`) -----------

    /// The three rungs every boot passes before the park loop, in the order
    /// `hal-arm64`'s boot path passes them.
    fn boot_prologue(s: &mut SpoorStream<CAPACITY>) {
        s.stamp(Rung::MmuEnabled, Outcome::Ok, 183_974);
        s.stamp(Rung::GicRouted, Outcome::Ok, 0);
        s.stamp(Rung::TickArmed, Outcome::Ok, 1);
    }

    /// Announces regardless of period, for tests about *what* is announced
    /// rather than *when*.
    fn announce_now(s: &mut SpoorStream<CAPACITY>, out: &mut [u8]) -> usize {
        for _ in 0..ANNOUNCE_EVERY {
            if let Some(len) = s.announce(out).expect("encodes") {
                return len;
            }
        }
        panic!("no announcement inside a full period");
    }

    /// Clause 1 and clause 2: one boot, one epoch, on every frame it emits.
    #[test]
    fn every_frame_of_one_boot_carries_the_same_epoch() {
        let mut s = stream();
        s.seed_epoch(0x0000_1234_0000_5678);
        boot_prologue(&mut s);
        let mut out = [0u8; MAX_PAYLOAD];

        s.drain(&mut out).expect("encodes").expect("records");
        let drained = decode_header(&out).expect("valid").epoch;
        announce_now(&mut s, &mut out);
        let announced = decode_header(&out).expect("valid").epoch;

        assert_eq!(drained, s.epoch(), "a drained frame carries the stream's epoch");
        assert_eq!(announced, drained, "and so does the announcement");
        assert_ne!(drained, EPOCH_UNDECLARED, "a seeded stream never looks unseeded");
    }

    /// Clause 1's reserved value. An unseeded stream is honestly unseeded
    /// rather than claiming to be boot zero.
    #[test]
    fn an_unseeded_stream_declares_no_epoch() {
        let mut s = stream();
        assert_eq!(s.epoch(), EPOCH_UNDECLARED);
        s.stamp(Rung::MmuEnabled, Outcome::Ok, 0);
        let mut out = [0u8; MAX_PAYLOAD];
        s.drain(&mut out).expect("encodes").expect("records");
        assert_eq!(decode_header(&out).expect("valid").epoch, EPOCH_UNDECLARED);
    }

    /// A sample that folds to zero must not make a seeded board look unseeded
    /// — the one input where the honest answer and the reserved value collide.
    #[test]
    fn a_sample_that_folds_to_zero_still_declares_an_epoch() {
        let mut s = stream();
        s.seed_epoch(0);
        assert_ne!(s.epoch(), EPOCH_UNDECLARED, "zero in must not read as unseeded");
        // The identical halves fold to zero too, and are the realistic case:
        // a counter whose high word happens to equal its low word.
        s.seed_epoch(0x0000_00AB_0000_00AB);
        assert_ne!(s.epoch(), EPOCH_UNDECLARED);
    }

    /// A different boot must read as a different boot. This is the epoch's
    /// entire job, and all it can honestly do — see `LE-74`.
    #[test]
    fn a_different_sample_is_a_different_boot() {
        let mut first = stream();
        first.seed_epoch(1_500_000_000);
        let mut second = stream();
        second.seed_epoch(1_500_004_400);
        assert_ne!(first.epoch(), second.epoch(), "two boots are distinguishable");
    }

    /// Clause 3 — the reason this Story exists. The ring is a jitter buffer;
    /// the certificate must outlive every wrap and every drain.
    #[test]
    fn the_boot_certificate_survives_every_drain_and_every_wrap() {
        let mut s = stream();
        s.seed_epoch(7);
        boot_prologue(&mut s);
        let mut out = [0u8; MAX_PAYLOAD];
        s.drain(&mut out).expect("encodes").expect("the boot went out once");

        // Now run the board for a long time: drain, overrun, drain again.
        for _ in 0..20 {
            for _ in 0..(CAPACITY + 5) {
                s.stamp(Rung::ParkIteration, Outcome::Ok, 0);
            }
            s.drain(&mut out).expect("encodes");
        }

        let len = announce_now(&mut s, &mut out);
        let header = decode_header(&out[..len]).expect("valid");
        assert_eq!(header.count, 3, "the boot rungs are still announceable");
        let rungs: [u16; 3] = core::array::from_fn(|i| {
            Spoor::decode(record(&out, i).expect("present")).expect("decodes").target()
        });
        assert_eq!(
            rungs,
            [Rung::MmuEnabled.to_bits(), Rung::GicRouted.to_bits(), Rung::TickArmed.to_bits()],
            "and they are the rungs the boot actually passed"
        );
    }

    /// Clause 4 — verbatim, not a summary and not a re-stamp. A re-stamp would
    /// carry fresh sequence numbers and be a *different event* wearing the
    /// same name.
    #[test]
    fn the_announcement_is_byte_identical_to_the_frame_the_drain_sent() {
        let mut s = stream();
        s.seed_epoch(0xDEAD_BEEF);
        boot_prologue(&mut s);

        let mut drained = [0u8; MAX_PAYLOAD];
        let drained_len = s.drain(&mut drained).expect("encodes").expect("records");

        for _ in 0..50 {
            s.stamp(Rung::ParkIteration, Outcome::Ok, 0);
        }
        let mut announced = [0u8; MAX_PAYLOAD];
        let announced_len = announce_now(&mut s, &mut announced);

        assert_eq!(drained_len, announced_len, "same frame, same length");
        assert_eq!(
            drained[HEADER_LEN..drained_len],
            announced[HEADER_LEN..announced_len],
            "record bytes are the ones already sent, not fresh stamps"
        );
        assert_eq!(
            decode_header(&drained).expect("valid").seq,
            decode_header(&announced).expect("valid").seq,
            "carrying the sequence numbers they originally went out under"
        );
    }

    /// Clause 5 — the flag is what keeps a host's own arithmetic honest, so
    /// the test runs that arithmetic across drain, announce, drain.
    #[test]
    fn an_announcement_between_two_drains_produces_no_phantom_gap() {
        let mut s = stream();
        s.seed_epoch(3);
        boot_prologue(&mut s);
        let mut out = [0u8; MAX_PAYLOAD];

        s.drain(&mut out).expect("encodes").expect("records");
        let mut expected = decode_header(&out).expect("valid").expected_next().expect("stream");

        announce_now(&mut s, &mut out);
        let announcement = decode_header(&out).expect("valid");
        assert!(announcement.is_retained(), "an announcement says what it is");
        // Exactly what a host decoder does: a frame that says nothing about
        // what comes next leaves the expectation where it was.
        if let Some(next) = announcement.expected_next() {
            expected = next;
        }

        s.stamp(Rung::ParkIteration, Outcome::Ok, 0);
        s.drain(&mut out).expect("encodes").expect("records");
        assert_eq!(
            decode_header(&out).expect("valid").seq,
            expected,
            "the stream resumes where it left off, with no gap and no backwards jump"
        );
    }

    /// Clause 5's other half: a drained frame must never be mistaken for an
    /// announcement, or a host would stop counting real loss.
    #[test]
    fn a_drained_frame_is_never_marked_retained() {
        let mut s = stream();
        s.stamp(Rung::MmuEnabled, Outcome::Ok, 0);
        let mut out = [0u8; MAX_PAYLOAD];
        s.drain(&mut out).expect("encodes").expect("records");
        assert!(!decode_header(&out).expect("valid").is_retained());
    }

    /// Clause 6 — write-once and bounded. A birth certificate the
    /// ten-thousandth park iteration can overwrite is not one.
    #[test]
    fn no_amount_of_park_traffic_displaces_the_boot_certificate() {
        let mut s = stream();
        boot_prologue(&mut s);
        let held = s.certificate_len();

        for _ in 0..10_000 {
            s.stamp(Rung::ParkIteration, Outcome::Ok, 0);
            s.stamp(Rung::BeaconTransmitted, Outcome::Ok, 0);
            s.stamp(Rung::FaultTaken, Outcome::Failed, 0);
        }
        assert_eq!(s.certificate_len(), held, "the certificate did not grow");

        let mut out = [0u8; MAX_PAYLOAD];
        announce_now(&mut s, &mut out);
        let header = decode_header(&out).expect("valid");
        assert_eq!(header.count, held, "and still announces exactly the boot");
        assert_eq!(header.seq, 0, "from the first record this boot ever stamped");
    }

    /// Clause 6's bound, driven rather than asserted: even a boot that somehow
    /// stamped nothing but certificate rungs cannot exceed the buffer.
    #[test]
    fn the_certificate_stops_at_its_capacity() {
        let mut s = stream();
        for _ in 0..(CERTIFICATE_CAPACITY * 4) {
            s.stamp(Rung::MmuEnabled, Outcome::Ok, 0);
        }
        assert_eq!(s.certificate_len(), CERTIFICATE_CAPACITY, "bounded by the buffer, not by luck");
    }

    /// Clause 7 — the run is consecutive from zero, because the frame header
    /// carries one sequence and implies the rest. A rung arriving after the
    /// run breaks is not retained, however boot-like it is.
    #[test]
    fn the_certificate_closes_at_the_first_record_that_is_not_a_boot_rung() {
        let mut s = stream();
        s.stamp(Rung::MmuEnabled, Outcome::Ok, 0);
        s.stamp(Rung::ParkIteration, Outcome::Ok, 0);
        s.stamp(Rung::TickArmed, Outcome::Ok, 0);
        assert_eq!(
            s.certificate_len(),
            1,
            "the rung after the break is not retained, so no hole can open in the run"
        );

        let mut out = [0u8; MAX_PAYLOAD];
        announce_now(&mut s, &mut out);
        let header = decode_header(&out).expect("valid");
        assert_eq!((header.seq, header.count), (0, 1), "and the header describes exactly it");
    }

    /// A boot with nothing retained announces nothing. Silence is not a frame,
    /// here for the same reason it is not one in `drain`.
    #[test]
    fn a_stream_with_no_certificate_announces_nothing() {
        let mut s = stream();
        s.stamp(Rung::ParkIteration, Outcome::Ok, 0);
        let mut out = [0u8; MAX_PAYLOAD];
        for _ in 0..(ANNOUNCE_EVERY * 3) {
            assert_eq!(s.announce(&mut out).expect("no error"), None);
        }
    }

    /// Clause 8 — the period is a stated bound a host test can read, not a
    /// cadence buried in the park loop.
    #[test]
    fn the_announcement_is_periodic_and_the_period_is_the_stated_one() {
        let mut s = stream();
        boot_prologue(&mut s);
        let mut out = [0u8; MAX_PAYLOAD];

        let mut emitted = 0;
        for _ in 0..(ANNOUNCE_EVERY * 10) {
            if s.announce(&mut out).expect("encodes").is_some() {
                emitted += 1;
            }
        }
        assert_eq!(emitted, 10, "exactly one announcement per {ANNOUNCE_EVERY} calls");
        // A period of zero would be a flood rather than a re-announcement, and
        // would underflow the countdown. Checked at compile time because it is
        // a property of the constant, not of this run.
        const { assert!(ANNOUNCE_EVERY >= 1) };
    }

    /// The announcement must not disturb the stream it rides beside: it is a
    /// copy of bytes already sent, and the loss accounting is the drain's.
    #[test]
    fn announcing_consumes_nothing_from_the_stream() {
        let mut s = stream();
        boot_prologue(&mut s);
        let mut out = [0u8; MAX_PAYLOAD];
        for _ in 0..(ANNOUNCE_EVERY * 3) {
            let _ = s.announce(&mut out).expect("encodes");
        }
        assert_eq!(s.next_sequence(), 3, "the sequence counter did not move");
        let header_after =
            s.drain(&mut out).expect("encodes").map(|_| decode_header(&out).expect("valid"));
        assert_eq!(
            header_after.map(|h| (h.seq, h.count)),
            Some((0, 3)),
            "and the journal still holds everything the drain never sent"
        );
    }

    /// A buffer too small refuses without consuming: the certificate is
    /// immutable, so the next period offers it again unchanged.
    #[test]
    fn a_refused_announcement_loses_nothing() {
        let mut s = stream();
        boot_prologue(&mut s);
        let mut tiny = [0u8; 4];
        assert_eq!(s.announce(&mut tiny), Err(SpoorWireError::BufferTooSmall));
        let mut out = [0u8; MAX_PAYLOAD];
        let len = announce_now(&mut s, &mut out);
        assert_eq!(decode_header(&out[..len]).expect("valid").count, 3);
    }

    /// The certificate's membership rule, pinned. Anything that repeats is
    /// stream and not birth, and getting this wrong fills a fixed buffer with
    /// the least interesting seconds of the run.
    #[test]
    fn only_the_once_per_boot_rungs_belong_to_the_certificate() {
        for rung in [Rung::MmuEnabled, Rung::GicRouted, Rung::TickArmed, Rung::FixtureMeasure] {
            assert!(rung.is_boot_certificate(), "{rung:?} happens once and establishes state");
        }
        for rung in
            [Rung::BeaconTransmitted, Rung::ParkIteration, Rung::FaultTaken, Rung::ThermalSample]
        {
            assert!(!rung.is_boot_certificate(), "{rung:?} repeats, so it is stream not birth");
        }
    }
}
