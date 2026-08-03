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
    /// The root port's own config header did not answer with the Broadcom
    /// vendor id — nothing else it says can be believed. Carries the raw
    /// vendor/device dword (`STORY-P1-09-10`).
    RootVendor(u32),
    /// Bus 1 device 0 did not answer with the Raspberry Pi vendor id — the
    /// thing behind the link is not the RP1 this slice knows. Carries the
    /// raw vendor/device dword (`STORY-P1-09-10`).
    EndpointVendor(u32),
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
    let window = read_window(rc);
    validate_window(&window)?;
    Ok(window)
}

fn read_window<M: Mmio>(rc: &M) -> OutboundWindow {
    OutboundWindow::decode(
        rc.read_u32(register::WIN0_LO),
        rc.read_u32(register::WIN0_HI),
        rc.read_u32(register::WIN0_BASE_LIMIT),
        rc.read_u32(register::WIN0_BASE_HI),
        rc.read_u32(register::WIN0_LIMIT_HI),
    )
}

/// The `WIN0` programming values (`STORY-P1-09-09`): the working system's
/// own mapping, transcribed from the on-silicon capture
/// (`pios-ground-truth-2026-08-03.txt`, dmesg
/// `MEM 0x1f00000000..0x1ffffffffb -> 0x0000000000`) and pinned by decoding
/// them back through [`OutboundWindow::decode`] in the tests. PCI address
/// zero in both halves; CPU base `0x1F` gigabytes with base bits `[31:20]`
/// zero (packed into `BASE_LIMIT[15:4]`), CPU limit `0x1F_FFFx_xxxx` (limit
/// bits `[31:20]` all ones packed into `BASE_LIMIT[31:20]`).
pub mod window_program {
    /// `WIN0_LO` — PCI address low half: zero.
    pub const LO: u32 = 0;
    /// `WIN0_HI` — PCI address high half: zero.
    pub const HI: u32 = 0;
    /// `WIN0_BASE_LIMIT` — base `[31:20]` = 0 in bits `[15:4]`, limit
    /// `[31:20]` = 0xFFF in bits `[31:20]`.
    pub const BASE_LIMIT: u32 = 0xFFF0_0000;
    /// `WIN0_BASE_HI` — CPU base bits `[39:32]`.
    pub const BASE_HI: u32 = 0x1F;
    /// `WIN0_LIMIT_HI` — CPU limit bits `[39:32]`.
    pub const LIMIT_HI: u32 = 0x1F;
}

/// The enumeration constants (`STORY-P1-09-10` / `TEST-P1-09-10-A` clause
/// 1): every value is the working system's, from the on-silicon capture's
/// `lspci -vv` (`pios-ground-truth-2026-08-03.txt`), and the config-access
/// mechanism is `rpi-6.12.y` `drivers/pci/controller/pcie-brcmstb.c`
/// (retrieved 2026-08-03): the root port's config header is memory-mapped
/// at the controller base; downstream config sets `EXT_CFG_INDEX` to the
/// standard ECAM packing `bus << 20 | devfn << 12` and reads through the
/// 4 KiB `EXT_CFG_DATA` window.
pub mod config {
    /// Root-port vendor/device dword: config header offset 0, at the base.
    pub const RC_VENDOR: usize = 0x00;
    /// Root-port command/status dword. The status half is write-1-to-clear
    /// and belongs to nobody here — command writes mask it to zero.
    pub const RC_COMMAND: usize = 0x04;
    /// Root-port primary/secondary/subordinate bus numbers dword.
    pub const RC_BUS_NUMBERS: usize = 0x18;
    /// Root-port memory base/limit dword — what the bridge forwards.
    pub const RC_MEM_WINDOW: usize = 0x20;
    /// `PCIE_EXT_CFG_INDEX` — selects the downstream config target.
    pub const EXT_CFG_INDEX: usize = 0x9000;
    /// `PCIE_EXT_CFG_DATA` — the 4 KiB window the target's header shows in.
    pub const EXT_CFG_DATA: usize = 0x8000;
    /// The endpoint's vendor/device dword, through the data window.
    pub const EP_VENDOR: usize = EXT_CFG_DATA;
    /// The endpoint's command/status dword, same mask discipline.
    pub const EP_COMMAND: usize = EXT_CFG_DATA + 0x04;
    /// ECAM index for bus 1, device 0, function 0: `1 << 20`.
    pub const RP1_INDEX: u32 = 1 << 20;
    /// `primary=00, secondary=01, subordinate=01`, latency 0 — the lspci
    /// bridge line, byte-packed.
    pub const BUS_NUMBERS: u32 = 0x0001_0100;
    /// `Memory behind bridge: 00000000-004fffff` — base `0x0000` and limit
    /// `0x0040` halves ⇒ bus `0x0..=0x4fffff`.
    pub const MEM_WINDOW: u32 = 0x0040_0000;
    /// Command bits: memory decode + bus master, as both devices read
    /// (`Mem+ BusMaster+`).
    pub const COMMAND_ENABLE: u32 = 0x6;
    /// The root port's vendor: Broadcom.
    pub const BROADCOM_VENDOR: u16 = 0x14E4;
    /// The endpoint's vendor: Raspberry Pi Ltd.
    pub const RPI_VENDOR: u16 = 0x1DE4;
}

