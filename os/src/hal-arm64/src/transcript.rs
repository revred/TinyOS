//! The measurement transcript (`STORY-P1-07-06`): every line the board's
//! `fixture_measure` emits, kept so the park loop can carry the envelope on
//! the channels this bench actually has — painted on the canvas and
//! transmitted line-by-line as `TOS64-*` Ethernet frames for the host's
//! packet capture, the owner's standing direction ("diagnosis moves onto the
//! cable") applied to the first hardware measurement.
//!
//! Pure buffer type here, host-tested; the one static instance at the bottom
//! is the board's.

/// Capacity of the whole transcript: the envelope is ~11 lines of ≤ 140
/// bytes. Overflow is dropped, never wrapped — a truncated transcript reads
/// as truncated, a wrapped one lies.
pub const TRANSCRIPT_CAPACITY: usize = 2048;

/// Most lines the transcript will index — envelope plus chatter headroom.
pub const MAX_LINES: usize = 24;

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
