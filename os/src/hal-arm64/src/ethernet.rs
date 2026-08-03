//! `FEAT-P1-09` orchestration: probe → identify → PHY → link → beacon, and
//! the one `TOS64-LINK/1` line that reports whatever was learned.
//!
//! The pipeline is pure over the [`crate::pl011::Mmio`] seam and host-tested
//! end-to-end; the aarch64 glue at the bottom of this file owns the real
//! addresses, the pinned beacon buffer, the barrier before transmit start,
//! and the beacon-forever park. The report line is emitted exactly once,
//! strictly after `TOS64-RESULT/1` and the splash — the serial line is
//! evidence, and this Feature appends to it; it never reorders it.

use crate::gem::{self, LinkState, MdioPort, PhyOutcome, Speed, TxError};
use crate::pcie::{self, LinkAbsent};
use crate::pl011::Mmio;

/// Everything one discovery pass learned, in the order it was learned.
/// Every arm is reportable; nothing here is an error in the panicking sense.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Discovery {
    /// The link or window gates refused; the window was never read.
    LinkAbsent(LinkAbsent),
    /// The window answered but the identity readback was refused.
    IdentityRefused(gem::IdentityError),
    /// A GEM answered; the PHY scan and link read follow.
    Present {
        /// The GEM revision, from `MID`.
        revision: u16,
        /// What the PHY scan concluded.
        phy: PhyOutcome,
        /// The link, if a known PHY answered.
        link: Option<LinkState>,
    },
}

/// Runs the device half of "the cable is the signal" over any two MMIO
/// devices: the PCIe2 controller and the GEM window. The GEM device is not
/// touched unless the controller's gates pass — the tests assert that with a
/// device double that panics on any access.
///
/// `release_phy` is `STORY-P1-09-04`'s reset release, run exactly once,
/// strictly after the GEM identity validates and strictly before the
/// management port opens; `false` means the release aborted (stuck counter)
/// and the scan is skipped — a PHY still held in reset answers nobody.
pub fn discover<R: Mmio, G: Mmio>(
    root_complex: &R,
    gem_window: G,
    release_phy: impl FnOnce() -> bool,
) -> Discovery {
    // `STORY-P1-09-09`/`-10`: the establishment pass — link gates, the
    // window with its programming fallback, then the enumeration that tells
    // the bridge where to forward and verifies who is answering. Only after
    // all of it does the first bus read (the identity below) happen.
    if let Err(absent) = pcie::establish(root_complex) {
        return Discovery::LinkAbsent(absent);
    }
    let identity = match gem::parse_module_id(gem_window.read_u32(gem::register::MID)) {
        Ok(identity) => identity,
        Err(refused) => return Discovery::IdentityRefused(refused),
    };
    if !release_phy() {
        return Discovery::Present {
            revision: identity.revision,
            phy: PhyOutcome::ReleaseStuck,
            link: None,
        };
    }
    let port = MdioPort::enable(gem_window);
    let phy = gem::scan_for_phy(&port);
    let link = match phy {
        PhyOutcome::Known { address, .. } => gem::read_link(&port, address).ok(),
        _ => None,
    };
    let _device = port.finish();
    Discovery::Present { revision: identity.revision, phy, link }
}

/// What happened to the beacon, for the report line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeaconField {
    /// The first frame completed and the park loop keeps beaconing.
    Running,
    /// A transmit refused; beaconing is permanently stopped (fail-safe).
    Stopped(TxError),
    /// The link was down, unresolved, or the PHY unknown — nothing was sent.
    Skipped,
}

/// Capacity of the report-line buffer. The longest line (present, known PHY,
/// link up, beacon stopped with a status word) is under 100 bytes; the tests
/// pin real lengths.
pub const LINK_LINE_CAPACITY: usize = 128;

struct LineBuilder {
    bytes: [u8; LINK_LINE_CAPACITY],
    at: usize,
}

impl LineBuilder {
    const fn new() -> Self {
        LineBuilder { bytes: [0; LINK_LINE_CAPACITY], at: 0 }
    }

    fn push(&mut self, text: &str) {
        for byte in text.as_bytes() {
            if self.at < LINK_LINE_CAPACITY {
                self.bytes[self.at] = *byte;
                self.at += 1;
            }
        }
    }

    /// Decimal, most significant digit first, no leading zeros.
    fn push_dec(&mut self, value: u32) {
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
            if self.at < LINK_LINE_CAPACITY {
                self.bytes[self.at] = digits[count];
                self.at += 1;
            }
        }
    }

    /// Lower-case hex, fixed width, most significant nibble first.
    fn push_hex(&mut self, value: u64, nibbles: usize) {
        let mut index = nibbles;
        while index > 0 {
            index -= 1;
            let nibble = ((value >> (index * 4)) & 0xF) as u8;
            let digit = if nibble < 10 { b'0' + nibble } else { b'a' + (nibble - 10) };
            if self.at < LINK_LINE_CAPACITY {
                self.bytes[self.at] = digit;
                self.at += 1;
            }
        }
    }
}

/// Formats the single `TOS64-LINK/1` report line. Pure; every shape is
/// pinned by a host test, because this line is what a capture is compared
/// against.
pub fn link_line(discovery: &Discovery, beacon: BeaconField) -> ([u8; LINK_LINE_CAPACITY], usize) {
    let mut line = LineBuilder::new();
    line.push("TOS64-LINK/1 ");
    match discovery {
        Discovery::LinkAbsent(absent) => {
            line.push("rp1=absent reason=");
            let (reason, detail, nibbles) = match absent {
                LinkAbsent::PortNotRc(word) => ("port-not-rc", u64::from(*word), 8),
                LinkAbsent::PhyDown(word) => ("pcie-phy-down", u64::from(*word), 8),
                LinkAbsent::LinkDown(word) => ("pcie-link-down", u64::from(*word), 8),
                LinkAbsent::WindowBase(base) => ("window-base", *base, 10),
                LinkAbsent::WindowPci(pci) => ("window-pci", *pci, 10),
                LinkAbsent::WindowSpan(limit) => ("window-span", *limit, 10),
                LinkAbsent::RootVendor(word) => ("root-vendor", u64::from(*word), 8),
                LinkAbsent::EndpointVendor(word) => ("endpoint-vendor", u64::from(*word), 8),
            };
            line.push(reason);
            line.push(" detail=0x");
            line.push_hex(detail, nibbles);
        }
        Discovery::IdentityRefused(refused) => {
            line.push("rp1=absent reason=");
            match refused {
                gem::IdentityError::FloatingBus => line.push("id-floating"),
                gem::IdentityError::AllZeros => line.push("id-zero"),
                gem::IdentityError::WrongModule(module) => {
                    line.push("id-module detail=0x");
                    line.push_hex(u64::from(*module), 4);
                }
            }
        }
        Discovery::Present { revision, phy, link } => {
            line.push("rp1=present id=0x");
            line.push_hex(u64::from(*revision), 4);
            match phy {
                PhyOutcome::Known { id1, id2, .. } => {
                    line.push(" phy=0x");
                    line.push_hex((u64::from(*id1) << 16) | u64::from(*id2), 8);
                }
                PhyOutcome::Unknown { id1, id2, .. } => {
                    line.push(" phy=unknown id=0x");
                    line.push_hex((u64::from(*id1) << 16) | u64::from(*id2), 8);
                }
                PhyOutcome::Absent => line.push(" phy=absent"),
                PhyOutcome::PortWedged => line.push(" phy=wedged"),
                PhyOutcome::ReleaseStuck => line.push(" phy=unreleased"),
            }
            match link {
                Some(LinkState::Up { speed, full_duplex }) => {
                    line.push(" link=up speed=");
                    line.push(match speed {
                        Speed::Mbps1000 => "1000",
                        Speed::Mbps100 => "100",
                        Speed::Mbps10 => "10",
                    });
                    line.push(" duplex=");
                    line.push(if *full_duplex { "full" } else { "half" });
                }
                Some(LinkState::Down) => line.push(" link=down"),
                Some(LinkState::Unresolved) => line.push(" link=unresolved"),
                None => line.push(" link=unread"),
            }
        }
    }
    match beacon {
        BeaconField::Running => line.push(" beacon=running"),
        BeaconField::Skipped => line.push(" beacon=skipped"),
        BeaconField::Stopped(TxError::Timeout) => line.push(" beacon=stopped reason=timeout"),
        BeaconField::Stopped(TxError::MacError(status)) => {
            line.push(" beacon=stopped reason=mac detail=0x");
            line.push_hex(u64::from(status), 8);
        }
    }
    line.push("\n");
    (line.bytes, line.at)
}

