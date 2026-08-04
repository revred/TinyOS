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
    /// returns the ring's DMA address. Since `STORY-P1-07-03` this memory is
    /// Normal Write-Back cacheable, and the GEM masters its reads from DRAM
    /// behind the CPU's caches — so the stores are made visible with an
    /// explicit clean to the point of coherency (which ends in `dsb sy`,
    /// ordering the maintenance before the MAC is started). The GEM's own
    /// write-back of the used bit into the ring is never read by this code,
    /// so no invalidate is owed on the return path.
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
        splash: Option<crate::hdmi::FramebufferInfo>,
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
        let mut console = crate::canvas::SimplefbSurface;
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
                            } else {
                                frame_seq = frame_seq.wrapping_add(1);
                            }
                        }
                        None => beaconing = false,
                    }
                }
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
