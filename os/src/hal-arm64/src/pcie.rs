//! The firmware-kept PCIe link to RP1, interrogated before it is trusted.
//!
//! `STORY-P1-09-01` / `TEST-P1-09-01-A`. The Pi 5 firmware brings the ×4 link
//! to the RP1 southbridge up for its own use and — unless `config.txt` says
//! `pciex4_reset=0` — resets it before entering the kernel. A read through the
//! outbound window at [`crate::board::RP1_WINDOW_BASE`] on a reset link is a
//! data abort, so this module never starts there: the controller's own
//! registers at [`crate::board::PCIE2_BASE`] are always mapped and always
//! answer, and the probe requires them to pass two gates — link status, then
//! outbound-window readback — before the window earns its single identity
//! read.
//!
//! Everything above the [`crate::pl011::Mmio`] seam is pure and host-tested;
//! the aarch64 glue lives with the callers in `ethernet.rs`. Register offsets
//! and masks are transcribed from Raspberry Pi Linux `rpi-6.12.y`
//! `drivers/pci/controller/pcie-brcmstb.c` (retrieved 2026-08-03) and pinned
//! by this module's tests.

use crate::board;
use crate::pl011::Mmio;

/// Register offsets inside the PCIe2 controller window, with the driver's own
/// names in the doc comments so the transcription is checkable line-by-line.
pub mod register {
    /// `PCIE_MISC_PCIE_STATUS` — link and mode status.
    pub const STATUS: usize = 0x4068;
    /// `PCIE_MISC_CPU_2_PCIE_MEM_WIN0_LO` — outbound window 0, PCI address
    /// low half (low bits carry no flags on this IP).
    pub const WIN0_LO: usize = 0x400C;
    /// `PCIE_MISC_CPU_2_PCIE_MEM_WIN0_HI` — outbound window 0, PCI address
    /// high half.
    pub const WIN0_HI: usize = 0x4010;
    /// `PCIE_MISC_CPU_2_PCIE_MEM_WIN0_BASE_LIMIT` — CPU base and limit,
    /// bits `[31:20]` of each, packed base-low/limit-high.
    pub const WIN0_BASE_LIMIT: usize = 0x4070;
    /// `PCIE_MISC_CPU_2_PCIE_MEM_WIN0_BASE_HI` — CPU base bits `[39:32]`.
    pub const WIN0_BASE_HI: usize = 0x4080;
    /// `PCIE_MISC_CPU_2_PCIE_MEM_WIN0_LIMIT_HI` — CPU limit bits `[39:32]`.
    pub const WIN0_LIMIT_HI: usize = 0x4084;
}

/// Bit masks inside [`register::STATUS`].
pub mod status {
    /// `PCIE_MISC_PCIE_STATUS_PCIE_PORT_MASK` — the port is in root-complex
    /// mode (as opposed to endpoint mode).
    pub const PORT_IS_RC: u32 = 0x80;
    /// `PCIE_MISC_PCIE_STATUS_PCIE_DL_ACTIVE_MASK` — the data link is up.
    pub const DL_ACTIVE: u32 = 0x20;
    /// `PCIE_MISC_PCIE_STATUS_PCIE_PHYLINKUP_MASK` — the PHY layer trained.
    pub const PHY_LINK_UP: u32 = 0x10;
}

/// Why the probe concluded `rp1=absent` without touching the window.
///
/// Every arm is a distinct driven rejection in this module's tests, and every
/// arm carries the readback that condemned it — the report is the evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkAbsent {
    /// The status register says the port is not in root-complex mode — either
    /// the firmware reset the controller (`pciex4_reset=0` missing) or the
    /// register read as garbage. Carries the raw status word.
    PortNotRc(u32),
    /// The PHY layer never trained. Carries the raw status word.
    PhyDown(u32),
    /// The PHY trained but the data link is not active. Carries the raw
    /// status word.
    LinkDown(u32),
    /// The outbound window does not start at the recorded CPU base. Carries
    /// the decoded CPU base.
    WindowBase(u64),
    /// The outbound window does not map PCI address zero (where the firmware
    /// assigns RP1's peripheral BAR). Carries the decoded PCI base.
    WindowPci(u64),
    /// The outbound window is smaller than RP1's peripheral space. Carries
    /// the decoded CPU limit.
    WindowSpan(u64),
}

/// Outbound window 0, decoded from its five registers — a pure function of
/// the readback, so the encoding is pinned on the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutboundWindow {
    /// First CPU-physical address the window claims.
    pub cpu_base: u64,
    /// Last CPU-physical address the window claims (inclusive).
    pub cpu_limit: u64,
    /// PCI-bus address the window's CPU base is translated to.
    pub pci_base: u64,
}

