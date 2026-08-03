//! The discovery diagnostic instruments (`STORY-P1-09-07`/`-11`,
//! `STORY-P1-07-09`): how a refusal is *pronounced* once
//! [`crate::etherrors`] has named its code and detail — the plain blink
//! count, the spelled decimal sentence, the latch that keeps a sentence in
//! flight whole, the per-tick lamp decision, and the fixed-shape canvas
//! text.
//!
//! Everything here is pure and waitless: a function of (sentence, tick),
//! never of the hardware. The one property the whole language serves is
//! `TEST-P1-09-11-A`'s: **two honest counts of the same sentence agree.**

/// Ticks per blink half-phase: 300 ms on, 300 ms off at the 10 Hz tick.
pub const BLINK_HALF_TICKS: u32 = 3;
/// Ticks of trailing darkness after the count — long enough (2 s) that a
/// human never runs two periods together.
pub const BLINK_GAP_TICKS: u32 = 20;

/// The lamp value for a refusal `code` at 10 Hz `tick` — a pure function,
/// pinned tick-by-tick (`TEST-P1-09-07-A` clause 2): `code` blinks of
/// [`BLINK_HALF_TICKS`] on/off, then [`BLINK_GAP_TICKS`] dark, repeating.
pub const fn blink_lamp_at(code: u8, tick: u32) -> bool {
    let blink_span = BLINK_HALF_TICKS * 2;
    let period = code as u32 * blink_span + BLINK_GAP_TICKS;
    let t = tick % period;
    t < code as u32 * blink_span && t % blink_span < BLINK_HALF_TICKS
}

/// Number of digit groups in one lamp sentence: two for the code (ones,
/// tens), five for the detail (ones through ten-thousands).
pub const SENTENCE_GROUPS: usize = 7;

/// One refusal, spelled: each group is its digit's blink count — a digit
/// 1–9 as that many fat 300 ms flashes, **zero as one long steady burn**
/// (a single 100 ms blip; the owner's refinement — no digit is ever
/// silence, and nobody counts to ten). Least-significant digit first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sentence {
    /// Decimal digits per group (0–9), in transmission order.
    pub groups: [u8; SENTENCE_GROUPS],
}

/// Builds the sentence for a refusal code and its sixteen-bit detail.
#[must_use]
pub fn sentence_for(code: u8, detail: u16) -> Sentence {
    let mut groups = [0u8; SENTENCE_GROUPS];
    groups[0] = code % 10;
    groups[1] = code / 10 % 10;
    let mut value = detail as u32;
    let mut index = 2;
    while index < SENTENCE_GROUPS {
        groups[index] = (value % 10) as u8;
        value /= 10;
        index += 1;
    }
    Sentence { groups }
}

/// Ticks a zero digit burns solid: 1.5 s of steady ON — five times a fat
/// flash, the only steady light in the whole language. (The first attempt
/// was a 100 ms flicker; on the board it read as a 1. The measurement
/// governs the display too.)
pub const ZERO_BURN_TICKS: u32 = 15;
/// Total span of a zero digit's group: the burn plus its own dark tail.
pub const ZERO_SPAN_TICKS: u32 = ZERO_BURN_TICKS + BLINK_HALF_TICKS;

/// Span of one digit group in ticks.
const fn group_span(digit: u8) -> u32 {
    if digit == 0 {
        ZERO_SPAN_TICKS
    } else {
        digit as u32 * BLINK_HALF_TICKS * 2
    }
}

/// Dark ticks between digit groups — long enough that no one mistakes a
/// group boundary for a blink's off half.
pub const GROUP_GAP_TICKS: u32 = 12;
/// Dark ticks after the last group — longer still, so the sentence's start
/// is unmistakable.
pub const SENTENCE_GAP_TICKS: u32 = 35;

/// Ticks in one full sentence period.
#[must_use]
pub fn sentence_period(sentence: &Sentence) -> u32 {
    let spans: u32 = sentence.groups.iter().map(|&g| group_span(g)).sum();
    spans + (SENTENCE_GROUPS as u32 - 1) * GROUP_GAP_TICKS + SENTENCE_GAP_TICKS
}