/// The introduction (`STORY-P1-09-10`): verifies who is answering before
/// every write, programs the routing the working system carries, and
/// refuses honestly when either vendor gate fails. Idempotent — safe on
/// every re-probe pass.
pub fn enumerate<M: Mmio>(rc: &M) -> Result<(), LinkAbsent> {
    // TEST-P1-09-10-A clause 2: the root vendor gate comes before any
    // write; a wrong answer leaves the controller untouched.
    let root = rc.read_u32(config::RC_VENDOR);
    if root as u16 != config::BROADCOM_VENDOR {
        return Err(LinkAbsent::RootVendor(root));
    }
    rc.write_u32(config::RC_BUS_NUMBERS, config::BUS_NUMBERS);
    rc.write_u32(config::RC_MEM_WINDOW, config::MEM_WINDOW);
    let command = rc.read_u32(config::RC_COMMAND);
    rc.write_u32(config::RC_COMMAND, (command & 0xFFFF) | config::COMMAND_ENABLE);
    rc.write_u32(config::EXT_CFG_INDEX, config::RP1_INDEX);
    // The endpoint gate: nothing of RP1's is written unless RP1 answered.
    let endpoint = rc.read_u32(config::EP_VENDOR);
    if endpoint as u16 != config::RPI_VENDOR {
        return Err(LinkAbsent::EndpointVendor(endpoint));
    }
    let command = rc.read_u32(config::EP_COMMAND);
    rc.write_u32(config::EP_COMMAND, (command & 0xFFFF) | config::COMMAND_ENABLE);
    Ok(())
}

/// One full establishment pass: link gates, window (with the
/// `STORY-P1-09-09` programming fallback), then the introduction. The
/// window registers are controller-local, so their order relative to the
/// enumeration is free; the *bus* traffic — the GEM identity read — happens
/// only after all of this succeeds.
pub fn establish<M: Mmio>(rc: &M) -> Result<OutboundWindow, LinkAbsent> {
    let window = probe_or_program(rc)?;
    enumerate(rc)?;
    Ok(window)
}

/// Whether a refusal is about the window's contents (programmable) rather
/// than the link's existence (never written to — programming a window on a
/// dead controller is a hopeful write into the dark).
pub const fn window_class(absent: &LinkAbsent) -> bool {
    matches!(
        absent,
        LinkAbsent::WindowBase(_) | LinkAbsent::WindowPci(_) | LinkAbsent::WindowSpan(_)
    )
}