impl OutboundWindow {
    /// Decodes the brcmstb window encoding: `BASE_LIMIT` packs base bits
    /// `[31:20]` into `[15:4]` and limit bits `[31:20]` into `[31:20]`; the
    /// `_HI` registers carry bits `[39:32]`; a limit names its megabyte
    /// inclusively, so the low 20 bits of the decoded limit are ones.
    pub const fn decode(lo: u32, hi: u32, base_limit: u32, base_hi: u32, limit_hi: u32) -> Self {
        let cpu_base = ((base_hi as u64 & 0xFF) << 32) | (((base_limit as u64 >> 4) & 0xFFF) << 20);
        let cpu_limit = ((limit_hi as u64 & 0xFF) << 32)
            | (((base_limit as u64 >> 20) & 0xFFF) << 20)
            | 0xF_FFFF;
        let pci_base = ((hi as u64) << 32) | lo as u64;
        OutboundWindow { cpu_base, cpu_limit, pci_base }
    }
}

/// Gate one: the status word must say RC mode, PHY trained, data link active —
/// checked in that order so the reported reason names the first missing rung,
/// not the last.
pub const fn link_gates(status_word: u32) -> Result<(), LinkAbsent> {
    if status_word & status::PORT_IS_RC == 0 {
        return Err(LinkAbsent::PortNotRc(status_word));
    }
    if status_word & status::PHY_LINK_UP == 0 {
        return Err(LinkAbsent::PhyDown(status_word));
    }
    if status_word & status::DL_ACTIVE == 0 {
        return Err(LinkAbsent::LinkDown(status_word));
    }
    Ok(())
}

/// Gate two: the window readback must actually map
/// [`board::RP1_WINDOW_BASE`]` .. +`[`board::RP1_WINDOW_MIN_SPAN`] onto PCI
/// address zero. A link that is up behind a wrong window would turn the
/// identity read into a read of something else that answers.
pub const fn validate_window(window: &OutboundWindow) -> Result<(), LinkAbsent> {
    if window.cpu_base != board::RP1_WINDOW_BASE {
        return Err(LinkAbsent::WindowBase(window.cpu_base));
    }
    if window.pci_base != 0 {
        return Err(LinkAbsent::WindowPci(window.pci_base));
    }
    let last_needed = board::RP1_WINDOW_BASE + (board::RP1_WINDOW_MIN_SPAN - 1);
    if window.cpu_limit < last_needed {
        return Err(LinkAbsent::WindowSpan(window.cpu_limit));
    }
    Ok(())
}