/// Builds one `TOS64-BEAT/1` heartbeat line (`STORY-P1-09-05`): emitted every
/// park period so a passive listener can find a powered board without any
/// operator timing, and so a wrong UART clock becomes bytes at a findable
/// baud instead of silence. `fb_granted` reports whether the splash's
/// firmware framebuffer exchange succeeded — the field 06A's Question 1
/// needs: it splits "firmware refused the mailbox path" from "wrong plug
/// conditions" with no monitor involved. Pure; pinned by the tests.
pub fn heartbeat_line(
    seq: u32,
    beaconing: bool,
    fb_granted: bool,
) -> ([u8; LINK_LINE_CAPACITY], usize) {
    let mut line = LineBuilder::new();
    line.push("TOS64-BEAT/1 seq=");
    line.push_dec(seq);
    line.push(" state=");
    line.push(if beaconing { "beaconing" } else { "parked" });
    line.push(" fb=");
    line.push(if fb_granted { "granted" } else { "refused" });
    line.push("\n");
    (line.bytes, line.at)
}

/// Emits one heartbeat over the UART; `false` means the write refused and
/// heartbeating must stop permanently — fail-safe over keep-trying, and the
/// park itself is never disturbed.
pub fn emit_heartbeat<M: Mmio>(
    uart: &crate::pl011::Pl011<M>,
    seq: u32,
    beaconing: bool,
    fb_granted: bool,
) -> bool {
    let (line, len) = heartbeat_line(seq, beaconing, fb_granted);
    match core::str::from_utf8(&line[..len]) {
        Ok(text) => uart.write_str(text).is_ok(),
        Err(_) => false,
    }
}

/// Whether a discovery outcome earns a transmit attempt: only a present GEM,
/// a known PHY, and a resolved-up link do. Everything else skips — never a
/// frame into a dead or mismatched wire.
pub const fn beacon_eligible(discovery: &Discovery) -> Option<(Speed, bool)> {
    match discovery {
        Discovery::Present {
            phy: PhyOutcome::Known { .. },
            link: Some(LinkState::Up { speed, full_duplex }),
            ..
        } => Some((*speed, *full_duplex)),
        _ => None,
    }
}

/// `STORY-P1-09-06`: which discovery outcomes earn a link watch — a known
/// PHY whose link was anything but resolved-up when discovery looked. An
/// unknown or absent PHY earns nothing: the watch can wait forever, but it
/// may never widen what discovery was willing to believe.
pub const fn watch_from(discovery: &Discovery) -> Option<u8> {
    match discovery {
        Discovery::Present { phy: PhyOutcome::Known { address, .. }, link, .. } => match link {
            Some(LinkState::Up { .. }) => None,
            _ => Some(*address),
        },
        _ => None,
    }
}

/// One once-per-second link-watch step (`STORY-P1-09-06`): re-reads the link
/// and returns the negotiated rate the moment the wire has trained. The wire
/// decides *when*; this function only decides *whether* — it contains no
/// timing constant, because different hardware negotiates at different
/// speeds and no one bench's number may become the design.
///
/// Fail-safe: a wedged management port ends the watch permanently — `watch`
/// is taken to `None` and no later tick retries against it.
pub fn watch_step<M: Mmio>(
    watch: &mut Option<u8>,
    port: &MdioPort<M>,
) -> Option<(Speed, bool)> {
    let address = (*watch)?;
    match gem::read_link(port, address) {
        Ok(LinkState::Up { speed, full_duplex }) => {
            *watch = None;
            Some((speed, full_duplex))
        }
        Ok(_) => None,
        Err(gem::MdioError::Timeout) => {
            // TEST-P1-09-06-A clause 2: the wedge is terminal for the watch
            // and for nothing else.
            *watch = None;
            None
        }
    }
}

/// `STORY-P1-09-07`: the first refused rung of discovery as a blink count —
/// the confession the proven lamp can carry when serial is dead and the
/// screen is dark. `None` is health: a known PHY keeps the plain pulse,
/// whatever its link state, so the lamp's ordinary language is undiluted.
///
/// Matched exhaustively on purpose (`TEST-P1-09-07-A` clause 1): a future
/// `Discovery` arm fails to compile here rather than silently sharing a code.
pub const fn blink_code(discovery: &Discovery) -> Option<u8> {
    match discovery {
        Discovery::LinkAbsent(absent) => Some(match absent {
            LinkAbsent::PortNotRc(_) => 1,
            LinkAbsent::PhyDown(_) => 2,
            LinkAbsent::LinkDown(_) => 3,
            LinkAbsent::WindowBase(_) => 4,
            LinkAbsent::WindowPci(_) => 5,
            LinkAbsent::WindowSpan(_) => 6,
            LinkAbsent::RootVendor(_) => 14,
            LinkAbsent::EndpointVendor(_) => 15,
        }),
        Discovery::IdentityRefused(refused) => Some(match refused {
            gem::IdentityError::FloatingBus => 7,
            gem::IdentityError::AllZeros => 8,
            gem::IdentityError::WrongModule(_) => 9,
        }),
        Discovery::Present { phy, .. } => match phy {
            gem::PhyOutcome::ReleaseStuck => Some(10),
            gem::PhyOutcome::Absent => Some(11),
            gem::PhyOutcome::PortWedged => Some(12),
            gem::PhyOutcome::Unknown { .. } => Some(13),
            gem::PhyOutcome::Known { .. } => None,
        },
    }
}

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

