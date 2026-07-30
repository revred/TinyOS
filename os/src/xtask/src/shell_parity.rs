//! `check-shell-parity` — the host half of `TEST-P2-07-01-A`.
//!
//! Boots the `shell-batch` fixture with serial capture and byte-compares the
//! transcript against the committed golden file. Line endings are normalised
//! (`CRLF` → `LF`) on both sides so a checkout's eol policy can never fake a
//! divergence or hide one that matters — everything else is exact.

/// The parsed spoor trailer (`LE-56`): the fixture's post-transcript
/// `TINYOS-SPOOR/1 len=<n> denials=<n>` marker line, the third parity signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpoorTrailer {
    /// Spoors the in-guest denial journal holds after the run.
    pub len: u32,
    /// Denials the batch runner counted.
    pub denials: u32,
}

impl SpoorTrailer {
    /// The corroboration check: the journal length must equal the denial
    /// count — two independently-maintained counters agreeing on one fact.
    /// `Ok` carries the corroborated count as evidence.
    pub fn corroborated(self) -> Result<u32, String> {
        if self.len == self.denials {
            Ok(self.len)
        } else {
            Err(format!(
                "spoor journal does not corroborate the denial count (TINYOS-SPOOR/1 len={} denials={})",
                self.len, self.denials
            ))
        }
    }
}

/// The marker that starts the fixture's spoor trailer line. Versioned like
/// `TINYOS-MEAS/2`: a format change bumps the suffix and fails old parsers
/// closed instead of silently misreading them.
const SPOOR_MARKER: &str = "TINYOS-SPOOR/1";

/// Split a serial capture at the spoor trailer: everything before the marker
/// line is the transcript (byte-untouched, for the sacred golden comparison);
/// the marker line parses into a [`SpoorTrailer`]. A missing or malformed
/// trailer is a FAIL, never a skip — the third signal must be affirmatively
/// present. The marker only counts at the start of a line, so a transcript
/// merely containing the string (e.g. an ECHO of it) can't forge a trailer.
pub fn split_capture(capture: &str) -> Result<(String, SpoorTrailer), String> {
    let marker_at = if capture.starts_with(SPOOR_MARKER) {
        Some(0)
    } else {
        capture.find(&format!("\n{SPOOR_MARKER}")).map(|i| i + 1)
    };
    let Some(start) = marker_at else {
        return Err(format!(
            "spoor trailer missing: no line starts with {SPOOR_MARKER} — the fixture must emit it after the transcript"
        ));
    };
    let transcript = capture[..start].to_string();
    let line = capture[start..].lines().next().unwrap_or("").trim_end_matches('\r');

    let malformed = |detail: &str| {
        format!("spoor trailer malformed ({detail}): {line:?} — expected {SPOOR_MARKER} len=<n> denials=<n>")
    };
    let rest = line.strip_prefix(SPOOR_MARKER).expect("line starts with marker");
    let mut fields = rest.split_whitespace();
    let len = fields
        .next()
        .and_then(|f| f.strip_prefix("len="))
        .ok_or_else(|| malformed("missing len="))?
        .parse::<u32>()
        .map_err(|_| malformed("len is not a number"))?;
    let denials = fields
        .next()
        .and_then(|f| f.strip_prefix("denials="))
        .ok_or_else(|| malformed("missing denials="))?
        .parse::<u32>()
        .map_err(|_| malformed("denials is not a number"))?;
    if fields.next().is_some() {
        return Err(malformed("unexpected extra field"));
    }
    Ok((transcript, SpoorTrailer { len, denials }))
}

