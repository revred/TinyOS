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
use crate::gem_receive;
use crate::pcie::{self, LinkAbsent};
use crate::pl011::Mmio;
use crate::rp1_clocks::{self, ClockRefused};

/// Everything one discovery pass learned, in the order it was learned.
/// Every arm is reportable; nothing here is an error in the panicking sense.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Discovery {
    /// The link or window gates refused; the window was never read.
    LinkAbsent(LinkAbsent),
    /// The clock rung refused (`STORY-P1-09-12`); the GEM was never read —
    /// a block whose current is off answers only poison.
    ClockRefused(ClockRefused),
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
pub fn discover<R: Mmio, C: Mmio, G: Mmio>(
    root_complex: &R,
    clocks: &C,
    gem_window: G,
    release_phy: impl FnOnce() -> bool,
) -> Discovery {
    // `STORY-P1-09-09`/`-10`: the establishment pass — link gates, the
    // window with its programming fallback, then the enumeration that tells
    // the bridge where to forward and verifies who is answering. Only after
    // all of it does the first read through the window happen.
    if let Err(absent) = pcie::establish(root_complex) {
        return Discovery::LinkAbsent(absent);
    }
    // `STORY-P1-09-12`: the current before the question — the first light
    // boot read `ID-MODULE 0xDEAD` (fabric poison) here, and the live Pi OS
    // capture proved the same register answers once the two gateable
    // Ethernet clocks are enabled. Refusals carry their readback.
    if let Err(refused) = rp1_clocks::enable_ethernet_clocks(clocks) {
        return Discovery::ClockRefused(refused);
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
                LinkAbsent::BarSilent(word) => ("bar-silent", u64::from(*word), 8),
                LinkAbsent::BarNotHeld(word) => ("bar-held", u64::from(*word), 8),
                LinkAbsent::InboundNotHeld(word) => ("ibw-held", u64::from(*word), 8),
                LinkAbsent::InboundRemapNotHeld(word) => ("ibw-remap", u64::from(*word), 8),
            };
            line.push(reason);
            line.push(" detail=0x");
            line.push_hex(detail, nibbles);
        }
        Discovery::ClockRefused(refused) => {
            line.push("rp1=absent reason=");
            let (reason, readback) = match refused {
                ClockRefused::BlockSilent { sel } => ("clk-silent", *sel),
                ClockRefused::EnableNotHeld { ctrl } => ("clk-enable", *ctrl),
                ClockRefused::NeverRan { ctrl } => ("clk-stuck", *ctrl),
            };
            line.push(reason);
            line.push(" detail=0x");
            line.push_hex(u64::from(readback), 8);
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

/// Milliseconds the park loop waits per tick.
///
/// The tick is the lamp's resolution; the beat below is everything else's.
pub const PARK_TICK_MS: u32 = 100;

/// Ticks per park **beat** — the pass that stamps, dispatches, transmits and
/// drains. Everything periodic except the lamp happens on this multiple.
pub const PARK_BEAT_TICKS: u32 = 10;

/// The beat period. **1 Hz, and `LE-99` is why this is a named derivation
/// rather than two literals twenty lines apart.**
///
/// `PERF-D05-G23` measured one spoor stamp at +110 cycles p99 on a 1650-cycle
/// dispatch round — 6.7% against a 2% allowance, a fail by 3.3x — and the
/// shipping park loop stamps once per round, exactly as the fixture arm does.
/// The gate's OTHER clause, `<= 2% CPU cycles`, passes by roughly seven orders
/// of magnitude, and the only reason is this constant: one round per second is
/// 110 cycles per second on a 2.4 GHz core.
///
/// So this number is load-bearing for a filed gate verdict, and nothing tied
/// the two together until `LE-99`. Raising the cadence does not breach a test
/// somewhere else — it silently makes the per-round overhead the CPU-cycles
/// figure too, with no symptom on the wire. The test that guards it is in this
/// file and it names the gate.
pub const PARK_BEAT_MS: u32 = PARK_TICK_MS * PARK_BEAT_TICKS;

/// The park loop's verdict for one beat (`STORY-P1-09-14`): the three
/// silences the first trained wire exposed, each named. `Beaconing` is the
/// healthy hum; `parked` now carries its watch; a refused transmit is
/// `Stopped` with its error, permanently — never re-labelled "parked".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParkState {
    /// The beacon transmits every period.
    Beaconing,
    /// Parked with the link watch alive and polling.
    WatchAlive,
    /// Parked with the watch dead — the management port wedged; terminal,
    /// like the watch itself (`STORY-P1-09-06`).
    WatchDead,
    /// Parked with nothing to watch (absent chain, unknown PHY, or a link
    /// that was already up when discovery looked).
    Unwatched,
    /// A transmit refused and beaconing stopped permanently; the error is
    /// spoken every period.
    Stopped(TxError),
}

/// Builds the one-per-boot `TOS64-DISPLAY/1` line: the firmware's native-size
/// answer (the canvas gate, `LE-98`) and the mailbox framebuffer grant, as
/// this boot saw them. Recorded into the transcript so the *wire* carries the
/// display diagnosis — until 2026-08-07 it reached only the canvas, which is
/// dark exactly when the answer matters, and the UART, which this bench has
/// never captured a byte from (`LE-47`). Pure; pinned by the tests.
pub fn display_line(
    native: Option<(u32, u32)>,
    fb_granted: bool,
) -> ([u8; LINK_LINE_CAPACITY], usize) {
    let mut line = LineBuilder::new();
    line.push("TOS64-DISPLAY/1 native=");
    match native {
        Some((width, height)) => {
            line.push_dec(width);
            line.push("x");
            line.push_dec(height);
        }
        None => line.push("none"),
    }
    line.push(" fb=");
    line.push(if fb_granted { "granted" } else { "refused" });
    line.push("\n");
    (line.bytes, line.at)
}

/// Builds one `TOS64-BEAT/1` heartbeat line (`STORY-P1-09-05`, extended by
/// `STORY-P1-09-14`): emitted every park period so a passive listener can
/// find a powered board without any operator timing, and so a wrong UART
/// clock becomes bytes at a findable baud instead of silence. `fb_granted`
/// reports whether the splash's firmware framebuffer exchange succeeded —
/// the field 06A's Question 1 needs. Pure; pinned by the tests.
pub fn heartbeat_line(
    seq: u32,
    park: ParkState,
    fb_granted: bool,
) -> ([u8; LINK_LINE_CAPACITY], usize) {
    let mut line = LineBuilder::new();
    line.push("TOS64-BEAT/1 seq=");
    line.push_dec(seq);
    line.push(" state=");
    match park {
        ParkState::Beaconing => line.push("beaconing"),
        ParkState::WatchAlive => line.push("parked watch=alive"),
        ParkState::WatchDead => line.push("parked watch=dead"),
        ParkState::Unwatched => line.push("parked watch=none"),
        ParkState::Stopped(TxError::Timeout) => line.push("stopped reason=timeout"),
        ParkState::Stopped(TxError::MacError(status)) => {
            line.push("stopped reason=mac detail=0x");
            line.push_hex(u64::from(status), 8);
        }
    }
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
    park: ParkState,
    fb_granted: bool,
) -> bool {
    let (line, len) = heartbeat_line(seq, park, fb_granted);
    match core::str::from_utf8(&line[..len]) {
        Ok(text) => uart.write_str(text).is_ok(),
        Err(_) => false,
    }
}