/// `STORY-P1-09-08`: whether a discovery outcome is due a second look. Every
/// outcome short of a present GEM is a state to watch, not a verdict to
/// keep — the confession boot measured `DL_ACTIVE` clear at one early
/// moment, and a gate that may open late is re-read, never re-trusted. A
/// present GEM is final whatever its PHY or link state: those rungs have
/// their own channels (the confession, the link watch).
pub const fn reprobe_due(discovery: &Discovery) -> bool {
    !matches!(discovery, Discovery::Present { .. })
}

/// `STORY-P1-09-11`: the sixteen decisive bits each refusal spells after
/// its code — the wrong module itself, a vendor or status word's low half,
/// a window address in whole megabytes. Health never spells; the caller
/// only asks after [`blink_code`] said there is a refusal.
pub const fn blink_detail(discovery: &Discovery) -> u16 {
    match discovery {
        Discovery::LinkAbsent(absent) => match absent {
            LinkAbsent::PortNotRc(word)
            | LinkAbsent::PhyDown(word)
            | LinkAbsent::LinkDown(word)
            | LinkAbsent::RootVendor(word)
            | LinkAbsent::EndpointVendor(word) => *word as u16,
            LinkAbsent::WindowBase(value)
            | LinkAbsent::WindowPci(value)
            | LinkAbsent::WindowSpan(value) => (*value >> 20) as u16,
        },
        Discovery::IdentityRefused(refused) => match refused {
            gem::IdentityError::FloatingBus => 0xFFFF,
            gem::IdentityError::AllZeros => 0,
            gem::IdentityError::WrongModule(module) => *module,
        },
        Discovery::Present { phy, .. } => match phy {
            gem::PhyOutcome::Unknown { id1, .. } => *id1,
            _ => 0,
        },
    }
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
        let gap = if index == SENTENCE_GROUPS - 1 {
            SENTENCE_GAP_TICKS
        } else {
            GROUP_GAP_TICKS
        };
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
    use core::cell::Cell;

    /// A GEM double that panics on any access — proof the window is never
    /// touched behind failed gates.
    struct UntouchableGem;

    impl Mmio for UntouchableGem {
        fn read_u32(&self, offset: usize) -> u32 {
            panic!("the GEM window was read (offset {offset:#x}) behind a failed gate");
        }

        fn write_u32(&self, offset: usize, _value: u32) {
            panic!("the GEM window was written (offset {offset:#x}) behind a failed gate");
        }
    }

    /// A dead root complex: status reads zero.
    struct DeadRc;

    impl Mmio for DeadRc {
        fn read_u32(&self, _offset: usize) -> u32 {
            0
        }

        fn write_u32(&self, offset: usize, _value: u32) {
            panic!("the probe wrote {offset:#x}");
        }
    }

    // TEST-P1-09-01-A clause 2, end to end: a failed gate means the window is
    // never read.

    #[test]
    fn a_dead_link_never_touches_the_gem_window_or_the_gpio() {
        assert_eq!(
            discover(&DeadRc, UntouchableGem, || panic!("release ran behind a failed gate")),
            Discovery::LinkAbsent(LinkAbsent::PortNotRc(0))
        );
    }

    /// A healthy root complex answering the firmware-shaped window and the
    /// enumeration's config traffic, plus a GEM whose reads come from a
    /// tiny closure table. Writes are accepted only at the introduction's
    /// five registers — anything else still panics.
    struct HealthyRc;

    impl Mmio for HealthyRc {
        fn read_u32(&self, offset: usize) -> u32 {
            match offset {
                pcie::register::STATUS => 0x80 | 0x20 | 0x10,
                pcie::register::WIN0_BASE_LIMIT => 0x03F0_0000,
                pcie::register::WIN0_BASE_HI | pcie::register::WIN0_LIMIT_HI => 0x1F,
                pcie::config::RC_VENDOR => 0x2712_14E4,
                pcie::config::EP_VENDOR => 0x0001_1DE4,
                _ => 0,
            }
        }

        fn write_u32(&self, offset: usize, _value: u32) {
            match offset {
                pcie::config::RC_BUS_NUMBERS
                | pcie::config::RC_MEM_WINDOW
                | pcie::config::RC_COMMAND
                | pcie::config::EXT_CFG_INDEX
                | pcie::config::EP_COMMAND => {}
                other => panic!("the pipeline wrote {other:#x}"),
            }
        }
    }

    /// A GEM double scripted just enough for the full pipeline: identity,
    /// then a known PHY at address 1, then a gigabit link.
    struct PipelineGem {
        man_reads: Cell<usize>,
    }

    impl PipelineGem {
        fn new() -> Self {
            PipelineGem { man_reads: Cell::new(0) }
        }
    }

    impl Mmio for PipelineGem {
        fn read_u32(&self, offset: usize) -> u32 {
            match offset {
                gem::register::MID => 0x0007_0109,
                gem::register::NSR => gem::nsr::MDIO_IDLE,
                gem::register::NCR | gem::register::NCFGR => 0,
                gem::register::MAN => {
                    let index = self.man_reads.get();
                    self.man_reads.set(index + 1);
                    // Scan: addr 0 silent, addr 1 answers the Broadcom pair;
                    // then BMSR twice (latched then live), then 1000BASE-T
                    // partner status.
                    [
                        0xFFFF,
                        0x600D,
                        0x84A2,
                        0x0000,
                        u32::from(gem::bmsr::LINK_UP | gem::bmsr::AUTONEG_COMPLETE),
                        1 << 11,
                    ][index]
                }
                other => panic!("unexpected GEM read {other:#x}"),
            }
        }

        fn write_u32(&self, _offset: usize, _value: u32) {}
    }

    #[test]
    fn the_full_pipeline_reports_identity_phy_and_link() {
        let released = Cell::new(0u32);
        let discovery = discover(&HealthyRc, PipelineGem::new(), || {
            released.set(released.get() + 1);
            true
        });
        assert_eq!(released.get(), 1, "the release runs exactly once");
        assert_eq!(
            discovery,
            Discovery::Present {
                revision: 0x0109,
                phy: PhyOutcome::Known { address: 1, id1: 0x600D, id2: 0x84A2 },
                link: Some(LinkState::Up { speed: Speed::Mbps1000, full_duplex: true }),
            }
        );
        assert_eq!(beacon_eligible(&discovery), Some((Speed::Mbps1000, true)));
    }

    #[test]
    fn a_refused_identity_stops_the_pipeline_before_the_management_port() {
        struct FloatingGem;
        impl Mmio for FloatingGem {
            fn read_u32(&self, offset: usize) -> u32 {
                assert_eq!(offset, gem::register::MID, "only the identity may be read");
                0xFFFF_FFFF
            }
            fn write_u32(&self, offset: usize, _value: u32) {
                panic!("wrote {offset:#x} after a refused identity");
            }
        }
        assert_eq!(
            discover(&HealthyRc, FloatingGem, || panic!("release ran after a refused identity")),
            Discovery::IdentityRefused(gem::IdentityError::FloatingBus)
        );
    }

    // TEST-P1-09-04-A clause 4: the release sits between identity and the
    // scan, and an aborted release skips the scan honestly.

    #[test]
    fn an_aborted_release_skips_the_scan_and_reports_unreleased() {
        struct IdentityOnlyGem;
        impl Mmio for IdentityOnlyGem {
            fn read_u32(&self, offset: usize) -> u32 {
                assert_eq!(offset, gem::register::MID, "the scan must not run unreleased");
                0x0007_0109
            }
            fn write_u32(&self, offset: usize, _value: u32) {
                panic!("wrote {offset:#x} after an aborted release");
            }
        }
        let discovery = discover(&HealthyRc, IdentityOnlyGem, || false);
        assert_eq!(
            discovery,
            Discovery::Present { revision: 0x0109, phy: PhyOutcome::ReleaseStuck, link: None }
        );
        assert_eq!(beacon_eligible(&discovery), None);
        assert_eq!(
            line_text(&discovery, BeaconField::Skipped),
            "TOS64-LINK/1 rp1=present id=0x0109 phy=unreleased link=unread beacon=skipped\n"
        );
    }

    // TEST-P1-09-01-A clause 5 / TEST-P1-09-02-A clause 5 / TEST-P1-09-03-A:
    // the report line's shapes are pinned.

    fn line_text(discovery: &Discovery, beacon: BeaconField) -> String {
        let (bytes, len) = link_line(discovery, beacon);
        String::from_utf8(bytes[..len].to_vec()).expect("the line is ASCII")
    }

    #[test]
    fn the_absent_line_names_the_first_failed_rung_and_its_readback() {
        assert_eq!(
            line_text(&Discovery::LinkAbsent(LinkAbsent::PortNotRc(0)), BeaconField::Skipped),
            "TOS64-LINK/1 rp1=absent reason=port-not-rc detail=0x00000000 beacon=skipped\n"
        );
        assert_eq!(
            line_text(
                &Discovery::LinkAbsent(LinkAbsent::WindowBase(0x0000_001E_0000_0000)),
                BeaconField::Skipped
            ),
            "TOS64-LINK/1 rp1=absent reason=window-base detail=0x1e00000000 beacon=skipped\n"
        );
        assert_eq!(
            line_text(
                &Discovery::IdentityRefused(gem::IdentityError::WrongModule(2)),
                BeaconField::Skipped
            ),
            "TOS64-LINK/1 rp1=absent reason=id-module detail=0x0002 beacon=skipped\n"
        );
        // TEST-P1-09-10-A clause 3: the vendor refusals carry their readback.
        assert_eq!(
            line_text(
                &Discovery::LinkAbsent(LinkAbsent::RootVendor(0xFFFF_FFFF)),
                BeaconField::Skipped
            ),
            "TOS64-LINK/1 rp1=absent reason=root-vendor detail=0xffffffff beacon=skipped\n"
        );
        assert_eq!(
            line_text(
                &Discovery::LinkAbsent(LinkAbsent::EndpointVendor(0x0001_2E8A)),
                BeaconField::Skipped
            ),
            "TOS64-LINK/1 rp1=absent reason=endpoint-vendor detail=0x00012e8a beacon=skipped\n"
        );
    }

    #[test]
    fn the_present_line_reports_identity_phy_link_and_beacon() {
        let discovery = Discovery::Present {
            revision: 0x0109,
            phy: PhyOutcome::Known { address: 1, id1: 0x600D, id2: 0x84A2 },
            link: Some(LinkState::Up { speed: Speed::Mbps1000, full_duplex: true }),
        };
        assert_eq!(
            line_text(&discovery, BeaconField::Running),
            "TOS64-LINK/1 rp1=present id=0x0109 phy=0x600d84a2 link=up speed=1000 \
             duplex=full beacon=running\n"
        );
    }

    #[test]
    fn a_down_link_and_a_stopped_beacon_are_honest_shapes() {
        let down = Discovery::Present {
            revision: 0x0109,
            phy: PhyOutcome::Known { address: 1, id1: 0x600D, id2: 0x84A2 },
            link: Some(LinkState::Down),
        };
        assert_eq!(
            line_text(&down, BeaconField::Skipped),
            "TOS64-LINK/1 rp1=present id=0x0109 phy=0x600d84a2 link=down beacon=skipped\n"
        );
        assert_eq!(beacon_eligible(&down), None);
        let up = Discovery::Present {
            revision: 0x0109,
            phy: PhyOutcome::Known { address: 1, id1: 0x600D, id2: 0x84A2 },
            link: Some(LinkState::Up { speed: Speed::Mbps100, full_duplex: false }),
        };
        assert_eq!(
            line_text(&up, BeaconField::Stopped(TxError::MacError(0x40))),
            "TOS64-LINK/1 rp1=present id=0x0109 phy=0x600d84a2 link=up speed=100 \
             duplex=half beacon=stopped reason=mac detail=0x00000040\n"
        );
    }

    // TEST-P1-09-05-A clauses 1 and 3: the heartbeat is exact bytes and
    // stops fail-safe on a refused write.

    #[test]
    fn the_heartbeat_line_is_exact_bytes_with_every_field_driven() {
        let (bytes, len) = heartbeat_line(1, true, true);
        assert_eq!(
            core::str::from_utf8(&bytes[..len]).unwrap(),
            "TOS64-BEAT/1 seq=1 state=beaconing fb=granted\n"
        );
        let (bytes, len) = heartbeat_line(42, false, false);
        assert_eq!(
            core::str::from_utf8(&bytes[..len]).unwrap(),
            "TOS64-BEAT/1 seq=42 state=parked fb=refused\n"
        );
        let (a, len_a) = heartbeat_line(7, true, false);
        let (b, len_b) = heartbeat_line(8, true, false);
        assert_eq!(len_a, len_b);
        let differing: Vec<usize> = (0..len_a).filter(|&i| a[i] != b[i]).collect();
        assert_eq!(differing.len(), 1, "the sequence digit is the only variance");
    }

    #[test]
    fn a_refused_uart_write_stops_the_heartbeat_permanently() {
        use crate::pl011::{register, Pl011};
        /// Always-ready wire, so the write succeeds.
        struct ReadyWire;
        impl Mmio for ReadyWire {
            fn read_u32(&self, _offset: usize) -> u32 {
                0
            }
            fn write_u32(&self, _offset: usize, _value: u32) {}
        }
        assert!(emit_heartbeat(&Pl011::new(ReadyWire), 1, false, true));
        /// A wedged transmit FIFO: the flag register always reads full, so
        /// `write_str` times out and the heartbeat must report stop.
        struct WedgedWire;
        impl Mmio for WedgedWire {
            fn read_u32(&self, offset: usize) -> u32 {
                if offset == register::FR {
                    1 << 5 // TXFF: transmit FIFO full, forever
                } else {
                    0
                }
            }
            fn write_u32(&self, _offset: usize, _value: u32) {}
        }
        assert!(!emit_heartbeat(&Pl011::new(WedgedWire), 2, true, false));
    }

    // TEST-P1-09-08-A clause 1: re-probe eligibility is total, refusal-only.

    #[test]
    fn every_outcome_short_of_a_present_gem_is_due_a_second_look() {
        assert!(reprobe_due(&Discovery::LinkAbsent(LinkAbsent::PortNotRc(0))));
        assert!(reprobe_due(&Discovery::LinkAbsent(LinkAbsent::LinkDown(0x90))));
        assert!(reprobe_due(&Discovery::LinkAbsent(LinkAbsent::WindowSpan(1))));
        assert!(reprobe_due(&Discovery::IdentityRefused(gem::IdentityError::FloatingBus)));
        // Present is final in every shape: those rungs have their own
        // channels (the confession, the link watch).
        for phy in [
            PhyOutcome::Known { address: 1, id1: 0x600D, id2: 0x84A2 },
            PhyOutcome::Unknown { address: 0, id1: 1, id2: 2 },
            PhyOutcome::Absent,
            PhyOutcome::PortWedged,
            PhyOutcome::ReleaseStuck,
        ] {
            assert!(
                !reprobe_due(&Discovery::Present { revision: 1, phy, link: None }),
                "{phy:?} must be final"
            );
        }
    }

    /// A controller whose status names the confession boot's exact reading:
    /// RC mode and PCIe PHY up, `DL_ACTIVE` clear.
    struct DlDownRc;

    impl Mmio for DlDownRc {
        fn read_u32(&self, offset: usize) -> u32 {
            match offset {
                pcie::register::STATUS => 0x80 | 0x10,
                other => panic!("a refused link earns exactly one read, not {other:#x}"),
            }
        }

        fn write_u32(&self, offset: usize, _value: u32) {
            panic!("the probe wrote {offset:#x}");
        }
    }

    // TEST-P1-09-08-A clauses 2 and 3: refused re-probes touch nothing
    // downstream, and a late settle runs the pipeline once with the release
    // still exactly once — counted across every pass.
    #[test]
    fn a_late_data_link_is_caught_with_the_release_still_run_exactly_once() {
        // Any number of refused passes: window and GPIO untouchable.
        for _ in 0..5 {
            let refused = discover(&DlDownRc, UntouchableGem, || {
                panic!("release behind a failed gate")
            });
            assert_eq!(refused, Discovery::LinkAbsent(LinkAbsent::LinkDown(0x90)));
            assert!(reprobe_due(&refused));
            assert_eq!(blink_code(&refused), Some(3), "the lamp keeps counting 3");
        }
        // The pass where the gate finally reads settled: the whole pipeline,
        // the release exactly once, and every channel adopts the outcome.
        let released = Cell::new(0u32);
        let settled = discover(&HealthyRc, PipelineGem::new(), || {
            released.set(released.get() + 1);
            true
        });
        assert_eq!(released.get(), 1, "one release across all passes");
        assert!(!reprobe_due(&settled), "a present GEM is final");
        assert_eq!(blink_code(&settled), None, "the lamp returns to the plain pulse");
        assert_eq!(watch_from(&settled), None, "the scripted link is already up");
        assert_eq!(beacon_eligible(&settled), Some((Speed::Mbps1000, true)));
    }

    // TEST-P1-09-07-A clause 1: the mapping is total, distinct, refusal-only.

    #[test]
    fn every_refusal_earns_a_distinct_code_and_health_earns_none() {
        let refusals: Vec<Discovery> = vec![
            Discovery::LinkAbsent(LinkAbsent::PortNotRc(0)),
            Discovery::LinkAbsent(LinkAbsent::PhyDown(0x80)),
            Discovery::LinkAbsent(LinkAbsent::LinkDown(0x90)),
            Discovery::LinkAbsent(LinkAbsent::WindowBase(0)),
            Discovery::LinkAbsent(LinkAbsent::WindowPci(1)),
            Discovery::LinkAbsent(LinkAbsent::WindowSpan(2)),
            Discovery::LinkAbsent(LinkAbsent::RootVendor(0xFFFF_FFFF)),
            Discovery::LinkAbsent(LinkAbsent::EndpointVendor(0)),
            Discovery::IdentityRefused(gem::IdentityError::FloatingBus),
            Discovery::IdentityRefused(gem::IdentityError::AllZeros),
            Discovery::IdentityRefused(gem::IdentityError::WrongModule(2)),
            Discovery::Present { revision: 1, phy: PhyOutcome::ReleaseStuck, link: None },
            Discovery::Present { revision: 1, phy: PhyOutcome::Absent, link: None },
            Discovery::Present { revision: 1, phy: PhyOutcome::PortWedged, link: None },
            Discovery::Present {
                revision: 1,
                phy: PhyOutcome::Unknown { address: 0, id1: 1, id2: 2 },
                link: None,
            },
        ];
        let codes: Vec<u8> =
            refusals.iter().map(|d| blink_code(d).expect("every refusal speaks")).collect();
        let mut deduped = codes.clone();
        deduped.sort_unstable();
        deduped.dedup();
        assert_eq!(deduped.len(), codes.len(), "no two refusals may share a code: {codes:?}");
        assert!(codes.iter().all(|&code| code > 0));
        // The first rung of each family, pinned by number so a session log
        // can be read years later without the source.
        assert_eq!(blink_code(&refusals[0]), Some(1));
        assert_eq!(blink_code(&refusals[6]), Some(14), "root-vendor counts 14");
        assert_eq!(blink_code(&refusals[7]), Some(15), "endpoint-vendor counts 15");
        assert_eq!(blink_code(&refusals[8]), Some(7));
        assert_eq!(blink_code(&refusals[11]), Some(10));
        // Health — a known PHY in any link state — keeps the plain pulse.
        for link in [
            None,
            Some(LinkState::Down),
            Some(LinkState::Unresolved),
            Some(LinkState::Up { speed: Speed::Mbps1000, full_duplex: true }),
        ] {
            let healthy = Discovery::Present {
                revision: 0x0109,
                phy: PhyOutcome::Known { address: 1, id1: 0x600D, id2: 0x84A2 },
                link,
            };
            assert_eq!(blink_code(&healthy), None, "health never blinks a code: {link:?}");
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

    // TEST-P1-09-11-A clause 3: detail selection is total, arm by arm.

    #[test]
    fn every_refusal_selects_its_named_sixteen_bits() {
        use gem::IdentityError;
        assert_eq!(blink_detail(&Discovery::IdentityRefused(IdentityError::WrongModule(2))), 2);
        assert_eq!(
            blink_detail(&Discovery::IdentityRefused(IdentityError::FloatingBus)),
            0xFFFF
        );
        assert_eq!(blink_detail(&Discovery::IdentityRefused(IdentityError::AllZeros)), 0);
        assert_eq!(blink_detail(&Discovery::LinkAbsent(LinkAbsent::LinkDown(0x90))), 0x90);
        assert_eq!(
            blink_detail(&Discovery::LinkAbsent(LinkAbsent::RootVendor(0x2712_14E4))),
            0x14E4,
            "a vendor dword spells its vendor half"
        );
        // Window addresses spell the low sixteen bits of their megabyte
        // index — 0x1E_0000_0000 is MiB 0x1E000, truncating to 0xE000.
        assert_eq!(
            blink_detail(&Discovery::LinkAbsent(LinkAbsent::WindowBase(0x0000_001E_0000_0000))),
            0xE000
        );
        assert_eq!(
            blink_detail(&Discovery::Present {
                revision: 1,
                phy: PhyOutcome::Unknown { address: 0, id1: 0x0141, id2: 0x0C86 },
                link: None,
            }),
            0x0141,
            "an unknown PHY spells its ID1"
        );
    }

    // TEST-P1-09-06-A: the link watch. A scripted management port whose MAN
    // answers are a queue; NSR is idle unless the script says wedged.

    use core::cell::RefCell;
    use std::collections::VecDeque;

    struct ScriptedLink {
        man: RefCell<VecDeque<u32>>,
        wedged: bool,
    }

    impl ScriptedLink {
        fn answering(man: &[u32]) -> Self {
            ScriptedLink { man: RefCell::new(man.iter().copied().collect()), wedged: false }
        }

        fn wedged() -> Self {
            ScriptedLink { man: RefCell::new(VecDeque::new()), wedged: true }
        }

        /// One poll's worth of down: the latched read then the live read,
        /// both without the link bit.
        fn push_down(&self) {
            self.man.borrow_mut().extend([0u32, 0]);
        }

        /// One poll's worth of gigabit-full up: latched, live (up and
        /// negotiated), then the 1000BASE-T partner status.
        fn push_up(&self) {
            let live = u32::from(gem::bmsr::LINK_UP | gem::bmsr::AUTONEG_COMPLETE);
            self.man.borrow_mut().extend([0, live, 1 << 11]);
        }
    }

    impl Mmio for ScriptedLink {
        fn read_u32(&self, offset: usize) -> u32 {
            match offset {
                gem::register::NSR => {
                    if self.wedged {
                        0
                    } else {
                        gem::nsr::MDIO_IDLE
                    }
                }
                gem::register::MAN => self.man.borrow_mut().pop_front().expect("scripted answer"),
                gem::register::NCR | gem::register::NCFGR => 0,
                other => panic!("unexpected read {other:#x}"),
            }
        }

        fn write_u32(&self, _offset: usize, _value: u32) {}
    }

    fn known_down() -> Discovery {
        Discovery::Present {
            revision: 0x0109,
            phy: PhyOutcome::Known { address: 1, id1: 0x600D, id2: 0x84A2 },
            link: Some(LinkState::Down),
        }
    }

    #[test]
    fn only_a_known_phy_without_a_resolved_link_earns_a_watch() {
        assert_eq!(watch_from(&known_down()), Some(1));
        let unresolved = Discovery::Present {
            revision: 0x0109,
            phy: PhyOutcome::Known { address: 3, id1: 0x600D, id2: 0x84A2 },
            link: Some(LinkState::Unresolved),
        };
        assert_eq!(watch_from(&unresolved), Some(3));
        let unread = Discovery::Present {
            revision: 0x0109,
            phy: PhyOutcome::Known { address: 1, id1: 0x600D, id2: 0x84A2 },
            link: None,
        };
        assert_eq!(watch_from(&unread), Some(1));
        // Already up: nothing to watch for.
        let up = Discovery::Present {
            revision: 0x0109,
            phy: PhyOutcome::Known { address: 1, id1: 0x600D, id2: 0x84A2 },
            link: Some(LinkState::Up { speed: Speed::Mbps1000, full_duplex: true }),
        };
        assert_eq!(watch_from(&up), None);
        // The watch may never widen what discovery believed.
        let unknown = Discovery::Present {
            revision: 0x0109,
            phy: PhyOutcome::Unknown { address: 0, id1: 0x0141, id2: 0x0C86 },
            link: None,
        };
        assert_eq!(watch_from(&unknown), None);
        assert_eq!(watch_from(&Discovery::LinkAbsent(LinkAbsent::PortNotRc(0))), None);
    }

    // TEST-P1-09-06-A clause 1: a late link-up starts the beacon, for any N —
    // the decision contains no timing constant of its own.
    #[test]
    fn a_late_link_up_resolves_on_exactly_the_poll_the_wire_trains() {
        for n in [3usize, 7] {
            let script = ScriptedLink::answering(&[]);
            for _ in 0..n {
                script.push_down();
            }
            script.push_up();
            let port = MdioPort::enable(script);
            let mut watch = watch_from(&known_down());
            for poll in 0..n {
                assert_eq!(watch_step(&mut watch, &port), None, "down at poll {poll}");
                assert!(watch.is_some(), "still watching after poll {poll}");
            }
            assert_eq!(
                watch_step(&mut watch, &port),
                Some((Speed::Mbps1000, true)),
                "the wire trained on poll {n}"
            );
            assert_eq!(watch, None, "a resolved watch is finished");
        }
    }

    // TEST-P1-09-06-A clause 1, the heartbeat half: the composition the park
    // loop performs, pinned — the line flips to beaconing on the tick the
    // watch resolves and never before.
    #[test]
    fn the_heartbeat_flips_to_beaconing_on_the_tick_the_watch_resolves() {
        let script = ScriptedLink::answering(&[]);
        script.push_down();
        script.push_down();
        script.push_up();
        let port = MdioPort::enable(script);
        let mut watch = watch_from(&known_down());
        let mut beaconing = false;
        let mut states = Vec::new();
        for seq in 1..=3u32 {
            if watch_step(&mut watch, &port).is_some() {
                beaconing = true;
            }
            let (line, len) = heartbeat_line(seq, beaconing, false);
            states.push(String::from_utf8(line[..len].to_vec()).unwrap());
        }
        assert!(states[0].contains("state=parked"));
        assert!(states[1].contains("state=parked"));
        assert!(states[2].contains("state=beaconing"));
    }

    // TEST-P1-09-06-A clause 2: a wedged port ends the watch permanently,
    // and a finished watch never touches the port again.
    #[test]
    fn a_wedged_port_ends_the_watch_permanently() {
        let port = MdioPort::enable(ScriptedLink::wedged());
        let mut watch = Some(1u8);
        assert_eq!(watch_step(&mut watch, &port), None);
        assert_eq!(watch, None, "the wedge is terminal");

        struct Untouchable;
        impl Mmio for Untouchable {
            fn read_u32(&self, offset: usize) -> u32 {
                panic!("a finished watch read {offset:#x}");
            }
            fn write_u32(&self, offset: usize, _value: u32) {
                panic!("a finished watch wrote {offset:#x}");
            }
        }
        let untouchable = MdioPort::wrap_untouched_for_test(Untouchable);
        assert_eq!(watch_step(&mut watch, &untouchable), None);
    }

    // TEST-P1-09-06-A clause 3: a link that never trains stays honestly
    // parked — the watch can wait forever but can never invent a beacon.
    #[test]
    fn a_link_that_never_trains_never_resolves_and_never_stops_watching() {
        let script = ScriptedLink::answering(&[]);
        for _ in 0..50 {
            script.push_down();
        }
        let port = MdioPort::enable(script);
        let mut watch = watch_from(&known_down());
        for _ in 0..50 {
            assert_eq!(watch_step(&mut watch, &port), None);
            assert!(watch.is_some());
        }
    }

    #[test]
    fn an_unknown_phy_skips_the_beacon_by_construction() {
        let unknown = Discovery::Present {
            revision: 0x0109,
            phy: PhyOutcome::Unknown { address: 0, id1: 0x0141, id2: 0x0C86 },
            link: None,
        };
        assert_eq!(beacon_eligible(&unknown), None);
        assert_eq!(
            line_text(&unknown, BeaconField::Skipped),
            "TOS64-LINK/1 rp1=present id=0x0109 phy=unknown id=0x01410c86 link=unread \
             beacon=skipped\n"
        );
    }
}

// --- aarch64 glue -----------------------------------------------------------
//
// Everything above this line is host-tested; everything below is the thin
// glue those tests cannot reach: real addresses, the pinned buffer and ring,
// the barrier that orders memory before the MAC starts, and the
// beacon-forever park.

/// One beacon per second, expressed in generic-timer ticks at runtime.
#[cfg(target_arch = "aarch64")]
mod glue {
    use super::*;
    use crate::board;
    use crate::pl011::{Pl011, VolatileMmio};

    /// The beacon buffer and transmit ring, 64-byte aligned so descriptors
    /// never straddle what the MAC fetches. One static grant, per `LE-67`.
    #[repr(C, align(64))]
    struct BeaconMemory {
        ring: [[u32; 4]; 2],
        frame: [u8; gem::BEACON_CAPACITY],
    }

    static mut BEACON_MEMORY: BeaconMemory =
        BeaconMemory { ring: [[0; 4]; 2], frame: [0; gem::BEACON_CAPACITY] };

    /// Bound on the ticks-elapsed busy wait, so a stuck counter converts to
    /// a stop instead of a silent hang (`SEC-20` — no unbounded wait, even
    /// in a park loop).
    const WAIT_SPINS_LIMIT: u32 = 2_000_000_000;

    fn counter_ticks() -> u64 {
        let value: u64;
        // SAFETY: side-effect-free read of the always-readable virtual
        // counter, the cycle source `STORY-P1-01-03` recorded.
        unsafe {
            core::arch::asm!("mrs {v}, CNTVCT_EL0", v = out(reg) value,
                options(nomem, nostack, preserves_flags));
        }
        value
    }

    fn counter_frequency() -> u64 {
        let value: u64;
        // SAFETY: as above, for the counter frequency register.
        unsafe {
            core::arch::asm!("mrs {v}, CNTFRQ_EL0", v = out(reg) value,
                options(nomem, nostack, preserves_flags));
        }
        value
    }

    /// Waits `ticks` counter ticks; returns `false` if the counter never
    /// advanced far enough within the spin bound (a stuck counter).
    fn wait_ticks(ticks: u64) -> bool {
        let start = counter_ticks();
        let mut spins = 0u32;
        while counter_ticks().wrapping_sub(start) < ticks {
            spins += 1;
            if spins >= WAIT_SPINS_LIMIT {
                return false;
            }
        }
        true
    }

    /// Bounded millisecond wait for `STORY-P1-09-04`'s hold and settle and
    /// `STORY-P1-09-05`'s park tick.
    fn wait_millis(ms: u32) -> bool {
        wait_ticks((counter_frequency().max(1) / 1000).max(1) * u64::from(ms))
    }

    /// Writes the frame and ring for `seq` into the pinned memory and
    /// returns the ring's DMA address. Plain stores are sufficient: with the
    /// MMU off every access is Device-nGnRnE and nothing is cached; the
    /// `dsb sy` afterwards orders them before the MAC is started.
    fn stage_frame(seq: u32) -> u64 {
        let (frame, len) = gem::beacon_frame(seq);
        // SAFETY: single core, interrupts masked since `drop_to_el1`, and
        // this function is the only writer of `BEACON_MEMORY`; the raw
        // pointer avoids taking a reference to a mutable static.
        unsafe {
            let memory = core::ptr::addr_of_mut!(BEACON_MEMORY);
            (*memory).frame = frame;
            let frame_dma = board::RP1_DMA_RAM_BASE + core::ptr::addr_of!((*memory).frame) as u64;
            (*memory).ring = gem::tx_ring(frame_dma, len);
            core::arch::asm!("dsb sy", options(nostack, preserves_flags));
            board::RP1_DMA_RAM_BASE + core::ptr::addr_of!((*memory).ring) as u64
        }
    }

    /// The whole device half of 04A's sentence, then the park that keeps
    /// announcing on every channel the board has (`STORY-P1-09-05`):
    /// discover, attempt the first beacon, report the one `TOS64-LINK/1`
    /// line — then tick at ~10 Hz, animating the splash surface every tick
    /// and emitting a serial heartbeat plus (if eligible) a beacon frame
    /// every tenth. Each channel stops independently and fail-safe; when
    /// every channel has stopped, the board is simply parked.
    pub fn announce_and_park(
        uart: &Pl011<VolatileMmio>,
        splash: Option<crate::hdmi::FramebufferInfo>,
    ) -> ! {
        // SAFETY: the constants are the recorded CPU-physical bases of the
        // PCIe2 controller block and the GEM window; both are naturally
        // aligned register files this core may access uncached.
        let root_complex = unsafe { VolatileMmio::new(board::PCIE2_BASE) };
        // SAFETY: as above; the window is only dereferenced after the probe's
        // gates confirm the firmware kept it mapped.
        let gem_window =
            unsafe { VolatileMmio::new(board::RP1_WINDOW_BASE + board::RP1_GEM_OFFSET) };
        // SAFETY: as above — the window base itself, for the RP1 GPIO blocks
        // (`STORY-P1-09-04`); dereferenced only behind the probe's gates,
        // inside the span the window validation requires.
        let rp1_window = unsafe { VolatileMmio::new(board::RP1_WINDOW_BASE) };
        // SAFETY: the BCM2712 `gpio-brcmstb` block the board itself reported
        // on silicon (`pios-ground-truth-2026-08-03.txt`); single core, sole
        // writer. On the SoC side of every gate above — the lamp needs none
        // of them (`STORY-P1-07-08`).
        let stat_gpio = unsafe { VolatileMmio::new(board::STAT_GPIO_BASE) };

        let mut discovery = discover(&root_complex, gem_window, || {
            crate::rp1_gpio::release_phy_reset(&rp1_window, wait_millis).is_ok()
        });
        let beacon = match beacon_eligible(&discovery) {
            Some((speed, full_duplex)) => {
                let ring_dma = stage_frame(0);
                match gem::transmit_once(&gem_window, ring_dma, speed, full_duplex) {
                    Ok(()) => BeaconField::Running,
                    Err(refused) => BeaconField::Stopped(refused),
                }
            }
            None => BeaconField::Skipped,
        };

        let (line, len) = link_line(&discovery, beacon);
        if let Ok(text) = core::str::from_utf8(&line[..len]) {
            let _ = uart.write_str(text);
        }

        // `STORY-P1-07-09`: the firmware's canvas — background, title, and
        // the report line as text. UX beside the mailbox splash, never
        // instead of it; the serial line above stays the evidence.
        let mut console = crate::canvas::SimplefbSurface;
        crate::canvas::draw_frame(&mut console);
        crate::canvas::draw_line(
            &mut console,
            crate::canvas::REPORT_Y,
            &line[..len.saturating_sub(1)],
            crate::canvas::TEXT,
        );

        let mut beaconing = matches!(beacon, BeaconField::Running);
        let mut speed_config = beacon_eligible(&discovery);
        // `STORY-P1-09-06`: a known PHY whose link was not up when discovery
        // looked stays watched from the park loop — the wire decides when.
        let mut watch = watch_from(&discovery);
        let mut heartbeating = true;
        let fb_granted = splash.is_some();
        let mut animation = splash.map(|info| {
            (crate::hdmi::Framebuffer { info }, crate::hdmi::Bounce::new(info.width, info.height))
        });
        // `STORY-P1-09-07`/`-11`: the latch owns the lamp — outcome changes
        // are offered, adopted only at sentence boundaries.
        let mut lamp = SentenceLatch::new(
            blink_code(&discovery).map(|code| sentence_for(code, blink_detail(&discovery))),
        );
        let mut beat_seq: u32 = 1;
        let mut frame_seq: u32 = 1;
        let mut tick: u32 = 0;
        loop {
            if !wait_millis(100) {
                // A stuck counter stops every periodic channel at once.
                break;
            }
            tick = tick.wrapping_add(1);
            // TEST-P1-09-07-A clause 3: refusal speaks the blink code,
            // health pulses at 1 Hz — decided per tick by the pure function.
            match lamp.tick(tick) {
                LampAction::Set(on) => crate::stat_led::drive(&stat_gpio, on),
                LampAction::Toggle => crate::stat_led::toggle(&stat_gpio),
                LampAction::Idle => {}
            }
            if let Some((surface, bounce)) = animation.as_mut() {
                let (old_x, old_y, size) = (bounce.x, bounce.y, bounce.size);
                bounce.step(surface.info.width, surface.info.height);
                crate::hdmi::fill_rect(surface, old_x, old_y, size, size, crate::hdmi::BACKGROUND);
                crate::hdmi::fill_rect(
                    surface,
                    bounce.x,
                    bounce.y,
                    size,
                    size,
                    crate::hdmi::heartbeat_color(beaconing),
                );
            }
            if tick.is_multiple_of(10) {
                // TEST-P1-09-08-A clause 3: while discovery reports absence,
                // one re-probe per second. The gates keep refused passes away
                // from the window and the GPIO, so the release still runs at
                // most once — on the single pass where identity first
                // validates. A settled chain adopts every channel at once.
                if reprobe_due(&discovery) {
                    discovery = discover(&root_complex, gem_window, || {
                        crate::rp1_gpio::release_phy_reset(&rp1_window, wait_millis).is_ok()
                    });
                    lamp.offer(
                        blink_code(&discovery)
                            .map(|code| sentence_for(code, blink_detail(&discovery))),
                    );
                    if !reprobe_due(&discovery) {
                        speed_config = beacon_eligible(&discovery);
                        beaconing = speed_config.is_some();
                        watch = watch_from(&discovery);
                        // The canvas report line follows the newest outcome
                        // (UX; the serial line's exactly-once is untouched).
                        let field =
                            if beaconing { BeaconField::Running } else { BeaconField::Skipped };
                        let (line, len) = link_line(&discovery, field);
                        crate::canvas::draw_line(
                            &mut console,
                            crate::canvas::REPORT_Y,
                            &line[..len.saturating_sub(1)],
                            crate::canvas::TEXT,
                        );
                    }
                }
                // `STORY-P1-07-09`: the live status and any refusal, as text.
                let (status, status_len) = heartbeat_line(beat_seq, beaconing, fb_granted);
                crate::canvas::draw_line(
                    &mut console,
                    crate::canvas::STATUS_Y,
                    &status[..status_len.saturating_sub(1)],
                    crate::canvas::TEXT,
                );
                match blink_code(&discovery) {
                    Some(code) => crate::canvas::draw_line(
                        &mut console,
                        crate::canvas::REFUSAL_Y,
                        &refusal_text(code, blink_detail(&discovery)),
                        crate::canvas::ALERT,
                    ),
                    None => crate::canvas::draw_line(
                        &mut console,
                        crate::canvas::REFUSAL_Y,
                        b"                    ",
                        crate::canvas::TEXT,
                    ),
                }
                // TEST-P1-09-06-A clause 1: while the link is down, one
                // bounded poll per second — the beacon starts on whatever
                // tick the wire trains, with no bench-tuned constant.
                if !beaconing && speed_config.is_none() && watch.is_some() {
                    let port = MdioPort::enable(gem_window);
                    if let Some(config) = watch_step(&mut watch, &port) {
                        speed_config = Some(config);
                        beaconing = true;
                    }
                    let _device = port.finish();
                }
                if beaconing {
                    match speed_config {
                        Some((speed, full_duplex)) => {
                            let ring_dma = stage_frame(frame_seq);
                            if gem::transmit_once(&gem_window, ring_dma, speed, full_duplex)
                                .is_err()
                            {
                                // Fail-safe over keep-trying: one refusal
                                // ends the beacon permanently.
                                beaconing = false;
                            } else {
                                frame_seq = frame_seq.wrapping_add(1);
                            }
                        }
                        None => beaconing = false,
                    }
                }
                if heartbeating && !emit_heartbeat(uart, beat_seq, beaconing, fb_granted) {
                    heartbeating = false;
                }
                beat_seq = beat_seq.wrapping_add(1);
                if !beaconing
                    && !heartbeating
                    && animation.is_none()
                    && watch.is_none()
                    && !reprobe_due(&discovery)
                {
                    // A live watch — of the link or of the probe — keeps the
                    // loop alive: a cable plugged in or a data link settling
                    // next week must still start the beacon
                    // (`STORY-P1-09-06`/`-08`). The lamp freezes only here,
                    // with every channel honestly finished.
                    break;
                }
            }
        }
        crate::boot::park()
    }
}

#[cfg(target_arch = "aarch64")]
pub use glue::announce_and_park;
