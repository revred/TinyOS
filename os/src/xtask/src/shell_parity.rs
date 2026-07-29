//! `check-shell-parity` — the host half of `TEST-P2-07-01-A`.
//!
//! Boots the `shell-batch` fixture with serial capture and byte-compares the
//! transcript against the committed golden file. Line endings are normalised
//! (`CRLF` → `LF`) on both sides so a checkout's eol policy can never fake a
//! divergence or hide one that matters — everything else is exact.

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