/// Builds one `TOS64-RX/1` line (`STORY-P1-09-16`): the board's first inbound
/// channel, reported every beat on the canvas.
///
/// Both counts are always present, including while the channel is idle, and
/// that is deliberate: `accepted=0 refused=0` is a claim (nothing has arrived)
/// where a missing field is only an absence of information. The refused count
/// is what makes an accepted count mean anything — a board that counts one
/// frame has been shown to hear, and only a board that counts a refusal has
/// been shown to decline.
///
/// Pure; pinned by the tests.
pub fn receive_line(
    state: gem_receive::ReceiveState,
    accepted: u32,
    refused: u32,
) -> ([u8; LINK_LINE_CAPACITY], usize) {
    let mut line = LineBuilder::new();
    line.push("TOS64-RX/1 state=");
    match state {
        gem_receive::ReceiveState::Idle => line.push("idle"),
        gem_receive::ReceiveState::Listening => line.push("listening"),
        gem_receive::ReceiveState::Stopped(gem_receive::ReceiveError::Overrun) => {
            line.push("stopped reason=overrun");
        }
        gem_receive::ReceiveState::Stopped(gem_receive::ReceiveError::BufferUnavailable) => {
            line.push("stopped reason=nobuffer");
        }
        gem_receive::ReceiveState::Refused(gem_receive::EnableError::UnencodableBufferSize) => {
            line.push("refused reason=size");
        }
        gem_receive::ReceiveState::Refused(gem_receive::EnableError::MisalignedRing) => {
            line.push("refused reason=align");
        }
    }
    line.push(" accepted=");
    line.push_dec(accepted);
    line.push(" refused=");
    line.push_dec(refused);
    line.push("\n");
    (line.bytes, line.at)
}

