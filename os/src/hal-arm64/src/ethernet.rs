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
pub fn discover<R: Mmio, G: Mmio>(root_complex: &R, gem_window: G) -> Discovery {
    if let Err(absent) = pcie::probe(root_complex) {
        return Discovery::LinkAbsent(absent);
    }
    let identity = match gem::parse_module_id(gem_window.read_u32(gem::register::MID)) {
        Ok(identity) => identity,
        Err(refused) => return Discovery::IdentityRefused(refused),
    };
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
    fn a_dead_link_never_touches_the_gem_window() {
        assert_eq!(
            discover(&DeadRc, UntouchableGem),
            Discovery::LinkAbsent(LinkAbsent::PortNotRc(0))
        );
    }

    /// A healthy root complex answering the firmware-shaped window, plus a
    /// GEM whose reads come from a tiny closure table.
    struct HealthyRc;

    impl Mmio for HealthyRc {
        fn read_u32(&self, offset: usize) -> u32 {
            match offset {
                pcie::register::STATUS => 0x80 | 0x20 | 0x10,
                pcie::register::WIN0_BASE_LIMIT => 0x03F0_0000,
                pcie::register::WIN0_BASE_HI | pcie::register::WIN0_LIMIT_HI => 0x1F,
                _ => 0,
            }
        }

        fn write_u32(&self, offset: usize, _value: u32) {
            panic!("the probe wrote {offset:#x}");
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
        let discovery = discover(&HealthyRc, PipelineGem::new());
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
            discover(&HealthyRc, FloatingGem),
            Discovery::IdentityRefused(gem::IdentityError::FloatingBus)
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

    /// Waits roughly one second; returns `false` if the counter never
    /// advanced far enough within the spin bound (a stuck counter).
    fn wait_one_period() -> bool {
        let start = counter_ticks();
        let period = counter_frequency().max(1);
        let mut spins = 0u32;
        while counter_ticks().wrapping_sub(start) < period {
            spins += 1;
            if spins >= WAIT_SPINS_LIMIT {
                return false;
            }
        }
        true
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
    /// announcing: discover, attempt the first beacon, report the one
    /// `TOS64-LINK/1` line, then beacon once per period until a transmit
    /// refuses — after which the board is simply parked, fail-safe.
    pub fn announce_and_park(uart: &Pl011<VolatileMmio>) -> ! {
        // SAFETY: the constants are the recorded CPU-physical bases of the
        // PCIe2 controller block and the GEM window; both are naturally
        // aligned register files this core may access uncached.
        let root_complex = unsafe { VolatileMmio::new(board::PCIE2_BASE) };
        // SAFETY: as above; the window is only dereferenced after the probe's
        // gates confirm the firmware kept it mapped.
        let gem_window =
            unsafe { VolatileMmio::new(board::RP1_WINDOW_BASE + board::RP1_GEM_OFFSET) };

        let discovery = discover(&root_complex, gem_window);
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

        if let (BeaconField::Running, Some((speed, full_duplex))) =
            (beacon, beacon_eligible(&discovery))
        {
            let mut seq: u32 = 1;
            loop {
                if !wait_one_period() {
                    break;
                }
                let ring_dma = stage_frame(seq);
                if gem::transmit_once(&gem_window, ring_dma, speed, full_duplex).is_err() {
                    // Fail-safe over keep-trying: one refusal ends the
                    // beacon; the board stays parked and diagnosable over
                    // serial and splash.
                    break;
                }
                seq = seq.wrapping_add(1);
            }
        }
        crate::boot::park()
    }
}

#[cfg(target_arch = "aarch64")]
pub use glue::announce_and_park;