/// One probe pass with the programming fallback (`STORY-P1-09-09`): probe;
/// on a window-class refusal, write the recorded mapping exactly once and
/// validate again. The second verdict is final either way — belief comes
/// from the re-read, never from the write. Link-class refusals return
/// without a single write.
pub fn probe_or_program<M: Mmio>(rc: &M) -> Result<OutboundWindow, LinkAbsent> {
    match probe(rc) {
        Ok(window) => Ok(window),
        Err(absent) if window_class(&absent) => {
            // TEST-P1-09-09-A clause 2: exactly these five, in this order,
            // once. The link gates above already passed on this pass.
            rc.write_u32(register::WIN0_LO, window_program::LO);
            rc.write_u32(register::WIN0_HI, window_program::HI);
            rc.write_u32(register::WIN0_BASE_LIMIT, window_program::BASE_LIMIT);
            rc.write_u32(register::WIN0_BASE_HI, window_program::BASE_HI);
            rc.write_u32(register::WIN0_LIMIT_HI, window_program::LIMIT_HI);
            // TEST-P1-09-09-A clause 3: the re-read is the verdict.
            let window = read_window(rc);
            validate_window(&window)?;
            Ok(window)
        }
        Err(absent) => Err(absent),
    }
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

    // TEST-P1-09-09-A clause 1: the programmed mapping is the capture's,
    // pinned by decoding it back through the already-pinned decoder.

    #[test]
    fn the_programming_values_decode_to_the_captured_working_mapping() {
        let window = OutboundWindow::decode(
            window_program::LO,
            window_program::HI,
            window_program::BASE_LIMIT,
            window_program::BASE_HI,
            window_program::LIMIT_HI,
        );
        assert_eq!(window.cpu_base, board::RP1_WINDOW_BASE);
        assert_eq!(window.pci_base, 0);
        assert!(window.cpu_limit >= board::RP1_WINDOW_BASE + (board::RP1_WINDOW_MIN_SPAN - 1));
        // The dmesg line's whole range: 0x1f00000000..=0x1fffffffff at MB
        // granularity (the decoder names limits inclusively).
        assert_eq!(window.cpu_limit, 0x0000_001F_FFFF_FFFF);
        assert_eq!(validate_window(&window), Ok(()));
    }

    /// A controller that answers healthy link gates but a garbage window,
    /// records every write, and — once programmed — answers the programmed
    /// values back (`accepts` true) or keeps answering garbage.
    struct ProgrammableRc {
        accepts: bool,
        programmed: RefCell<Vec<(usize, u32)>>,
    }

    impl ProgrammableRc {
        fn new(accepts: bool) -> Self {
            ProgrammableRc { accepts, programmed: RefCell::new(Vec::new()) }
        }
    }

    impl Mmio for ProgrammableRc {
        fn read_u32(&self, offset: usize) -> u32 {
            let programmed = self.programmed.borrow();
            let answered = |register: usize| {
                programmed
                    .iter()
                    .rev()
                    .find(|(offset, _)| *offset == register)
                    .map(|(_, value)| *value)
            };
            match offset {
                register::STATUS => status::PORT_IS_RC | status::PHY_LINK_UP | status::DL_ACTIVE,
                register::WIN0_LO | register::WIN0_HI => {
                    if self.accepts { answered(offset).unwrap_or(0) } else { 0 }
                }
                register::WIN0_BASE_LIMIT | register::WIN0_BASE_HI | register::WIN0_LIMIT_HI => {
                    if self.accepts {
                        // Garbage until programmed; the programmed value after.
                        answered(offset).unwrap_or(0)
                    } else {
                        0
                    }
                }
                other => panic!("unexpected read {other:#x}"),
            }
        }

        fn write_u32(&self, offset: usize, value: u32) {
            self.programmed.borrow_mut().push((offset, value));
        }
    }

    // TEST-P1-09-09-A clause 2: only window-class refusals program, exactly
    // once, exactly these five registers, in order.

    #[test]
    fn a_window_refusal_programs_the_five_registers_once_and_believes_the_reread() {
        let rc = ProgrammableRc::new(true);
        let window = probe_or_program(&rc).expect("an accepting controller validates");
        assert_eq!(window.cpu_base, board::RP1_WINDOW_BASE);
        assert_eq!(
            *rc.programmed.borrow(),
            vec![
                (register::WIN0_LO, window_program::LO),
                (register::WIN0_HI, window_program::HI),
                (register::WIN0_BASE_LIMIT, window_program::BASE_LIMIT),
                (register::WIN0_BASE_HI, window_program::BASE_HI),
                (register::WIN0_LIMIT_HI, window_program::LIMIT_HI),
            ]
        );
    }

    #[test]
    fn a_link_class_refusal_never_writes_anything() {
        let mut rc = ScriptedRc::healthy();
        rc.status_word = 0; // PortNotRc — ScriptedRc panics on any write.
        assert_eq!(probe_or_program(&rc), Err(LinkAbsent::PortNotRc(0)));
        let mut rc = ScriptedRc::healthy();
        rc.status_word = status::PORT_IS_RC; // PhyDown.
        assert_eq!(
            probe_or_program(&rc),
            Err(LinkAbsent::PhyDown(status::PORT_IS_RC))
        );
    }

    // TEST-P1-09-09-A clause 3: the second verdict is final.

    #[test]
    fn a_window_that_still_refuses_after_programming_is_reported_not_rewritten() {
        let rc = ProgrammableRc::new(false);
        assert_eq!(probe_or_program(&rc), Err(LinkAbsent::WindowBase(0)));
        assert_eq!(rc.programmed.borrow().len(), 5, "one burst, never a second");
    }

    #[test]
    fn a_healthy_window_is_validated_without_a_single_write() {
        let rc = ScriptedRc::healthy();
        probe_or_program(&rc).expect("healthy readback probes present");
        // ScriptedRc panics on write, so arriving here is the assertion.
    }

    // TEST-P1-09-10-A clause 1: every enumeration value is the capture's.

    #[test]
    fn the_enumeration_values_are_the_lspci_lines_pinned() {
        // "Bus: primary=00, secondary=01, subordinate=01, sec-latency=0".
        assert_eq!(config::BUS_NUMBERS.to_le_bytes(), [0x00, 0x01, 0x01, 0x00]);
        // "Memory behind bridge: 00000000-004fffff": base half 0x0000,
        // limit half 0x0040 — limit names its megabyte inclusively.
        assert_eq!(config::MEM_WINDOW & 0xFFFF, 0x0000);
        assert_eq!((config::MEM_WINDOW >> 16) & 0xFFF0, 0x0040);
        let limit_top = u64::from((config::MEM_WINDOW >> 16) & 0xFFF0) << 16 | 0xF_FFFF;
        assert_eq!(limit_top, 0x004F_FFFF, "forwards exactly the 5 MiB the capture shows");
        // ECAM: bus 1, device 0, function 0.
        assert_eq!(config::RP1_INDEX, 1 << 20);
        // "Control: ... Mem+ BusMaster+" on both devices.
        assert_eq!(config::COMMAND_ENABLE, 0b110);
        // Both access registers live inside the mapped controller window.
        const {
            assert!(config::EXT_CFG_INDEX < board::PCIE2_SIZE);
            assert!(config::EP_COMMAND < board::PCIE2_SIZE);
        }
    }

    /// Records the introduction's traffic; scripts both vendors and hostile
    /// write-1-to-clear status halves in both command registers.
    struct EnumerableRc {
        endpoint_vendor: u32,
        log: RefCell<Vec<(char, usize, u32)>>,
    }

    impl EnumerableRc {
        fn new(endpoint_vendor: u32) -> Self {
            EnumerableRc { endpoint_vendor, log: RefCell::new(Vec::new()) }
        }

        fn writes(&self) -> Vec<(usize, u32)> {
            self.log
                .borrow()
                .iter()
                .filter(|(kind, ..)| *kind == 'w')
                .map(|(_, offset, value)| (*offset, *value))
                .collect()
        }
    }

    impl Mmio for EnumerableRc {
        fn read_u32(&self, offset: usize) -> u32 {
            self.log.borrow_mut().push(('r', offset, 0));
            match offset {
                config::RC_VENDOR => 0x2712_14E4,
                config::RC_COMMAND => 0xABCD_0000, // hostile W1C status half
                config::EP_VENDOR => self.endpoint_vendor,
                config::EP_COMMAND => 0xF00F_0400,
                other => panic!("unexpected read {other:#x}"),
            }
        }

        fn write_u32(&self, offset: usize, value: u32) {
            self.log.borrow_mut().push(('w', offset, value));
        }
    }

    // TEST-P1-09-10-A clause 2: exact, ordered, masked.

    #[test]
    fn the_introduction_is_exact_ordered_and_masks_the_status_half() {
        let rc = EnumerableRc::new(0x0001_1DE4);
        enumerate(&rc).expect("both vendors answer");
        assert_eq!(
            rc.writes(),
            vec![
                (config::RC_BUS_NUMBERS, config::BUS_NUMBERS),
                (config::RC_MEM_WINDOW, config::MEM_WINDOW),
                // Status half zeroed: 0xABCD would clear W1C bits if echoed.
                (config::RC_COMMAND, 0x0000_0006),
                (config::EXT_CFG_INDEX, config::RP1_INDEX),
                // Endpoint command keeps its own low half (0x0400) plus ours.
                (config::EP_COMMAND, 0x0000_0406),
            ]
        );
    }

    #[test]
    fn a_wrong_root_vendor_refuses_with_zero_writes() {
        struct WrongRoot;
        impl Mmio for WrongRoot {
            fn read_u32(&self, offset: usize) -> u32 {
                assert_eq!(offset, config::RC_VENDOR, "only the gate may be read");
                0xFFFF_FFFF
            }
            fn write_u32(&self, offset: usize, _value: u32) {
                panic!("wrote {offset:#x} past a failed root gate");
            }
        }
        assert_eq!(enumerate(&WrongRoot), Err(LinkAbsent::RootVendor(0xFFFF_FFFF)));
    }

    // TEST-P1-09-10-A clause 2, endpoint half: bridge setup happened, but
    // nothing of the stranger's is written.
    #[test]
    fn a_wrong_endpoint_vendor_refuses_before_any_endpoint_write() {
        let rc = EnumerableRc::new(0xFFFF_FFFF);
        assert_eq!(enumerate(&rc), Err(LinkAbsent::EndpointVendor(0xFFFF_FFFF)));
        let writes = rc.writes();
        assert_eq!(writes.len(), 4, "bridge setup plus the index, nothing further");
        assert!(
            !writes.iter().any(|(offset, _)| *offset == config::EP_COMMAND),
            "no write belongs to a stranger"
        );
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
