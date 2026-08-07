//! The measurement transcript (`STORY-P1-07-06`): every line the board's
//! `fixture_measure` emits, kept so the park loop can carry the envelope on
//! the channels this bench actually has — painted on the canvas and
//! transmitted line-by-line as `TOS64-*` Ethernet frames for the host's
//! packet capture, the owner's standing direction ("diagnosis moves onto the
//! cable") applied to the first hardware measurement.
//!
//! Pure buffer type here, host-tested; the one static instance at the bottom
//! is the board's.

/// Longest line the envelope can produce, with margin.
///
/// The real worst case observed on the wire is **169 bytes** — a `METRIC`
/// line whose metric name is the longest in the set
/// (`pool_u64x64_alloc_free_round_trip_per_op_of_8_spoored`, 52 characters)
/// followed by six percentile fields. Named here so the capacity below is
/// derived from it rather than guessed beside it.
///
/// **256 rather than a snug fit over 169**, because the thing that grows is
/// the metric *name*, and a bound with 20 bytes of headroom is one rename
/// away from the failure it was written to prevent. The cost of the margin is
/// static bytes on a board with gigabytes; the cost of getting it wrong is a
/// board run whose numbers never reach the wire.
pub const MAX_LINE_BYTES: usize = 256;

/// Capacity of the whole transcript, **derived** so that [`MAX_LINES`] lines
/// of [`MAX_LINE_BYTES`] cannot overflow it.
///
/// Overflow is dropped, never wrapped — a truncated transcript reads as
/// truncated, a wrapped one lies. That honesty is what made the 2026-08-06
/// failure diagnosable in one capture instead of producing a plausible wrong
/// number, and it is why the buffer keeps the behaviour.
///
/// **It was a hand-picked 2048 until 2026-08-06, documented as "~11 lines of
/// ≤ 140 bytes", and adding a twelfth metric silently overran it.** The
/// `PERF-D07-G23` spoor-enabled arm's line reached the wire carrying its
/// *name* and none of its numbers, and the `END metrics=12` line never
/// arrived at all — so the measurement ran correctly on silicon and the
/// transport dropped it. A capacity that is a constant beside its consumers,
/// rather than a function of them, is a capacity that goes stale the moment
/// anyone adds a line; [`tests::the_capacity_holds_a_full_transcript`] is now
/// what fails instead of a board run.
pub const TRANSCRIPT_CAPACITY: usize = MAX_LINES * MAX_LINE_BYTES;

/// Most lines the transcript will index — envelope plus chatter headroom.
///
/// The envelope is `BEGIN` + one per metric + `END`, plus the fixture's
/// chatter lines: two trailing ones since 2026-08-06 and, since 2026-08-07,
/// the three `ADR 0005` qualification lines (`boot_entry`, `counter_split`,
/// `residency_probe` — `LE-103`) and the `TOS64-RESULT/1` verdict (`LE-110`'s
/// caveat closed: a capture now carries its own pass/fail). The 14 metrics of
/// 2026-08-06 plus six such lines put it at **22**, and the six spare lines
/// are what `LE-89` bought — a metric added without a capacity change is an
/// ordinary edit rather than a lost board run.
pub const MAX_LINES: usize = 28;

/// An append-only line buffer with bounded copy-out access.
pub struct TranscriptBuffer {
    bytes: [u8; TRANSCRIPT_CAPACITY],
    len: usize,
}

impl TranscriptBuffer {
    /// An empty transcript.
    #[must_use]
    pub const fn new() -> Self {
        TranscriptBuffer { bytes: [0; TRANSCRIPT_CAPACITY], len: 0 }
    }