/// The lamp value for a sentence at a 10 Hz tick — pure, waitless,
/// stateless (`TEST-P1-09-11-A` clause 2).
#[must_use]
pub fn sentence_lamp_at(sentence: &Sentence, tick: u32) -> bool {
    let blink_span = BLINK_HALF_TICKS * 2;
    let mut t = tick % sentence_period(sentence);
    for (index, &group) in sentence.groups.iter().enumerate() {
        let span = group_span(group);
        if t < span {
            return if group == 0 {
                t < ZERO_BURN_TICKS
            } else {
                t % blink_span < BLINK_HALF_TICKS
            };
        }
        t -= span;
        let gap = if index == SENTENCE_GROUPS - 1 { SENTENCE_GAP_TICKS } else { GROUP_GAP_TICKS };
        if t < gap {
            return false;
        }
        t -= gap;
    }
    false
}

/// `STORY-P1-09-11` (amended after the first spelled boot): a sentence in
/// flight is never replaced. The first transcription attempt failed because
/// a flickering readback swapped sentences mid-read; the latch adopts a
/// changed outcome only at a period boundary, so every counted sentence is
/// internally consistent and a flickering rung reads as *clean alternating
/// sentences*, which is itself the diagnosis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SentenceLatch {
    current: Option<Sentence>,
    /// Tick at which the current sentence's period started.
    phase: u32,
    pending: Option<Option<Sentence>>,
}

impl SentenceLatch {
    /// Starts with the boot-time outcome's sentence.
    #[must_use]
    pub const fn new(initial: Option<Sentence>) -> Self {
        SentenceLatch { current: initial, phase: 0, pending: None }
    }

    /// Offers a (possibly changed) outcome; adopted at the next boundary.
    pub fn offer(&mut self, sentence: Option<Sentence>) {
        if sentence != self.current {
            self.pending = Some(sentence);
        } else {
            self.pending = None;
        }
    }

    /// One 10 Hz tick: returns what the lamp should do. Health (no
    /// sentence) keeps the plain pulse and adopts changes immediately —
    /// there is nothing in flight to protect.
    pub fn tick(&mut self, tick: u32) -> LampAction {
        match self.current {
            Some(sentence) => {
                let elapsed = tick.wrapping_sub(self.phase);
                if elapsed >= sentence_period(&sentence) {
                    self.phase = tick;
                    if let Some(pending) = self.pending.take() {
                        self.current = pending;
                        return self.tick(tick);
                    }
                }
                LampAction::Set(sentence_lamp_at(&sentence, tick.wrapping_sub(self.phase)))
            }
            None => {
                if let Some(pending) = self.pending.take() {
                    self.current = pending;
                    self.phase = tick;
                    if self.current.is_some() {
                        return self.tick(tick);
                    }
                }
                lamp_action(None, tick)
            }
        }
    }
}

/// `STORY-P1-07-09`: the refusal as canvas text — what the lamp spells in
/// blinks, the monitor states in one fixed-shape line.
#[must_use]
pub fn refusal_text(code: u8, detail: u16) -> [u8; 20] {
    let mut text = *b"CODE 00 DETAIL 00000";
    text[5] = b'0' + code / 10;
    text[6] = b'0' + code % 10;
    let mut value = detail;
    let mut index = 19;
    loop {
        text[index] = b'0' + (value % 10) as u8;
        value /= 10;
        if index == 15 {
            break;
        }
        index -= 1;
    }
    text
}

/// What the park loop does to the lamp on one tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LampAction {
    /// Drive the lamp to exactly this state (the confession pattern).
    Set(bool),
    /// Flip it (the plain 1 Hz pulse, on every tenth tick).
    Toggle,
    /// Leave it alone.
    Idle,
}