/// Derives one beat's park verdict from the loop's channels
/// (`STORY-P1-09-14`) — pure, so the precedence is pinned on the host: a
/// stopped transmit outranks everything but an active beacon (a beacon that
/// stopped is `Stopped`, not `Beaconing`), then the watch speaks its state.
pub const fn park_state(
    beaconing: bool,
    stopped: Option<TxError>,
    watch_alive: bool,
    watch_dead: bool,
) -> ParkState {
    if beaconing {
        ParkState::Beaconing
    } else if let Some(error) = stopped {
        ParkState::Stopped(error)
    } else if watch_dead {
        ParkState::WatchDead
    } else if watch_alive {
        ParkState::WatchAlive
    } else {
        ParkState::Unwatched
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
pub fn watch_step<M: Mmio>(watch: &mut Option<u8>, port: &MdioPort<M>) -> Option<(Speed, bool)> {
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

/// `STORY-P1-09-08`: whether a discovery outcome is due a second look. Every
/// outcome short of a present GEM is a state to watch, not a verdict to
/// keep — the confession boot measured `DL_ACTIVE` clear at one early
/// moment, and a gate that may open late is re-read, never re-trusted. A
/// present GEM is final whatever its PHY or link state: those rungs have
/// their own channels (the confession, the link watch).
pub const fn reprobe_due(discovery: &Discovery) -> bool {
    !matches!(discovery, Discovery::Present { .. })
}

// The refusal taxonomy (codes and decisive bits) and its pronunciation
// (blink pattern, spelled sentence, latch, canvas text) live one seam out,
// so this file reads as the pipeline it is. Re-exported here because the
// park loop composes them and the Tier 1 transcription instructions cite
// this module.
pub use crate::ethernostics::{
    blink_lamp_at, lamp_action, refusal_text, sentence_for, sentence_lamp_at, sentence_period,
    LampAction, Sentence, SentenceLatch, BLINK_GAP_TICKS, BLINK_HALF_TICKS, GROUP_GAP_TICKS,
    SENTENCE_GAP_TICKS, SENTENCE_GROUPS, ZERO_BURN_TICKS, ZERO_SPAN_TICKS,
};
pub use crate::etherrors::{blink_code, blink_detail};

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    /// This file's own text, read at compile time — the only thing that can see
    /// the park loop, because the loop is `#[cfg(target_arch = "aarch64")]` glue
    /// and no host test can call it. Same mechanism `kernel::measure_phases`
    /// uses to hold its timed regions.
    const SOURCE: &str = include_str!("ethernet.rs");

    // A source-level test necessarily quotes the strings it hunts for, so
    // every needle below is ASSEMBLED from halves: written whole, each one
    // would match its own definition and the search would find this module
    // instead of the code. Found the hard way — the first cut of these tests
    // failed against themselves, twice, at two different nesting depths.
    const GLUE_BANNER: &str = concat!("// --- aarch64", " glue");
    const BEAT_GATE: &str = concat!("if tick.is_multiple_of(", "PARK_BEAT_TICKS) {");
    const TICK_WAIT: &str = concat!("if !wait_millis(", "PARK_TICK_MS) {");
    const DISPATCH_CALL: &str = concat!("crate::spoor::", "dispatch_round()");
    const LITERAL_WAIT: &str = concat!("wait_millis(", "100)");

    /// The park loop's source and **only** it: everything after the glue
    /// banner, which sits below this module.
    fn glue_source() -> &'static str {
        SOURCE.split_once(GLUE_BANNER).expect("the glue banner exists").1
    }

    /// How many lines of the once-per-beat block contain `needle`, ignoring
    /// comments and blanks.
    ///
    /// Iterator rather than a collected `Vec`: this crate is `no_std` with no
    /// allocator, and the test build has no `alloc` either — the same property
    /// `PERF-Dnn-G11` records for the shipped code.
    fn beat_lines_containing(needle: &str) -> usize {
        glue_source()
            .split_once(BEAT_GATE)
            .expect("the park loop gates its beat on PARK_BEAT_TICKS")
            .1
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("//"))
            .filter(|line| line.contains(needle))
            .count()
    }

    /// `LE-99`, first half. `PERF-D05-G23`'s filed note says the shipping park
    /// loop exceeds the gate's per-round stamp budget by 3.3x and that the ONLY
    /// thing making that harmless is the beat: one dispatch round per second,
    /// so 110 cycles of stamp per second on a 2.4 GHz core.
    ///
    /// Nothing tied that sentence to the code. This does. It is a structural
    /// assertion and deliberately encodes **no measured figure** — the cycle
    /// costs belong to a boot, not to a design constant.
    #[test]
    fn the_park_beat_is_one_hertz_and_perf_d05_g23_depends_on_it() {
        assert_eq!(
            PARK_TICK_MS * PARK_BEAT_TICKS,
            PARK_BEAT_MS,
            "the beat period must be the product of its two factors"
        );
        assert_eq!(
            PARK_BEAT_MS, 1_000,
            "THE BEAT IS NO LONGER 1 Hz. PERF-D05-G23 fails its p99 clause by 3.3x per dispatch \
             round and passes its CPU-cycles clause only because one round runs per second. \
             Raising the cadence makes the per-round overhead the CPU figure too. Re-derive the \
             gate's note before changing this (LE-99)"
        );
    }

    /// The constants must be what the loop actually uses, or they are two
    /// numbers in a doc comment. `LE-80`'s lesson: a mirror nobody asserts is
    /// two values that agree today.
    #[test]
    fn the_park_loop_paces_itself_from_the_named_constants() {
        assert!(glue_source().contains(TICK_WAIT), "the tick wait must use PARK_TICK_MS");
        assert!(glue_source().contains(BEAT_GATE), "the beat gate must use PARK_BEAT_TICKS");
        assert!(
            !glue_source().contains(LITERAL_WAIT),
            "a literal cadence beside a named one is the drift LE-99 asks to prevent"
        );
    }

    /// `LE-99`, second half as far as this crate can see it: **one dispatch
    /// round per beat.** The kernel side pins one stamp per round; together
    /// they are one stamp per second, which is the number `PERF-D05-G23`'s
    /// CPU-cycles clause rests on.
    #[test]
    fn the_beat_runs_exactly_one_dispatch_round() {
        let rounds = beat_lines_containing(DISPATCH_CALL);
        assert_eq!(
            rounds, 1,
            "a second dispatch round per beat doubles the stamp rate PERF-D05-G23 was read \
             against (LE-99)"
        );
    }

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

    /// A clocks block that must never be touched — the rung sits strictly
    /// behind the establishment gates (`TEST-P1-09-12-A` clause 4).
    struct UntouchableClocks;

    impl Mmio for UntouchableClocks {
        fn read_u32(&self, offset: usize) -> u32 {
            panic!("read clocks {offset:#x} behind a failed gate");
        }

        fn write_u32(&self, offset: usize, _value: u32) {
            panic!("wrote clocks {offset:#x} behind a failed gate");
        }
    }

    /// A clocks block whose two Ethernet clocks already run — the pipeline's
    /// happy path, and the idempotent re-probe: reads answer, writes panic.
    struct RunningClocks;

    impl Mmio for RunningClocks {
        fn read_u32(&self, offset: usize) -> u32 {
            match offset {
                crate::rp1_clocks::register::SYS_SEL => 0x4,
                crate::rp1_clocks::register::ETH_CTRL
                | crate::rp1_clocks::register::ETH_TSU_CTRL => {
                    crate::rp1_clocks::CTRL_ENABLE | crate::rp1_clocks::CTRL_RUNNING
                }
                other => panic!("unexpected clocks read {other:#x}"),
            }
        }

        fn write_u32(&self, offset: usize, _value: u32) {
            panic!("wrote clocks {offset:#x} on the already-running pass");
        }
    }

    // TEST-P1-09-01-A clause 2, end to end: a failed gate means the window is
    // never read.

    #[test]
    fn a_dead_link_never_touches_the_gem_window_or_the_gpio() {
        assert_eq!(
            discover(&DeadRc, &UntouchableClocks, UntouchableGem, || panic!(
                "release ran behind a failed gate"
            )),
            Discovery::LinkAbsent(LinkAbsent::PortNotRc(0))
        );
    }

    // TEST-P1-09-12-A clause 4: the clock rung sits between enumeration and
    // identity — a refused clock leaves the GEM untouched, and the refusal
    // speaks its code, its detail, and its report name.

    #[test]
    fn a_poisoned_clocks_block_stops_the_pipeline_before_the_gem() {
        struct PoisonedClocks;
        impl Mmio for PoisonedClocks {
            fn read_u32(&self, offset: usize) -> u32 {
                assert_eq!(
                    offset,
                    crate::rp1_clocks::register::SYS_SEL,
                    "only the pre-flight may read"
                );
                0xDEAD_0000
            }
            fn write_u32(&self, _offset: usize, _value: u32) {
                panic!("wrote a block that answered poison");
            }
        }
        let discovery = discover(&HealthyRc, &PoisonedClocks, UntouchableGem, || {
            panic!("release ran after a refused clock rung")
        });
        assert_eq!(
            discovery,
            Discovery::ClockRefused(ClockRefused::BlockSilent { sel: 0xDEAD_0000 })
        );
        assert_eq!(blink_code(&discovery), Some(16));
        assert_eq!(blink_detail(&discovery), 0xDEAD, "poison spells 57005");
        assert!(reprobe_due(&discovery), "a refused clock rung earns the second look");
        assert_eq!(
            line_text(&discovery, BeaconField::Skipped),
            "TOS64-LINK/1 rp1=absent reason=clk-silent detail=0xdead0000 beacon=skipped\n"
        );
    }

    // TEST-P1-09-13-A clause 4: the BAR refusals speak their names.

    #[test]
    fn the_bar_refusal_arms_speak_their_codes_details_and_names() {
        let silent = Discovery::LinkAbsent(LinkAbsent::BarSilent(0xFFFF_FFF0));
        assert_eq!(blink_code(&silent), Some(19));
        assert_eq!(blink_detail(&silent), 0xFFFF);
        assert_eq!(
            line_text(&silent, BeaconField::Skipped),
            "TOS64-LINK/1 rp1=absent reason=bar-silent detail=0xfffffff0 beacon=skipped\n"
        );
        let held = Discovery::LinkAbsent(LinkAbsent::BarNotHeld(0xFFC0_0000));
        assert_eq!(blink_code(&held), Some(20));
        assert_eq!(blink_detail(&held), 0xFFC0);
        assert_eq!(
            line_text(&held, BeaconField::Skipped),
            "TOS64-LINK/1 rp1=absent reason=bar-held detail=0xffc00000 beacon=skipped\n"
        );
        assert!(reprobe_due(&silent), "a refused BAR earns the second look");
    }

    // TEST-P1-09-15-A clause 4: the inbound refusals speak their names.

    #[test]
    fn the_inbound_refusal_arms_speak_their_codes_details_and_names() {
        let held = Discovery::LinkAbsent(LinkAbsent::InboundNotHeld(0xABCD_F01C));
        assert_eq!(blink_code(&held), Some(21));
        assert_eq!(blink_detail(&held), 0xF01C);
        assert_eq!(
            line_text(&held, BeaconField::Skipped),
            "TOS64-LINK/1 rp1=absent reason=ibw-held detail=0xabcdf01c beacon=skipped\n"
        );
        let remap = Discovery::LinkAbsent(LinkAbsent::InboundRemapNotHeld(0xDEAD_DEAD));
        assert_eq!(blink_code(&remap), Some(22));
        assert_eq!(blink_detail(&remap), 0xDEAD);
        assert_eq!(
            line_text(&remap, BeaconField::Skipped),
            "TOS64-LINK/1 rp1=absent reason=ibw-remap detail=0xdeaddead beacon=skipped\n"
        );
        assert!(reprobe_due(&held), "a refused inbound window earns the second look");
    }

    #[test]
    fn the_clock_refusal_arms_speak_their_codes_details_and_names() {
        let dropped = Discovery::ClockRefused(ClockRefused::EnableNotHeld { ctrl: 0x0000_0400 });
        assert_eq!(blink_code(&dropped), Some(17));
        assert_eq!(blink_detail(&dropped), 0x0400, "the low half shows the missing enable");
        assert_eq!(
            line_text(&dropped, BeaconField::Skipped),
            "TOS64-LINK/1 rp1=absent reason=clk-enable detail=0x00000400 beacon=skipped\n"
        );
        let stuck = Discovery::ClockRefused(ClockRefused::NeverRan { ctrl: 0x0000_0800 });
        assert_eq!(blink_code(&stuck), Some(18));
        assert_eq!(blink_detail(&stuck), 0, "the status half shows running never answered");
        assert_eq!(
            line_text(&stuck, BeaconField::Skipped),
            "TOS64-LINK/1 rp1=absent reason=clk-stuck detail=0x00000800 beacon=skipped\n"
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
                // The BARs already hold their pinned addresses — the
                // settled shape, so the BAR rung performs zero writes
                // (TEST-P1-09-13-A clause 3) and the write panic below
                // keeps its teeth.
                offset if offset == pcie::config::EP_BARS[0] => 0x0041_0000,
                offset if offset == pcie::config::EP_BARS[2] => 0x0040_0000,
                other => {
                    // The inbound windows already hold their captured
                    // dwords — the settled shape, so the inbound pass
                    // performs zero writes (TEST-P1-09-15-A clause 3) and
                    // the write panic below keeps its teeth.
                    let mut window = 0;
                    while window < pcie::inbound::WINDOWS.len() {
                        for (offset, value) in pcie::inbound::window_dwords(window) {
                            if offset == other {
                                return value;
                            }
                        }
                        window += 1;
                    }
                    0
                }
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
        let discovery = discover(&HealthyRc, &RunningClocks, PipelineGem::new(), || {
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
            discover(&HealthyRc, &RunningClocks, FloatingGem, || panic!(
                "release ran after a refused identity"
            )),
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
        let discovery = discover(&HealthyRc, &RunningClocks, IdentityOnlyGem, || false);
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

    fn beat_text(seq: u32, park: ParkState, fb: bool) -> String {
        let (bytes, len) = heartbeat_line(seq, park, fb);
        String::from_utf8(bytes[..len].to_vec()).unwrap()
    }

    // TEST-P1-09-14-A clause 1: every park state prints a distinct line.

    #[test]
    fn every_park_state_prints_a_distinct_pinned_line() {
        assert_eq!(
            beat_text(9, ParkState::WatchAlive, false),
            "TOS64-BEAT/1 seq=9 state=parked watch=alive fb=refused\n"
        );
        assert_eq!(
            beat_text(9, ParkState::WatchDead, false),
            "TOS64-BEAT/1 seq=9 state=parked watch=dead fb=refused\n"
        );
        assert_eq!(
            beat_text(9, ParkState::Unwatched, false),
            "TOS64-BEAT/1 seq=9 state=parked watch=none fb=refused\n"
        );
        assert_eq!(
            beat_text(9, ParkState::Stopped(TxError::Timeout), false),
            "TOS64-BEAT/1 seq=9 state=stopped reason=timeout fb=refused\n"
        );
        assert_eq!(
            beat_text(9, ParkState::Stopped(TxError::MacError(0x40)), false),
            "TOS64-BEAT/1 seq=9 state=stopped reason=mac detail=0x00000040 fb=refused\n"
        );
        // No two verdicts share a line.
        let all = [
            ParkState::Beaconing,
            ParkState::WatchAlive,
            ParkState::WatchDead,
            ParkState::Unwatched,
            ParkState::Stopped(TxError::Timeout),
            ParkState::Stopped(TxError::MacError(0x40)),
        ];
        let mut lines: Vec<String> = all.iter().map(|&p| beat_text(1, p, true)).collect();
        lines.sort();
        lines.dedup();
        assert_eq!(lines.len(), all.len(), "every park state must speak differently");
    }

    // TEST-P1-09-14-A clauses 2 and 3: the derivation's precedence is pinned.

    #[test]
    fn the_park_verdict_precedence_is_pinned() {
        assert_eq!(park_state(true, None, false, false), ParkState::Beaconing);
        // A stopped transmit outranks the watch's remains.
        assert_eq!(
            park_state(false, Some(TxError::Timeout), false, false),
            ParkState::Stopped(TxError::Timeout)
        );
        assert_eq!(
            park_state(false, Some(TxError::MacError(7)), true, true),
            ParkState::Stopped(TxError::MacError(7)),
            "a spoken stop is never re-labelled by a later watch state"
        );
        // The wedge outranks alive (a dead watch cannot also be polling).
        assert_eq!(park_state(false, None, false, true), ParkState::WatchDead);
        assert_eq!(park_state(false, None, true, false), ParkState::WatchAlive);
        assert_eq!(park_state(false, None, false, false), ParkState::Unwatched);
    }

    // `LE-98` / the owner's standing direction (diagnosis moves onto the
    // cable): the display outcome must be statable on the wire, because the
    // two channels that carried it — the canvas and the UART — are dark or
    // unproven exactly when the answer matters.
    #[test]
    fn the_display_line_reports_what_the_firmware_said_at_boot() {
        let (bytes, len) = display_line(Some((1920, 1080)), false);
        assert_eq!(
            core::str::from_utf8(&bytes[..len]).unwrap(),
            "TOS64-DISPLAY/1 native=1920x1080 fb=refused\n"
        );
        // `native=none` is the arm the blank-monitor diagnosis turns on: the
        // firmware could not name a display, so the canvas gate never opened.
        let (bytes, len) = display_line(None, true);
        assert_eq!(
            core::str::from_utf8(&bytes[..len]).unwrap(),
            "TOS64-DISPLAY/1 native=none fb=granted\n"
        );
    }

    #[test]
    fn the_heartbeat_line_is_exact_bytes_with_every_field_driven() {
        let (bytes, len) = heartbeat_line(1, ParkState::Beaconing, true);
        assert_eq!(
            core::str::from_utf8(&bytes[..len]).unwrap(),
            "TOS64-BEAT/1 seq=1 state=beaconing fb=granted\n"
        );
        let (bytes, len) = heartbeat_line(42, ParkState::Unwatched, false);
        assert_eq!(
            core::str::from_utf8(&bytes[..len]).unwrap(),
            "TOS64-BEAT/1 seq=42 state=parked watch=none fb=refused\n"
        );
        let (a, len_a) = heartbeat_line(7, ParkState::Beaconing, false);
        let (b, len_b) = heartbeat_line(8, ParkState::Beaconing, false);
        assert_eq!(len_a, len_b);
        let differing: Vec<usize> = (0..len_a).filter(|&i| a[i] != b[i]).collect();
        assert_eq!(differing.len(), 1, "the sequence digit is the only variance");
    }

    // TEST-P1-09-16-A clause 3, the reported half: the inbound row.

    #[test]
    fn the_receive_line_is_exact_bytes_and_always_carries_both_counts() {
        use crate::gem_receive::{EnableError, ReceiveError, ReceiveState};
        let (bytes, len) = receive_line(ReceiveState::Idle, 0, 0);
        assert_eq!(
            core::str::from_utf8(&bytes[..len]).unwrap(),
            "TOS64-RX/1 state=idle accepted=0 refused=0\n",
            "zero is a claim; a missing field would only be an absence of information"
        );
        let (bytes, len) = receive_line(ReceiveState::Listening, 1, 0);
        assert_eq!(
            core::str::from_utf8(&bytes[..len]).unwrap(),
            "TOS64-RX/1 state=listening accepted=1 refused=0\n"
        );
        let (bytes, len) = receive_line(ReceiveState::Listening, 3, 17);
        assert_eq!(
            core::str::from_utf8(&bytes[..len]).unwrap(),
            "TOS64-RX/1 state=listening accepted=3 refused=17\n"
        );
        // Every stop reason is spoken, never relabelled as quiet.
        for (state, expected) in [
            (ReceiveState::Stopped(ReceiveError::Overrun), "stopped reason=overrun"),
            (ReceiveState::Stopped(ReceiveError::BufferUnavailable), "stopped reason=nobuffer"),
            (ReceiveState::Refused(EnableError::UnencodableBufferSize), "refused reason=size"),
            (ReceiveState::Refused(EnableError::MisalignedRing), "refused reason=align"),
        ] {
            let (bytes, len) = receive_line(state, 2, 5);
            let text = core::str::from_utf8(&bytes[..len]).unwrap();
            assert_eq!(text, format!("TOS64-RX/1 state={expected} accepted=2 refused=5\n"));
        }
    }

    #[test]
    fn the_receive_line_never_overruns_its_buffer_at_the_widest_counts() {
        use crate::gem_receive::{ReceiveError, ReceiveState};
        let (_, len) = receive_line(
            ReceiveState::Stopped(ReceiveError::BufferUnavailable),
            u32::MAX,
            u32::MAX,
        );
        assert!(len < LINK_LINE_CAPACITY, "the longest line still fits, at {len} bytes");
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
        assert!(emit_heartbeat(&Pl011::new(ReadyWire), 1, ParkState::Unwatched, true));
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
        assert!(!emit_heartbeat(&Pl011::new(WedgedWire), 2, ParkState::Beaconing, false));
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
            let refused = discover(&DlDownRc, &UntouchableClocks, UntouchableGem, || {
                panic!("release behind a failed gate")
            });
            assert_eq!(refused, Discovery::LinkAbsent(LinkAbsent::LinkDown(0x90)));
            assert!(reprobe_due(&refused));
            assert_eq!(blink_code(&refused), Some(3), "the lamp keeps counting 3");
        }
        // The pass where the gate finally reads settled: the whole pipeline,
        // the release exactly once, and every channel adopts the outcome.
        let released = Cell::new(0u32);
        let settled = discover(&HealthyRc, &RunningClocks, PipelineGem::new(), || {
            released.set(released.get() + 1);
            true
        });
        assert_eq!(released.get(), 1, "one release across all passes");
        assert!(!reprobe_due(&settled), "a present GEM is final");
        assert_eq!(blink_code(&settled), None, "the lamp returns to the plain pulse");
        assert_eq!(watch_from(&settled), None, "the scripted link is already up");
        assert_eq!(beacon_eligible(&settled), Some((Speed::Mbps1000, true)));
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
            let park = park_state(beaconing, None, watch.is_some(), false);
            let (line, len) = heartbeat_line(seq, park, false);
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
    /// never straddle what the MAC fetches. **One static grant, per `LE-67`** —
    /// still exactly one, sized now for the largest frame any channel emits.
    ///
    /// That is the spoor frame (`STORY-P1-10-02`, 1510 bytes); the beacon and
    /// the transcript text frame use its prefix. Growing the single buffer
    /// does not widen the containment story `LE-67` records — the device is
    /// granted one pinned region and receive stays disabled — but it does mean
    /// the region a confused device could reach is larger, which is why the
    /// size is pinned to the frame format's own bound rather than rounded up.
    #[repr(C, align(64))]
    struct BeaconMemory {
        ring: [[u32; 4]; 2],
        frame: [u8; gem::SPOOR_FRAME_CAPACITY],
    }

    static mut BEACON_MEMORY: BeaconMemory =
        BeaconMemory { ring: [[0; 4]; 2], frame: [0; gem::SPOOR_FRAME_CAPACITY] };

    /// The receive ring and its single buffer (`STORY-P1-09-16`), 64-byte
    /// aligned for the same reason and **separate from [`BeaconMemory`] on
    /// purpose**.
    ///
    /// This is the second pinned grant, and the widening `LE-67` now records.
    /// It is not shared with the transmit staging region because the two
    /// directions have opposite writers: the CPU writes the transmit region
    /// and the device reads it; the device writes this one. Aliasing them
    /// would let a confused inbound write corrupt the frame the board is
    /// about to transmit — turning an inbound fault into an *outbound lie*,
    /// and every piece of evidence this project has is an outbound frame.
    #[repr(C, align(64))]
    struct ReceiveMemory {
        ring: [u32; 4],
        buffer: [u8; gem_receive::RECEIVE_BUFFER_BYTES],
    }

    static mut RECEIVE_MEMORY: ReceiveMemory =
        ReceiveMemory { ring: [0; 4], buffer: [0; gem_receive::RECEIVE_BUFFER_BYTES] };

    /// The DMA address of the receive buffer, as the device sees it.
    fn receive_buffer_dma() -> u64 {
        // SAFETY: address-of only; no reference to the mutable static is
        // taken and nothing is dereferenced.
        unsafe {
            let memory = core::ptr::addr_of!(RECEIVE_MEMORY);
            board::RP1_DMA_RAM_BASE + core::ptr::addr_of!((*memory).buffer) as u64
        }
    }

    /// Writes the one-descriptor ring, cleans it to the point of coherency,
    /// and arms the MAC. [`None`] on success; a refusal is returned so the
    /// canvas can say which one, and nothing is enabled in that case.
    fn arm_receive<M: Mmio>(device: &M) -> gem_receive::ReceiveState {
        let buffer_dma = receive_buffer_dma();
        let Some(ring) = gem_receive::receive_ring(buffer_dma) else {
            // The static is `align(64)`, so this is unreachable by
            // construction — and it is checked rather than asserted, because
            // an alignment that a future edit breaks must degrade to a
            // spoken refusal and not to a device writing at a shifted
            // address with the ownership bit flipped.
            return gem_receive::ReceiveState::Refused(gem_receive::EnableError::MisalignedRing);
        };
        // SAFETY: single core, and this function plus `poll_receive` are the
        // only writers of `RECEIVE_MEMORY`. The raw pointer avoids taking a
        // reference to a mutable static.
        let ring_dma = unsafe {
            let memory = core::ptr::addr_of_mut!(RECEIVE_MEMORY);
            (*memory).ring = ring;
            crate::mmu::clean_dcache_range(memory as usize, core::mem::size_of::<ReceiveMemory>());
            board::RP1_DMA_RAM_BASE + core::ptr::addr_of!((*memory).ring) as u64
        };
        match gem_receive::enable_receive(
            device,
            gem::BEACON_SOURCE_MAC,
            ring_dma,
            gem_receive::RECEIVE_BUFFER_BYTES,
        ) {
            Ok(()) => gem_receive::ReceiveState::Listening,
            Err(refused) => gem_receive::ReceiveState::Refused(refused),
        }
    }

    /// One bounded inbound poll: at most **one** descriptor examined, at most
    /// one frame admitted, per beat. Returns the new state and how much to add
    /// to each counter.
    ///
    /// The device wrote this memory behind the CPU's caches, so the region is
    /// cleaned-and-invalidated before the descriptor is read — the mirror of
    /// `stage_bytes`' clean, in the direction that actually needs it.
    fn poll_receive<M: Mmio>(
        device: &M,
        state: gem_receive::ReceiveState,
    ) -> (gem_receive::ReceiveState, u32, u32) {
        if state != gem_receive::ReceiveState::Listening {
            return (state, 0, 0);
        }
        // Status first: an overrun makes whatever is in the buffer suspect,
        // so it is read before the descriptor rather than after.
        match gem_receive::read_status(device) {
            Ok(_) => {}
            Err(error) => {
                gem_receive::disable_receive(device);
                return (gem_receive::ReceiveState::Stopped(error), 0, 0);
            }
        }
        // SAFETY: as `arm_receive`. The read is of memory this core owns and
        // the device writes; the maintenance below makes the device's stores
        // visible before either word is read.
        let (word0, word1, admission) = unsafe {
            let memory = core::ptr::addr_of_mut!(RECEIVE_MEMORY);
            crate::mmu::clean_invalidate_dcache_range(
                memory as usize,
                core::mem::size_of::<ReceiveMemory>(),
            );
            let word0 = core::ptr::read_volatile(core::ptr::addr_of!((*memory).ring[0]));
            let word1 = core::ptr::read_volatile(core::ptr::addr_of!((*memory).ring[1]));
            let admission = match gem_receive::classify_descriptor(word0, word1) {
                gem_receive::DescriptorState::MacOwns => None,
                gem_receive::DescriptorState::Refused(_) => {
                    // A descriptor that cannot be a frame is counted as a
                    // refusal and the buffer is re-armed; it is not an error
                    // condition, because the device is exactly the thing this
                    // Feature contracts as compromisable.
                    Some(gem_receive::Admission::Refused(gem_receive::FrameRefusal::TooShort))
                }
                gem_receive::DescriptorState::Frame { length } => {
                    // `length` is bounded by `classify_descriptor` against
                    // the region, which is what makes this slice safe — the
                    // device's own number is never trusted as a length.
                    // Built from a raw pointer rather than by indexing the
                    // place expression, so no reference to the mutable
                    // static is ever created (`dangerous_implicit_autorefs`).
                    let buffer = core::ptr::addr_of!((*memory).buffer).cast::<u8>();
                    let frame = core::slice::from_raw_parts(buffer, length);
                    Some(gem_receive::admit(frame, gem::BEACON_SOURCE_MAC))
                }
            };
            (word0, word1, admission)
        };
        let _ = (word0, word1);
        let Some(admission) = admission else {
            return (state, 0, 0);
        };
        // Re-arm: one descriptor, handed back explicitly. Until this happens
        // the MAC has nowhere to put a second frame, which is the property
        // that makes one-poll-per-beat a bound rather than a hope.
        // SAFETY: as above.
        unsafe {
            let memory = core::ptr::addr_of_mut!(RECEIVE_MEMORY);
            if let Some(ring) = gem_receive::rearm(receive_buffer_dma()) {
                (*memory).ring = ring;
            }
            crate::mmu::clean_dcache_range(memory as usize, core::mem::size_of::<ReceiveMemory>());
        }
        match admission {
            gem_receive::Admission::Accepted => (state, 1, 0),
            gem_receive::Admission::Refused(_) => (state, 0, 1),
        }
    }

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
    /// returns the ring's DMA address. Since `STORY-P1-07-03` this memory is
    /// Normal Write-Back cacheable, and the GEM masters its reads from DRAM
    /// behind the CPU's caches — so the stores are made visible with an
    /// explicit clean to the point of coherency (which ends in `dsb sy`,
    /// ordering the maintenance before the MAC is started). The GEM's own
    /// write-back of the used bit into the ring is never read by this code,
    /// so no invalidate is owed on the return path.
    fn stage_frame(seq: u32) -> u64 {
        let (frame, len) = gem::beacon_frame(seq);
        stage_bytes(&frame, len)
    }

    /// Stages one transcript line as a text frame (`STORY-P1-07-06`).
    #[cfg(feature = "fixture-measure")]
    fn stage_text_line(line: &[u8]) -> u64 {
        let (frame, len) = gem::text_frame(line);
        stage_bytes(&frame, len)
    }

    /// Stages one spoor frame (`STORY-P1-10-02`), or [`None`] when the payload
    /// does not fit — refused rather than truncated, because a shortened run
    /// of packed records decodes to a plausible lie.
    fn stage_spoor_payload(payload: &[u8]) -> Option<u64> {
        let (frame, len) = gem::payload_frame(payload)?;
        Some(stage_bytes(&frame, len))
    }

    /// Writes the frame bytes and the ring into the pinned memory, cleans
    /// the lines to the point of coherency, and returns the ring's DMA
    /// address — the one staging path both the beacon and the transcript
    /// frames go through.
    fn stage_bytes(frame: &[u8], len: usize) -> u64 {
        // SAFETY: single core, and this function is the only writer of
        // `BEACON_MEMORY`; the raw pointer avoids taking a reference to a
        // mutable static, and `frame.len()` is at most `TEXT_FRAME_CAPACITY`
        // by both callers' construction.
        unsafe {
            let memory = core::ptr::addr_of_mut!(BEACON_MEMORY);
            (&mut (*memory).frame)[..frame.len()].copy_from_slice(frame);
            let frame_dma = board::RP1_DMA_RAM_BASE + core::ptr::addr_of!((*memory).frame) as u64;
            (*memory).ring = gem::tx_ring(frame_dma, len);
            crate::mmu::clean_dcache_range(memory as usize, core::mem::size_of::<BeaconMemory>());
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
        splash: crate::hdmi::DisplayOutcome,
        boot_lines: &crate::canvas::BootLines<'_>,
        tick_refused: Option<&[u8]>,
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
        // SAFETY: as above — RP1's clock generator block (`STORY-P1-09-12`),
        // inside the validated span, read and written only after the
        // establishment gates pass.
        let clocks =
            unsafe { VolatileMmio::new(board::RP1_WINDOW_BASE + board::RP1_CLOCKS_OFFSET) };
        // SAFETY: the BCM2712 `gpio-brcmstb` block the board itself reported
        // on silicon (`pios-ground-truth-2026-08-03.txt`); single core, sole
        // writer. On the SoC side of every gate above — the lamp needs none
        // of them (`STORY-P1-07-08`).
        let stat_gpio = unsafe { VolatileMmio::new(board::STAT_GPIO_BASE) };

        let mut discovery = discover(&root_complex, &clocks, gem_window, || {
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
        //
        // `LE-98`: the canvas now exists only if the firmware reported a
        // display. Without one it paints nothing — and SAYS so on the serial
        // line, because a surface that refuses silently is the half-success
        // `LE-87` is about and is exactly how the 2026-08-06 run looked
        // healthy while writing 4 MB to an address nobody had verified.
        let mut console = crate::canvas::Canvas::permitted_by(&splash);
        if console.is_dark() {
            let _ = uart.write_str(
                "TOS64-CANVAS/1 painting=no reason=firmware-reported-no-display (LE-98)\n",
            );
        }
        crate::canvas::draw_frame(&mut console);
        crate::canvas::draw_line(
            &mut console,
            crate::canvas::REPORT_Y,
            &line[..len.saturating_sub(1)],
            crate::canvas::TEXT,
        );
        // `STORY-P1-07-03`/`-04`: the boot evidence lines, painted once —
        // they never change after boot, and the canvas is the channel the
        // owner transcribes into the ground-truth file. The tick row is
        // painted below: live if the tick runs, pinned to its refusal if not.
        crate::canvas::draw_line(
            &mut console,
            crate::canvas::MMU_Y,
            boot_lines.mmu,
            crate::canvas::TEXT,
        );
        crate::canvas::draw_line(
            &mut console,
            crate::canvas::CONF_Y,
            boot_lines.conf,
            crate::canvas::TEXT,
        );
        crate::canvas::draw_line(
            &mut console,
            crate::canvas::PMU_Y,
            boot_lines.pmu,
            crate::canvas::TEXT,
        );
        if let Some(refused) = tick_refused {
            crate::canvas::draw_line(
                &mut console,
                crate::canvas::TICK_Y,
                refused,
                crate::canvas::ALERT,
            );
        }
        // `STORY-P1-07-06`: the whole measurement transcript, painted once
        // at 1× scale — complete before this loop starts, and the reason the
        // canvas is a court record rather than a status display.
        #[cfg(feature = "fixture-measure")]
        {
            let mut line = [0u8; crate::gem::TEXT_FRAME_CAPACITY];
            for nth in 0..crate::transcript::line_count() {
                if let Some(len) = crate::transcript::copy_line(nth, &mut line) {
                    crate::canvas::draw_text(
                        &mut console,
                        crate::canvas::MARGIN_X,
                        crate::canvas::TRANSCRIPT_Y + nth as u32 * crate::canvas::TRANSCRIPT_STEP_Y,
                        crate::canvas::TRANSCRIPT_SCALE,
                        &line[..len],
                        crate::canvas::TEXT,
                        crate::hdmi::BACKGROUND,
                    );
                }
            }
        }

        let mut beaconing = matches!(beacon, BeaconField::Running);
        // `STORY-P1-09-14`: the park verdict's memory — a stopped transmit
        // and a dead watch each stay spoken until a settled re-probe.
        let mut stopped = match beacon {
            BeaconField::Stopped(error) => Some(error),
            _ => None,
        };
        let mut watch_dead = false;
        let mut speed_config = beacon_eligible(&discovery);
        // `STORY-P1-09-06`: a known PHY whose link was not up when discovery
        // looked stays watched from the park loop — the wire decides when.
        let mut watch = watch_from(&discovery);
        let mut heartbeating = true;
        let fb_granted = splash.framebuffer.is_some();
        // The display diagnosis onto the wire (`LE-98`): one transcript line,
        // recorded before the park loop starts cycling readers, so a passive
        // capture can say whether the firmware ever saw a display this boot.
        #[cfg(feature = "fixture-measure")]
        {
            let (line, len) = display_line(splash.native, fb_granted);
            crate::transcript::record(&line[..len]);
        }
        let mut animation = splash.framebuffer.map(|info| {
            (crate::hdmi::Framebuffer { info }, crate::hdmi::Bounce::new(info.width, info.height))
        });
        // `STORY-P1-09-07`/`-11`: the latch owns the lamp — outcome changes
        // are offered, adopted only at sentence boundaries.
        let mut lamp = SentenceLatch::new(
            blink_code(&discovery).map(|code| sentence_for(code, blink_detail(&discovery))),
        );
        // `STORY-P1-09-16`: the inbound channel. Idle until the board is
        // beaconing; both counters start at zero and are painted from the
        // first beat, because `accepted=0 refused=0` is a claim and a blank
        // row is not.
        let mut receive = gem_receive::ReceiveState::Idle;
        let mut rx_accepted: u32 = 0;
        let mut rx_refused: u32 = 0;
        let (rx_text, rx_len) = receive_line(receive, rx_accepted, rx_refused);
        crate::canvas::draw_line(
            &mut console,
            crate::canvas::RX_Y,
            &rx_text[..rx_len.saturating_sub(1)],
            crate::canvas::TEXT,
        );
        let mut beat_seq: u32 = 1;
        let mut frame_seq: u32 = 1;
        let mut tick: u32 = 0;
        loop {
            if !wait_millis(PARK_TICK_MS) {
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
            if tick.is_multiple_of(PARK_BEAT_TICKS) {
                // TEST-P1-09-08-A clause 3: while discovery reports absence,
                // one re-probe per second. The gates keep refused passes away
                // from the window and the GPIO, so the release still runs at
                // most once — on the single pass where identity first
                // validates. A settled chain adopts every channel at once.
                if reprobe_due(&discovery) {
                    discovery = discover(&root_complex, &clocks, gem_window, || {
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
                        // A settled chain resets the park verdict with
                        // every other channel (TEST-P1-09-14-A clause 3).
                        stopped = None;
                        watch_dead = false;
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
                // TEST-P1-09-06-A clause 1 / TEST-P1-09-14-A clause 2:
                // while the link is down, one bounded poll per second — and
                // the one call site that can tell a resolve from a wedge
                // records which one emptied the watch.
                if !beaconing && speed_config.is_none() && watch.is_some() {
                    let port = MdioPort::enable(gem_window);
                    match watch_step(&mut watch, &port) {
                        Some(config) => {
                            speed_config = Some(config);
                            beaconing = true;
                        }
                        None => watch_dead = watch.is_none(),
                    }
                    let _device = port.finish();
                }
                if beaconing {
                    match speed_config {
                        Some((speed, full_duplex)) => {
                            let ring_dma = stage_frame(frame_seq);
                            if let Err(refused) =
                                gem::transmit_once(&gem_window, ring_dma, speed, full_duplex)
                            {
                                // Fail-safe over keep-trying: one refusal
                                // ends the beacon permanently — and is
                                // spoken, never re-labelled "parked"
                                // (TEST-P1-09-14-A clause 3).
                                stopped = Some(refused);
                                beaconing = false;
                                crate::spoor::stamp(
                                    crate::spoor::Rung::BeaconTransmitted,
                                    crate::spoor::Verdict::Failed,
                                    frame_seq,
                                );
                            } else {
                                frame_seq = frame_seq.wrapping_add(1);
                                crate::spoor::stamp(
                                    crate::spoor::Rung::BeaconTransmitted,
                                    crate::spoor::Verdict::Ok,
                                    frame_seq,
                                );
                            }
                        }
                        None => beaconing = false,
                    }
                }

                // `STORY-P1-10-02`: the spoor stream leaves the board here.
                // Last of the channels deliberately — the rungs stamped by
                // this very pass are already in the journal, so a frame is
                // never one pass stale. Same fail-safe as every other channel:
                // one refused transmit ends it and is spoken, and a payload
                // that will not fit is refused rather than truncated.
                crate::spoor::stamp(
                    crate::spoor::Rung::ParkIteration,
                    crate::spoor::Verdict::Ok,
                    beat_seq,
                );
                // `LE-75`: the machine says how hot it is, once per beat. One
                // `ldr` from the AVS monitor, stamped **raw** — the board does
                // not convert, because the raw-to-millicelsius calibration is
                // unverified on this hardware and a converted value would
                // arrive as a confident number nobody could tell was wrong.
                //
                // Sensing only. Nothing here drives the fan or caps a clock,
                // and that separation is deliberate: an actuator fed by a
                // sensor nobody has validated turns a measurement error into a
                // physical one.
                crate::spoor::stamp(
                    crate::spoor::Rung::ThermalSample,
                    crate::spoor::Verdict::Ok,
                    crate::thermal::read_raw(),
                );
                // The kernel drives the machine here, once per beat: one
                // cooperative dispatch round, interrupts live, outside any
                // measured region. `run_once` switches into the task, the task
                // yields straight back, and the round stamps its own spoor from
                // the kernel side where the taxonomy lives.
                //
                // Paced BY the tick rather than called FROM it - a context
                // switch inside the handler would swap the stack underneath the
                // frame that will `eret`, and the handler is not reentrant.
                let _dispatched = crate::spoor::dispatch_round();
                if beaconing {
                    if let Some((speed, full_duplex)) = speed_config {
                        let mut payload = [0u8; gem::SPOOR_FRAME_CAPACITY - 14];
                        let len = crate::spoor::drain(&mut payload);
                        // Zero is "nothing to send", not "send nothing": an
                        // empty frame every pass would fill the wire with
                        // silence that looks like data.
                        if len > 0 {
                            if let Some(ring_dma) = stage_spoor_payload(&payload[..len]) {
                                if let Err(refused) =
                                    gem::transmit_once(&gem_window, ring_dma, speed, full_duplex)
                                {
                                    stopped = Some(refused);
                                    beaconing = false;
                                }
                            }
                        }
                        // `STORY-P1-10-04`: the retained boot certificate,
                        // re-announced on the kernel's own period. The boot
                        // rungs stamp once and the drain above clears them, so
                        // without this a listener that missed frame 0 learns
                        // from the sequence gap how many records it lost and
                        // never what they were — and boot state is the least
                        // repeatable part of the whole stream.
                        //
                        // Asked every pass, answered on the period: the cadence
                        // belongs to `kernel::spoor_stream` where a host test
                        // holds it, so this loop carries no policy. It reuses
                        // the same buffer and the same single pinned staging
                        // region — no second grant, and `LE-67`'s containment
                        // story is exactly as wide as it was.
                        let announced = crate::spoor::announce(&mut payload);
                        if announced > 0 {
                            if let Some(ring_dma) = stage_spoor_payload(&payload[..announced]) {
                                if let Err(refused) =
                                    gem::transmit_once(&gem_window, ring_dma, speed, full_duplex)
                                {
                                    stopped = Some(refused);
                                    beaconing = false;
                                }
                            }
                        }
                    }
                }
                // `STORY-P1-07-06`: one transcript line per beat rides the
                // wire behind the beacon, cycling — the owner's "diagnosis
                // moves onto the cable" applied to the first hardware
                // measurement. Same fail-safe as the beacon: one refusal ends
                // the channel and is spoken.
                //
                // **A full cycle is `count` beats, and the beat is 1 Hz, so it
                // is `count` seconds — 18 at the 14-metric envelope of
                // 2026-08-06.** Stated as a function of `count` rather than as
                // the "dozen-odd seconds" this comment used to claim: that was
                // written when the envelope was shorter, and a capture window
                // sized from a stale constant is how an operator concludes a
                // line was never transmitted when it simply had not come round
                // yet. Size the capture from `line_count`, with margin.
                #[cfg(feature = "fixture-measure")]
                if beaconing {
                    if let Some((speed, full_duplex)) = speed_config {
                        let count = crate::transcript::line_count();
                        if count > 0 {
                            let mut line = [0u8; crate::gem::TEXT_FRAME_CAPACITY];
                            let nth = beat_seq as usize % count;
                            if let Some(len) = crate::transcript::copy_line(nth, &mut line) {
                                let ring_dma = stage_text_line(&line[..len]);
                                if let Err(refused) =
                                    gem::transmit_once(&gem_window, ring_dma, speed, full_duplex)
                                {
                                    stopped = Some(refused);
                                    beaconing = false;
                                }
                            }
                        }
                    }
                }
                // `STORY-P1-09-16`: the board's first inbound poll, and the
                // one place in this loop that reads bytes it did not write.
                //
                // Armed only while the board is beaconing — we listen only on
                // a wire we are already speaking on, and a beacon that stops
                // takes the receiver down with it, so an enabled DMA engine
                // never outlives the channel that justified it. `Stopped` and
                // `Refused` are terminal: neither is `Idle`, so no later pass
                // re-arms them.
                //
                // Exactly one descriptor per beat. The one-descriptor ring
                // means the MAC has nowhere to put a second frame until this
                // poll hands the descriptor back, which is what makes a 1 Hz
                // poll a bound rather than a hope.
                if beaconing {
                    if receive == gem_receive::ReceiveState::Idle {
                        receive = arm_receive(&gem_window);
                    }
                    let (next, accepted, refused) = poll_receive(&gem_window, receive);
                    receive = next;
                    rx_accepted = rx_accepted.saturating_add(accepted);
                    rx_refused = rx_refused.saturating_add(refused);
                } else if receive == gem_receive::ReceiveState::Listening {
                    gem_receive::disable_receive(&gem_window);
                    receive = gem_receive::ReceiveState::Idle;
                }
                let (rx_text, rx_len) = receive_line(receive, rx_accepted, rx_refused);
                crate::canvas::draw_line(
                    &mut console,
                    crate::canvas::RX_Y,
                    &rx_text[..rx_len.saturating_sub(1)],
                    match receive {
                        gem_receive::ReceiveState::Stopped(_)
                        | gem_receive::ReceiveState::Refused(_) => crate::canvas::ALERT,
                        _ => crate::canvas::TEXT,
                    },
                );
                // `STORY-P1-07-04` clause 1: the ratio evidence accumulates
                // on screen once a second — unless the tick was refused, in
                // which case its row stays pinned to the refusal painted
                // above and this repaint is skipped.
                if tick_refused.is_none() {
                    let (tick_text, tick_len) = crate::tick::status_line();
                    crate::canvas::draw_line(
                        &mut console,
                        crate::canvas::TICK_Y,
                        &tick_text[..tick_len.saturating_sub(1)],
                        crate::canvas::TEXT,
                    );
                }
                let park = park_state(beaconing, stopped, watch.is_some(), watch_dead);
                // `STORY-P1-07-09`: the live status and any refusal, as text.
                let (status, status_len) = heartbeat_line(beat_seq, park, fb_granted);
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
                if heartbeating && !emit_heartbeat(uart, beat_seq, park, fb_granted) {
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