    /// Appends raw bytes, dropping anything past capacity.
    pub fn record(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            if self.len < TRANSCRIPT_CAPACITY {
                self.bytes[self.len] = byte;
                self.len += 1;
            }
        }
    }

    /// Number of non-empty lines recorded (capped at [`MAX_LINES`]).
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.lines().count()
    }

    /// Copies the `nth` non-empty line (0-based, line endings stripped) into
    /// `out`, returning the copied length — `None` past the last line.
    /// Copy-out rather than borrow, so the board-side static needs no long-
    /// lived shared reference.
    pub fn copy_line(&self, nth: usize, out: &mut [u8]) -> Option<usize> {
        let line = self.lines().nth(nth)?;
        let take = line.len().min(out.len());
        out[..take].copy_from_slice(&line[..take]);
        Some(take)
    }

    fn lines(&self) -> impl Iterator<Item = &[u8]> {
        self.bytes[..self.len]
            .split(|&byte| byte == b'\n')
            .map(|raw| match raw.split_last() {
                Some((b'\r', head)) => head,
                _ => raw,
            })
            .filter(|line| !line.is_empty())
            .take(MAX_LINES)
    }
}

impl Default for TranscriptBuffer {
    fn default() -> Self {
        TranscriptBuffer::new()
    }
}

// --- the board's one instance (`fixture-measure` only) -----------------------

#[cfg(all(target_arch = "aarch64", feature = "fixture-measure"))]
static mut TRANSCRIPT: TranscriptBuffer = TranscriptBuffer::new();

/// Appends to the board transcript. Single core, called only from the
/// measurement fixture before the park loop starts reading — the phases are
/// strictly ordered, so writer and readers never overlap.
#[cfg(all(target_arch = "aarch64", feature = "fixture-measure"))]
pub fn record(bytes: &[u8]) {
    // SAFETY: single core; the fixture is the only writer and runs to
    // completion before the park loop's readers start.
    unsafe { (*core::ptr::addr_of_mut!(TRANSCRIPT)).record(bytes) };
}

/// Lines currently recorded on the board transcript.
#[cfg(all(target_arch = "aarch64", feature = "fixture-measure"))]
#[must_use]
pub fn line_count() -> usize {
    // SAFETY: read-only view after the writer finished; single core.
    unsafe { (*core::ptr::addr_of!(TRANSCRIPT)).line_count() }
}