/// The per-tick lamp decision (`TEST-P1-09-07-A` clause 3 as amended by
/// `STORY-P1-09-11`): a refusal drives its spelled sentence; health pulses
/// at 1 Hz; nothing re-derives the discovery outcome — the sentence is
/// computed when the outcome is, never per tick.
pub fn lamp_action(sentence: Option<&Sentence>, tick: u32) -> LampAction {
    match sentence {
        Some(sentence) => LampAction::Set(sentence_lamp_at(sentence, tick)),
        None => {
            if tick.is_multiple_of(10) {
                LampAction::Toggle
            } else {
                LampAction::Idle
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ethernet::Discovery;
    use crate::etherrors::blink_code;
    use crate::gem::{LinkState, PhyOutcome};

    fn known_down() -> Discovery {
        Discovery::Present {
            revision: 0x0109,
            phy: PhyOutcome::Known { address: 1, id1: 0x600D, id2: 0x84A2 },
            link: Some(LinkState::Down),
        }
    }

    // TEST-P1-09-07-A clause 2: the pattern, pinned tick-by-tick.

    #[test]
    fn the_pattern_for_a_code_is_exact_and_periodic() {
        // Code 3: three 300 ms blinks, then two seconds of dark.
        let period = 3 * 6 + 20;
        let expected: Vec<bool> = [true, true, true, false, false, false]
            .repeat(3)
            .into_iter()
            .chain(std::iter::repeat_n(false, 20))
            .collect();
        let actual: Vec<bool> = (0..period).map(|t| blink_lamp_at(3, t)).collect();
        assert_eq!(actual, expected);
        // Periodicity: the same sentence forever.
        for tick in 0..period * 3 {
            assert_eq!(blink_lamp_at(3, tick), blink_lamp_at(3, tick + period));
        }
        // Code 1 pins the degenerate case: one blink, unambiguous gap.
        let one: Vec<bool> = (0..26).map(|t| blink_lamp_at(1, t)).collect();
        assert_eq!(&one[..6], &[true, true, true, false, false, false]);
        assert!(one[6..].iter().all(|&on| !on), "after the single blink, darkness");
    }

    // TEST-P1-09-07-A clause 3 (as amended by STORY-P1-09-11): the spelled
    // refusal never displaces the pulse.

    #[test]
    fn the_lamp_decision_speaks_the_sentence_on_refusal_and_pulses_on_health() {
        let sentence = sentence_for(9, 2);
        for tick in 0..120 {
            assert_eq!(
                lamp_action(Some(&sentence), tick),
                LampAction::Set(sentence_lamp_at(&sentence, tick))
            );
        }
        assert_eq!(lamp_action(None, 10), LampAction::Toggle);
        assert_eq!(lamp_action(None, 20), LampAction::Toggle);
        for tick in [1, 5, 9, 11, 19] {
            assert_eq!(lamp_action(None, tick), LampAction::Idle);
        }
        // A known PHY still waiting on the wire is health, not refusal —
        // the watch and the plain pulse coexist.
        assert_eq!(blink_code(&known_down()), None);
    }

    // TEST-P1-09-11-A clause 1: digit extraction at the boundaries.

    #[test]
    fn digits_are_least_significant_first_with_zero_as_a_flicker() {
        // Tonight's live case: code 9, module 0x0002 → "9, blip — 2 and four blips".
        assert_eq!(sentence_for(9, 2).groups, [9, 0, 2, 0, 0, 0, 0]);
        assert_eq!(sentence_for(15, 0).groups, [5, 1, 0, 0, 0, 0, 0]);
        assert_eq!(sentence_for(10, 65535).groups, [0, 1, 5, 3, 5, 5, 6]);
        assert_eq!(sentence_for(1, 9).groups, [1, 0, 9, 0, 0, 0, 0]);
        assert_eq!(sentence_for(3, 10).groups, [3, 0, 0, 1, 0, 0, 0]);
    }

    // TEST-P1-09-11-A clause 2: pure, pinned, and the gap hierarchy strict.

    #[test]
    fn the_sentence_timing_is_pinned_and_the_gap_hierarchy_is_strict() {
        const {
            assert!(BLINK_HALF_TICKS < GROUP_GAP_TICKS);
            assert!(GROUP_GAP_TICKS < SENTENCE_GAP_TICKS);
        }
        let sentence = sentence_for(1, 1); // [1, 0, 1, 0, 0, 0, 0]
        let period = sentence_period(&sentence);
        // Two fat single-blink groups (6 ticks each) and five flickers.
        assert_eq!(period, 2 * 6 + 5 * ZERO_SPAN_TICKS + 6 * GROUP_GAP_TICKS + SENTENCE_GAP_TICKS);
        // First group: one blink, then the group gap, then the zero flicker.
        let head: Vec<bool> = (0..6).map(|t| sentence_lamp_at(&sentence, t)).collect();
        assert_eq!(head, [true, true, true, false, false, false]);
        for t in 6..6 + GROUP_GAP_TICKS {
            assert!(!sentence_lamp_at(&sentence, t), "group gap is dark at {t}");
        }
        // The zero digit burns solid for its whole 1.5 s, then goes dark.
        for t in 0..ZERO_BURN_TICKS {
            assert!(sentence_lamp_at(&sentence, 6 + GROUP_GAP_TICKS + t), "burn tick {t}");
        }
        for t in ZERO_BURN_TICKS..ZERO_SPAN_TICKS {
            assert!(!sentence_lamp_at(&sentence, 6 + GROUP_GAP_TICKS + t));
        }
        // The tail is the long sentence gap, dark throughout.
        for t in period - SENTENCE_GAP_TICKS..period {
            assert!(!sentence_lamp_at(&sentence, t), "sentence gap is dark at {t}");
        }
        // Periodicity: the same sentence forever.
        for t in 0..period {
            assert_eq!(sentence_lamp_at(&sentence, t), sentence_lamp_at(&sentence, t + period));
        }
    }

    // TEST-P1-09-11-A clause 2, amended: a sentence in flight is never
    // replaced — a changed outcome is adopted only at a period boundary.

    #[test]
    fn a_sentence_in_flight_is_never_replaced_midway() {
        let first = sentence_for(8, 0);
        let second = sentence_for(9, 2);
        let period = sentence_period(&first);
        let mut latch = SentenceLatch::new(Some(first));
        // The outcome changes early in the first period…
        for tick in 0..period {
            if tick == 5 {
                latch.offer(Some(second));
            }
            // …but every tick of the whole period still spells the first.
            assert_eq!(
                latch.tick(tick),
                LampAction::Set(sentence_lamp_at(&first, tick)),
                "tick {tick} must stay on the latched sentence"
            );
        }
        // At the boundary the pending sentence is adopted, phase-fresh.
        assert_eq!(latch.tick(period), LampAction::Set(sentence_lamp_at(&second, 0)));
        assert_eq!(latch.tick(period + 1), LampAction::Set(sentence_lamp_at(&second, 1)));
    }

    #[test]
    fn health_adopts_immediately_and_a_recovered_chain_stops_spelling() {
        // A refusal that clears mid-sentence: the sentence finishes, then
        // the plain pulse takes over at the boundary.
        let sentence = sentence_for(3, 0x90);
        let period = sentence_period(&sentence);
        let mut latch = SentenceLatch::new(Some(sentence));
        latch.offer(None);
        for tick in 0..period {
            assert!(matches!(latch.tick(tick), LampAction::Set(_)));
        }
        assert_eq!(latch.tick(period), lamp_action(None, period));
        // And from health, a fresh refusal starts spelling at once — there
        // is nothing in flight to protect.
        let mut latch = SentenceLatch::new(None);
        latch.offer(Some(sentence));
        assert_eq!(latch.tick(40), LampAction::Set(sentence_lamp_at(&sentence, 0)));
    }

    // TEST-P1-07-09-A clause 3: the refusal as fixed-shape canvas text.

    #[test]
    fn the_refusal_text_is_fixed_shape_and_exact() {
        assert_eq!(&refusal_text(9, 2), b"CODE 09 DETAIL 00002");
        assert_eq!(&refusal_text(15, 65535), b"CODE 15 DETAIL 65535");
        assert_eq!(&refusal_text(3, 144), b"CODE 03 DETAIL 00144");
    }
}