/// Runs both gates against the controller, reading the status register first
/// and the window registers only if the link passed — the seam double in the
/// tests asserts that order, because reading window registers on a dead
/// controller is pointless and reading the *window itself* is forbidden here.
pub fn probe<M: Mmio>(rc: &M) -> Result<OutboundWindow, LinkAbsent> {
    link_gates(rc.read_u32(register::STATUS))?;
    let window = OutboundWindow::decode(
        rc.read_u32(register::WIN0_LO),
        rc.read_u32(register::WIN0_HI),
        rc.read_u32(register::WIN0_BASE_LIMIT),
        rc.read_u32(register::WIN0_BASE_HI),
        rc.read_u32(register::WIN0_LIMIT_HI),
    );
    validate_window(&window)?;
    Ok(window)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::RefCell;

    /// A scripted controller double: answers reads from a fixed map and
    /// records every access in order, so the tests can assert both the
    /// decision and the discipline (status before window, window never).
    struct ScriptedRc {
        status_word: u32,
        window: [u32; 5],
        accesses: RefCell<Vec<usize>>,
    }

    impl ScriptedRc {
        fn healthy() -> Self {
            // A window mapping CPU 0x1F_0000_0000..=0x1F_03FF_FFFF (4 MiB+)
            // onto PCI 0, the shape the firmware leaves behind.
            ScriptedRc {
                status_word: status::PORT_IS_RC | status::PHY_LINK_UP | status::DL_ACTIVE,
                window: [0, 0, 0x03F0_0000, 0x1F, 0x1F],
                accesses: RefCell::new(Vec::new()),
            }
        }

        fn order(&self) -> Vec<usize> {
            self.accesses.borrow().clone()
        }
    }

    impl Mmio for ScriptedRc {
        fn read_u32(&self, offset: usize) -> u32 {
            self.accesses.borrow_mut().push(offset);
            match offset {
                register::STATUS => self.status_word,
                register::WIN0_LO => self.window[0],
                register::WIN0_HI => self.window[1],
                register::WIN0_BASE_LIMIT => self.window[2],
                register::WIN0_BASE_HI => self.window[3],
                register::WIN0_LIMIT_HI => self.window[4],
                other => panic!("the probe read an unexpected register {other:#x}"),
            }
        }

        fn write_u32(&self, offset: usize, _value: u32) {
            panic!("the probe wrote {offset:#x}; a probe that writes is not a probe");
        }
    }

    // TEST-P1-09-01-A clause 2: the root complex answers before the window is
    // touched, and each missing rung is its own named rejection.

    #[test]
    fn a_kept_link_with_a_correct_window_reports_the_window() {
        let rc = ScriptedRc::healthy();
        let window = probe(&rc).expect("healthy readback probes present");
        assert_eq!(window.cpu_base, 0x0000_001F_0000_0000);
        assert_eq!(window.pci_base, 0);
        assert!(window.cpu_limit >= 0x0000_001F_003F_FFFF);
    }

    #[test]
    fn the_status_register_is_read_first_and_is_the_only_read_on_a_dead_link() {
        let mut rc = ScriptedRc::healthy();
        rc.status_word = 0;
        assert_eq!(probe(&rc), Err(LinkAbsent::PortNotRc(0)));
        assert_eq!(rc.order(), vec![register::STATUS], "a dead link earns exactly one read");
    }

    #[test]
    fn each_missing_status_rung_is_named_in_gate_order() {
        // RC mode missing dominates, then PHY, then DL — the reason names the
        // first rung that failed, which is the one an operator acts on.
        assert_eq!(link_gates(0), Err(LinkAbsent::PortNotRc(0)));
        let rc_only = status::PORT_IS_RC;
        assert_eq!(link_gates(rc_only), Err(LinkAbsent::PhyDown(rc_only)));
        let phy_up = status::PORT_IS_RC | status::PHY_LINK_UP;
        assert_eq!(link_gates(phy_up), Err(LinkAbsent::LinkDown(phy_up)));
        assert_eq!(link_gates(phy_up | status::DL_ACTIVE), Ok(()));
    }

    #[test]
    fn a_window_at_the_wrong_cpu_base_is_refused_with_the_base_it_read() {
        let mut rc = ScriptedRc::healthy();
        rc.window[3] = 0x1E; // CPU base bits [39:32] — one aperture off.
        assert_eq!(probe(&rc), Err(LinkAbsent::WindowBase(0x0000_001E_0000_0000)));
    }

    #[test]
    fn a_window_translating_to_a_nonzero_pci_address_is_refused() {
        let mut rc = ScriptedRc::healthy();
        rc.window[0] = 0x8000_0000;
        assert_eq!(probe(&rc), Err(LinkAbsent::WindowPci(0x8000_0000)));
    }

    #[test]
    fn a_window_smaller_than_the_peripheral_span_is_refused_with_its_limit() {
        let mut rc = ScriptedRc::healthy();
        // Limit inside the first megabyte: base and limit both at MB 0.
        rc.window[2] = 0x0000_0000;
        assert_eq!(probe(&rc), Err(LinkAbsent::WindowSpan(0x0000_001F_000F_FFFF)));
    }

    // Encoding pins: the decoder is the transcription, so it gets the same
    // treatment as a board constant.

    #[test]
    fn the_window_decoding_matches_the_brcmstb_packing() {
        let window = OutboundWindow::decode(0x1000, 0x2, 0x0430_0210, 0x1F, 0x20);
        // base: hi 0x1F, bits[31:20] = 0x021 (from bits [15:4] = 0x021).
        assert_eq!(window.cpu_base, 0x0000_001F_0210_0000);
        // limit: hi 0x20, bits[31:20] = 0x043, low 20 bits ones (inclusive).
        assert_eq!(window.cpu_limit, 0x0000_0020_043F_FFFF);
        // PCI address is a plain 64-bit split.
        assert_eq!(window.pci_base, 0x0000_0002_0000_1000);
    }

    #[test]
    fn the_register_offsets_are_the_brcmstb_transcriptions() {
        assert_eq!(register::STATUS, 0x4068);
        assert_eq!(register::WIN0_LO, 0x400C);
        assert_eq!(register::WIN0_HI, 0x4010);
        assert_eq!(register::WIN0_BASE_LIMIT, 0x4070);
        assert_eq!(register::WIN0_BASE_HI, 0x4080);
        assert_eq!(register::WIN0_LIMIT_HI, 0x4084);
        // Every offset lies inside the controller window the device tree maps.
        for offset in [
            register::STATUS,
            register::WIN0_LO,
            register::WIN0_HI,
            register::WIN0_BASE_LIMIT,
            register::WIN0_BASE_HI,
            register::WIN0_LIMIT_HI,
        ] {
            assert!(offset < board::PCIE2_SIZE, "offset {offset:#x} escapes the mapped window");
            assert_eq!(offset % 4, 0, "offset {offset:#x} is not word-aligned");
        }
    }

    #[test]
    fn the_status_masks_are_the_brcmstb_transcriptions() {
        assert_eq!(status::PORT_IS_RC, 0x80);
        assert_eq!(status::DL_ACTIVE, 0x20);
        assert_eq!(status::PHY_LINK_UP, 0x10);
    }
}