/// Copies the `nth` transcript line into `out` — see
/// [`TranscriptBuffer::copy_line`].
#[cfg(all(target_arch = "aarch64", feature = "fixture-measure"))]
pub fn copy_line(nth: usize, out: &mut [u8]) -> Option<usize> {
    // SAFETY: as in [`line_count`].
    unsafe { (*core::ptr::addr_of!(TRANSCRIPT)).copy_line(nth, out) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_recorded_split_and_copied_out_in_order() {
        let mut transcript = TranscriptBuffer::new();
        transcript.record(b"TOS64-MEAS/2 BEGIN tier=T1\n");
        transcript.record(b"TOS64-MEAS/2 METRIC domain=D07\r\n");
        transcript.record(b"TOS64-MEAS/2 END metrics=8\n");
        assert_eq!(transcript.line_count(), 3);
        let mut out = [0u8; 64];
        let len = transcript.copy_line(1, &mut out).expect("second line");
        assert_eq!(&out[..len], b"TOS64-MEAS/2 METRIC domain=D07" as &[u8]);
        assert_eq!(transcript.copy_line(3, &mut out), None);
    }

    /// The gate that should have existed before 2026-08-06.
    ///
    /// A full-length transcript — every indexable line at the longest line the
    /// envelope can produce — must fit whole. Written as a *behavioural* check
    /// rather than an arithmetic one on the two constants, because the thing
    /// that actually failed was a real recorded transcript losing its tail:
    /// the `PERF-D07-G23` arm's numbers and the `END` line, dropped in
    /// transport after the fixture had measured them correctly.
    #[test]
    fn the_capacity_holds_a_full_transcript() {
        let mut transcript = TranscriptBuffer::new();
        let mut line = [b'X'; MAX_LINE_BYTES];
        line[MAX_LINE_BYTES - 1] = b'\n';
        for _ in 0..MAX_LINES {
            transcript.record(&line);
        }
        assert_eq!(
            transcript.line_count(),
            MAX_LINES,
            "a full transcript must survive whole; if this fails, the envelope grew and \
             TRANSCRIPT_CAPACITY did not"
        );
        let mut out = [0u8; MAX_LINE_BYTES];
        let len = transcript.copy_line(MAX_LINES - 1, &mut out).expect("the last line exists");
        assert_eq!(
            len,
            MAX_LINE_BYTES - 1,
            "the LAST line is the one truncation eats first, so it is the one worth asserting"
        );
    }

    /// The real envelope, at the size that broke it, must fit with room left.
    ///
    /// Pins the actual failure rather than a synthetic worst case: 12 metrics,
    /// the longest metric name in the set, plus `BEGIN`, `END` and the
    /// fixture's two chatter lines.
    #[test]
    fn the_twelve_metric_envelope_that_overran_2048_now_fits() {
        const LONGEST_METRIC_LINE: &[u8] =
            b"TOS64-MEAS/2 METRIC domain=D07 metric=pool_u64x64_alloc_free_round_trip_per_op_of_8_spoored n=1000 dropped=0 warmup=100 min=481 p50=481 p99=481 p99_9=481 max=481 unit=cycles\n";
        assert!(
            LONGEST_METRIC_LINE.len() <= MAX_LINE_BYTES,
            "the observed worst-case line ({}) must fit MAX_LINE_BYTES ({MAX_LINE_BYTES})",
            LONGEST_METRIC_LINE.len()
        );

        let mut transcript = TranscriptBuffer::new();
        transcript.record(b"TOS64-MEAS/2 BEGIN tier=T1 arch=aarch64 platform=rpi5-bcm2712 qualification=none cycle_source=pmccntr_el0 overhead_cycles=43 cycles_per_us=2400\n");
        for _ in 0..12 {
            transcript.record(LONGEST_METRIC_LINE);
        }
        transcript.record(b"TOS64-MEAS/2 END metrics=12\n");
        transcript.record(b"fixture-measure metrics=12\n");
        transcript.record(b"fixture-measure cycle_source_conformance ok span=5507\n");

        assert_eq!(transcript.line_count(), 16, "BEGIN + 12 metrics + END + two chatter lines");
        let mut out = [0u8; MAX_LINE_BYTES];
        let len = transcript.copy_line(15, &mut out).expect("the conformance line exists");
        assert_eq!(
            &out[..len],
            b"fixture-measure cycle_source_conformance ok span=5507" as &[u8],
            "the LAST line must arrive whole — on 2026-08-06 the tail was silently dropped"
        );
    }

    #[test]
    fn overflow_truncates_rather_than_wrapping() {
        let mut transcript = TranscriptBuffer::new();
        for _ in 0..(TRANSCRIPT_CAPACITY + 100) {
            transcript.record(b"A");
        }
        transcript.record(b"\nNEXT\n");
        // Everything after capacity was dropped, so the one giant line is
        // all there is — and it is capacity-sized, not capacity-plus-wrap.
        assert_eq!(transcript.line_count(), 1);
        let mut out = [0u8; TRANSCRIPT_CAPACITY + 32];
        let len = transcript.copy_line(0, &mut out).expect("the giant line");
        assert_eq!(len, TRANSCRIPT_CAPACITY);
    }

    #[test]
    fn a_short_destination_copies_a_bounded_prefix() {
        let mut transcript = TranscriptBuffer::new();
        transcript.record(b"TOS64-RESULT/1 fixture=measure ok=true\n");
        let mut tiny = [0u8; 8];
        assert_eq!(transcript.copy_line(0, &mut tiny), Some(8));
        assert_eq!(&tiny, b"TOS64-RE");
    }
}