/// Compare `actual` against `golden` after eol normalisation. `Ok` carries the
/// line count as evidence; `Err` names the first divergent line, both spellings.
pub fn compare_transcript(actual: &str, golden: &str) -> Result<usize, String> {
    let normalise = |text: &str| text.replace("\r\n", "\n");
    let actual = normalise(actual);
    let golden = normalise(golden);
    if actual == golden {
        return Ok(actual.lines().count());
    }
    let mut actual_lines = actual.lines();
    let mut golden_lines = golden.lines();
    let mut line_number = 1usize;
    loop {
        match (actual_lines.next(), golden_lines.next()) {
            (Some(a), Some(g)) if a == g => line_number += 1,
            (Some(a), Some(g)) => {
                return Err(format!(
                    "transcript diverges from golden at line {line_number}:\n  fixture: {a:?}\n  golden:  {g:?}"
                ));
            }
            (Some(a), None) => {
                return Err(format!(
                    "fixture transcript has extra content from line {line_number}: {a:?}"
                ));
            }
            (None, Some(g)) => {
                return Err(format!(
                    "fixture transcript ends early; golden line {line_number} is missing: {g:?}"
                ));
            }
            (None, None) => {
                // Same lines but different trailing bytes (e.g. missing final newline).
                return Err("transcripts differ only in trailing bytes (final newline?)".into());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- The spoor-trailer splitter (`LE-56`, hand-2026-07-30/04A §1.4) ---

    #[test]
    fn splitter_separates_transcript_from_a_wellformed_trailer() {
        let capture = "line one\nline two\nTINYOS-SPOOR/1 len=1 denials=1\n";
        let (transcript, trailer) = split_capture(capture).expect("well-formed capture splits");
        assert_eq!(transcript, "line one\nline two\n", "transcript bytes untouched");
        assert_eq!(trailer.len, 1);
        assert_eq!(trailer.denials, 1);
    }

    #[test]
    fn splitter_handles_crlf_captures() {
        let capture = "a\r\nTINYOS-SPOOR/1 len=2 denials=2\r\n";
        let (transcript, trailer) = split_capture(capture).expect("CRLF capture splits");
        assert_eq!(transcript, "a\r\n");
        assert_eq!(trailer.len, 2);
        assert_eq!(trailer.denials, 2);
    }

    // Absent is a FAIL, never a skip: a fixture that stopped emitting the
    // trailer must red the gate, not silently drop the third signal.
    #[test]
    fn a_missing_trailer_fails_closed() {
        let error = split_capture("just a transcript\n").unwrap_err();
        assert!(error.contains("spoor trailer"), "{error}");
        assert!(error.contains("missing"), "{error}");
    }

    // Malformed is a FAIL, never a skip.
    #[test]
    fn a_malformed_trailer_fails_closed() {
        for capture in [
            "a\nTINYOS-SPOOR/1\n",
            "a\nTINYOS-SPOOR/1 len=x denials=1\n",
            "a\nTINYOS-SPOOR/1 len=1\n",
            "a\nTINYOS-SPOOR/1 denials=1 len=1\n",
            "a\nTINYOS-SPOOR/1 len=1 denials=\n",
        ] {
            let error = split_capture(capture).unwrap_err();
            assert!(error.contains("malformed"), "capture {capture:?}: {error}");
        }
    }

    // The marker must start a line — a transcript merely *containing* the
    // string (e.g. an ECHO of it) is not a trailer.
    #[test]
    fn a_mid_line_marker_is_not_a_trailer() {
        let error = split_capture("ECHO TINYOS-SPOOR/1 len=1 denials=1\n").unwrap_err();
        assert!(error.contains("missing"), "{error}");
    }

    // Corroboration: len must equal denials, or the third signal is red.
    #[test]
    fn a_count_mismatch_fails_corroboration() {
        let (_, trailer) =
            split_capture("a\nTINYOS-SPOOR/1 len=2 denials=1\n").expect("well-formed");
        let error = trailer.corroborated().unwrap_err();
        assert!(error.contains("does not corroborate"), "{error}");
        assert!(error.contains("len=2") && error.contains("denials=1"), "{error}");

        let (_, good) = split_capture("a\nTINYOS-SPOOR/1 len=1 denials=1\n").expect("well-formed");
        assert_eq!(good.corroborated().expect("equal counts corroborate"), 1);
    }

    #[test]
    fn identical_transcripts_pass_and_count_lines() {
        assert_eq!(compare_transcript("a\nb\n", "a\nb\n"), Ok(2));
        // CRLF on either side is not a divergence.
        assert_eq!(compare_transcript("a\r\nb\r\n", "a\nb\n"), Ok(2));
    }

    #[test]
    fn divergence_names_the_first_line_both_spellings() {
        let error = compare_transcript("a\nX\n", "a\nb\n").unwrap_err();
        assert!(error.contains("line 2"), "{error}");
        assert!(error.contains("\"X\"") && error.contains("\"b\""), "{error}");

        let error = compare_transcript("a\n", "a\nb\n").unwrap_err();
        assert!(error.contains("ends early"), "{error}");

        let error = compare_transcript("a\nb\nc\n", "a\nb\n").unwrap_err();
        assert!(error.contains("extra content"), "{error}");
    }
}
