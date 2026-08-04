//! The RP1's Cadence GEM Ethernet block: identity, management port, link
//! state, and the board-present beacon.
//!
//! `STORY-P1-09-01` (identity), `STORY-P1-09-02` (PHY and link) and
//! `STORY-P1-09-03` (beacon) — `TEST-P1-09-0{1,2,3}-A`. The discipline
//! throughout is the one `FEAT-P1-09` contracts: the device is a compromisable
//! C2 subject, so every readback is validated before belief, every poll is a
//! bounded countdown, receive is never enabled, and the only DMA grant is the
//! single pinned beacon buffer.
//!
//! Everything in this file is pure over the [`crate::pl011::Mmio`] seam and
//! host-tested; the aarch64 glue (real addresses, the pinned buffer, the
//! barrier before start) lives in `ethernet.rs`. Register offsets and bit
//! positions are transcribed from Raspberry Pi Linux `rpi-6.12.y`
//! `drivers/net/ethernet/cadence/macb.h` (retrieved 2026-08-03) and pinned by
//! this module's tests.

use crate::pl011::Mmio;

/// GEM register offsets, with the macb driver's names in the doc comments.
pub mod register {
    /// `NCR` — network control.
    pub const NCR: usize = 0x0000;
    /// `NCFGR` — network configuration.
    pub const NCFGR: usize = 0x0004;
    /// `NSR` — network status.
    pub const NSR: usize = 0x0008;
    /// `DMACFG` — DMA configuration (`GEM_DMACFG`).
    pub const DMACFG: usize = 0x0010;
    /// `TSR` — transmit status.
    pub const TSR: usize = 0x0014;
    /// `TBQP` — transmit buffer queue base, low 32 bits.
    pub const TBQP: usize = 0x001C;
    /// `MAN` — PHY maintenance (the MDIO shift register).
    pub const MAN: usize = 0x0034;
    /// `TBQPH` — transmit buffer queue base, high 32 bits (`GEM_TBQPH`).
    pub const TBQPH: usize = 0x04C8;
    /// `MID` — module identification (`GEM_MID`).
    pub const MID: usize = 0x00FC;
}

/// Bits inside [`register::NCR`].
pub mod ncr {
    /// `RE` — receive enable. Named so its *absence* can be tested: no code
    /// path in this Feature may ever set it.
    pub const RECEIVE_ENABLE: u32 = 1 << 2;
    /// `TE` — transmit enable.
    pub const TRANSMIT_ENABLE: u32 = 1 << 3;
    /// `MPE` — management port enable.
    pub const MANAGEMENT_PORT_ENABLE: u32 = 1 << 4;
    /// `TSTART` — start transmission.
    pub const TRANSMIT_START: u32 = 1 << 9;
}

/// Bits inside [`register::NCFGR`].
pub mod ncfgr {
    /// `SPD` — 100 Mbit when set (10 Mbit clear), meaningful only with
    /// [`GIGABIT`] clear.
    pub const SPEED_100: u32 = 1 << 0;
    /// `FD` — full duplex.
    pub const FULL_DUPLEX: u32 = 1 << 1;
    /// `GBE` — gigabit mode.
    pub const GIGABIT: u32 = 1 << 10;
    /// `CLK` — MDC clock divisor field, bits `[20:18]`.
    pub const MDC_DIVISOR_SHIFT: u32 = 18;
    /// Mask of the MDC divisor field.
    pub const MDC_DIVISOR_MASK: u32 = 0b111 << MDC_DIVISOR_SHIFT;
    /// Divisor code `0b111` = pclk/224 — the most conservative code the field
    /// can express. RP1's exact pclk is not transcribed anywhere in this
    /// repository, so the divisor is chosen to keep MDC below the 2.5 MHz
    /// clause-22 ceiling for any plausible pclk up to 560 MHz, trading MDIO
    /// speed (irrelevant here) for correctness on unobserved silicon.
    pub const MDC_DIVISOR_224: u32 = 0b111 << MDC_DIVISOR_SHIFT;
}

/// Bits inside [`register::NSR`].
pub mod nsr {
    /// `IDLE` — the PHY maintenance shift register has finished.
    pub const MDIO_IDLE: u32 = 1 << 2;
}

/// Bits inside [`register::DMACFG`].
pub mod dmacfg {
    /// `ADDR64` — 64-bit descriptor addressing. Required: system RAM sits at
    /// PCI `0x10_0000_0000` on the RP1's bus, above any 32-bit address.
    pub const ADDR64: u32 = 1 << 30;
}

/// Bits inside [`register::TSR`].
pub mod tsr {
    /// `COMP` — a frame completed transmission.
    pub const COMPLETE: u32 = 1 << 5;
    /// `UND` — transmit underrun: the DMA could not feed the MAC. On this
    /// path that means the address translation or the link speed is wrong.
    pub const UNDERRUN: u32 = 1 << 6;
    /// `RLE` — retry limit exceeded.
    pub const RETRY_EXHAUSTED: u32 = 1 << 2;
    /// `COL` — collision occurred (half-duplex only; on this path a symptom
    /// of a duplex mismatch).
    pub const COLLISION: u32 = 1 << 1;
}

/// How many times the MDIO idle poll spins before concluding the management
/// port is wedged. At pclk/224 a clause-22 frame is 64 MDC cycles ≈ 72 µs;
/// this bound is three decimal orders above it — it exists to convert a hang
/// into a return, not to enforce a latency budget.
pub const MDIO_POLL_LIMIT: u32 = 100_000;

/// How many times the transmit-status poll spins before concluding the frame
/// will never complete. A minimum frame at 10 Mbit is ≈ 58 µs on the wire;
/// the same three-orders rationale as [`MDIO_POLL_LIMIT`].
pub const TX_POLL_LIMIT: u32 = 1_000_000;

// --- identity (STORY-P1-09-01, the read through the window) -----------------

/// Why a module-identification readback was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    /// The register read all-ones — the signature of a floating bus or an
    /// unclaimed PCIe completion, not of any real module.
    FloatingBus,
    /// The register read zero — decoded but unbacked address space.
    AllZeros,
    /// A module answered, but it is not a GEM. Carries the module field.
    WrongModule(u16),
}

/// A validated GEM identity: the module field matched, the revision is
/// reported as evidence (the Pi 5's RP1 carries GEM_GXL revision `0x0109`,
/// but the revision is recorded, not gated — a new RP1 stepping is not an
/// absent device).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GemIdentity {
    /// The revision half of `MID`, reported in `TOS64-LINK/1 id=`.
    pub revision: u16,
}

/// The module field a GEM reports in `MID` bits `[31:16]`.
pub const GEM_MODULE_ID: u16 = 0x0007;

/// Validates a `MID` readback before anything else in the block is believed.
pub const fn parse_module_id(mid: u32) -> Result<GemIdentity, IdentityError> {
    if mid == u32::MAX {
        return Err(IdentityError::FloatingBus);
    }
    if mid == 0 {
        return Err(IdentityError::AllZeros);
    }
    let module = (mid >> 16) as u16;
    if module != GEM_MODULE_ID {
        return Err(IdentityError::WrongModule(module));
    }
    Ok(GemIdentity { revision: mid as u16 })
}

// --- the management port (STORY-P1-09-02) -----------------------------------

/// Builds the clause-22 read frame the `MAN` register shifts out: start `01`,
/// read opcode `10`, PHY address, register address, turnaround code `10`,
/// data zero. Pure so the bit layout is pinned by a host test.
pub const fn mdio_read_word(phy_address: u8, register_address: u8) -> u32 {
    (0b01 << 30)
        | (0b10 << 28)
        | ((phy_address as u32 & 0x1F) << 23)
        | ((register_address as u32 & 0x1F) << 18)
        | (0b10 << 16)
}

/// Why a management-port transaction failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MdioError {
    /// The idle bit never came back within [`MDIO_POLL_LIMIT`] polls.
    Timeout,
}

/// The management port over the seam: enables the port and sets the divisor
/// on construction, reads registers with a bounded idle poll, and puts the
/// port away on [`MdioPort::finish`]. Reads and writes touch only
/// `NCR`/`NCFGR`/`NSR`/`MAN` — the test double asserts that.
pub struct MdioPort<M: Mmio> {
    device: M,
}

impl<M: Mmio> MdioPort<M> {
    /// Enables the management port: conservative MDC divisor first, then the
    /// port-enable bit — both read-modify-write so nothing else in the
    /// device's configuration is disturbed.
    pub fn enable(device: M) -> Self {
        let ncfgr = device.read_u32(register::NCFGR);
        device.write_u32(
            register::NCFGR,
            (ncfgr & !ncfgr::MDC_DIVISOR_MASK) | ncfgr::MDC_DIVISOR_224,
        );
        let control = device.read_u32(register::NCR);
        device.write_u32(register::NCR, control | ncr::MANAGEMENT_PORT_ENABLE);
        MdioPort { device }
    }

    /// Reads one PHY register: shift the frame out, poll idle bounded, take
    /// the low half of `MAN` as the answer.
    pub fn read(&self, phy_address: u8, register_address: u8) -> Result<u16, MdioError> {
        self.device.write_u32(register::MAN, mdio_read_word(phy_address, register_address));
        for _ in 0..MDIO_POLL_LIMIT {
            if self.device.read_u32(register::NSR) & nsr::MDIO_IDLE != 0 {
                return Ok(self.device.read_u32(register::MAN) as u16);
            }
        }
        Err(MdioError::Timeout)
    }

    /// Test-only: wraps a device with none of the enable writes, for doubles
    /// that must prove they are never touched at all.
    #[cfg(test)]
    pub(crate) fn wrap_untouched_for_test(device: M) -> Self {
        MdioPort { device }
    }

    /// Disables the management port and returns the device, so the transmit
    /// path receives it back explicitly rather than through shared state.
    pub fn finish(self) -> M {
        let control = self.device.read_u32(register::NCR);
        self.device.write_u32(register::NCR, control & !ncr::MANAGEMENT_PORT_ENABLE);
        self.device
    }
}

/// The identity a Pi 5's on-board PHY is expected to report: a Broadcom
/// BCM54213PE. `ID2`'s low four bits carry the silicon revision, so the
/// comparison masks them — a new PHY stepping is the same part.
pub mod phy_identity {
    /// Expected `PHYSID1` (MDIO register 2).
    pub const ID1: u16 = 0x600D;
    /// Expected `PHYSID2` (MDIO register 3), revision nibble masked.
    pub const ID2_MASKED: u16 = 0x84A0;
    /// The mask that removes the revision nibble from `ID2`.
    pub const ID2_MASK: u16 = 0xFFF0;
}

/// Standard clause-22 PHY register addresses used on this path.
pub mod phy_register {
    /// `BMSR` — basic mode status.
    pub const STATUS: u8 = 1;
    /// `PHYSID1`.
    pub const ID1: u8 = 2;
    /// `PHYSID2`.
    pub const ID2: u8 = 3;
    /// `ANLPAR` — autonegotiation link-partner ability.
    pub const PARTNER_ABILITY: u8 = 5;
    /// `MASTER-SLAVE status` — 1000BASE-T link-partner capabilities.
    pub const GIGABIT_STATUS: u8 = 10;
}

/// Bits inside the `BMSR`.
pub mod bmsr {
    /// Link is up. Latched low: a read reports a *past* link loss until read
    /// again, which is why link state is always read twice.
    pub const LINK_UP: u16 = 1 << 2;
    /// Autonegotiation has completed.
    pub const AUTONEG_COMPLETE: u16 = 1 << 5;
}

/// What the PHY scan concluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhyOutcome {
    /// A PHY answered at `address` with the expected Broadcom identity.
    Known {
        /// The MDIO address that answered.
        address: u8,
        /// `PHYSID1` as read.
        id1: u16,
        /// `PHYSID2` as read, revision nibble included.
        id2: u16,
    },
    /// A PHY answered but its identity is not the expected part. Reported,
    /// then believed for nothing further.
    Unknown {
        /// The MDIO address that answered.
        address: u8,
        /// `PHYSID1` as read.
        id1: u16,
        /// `PHYSID2` as read.
        id2: u16,
    },
    /// No address answered with anything but all-ones.
    Absent,
    /// The management port itself stopped answering mid-scan.
    PortWedged,
    /// The reset release aborted (`STORY-P1-09-04`, stuck counter) and the
    /// scan never ran — the PHY is still held in reset, deliberately.
    ReleaseStuck,
}

/// Scans all 32 MDIO addresses in order and classifies the first responder.
/// All-ones is "nobody home at this address" (the bus idles high), not a
/// responder; the scan is bounded by construction.
pub fn scan_for_phy<M: Mmio>(port: &MdioPort<M>) -> PhyOutcome {
    for address in 0u8..32 {
        let id1 = match port.read(address, phy_register::ID1) {
            Ok(word) => word,
            Err(MdioError::Timeout) => return PhyOutcome::PortWedged,
        };
        if id1 == 0xFFFF {
            continue;
        }
        let id2 = match port.read(address, phy_register::ID2) {
            Ok(word) => word,
            Err(MdioError::Timeout) => return PhyOutcome::PortWedged,
        };
        if id1 == phy_identity::ID1 && (id2 & phy_identity::ID2_MASK) == phy_identity::ID2_MASKED {
            return PhyOutcome::Known { address, id1, id2 };
        }
        return PhyOutcome::Unknown { address, id1, id2 };
    }
    PhyOutcome::Absent
}

/// A negotiated link speed, as reported by the link partner's abilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Speed {
    /// 1000BASE-T.
    Mbps1000,
    /// 100BASE-TX.
    Mbps100,
    /// 10BASE-T.
    Mbps10,
}

/// The link, as the PHY sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    /// No link. An honest outcome, not an error.
    Down,
    /// Link up with a resolved rate.
    Up {
        /// Negotiated speed.
        speed: Speed,
        /// Negotiated duplex: `true` is full.
        full_duplex: bool,
    },
    /// Link up but autonegotiation has not completed (or resolved to nothing
    /// this code recognises) — reported, and the beacon declines to run.
    Unresolved,
}

/// Reads the link twice (the link bit is latched-low) and resolves the
/// negotiated rate from the partner-ability registers. The PHY advertises
/// everything by hardware default, so the best common denominator *is* the
/// partner's best.
pub fn read_link<M: Mmio>(port: &MdioPort<M>, address: u8) -> Result<LinkState, MdioError> {
    let _stale = port.read(address, phy_register::STATUS)?;
    let status = port.read(address, phy_register::STATUS)?;
    if status & bmsr::LINK_UP == 0 {
        return Ok(LinkState::Down);
    }
    if status & bmsr::AUTONEG_COMPLETE == 0 {
        return Ok(LinkState::Unresolved);
    }
    let gigabit = port.read(address, phy_register::GIGABIT_STATUS)?;
    if gigabit & (1 << 11) != 0 {
        return Ok(LinkState::Up { speed: Speed::Mbps1000, full_duplex: true });
    }
    if gigabit & (1 << 10) != 0 {
        return Ok(LinkState::Up { speed: Speed::Mbps1000, full_duplex: false });
    }
    let partner = port.read(address, phy_register::PARTNER_ABILITY)?;
    if partner & (1 << 8) != 0 {
        return Ok(LinkState::Up { speed: Speed::Mbps100, full_duplex: true });
    }
    if partner & (1 << 7) != 0 {
        return Ok(LinkState::Up { speed: Speed::Mbps100, full_duplex: false });
    }
    if partner & (1 << 6) != 0 {
        return Ok(LinkState::Up { speed: Speed::Mbps10, full_duplex: true });
    }
    if partner & (1 << 5) != 0 {
        return Ok(LinkState::Up { speed: Speed::Mbps10, full_duplex: false });
    }
    Ok(LinkState::Unresolved)
}

// --- the beacon frame (STORY-P1-09-03) ---------------------------------------

/// The beacon's source MAC: locally administered (`0x02` first octet), the
/// ASCII of `TOS64` behind it. A fixed constant rather than a board-serial
/// derivation: the mailbox query that could personalise it is display-path
/// machinery this Feature deliberately does not touch, and a fixed
/// locally-administered address is legal on a two-node point-to-point wire.
pub const BEACON_SOURCE_MAC: [u8; 6] = [0x02, 0x54, 0x4F, 0x53, 0x36, 0x34];

/// The beacon's EtherType: `0x88B5`, IEEE 802 local experimental — reserved
/// for exactly this kind of private point-to-point use.
pub const BEACON_ETHERTYPE: u16 = 0x88B5;

/// Capacity of the beacon buffer. One minimum-size frame; the payload is
/// bounded by construction and the tests pin the actual length.
pub const BEACON_CAPACITY: usize = 64;

/// Minimum Ethernet frame length without FCS (the MAC appends the FCS).
pub const MINIMUM_FRAME_LEN: usize = 60;

/// Capacity of a transcript text frame (`STORY-P1-07-06`): the 14-byte
/// header plus one `TOS64-MEAS/2` envelope line. Larger than
/// [`BEACON_CAPACITY`] because a `METRIC` line is ~130 bytes.
pub const TEXT_FRAME_CAPACITY: usize = 192;

/// Capacity of a spoor frame (`STORY-P1-10-02`): the 14-byte Ethernet header
/// plus a full `kernel::spoor_wire` payload of 184 packed records.
///
/// 1510 bytes, inside a standard 1500-byte MTU payload plus header, so nothing
/// here is ever handed to the MAC to fragment.
pub const SPOOR_FRAME_CAPACITY: usize = 14 + 1496;

/// Builds a broadcast frame whose payload is `payload` **verbatim** — the
/// binary carrier, beside [`text_frame`]'s textual one.
///
/// Returns [`None`] rather than truncating. Silently shortening a text line
/// costs a reader some characters; silently shortening a run of packed
/// records corrupts them, and a corrupted record decodes to a plausible lie.
#[must_use]
pub fn payload_frame(payload: &[u8]) -> Option<([u8; SPOOR_FRAME_CAPACITY], usize)> {
    if payload.len() > SPOOR_FRAME_CAPACITY - 14 {
        return None;
    }
    let mut frame = [0u8; SPOOR_FRAME_CAPACITY];
    frame[0..6].copy_from_slice(&[0xFF; 6]);
    frame[6..12].copy_from_slice(&BEACON_SOURCE_MAC);
    frame[12] = (BEACON_ETHERTYPE >> 8) as u8;
    frame[13] = BEACON_ETHERTYPE as u8;
    frame[14..14 + payload.len()].copy_from_slice(payload);
    let at = 14 + payload.len();
    let len = if at < MINIMUM_FRAME_LEN { MINIMUM_FRAME_LEN } else { at };
    Some((frame, len))
}

/// Builds a broadcast frame whose payload is `text` (truncated to fit) —
/// same destination, source and EtherType as the beacon, so the same
/// `pktmon`/Wireshark filter captures both. The transcript-on-the-wire
/// carrier (`STORY-P1-07-06`): each envelope line rides as one frame.
pub fn text_frame(text: &[u8]) -> ([u8; TEXT_FRAME_CAPACITY], usize) {
    let mut frame = [0u8; TEXT_FRAME_CAPACITY];
    frame[0..6].copy_from_slice(&[0xFF; 6]);
    frame[6..12].copy_from_slice(&BEACON_SOURCE_MAC);
    frame[12] = (BEACON_ETHERTYPE >> 8) as u8;
    frame[13] = BEACON_ETHERTYPE as u8;
    let take = text.len().min(TEXT_FRAME_CAPACITY - 14);
    frame[14..14 + take].copy_from_slice(&text[..take]);
    let at = 14 + take;
    let len = if at < MINIMUM_FRAME_LEN { MINIMUM_FRAME_LEN } else { at };
    (frame, len)
}

/// Builds the board-present beacon frame: broadcast destination,
/// [`BEACON_SOURCE_MAC`], [`BEACON_ETHERTYPE`], and a single
/// `TOS64-PRESENT/1` envelope line, zero-padded to the Ethernet minimum.
/// Returns the buffer and the frame length. Pure; pinned word-for-word by the
/// tests, with `seq` the only varying field.
pub fn beacon_frame(seq: u32) -> ([u8; BEACON_CAPACITY], usize) {
    let mut frame = [0u8; BEACON_CAPACITY];
    frame[0..6].copy_from_slice(&[0xFF; 6]);
    frame[6..12].copy_from_slice(&BEACON_SOURCE_MAC);
    frame[12] = (BEACON_ETHERTYPE >> 8) as u8;
    frame[13] = BEACON_ETHERTYPE as u8;
    let mut at = 14;
    for byte in b"TOS64-PRESENT/1 board=pi5-bcm2712 seq=" {
        frame[at] = *byte;
        at += 1;
    }
    // Decimal, most significant first, no leading zeros (a lone zero for 0).
    let mut digits = [0u8; 10];
    let mut count = 0;
    let mut rest = seq;
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
        frame[at] = digits[count];
        at += 1;
    }
    frame[at] = b'\n';
    at += 1;
    let len = if at < MINIMUM_FRAME_LEN { MINIMUM_FRAME_LEN } else { at };
    (frame, len)
}

// --- the transmit path (STORY-P1-09-03) --------------------------------------

/// Control-word bits of a GEM transmit descriptor.
pub mod tx_descriptor {
    /// `USED` — software owns the descriptor. Software clears it to hand the
    /// descriptor to the MAC; the MAC sets it back on completion.
    pub const USED: u32 = 1 << 31;
    /// `WRAP` — last descriptor in the ring; the MAC wraps to the base.
    pub const WRAP: u32 = 1 << 30;
    /// `LAST` — this buffer ends the frame.
    pub const LAST_BUFFER: u32 = 1 << 15;
    /// Mask of the buffer-length field.
    pub const LENGTH_MASK: u32 = 0x3FFF;
}

/// Builds the two-descriptor transmit ring in 64-bit (`ADDR64`) layout:
/// descriptor 0 carries the frame (owned by the MAC, last buffer, length);
/// descriptor 1 is a permanent stop (`USED | WRAP`) so the MAC never walks
/// past the one grant. Each descriptor is four words: address low, control,
/// address high, reserved. Pure; pinned by the tests.
pub const fn tx_ring(frame_dma_address: u64, frame_len: usize) -> [[u32; 4]; 2] {
    [
        [
            frame_dma_address as u32,
            (frame_len as u32 & tx_descriptor::LENGTH_MASK) | tx_descriptor::LAST_BUFFER,
            (frame_dma_address >> 32) as u32,
            0,
        ],
        [0, tx_descriptor::USED | tx_descriptor::WRAP, 0, 0],
    ]
}

/// Derives the `NCFGR` speed/duplex bits from the negotiated link — a pure
/// read-modify-write plan so a mismatch is a pinned test, not a field guess.
pub const fn speed_configuration(ncfgr_readback: u32, speed: Speed, full_duplex: bool) -> u32 {
    let mut value = ncfgr_readback & !(ncfgr::GIGABIT | ncfgr::SPEED_100 | ncfgr::FULL_DUPLEX);
    match speed {
        Speed::Mbps1000 => value |= ncfgr::GIGABIT,
        Speed::Mbps100 => value |= ncfgr::SPEED_100,
        Speed::Mbps10 => {}
    }
    if full_duplex {
        value |= ncfgr::FULL_DUPLEX;
    }
    value
}

/// Why a transmit attempt failed. Every arm permanently stops beaconing —
/// fail-safe over keep-trying.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxError {
    /// The completion bit never arrived within [`TX_POLL_LIMIT`] polls.
    Timeout,
    /// The MAC reported an error condition. Carries the raw `TSR` word.
    MacError(u32),
}

/// Points the MAC at the ring and transmits one frame: speed first, 64-bit
/// DMA addressing, queue base, transmit enable, start, then a bounded poll of
/// the status register. Receive is never enabled — the test double asserts
/// that absence. The descriptor and buffer bytes must already be in memory
/// (the aarch64 caller's job, barrier included).
pub fn transmit_once<M: Mmio>(
    device: &M,
    ring_dma_address: u64,
    speed: Speed,
    full_duplex: bool,
) -> Result<(), TxError> {
    let ncfgr_now = device.read_u32(register::NCFGR);
    device.write_u32(register::NCFGR, speed_configuration(ncfgr_now, speed, full_duplex));
    let dma_now = device.read_u32(register::DMACFG);
    device.write_u32(register::DMACFG, dma_now | dmacfg::ADDR64);
    device.write_u32(register::TBQP, ring_dma_address as u32);
    device.write_u32(register::TBQPH, (ring_dma_address >> 32) as u32);
    // Clear stale write-one-to-clear status before starting, so the poll
    // below reads this frame's outcome and not a previous attempt's.
    let stale = device.read_u32(register::TSR);
    device.write_u32(register::TSR, stale);
    let control = device.read_u32(register::NCR);
    device.write_u32(register::NCR, control | ncr::TRANSMIT_ENABLE);
    device.write_u32(register::NCR, control | ncr::TRANSMIT_ENABLE | ncr::TRANSMIT_START);
    for _ in 0..TX_POLL_LIMIT {
        let status = device.read_u32(register::TSR);
        if status & (tsr::UNDERRUN | tsr::RETRY_EXHAUSTED | tsr::COLLISION) != 0 {
            return Err(TxError::MacError(status));
        }
        if status & tsr::COMPLETE != 0 {
            return Ok(());
        }
    }
    Err(TxError::Timeout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::RefCell;

    /// A scripted GEM double: answers reads per-register from scripts (a
    /// queue per offset, then a steady value), records every access in order,
    /// and asserts the two absences this Feature contracts — no receive
    /// enable, no receive register.
    struct ScriptedGem {
        reads: RefCell<std::collections::HashMap<usize, Vec<u32>>>,
        steady: std::collections::HashMap<usize, u32>,
        writes: RefCell<Vec<(usize, u32)>>,
    }

    impl ScriptedGem {
        fn new() -> Self {
            ScriptedGem {
                reads: RefCell::new(std::collections::HashMap::new()),
                steady: std::collections::HashMap::new(),
                writes: RefCell::new(Vec::new()),
            }
        }

        fn steady(mut self, offset: usize, value: u32) -> Self {
            self.steady.insert(offset, value);
            self
        }

        fn script(self, offset: usize, values: &[u32]) -> Self {
            self.reads.borrow_mut().insert(offset, values.to_vec());
            self
        }

        fn writes(&self) -> Vec<(usize, u32)> {
            self.writes.borrow().clone()
        }

        fn written_to(&self, offset: usize) -> Vec<u32> {
            self.writes().iter().filter(|(o, _)| *o == offset).map(|(_, v)| *v).collect()
        }
    }

    impl Mmio for ScriptedGem {
        fn read_u32(&self, offset: usize) -> u32 {
            let management_or_tx = [
                register::NCR,
                register::NCFGR,
                register::NSR,
                register::MAN,
                register::MID,
                register::DMACFG,
                register::TSR,
                register::TBQP,
                register::TBQPH,
            ];
            assert!(
                management_or_tx.contains(&offset),
                "read of unexpected register {offset:#x} — this path touches only what it names"
            );
            let mut scripts = self.reads.borrow_mut();
            if let Some(queue) = scripts.get_mut(&offset) {
                if !queue.is_empty() {
                    return queue.remove(0);
                }
            }
            *self.steady.get(&offset).unwrap_or(&0)
        }

        fn write_u32(&self, offset: usize, value: u32) {
            if offset == register::NCR {
                assert_eq!(
                    value & ncr::RECEIVE_ENABLE,
                    0,
                    "receive was enabled; RCG-01's stronger form on this Feature is that \
                     remote bytes are not even read"
                );
            }
            self.writes.borrow_mut().push((offset, value));
        }
    }

    // TEST-P1-09-01-A clause 3: identity before belief.

    #[test]
    fn a_gem_module_id_is_believed_and_carries_its_revision() {
        assert_eq!(parse_module_id(0x0007_0109), Ok(GemIdentity { revision: 0x0109 }));
    }

    #[test]
    fn floating_bus_all_zeros_and_wrong_module_are_distinct_rejections() {
        assert_eq!(parse_module_id(0xFFFF_FFFF), Err(IdentityError::FloatingBus));
        assert_eq!(parse_module_id(0), Err(IdentityError::AllZeros));
        assert_eq!(parse_module_id(0x0002_0109), Err(IdentityError::WrongModule(0x0002)));
    }

    // TEST-P1-09-02-A clause 1: clause-22 framing is exact bits.

    #[test]
    // The literal's groups are the clause-22 frame's fields, deliberately
    // unequal — equal-width grouping would hide exactly the boundaries the
    // test exists to pin.
    #[allow(clippy::unusual_byte_groupings)]
    fn the_mdio_read_frame_is_exact_bits() {
        // start 01, op 10 (read), phy 0b00001, reg 0b00010, turnaround 10.
        assert_eq!(mdio_read_word(1, 2), 0b01_10_00001_00010_10_0000000000000000);
        // Address fields are masked to five bits, not silently widened.
        assert_eq!(mdio_read_word(0xFF, 0xFF), mdio_read_word(0x1F, 0x1F));
    }

    // TEST-P1-09-02-A clause 2: enabled, used, disabled — and nothing else.

    #[test]
    fn the_management_port_is_enabled_with_the_conservative_divisor_and_disabled_after() {
        let gem = ScriptedGem::new()
            .steady(register::NSR, nsr::MDIO_IDLE)
            .steady(register::NCFGR, 0x000A_0000); // pre-existing divisor bits
        let port = MdioPort::enable(&gem);
        let _ = port.read(1, phy_register::ID1);
        let device = port.finish();
        let ncfgr_writes = device.written_to(register::NCFGR);
        assert_eq!(ncfgr_writes.len(), 1, "one configuration write, read-modify-write");
        assert_eq!(ncfgr_writes[0] & ncfgr::MDC_DIVISOR_MASK, ncfgr::MDC_DIVISOR_224);
        let ncr_writes = device.written_to(register::NCR);
        assert_eq!(ncr_writes.first().map(|w| w & ncr::MANAGEMENT_PORT_ENABLE), Some(16));
        assert_eq!(
            ncr_writes.last().map(|w| w & ncr::MANAGEMENT_PORT_ENABLE),
            Some(0),
            "the port is put away"
        );
    }

    // TEST-P1-09-02-A clause 3: every transaction is bounded.

    #[test]
    fn a_never_idle_management_port_times_out_rather_than_hanging() {
        let gem = ScriptedGem::new().steady(register::NSR, 0);
        let port = MdioPort::enable(&gem);
        let before = gem.writes().len();
        assert_eq!(port.read(1, phy_register::STATUS), Err(MdioError::Timeout));
        let polls = {
            // Every poll is one NSR read; count reads via a second scripted
            // run is impossible here, so assert through the write count
            // instead: a timeout performs exactly one MAN write.
            gem.writes().len() - before
        };
        assert_eq!(polls, 1, "a timed-out read wrote the frame once and nothing else");
    }

    // TEST-P1-09-02-A clause 4: identity before belief.

    #[test]
    fn the_expected_phy_identity_is_known_and_revision_steppings_still_match() {
        let gem = ScriptedGem::new()
            .steady(register::NSR, nsr::MDIO_IDLE)
            .script(register::MAN, &[0x600D, 0x84A2]);
        let port = MdioPort::enable(&gem);
        assert_eq!(scan_for_phy(&port), PhyOutcome::Known { address: 0, id1: 0x600D, id2: 0x84A2 });
        let gem = ScriptedGem::new()
            .steady(register::NSR, nsr::MDIO_IDLE)
            .script(register::MAN, &[0x600D, 0x84AF]);
        let port = MdioPort::enable(&gem);
        assert_eq!(scan_for_phy(&port), PhyOutcome::Known { address: 0, id1: 0x600D, id2: 0x84AF });
    }

    #[test]
    fn an_unknown_phy_is_reported_and_believed_for_nothing_further() {
        let gem = ScriptedGem::new()
            .steady(register::NSR, nsr::MDIO_IDLE)
            .script(register::MAN, &[0x0141, 0x0C86]); // a Marvell part
        let port = MdioPort::enable(&gem);
        assert_eq!(
            scan_for_phy(&port),
            PhyOutcome::Unknown { address: 0, id1: 0x0141, id2: 0x0C86 }
        );
    }

    #[test]
    fn a_bus_that_answers_all_ones_everywhere_is_absent_not_a_responder() {
        let gem =
            ScriptedGem::new().steady(register::NSR, nsr::MDIO_IDLE).steady(register::MAN, 0xFFFF);
        let port = MdioPort::enable(&gem);
        assert_eq!(scan_for_phy(&port), PhyOutcome::Absent);
    }

    #[test]
    fn later_addresses_are_scanned_past_silent_ones() {
        // Address 0 idles high; address 1 is the PHY (each read is one MAN
        // write then one MAN read, so the script queue orders them).
        let gem = ScriptedGem::new()
            .steady(register::NSR, nsr::MDIO_IDLE)
            .script(register::MAN, &[0xFFFF, 0x600D, 0x84A2]);
        let port = MdioPort::enable(&gem);
        assert_eq!(scan_for_phy(&port), PhyOutcome::Known { address: 1, id1: 0x600D, id2: 0x84A2 });
    }

    // TEST-P1-09-02-A clause 5: link state is latched-aware.

    #[test]
    fn link_state_reads_the_status_twice_and_believes_the_second() {
        // First read carries a stale latched-low link bit; second is live.
        let gem = ScriptedGem::new().steady(register::NSR, nsr::MDIO_IDLE).script(
            register::MAN,
            &[0x0000, (bmsr::LINK_UP | bmsr::AUTONEG_COMPLETE) as u32, 1 << 11],
        );
        let port = MdioPort::enable(&gem);
        assert_eq!(
            read_link(&port, 1),
            Ok(LinkState::Up { speed: Speed::Mbps1000, full_duplex: true })
        );
    }

    #[test]
    fn a_down_link_is_an_honest_outcome() {
        let gem = ScriptedGem::new()
            .steady(register::NSR, nsr::MDIO_IDLE)
            .script(register::MAN, &[bmsr::LINK_UP as u32, 0x0000]);
        let port = MdioPort::enable(&gem);
        assert_eq!(read_link(&port, 1), Ok(LinkState::Down));
    }

    #[test]
    fn partner_abilities_resolve_downward_through_100_and_10() {
        let up = (bmsr::LINK_UP | bmsr::AUTONEG_COMPLETE) as u32;
        let gem = ScriptedGem::new()
            .steady(register::NSR, nsr::MDIO_IDLE)
            .script(register::MAN, &[up, up, 0, 1 << 8]);
        let port = MdioPort::enable(&gem);
        assert_eq!(
            read_link(&port, 1),
            Ok(LinkState::Up { speed: Speed::Mbps100, full_duplex: true })
        );
        let gem = ScriptedGem::new()
            .steady(register::NSR, nsr::MDIO_IDLE)
            .script(register::MAN, &[up, up, 0, 1 << 5]);
        let port = MdioPort::enable(&gem);
        assert_eq!(
            read_link(&port, 1),
            Ok(LinkState::Up { speed: Speed::Mbps10, full_duplex: false })
        );
        let gem = ScriptedGem::new()
            .steady(register::NSR, nsr::MDIO_IDLE)
            .script(register::MAN, &[up, up, 0, 0]);
        let port = MdioPort::enable(&gem);
        assert_eq!(read_link(&port, 1), Ok(LinkState::Unresolved));
    }

    // TEST-P1-09-03-A clause 1: the frame is exact bytes.

    #[test]
    fn the_beacon_frame_is_exact_bytes_padded_to_the_ethernet_minimum() {
        let (frame, len) = beacon_frame(7);
        assert_eq!(len, MINIMUM_FRAME_LEN);
        assert_eq!(&frame[0..6], &[0xFF; 6], "broadcast destination");
        assert_eq!(&frame[6..12], &BEACON_SOURCE_MAC);
        assert_eq!(&frame[12..14], &[0x88, 0xB5], "local experimental EtherType");
        let payload = core::str::from_utf8(&frame[14..len]).expect("ASCII payload");
        assert!(
            payload.starts_with("TOS64-PRESENT/1 board=pi5-bcm2712 seq=7\n"),
            "payload was {payload:?}"
        );
        assert!(
            payload["TOS64-PRESENT/1 board=pi5-bcm2712 seq=7\n".len()..].bytes().all(|b| b == 0),
            "padding is zeros"
        );
    }

    #[test]
    fn the_sequence_field_is_the_only_varying_bytes() {
        let (a, len_a) = beacon_frame(0);
        let (b, len_b) = beacon_frame(9);
        assert_eq!(len_a, len_b);
        let differing: Vec<usize> = (0..len_a).filter(|&i| a[i] != b[i]).collect();
        assert_eq!(differing.len(), 1, "exactly the sequence digit differs");
        assert_eq!(a[differing[0]], b'0');
        assert_eq!(b[differing[0]], b'9');
    }

    #[test]
    fn a_text_frame_carries_its_line_behind_the_same_header_as_the_beacon() {
        let (frame, len) = text_frame(b"TOS64-MEAS/2 END metrics=8");
        assert_eq!(&frame[0..6], &[0xFF; 6], "broadcast destination");
        assert_eq!(&frame[6..12], &BEACON_SOURCE_MAC);
        assert_eq!(frame[12], 0x88);
        assert_eq!(frame[13], 0xB5);
        assert_eq!(&frame[14..40], b"TOS64-MEAS/2 END metrics=8" as &[u8]);
        assert_eq!(len, MINIMUM_FRAME_LEN, "short payloads pad to the Ethernet minimum");
        assert!(frame[40..MINIMUM_FRAME_LEN].iter().all(|&b| b == 0), "zero padding");
        // A long line truncates to the capacity, never wraps or overruns.
        let long = [b'M'; TEXT_FRAME_CAPACITY * 2];
        let (frame, len) = text_frame(&long);
        assert_eq!(len, TEXT_FRAME_CAPACITY);
        assert!(frame[14..].iter().all(|&b| b == b'M'));
    }

    #[test]
    fn a_large_sequence_number_still_fits_the_buffer() {
        let (frame, len) = beacon_frame(u32::MAX);
        assert!(len <= BEACON_CAPACITY);
        let payload = core::str::from_utf8(&frame[14..len]).expect("ASCII payload");
        assert!(payload.contains("seq=4294967295\n"));
    }

    // TEST-P1-09-03-A clause 2: the ring is pinned and points at one buffer.

    #[test]
    fn the_transmit_ring_is_two_descriptors_frame_then_permanent_stop() {
        let ring = tx_ring(0x0000_0010_0008_1000, 60);
        assert_eq!(ring[0][0], 0x0008_1000, "address low");
        assert_eq!(ring[0][2], 0x0000_0010, "address high — the DMA offset is not optional");
        assert_eq!(ring[0][1], 60 | tx_descriptor::LAST_BUFFER, "owned by the MAC, last, length");
        assert_eq!(ring[1][1], tx_descriptor::USED | tx_descriptor::WRAP, "permanent stop");
        assert_eq!((ring[1][0], ring[1][2]), (0, 0), "the stop descriptor points nowhere");
    }

    // TEST-P1-09-03-A clause 5: speed follows the PHY.

    #[test]
    fn the_speed_configuration_is_a_pinned_read_modify_write() {
        let preserved = 0x0008_0000; // an unrelated NCFGR bit survives
        assert_eq!(
            speed_configuration(preserved | ncfgr::SPEED_100, Speed::Mbps1000, true),
            preserved | ncfgr::GIGABIT | ncfgr::FULL_DUPLEX
        );
        assert_eq!(
            speed_configuration(preserved | ncfgr::GIGABIT, Speed::Mbps100, false),
            preserved | ncfgr::SPEED_100
        );
        assert_eq!(speed_configuration(0, Speed::Mbps10, true), ncfgr::FULL_DUPLEX);
    }

    // TEST-P1-09-03-A clause 3: bounded and fail-safe.

    #[test]
    fn a_completed_transmit_programs_the_ring_in_order_and_succeeds() {
        let gem = ScriptedGem::new().script(register::TSR, &[0, 0, tsr::COMPLETE]);
        let ring_at = 0x0000_0010_0008_2000;
        assert_eq!(transmit_once(&gem, ring_at, Speed::Mbps1000, true), Ok(()));
        let offsets: Vec<usize> = gem.writes().iter().map(|(o, _)| *o).collect();
        assert_eq!(
            offsets,
            vec![
                register::NCFGR,
                register::DMACFG,
                register::TBQP,
                register::TBQPH,
                register::TSR,
                register::NCR,
                register::NCR,
            ],
            "speed, addressing, queue base, stale status clear, enable, start — in order"
        );
        assert_eq!(gem.written_to(register::TBQP), vec![0x0008_2000]);
        assert_eq!(gem.written_to(register::TBQPH), vec![0x0000_0010]);
        let dma = gem.written_to(register::DMACFG);
        assert_eq!(dma[0] & dmacfg::ADDR64, dmacfg::ADDR64, "64-bit addressing is not optional");
        let last_ncr = *gem.written_to(register::NCR).last().unwrap();
        assert_eq!(last_ncr & ncr::TRANSMIT_START, ncr::TRANSMIT_START);
    }

    #[test]
    fn a_transmit_that_never_completes_times_out_rather_than_hanging() {
        let gem = ScriptedGem::new().steady(register::TSR, 0);
        assert_eq!(
            transmit_once(&gem, 0x0000_0010_0000_0000, Speed::Mbps100, true),
            Err(TxError::Timeout)
        );
    }

    #[test]
    fn a_mac_error_is_reported_with_the_status_that_condemned_it() {
        let gem = ScriptedGem::new().steady(register::TSR, tsr::UNDERRUN);
        assert_eq!(
            transmit_once(&gem, 0x0000_0010_0000_0000, Speed::Mbps1000, true),
            Err(TxError::MacError(tsr::UNDERRUN))
        );
    }

    // TEST-P1-09-03-A clause 4: receive is disabled, and that absence is
    // tested — the double's write assertion fires on any NCR write carrying
    // the receive bit, so every test above is also this test. This one makes
    // the absence a named claim rather than a side effect.

    #[test]
    fn no_path_in_this_module_ever_enables_receive() {
        let gem = ScriptedGem::new()
            .steady(register::NSR, nsr::MDIO_IDLE)
            .script(register::TSR, &[tsr::COMPLETE]);
        let port = MdioPort::enable(&gem);
        let _ = port.read(1, phy_register::STATUS);
        let device = port.finish();
        let _ = transmit_once(&device, 0x0000_0010_0000_0000, Speed::Mbps1000, true);
        for value in device.written_to(register::NCR) {
            assert_eq!(value & ncr::RECEIVE_ENABLE, 0);
        }
    }

    #[test]
    fn the_register_offsets_are_the_macb_transcriptions() {
        assert_eq!(register::NCR, 0x0000);
        assert_eq!(register::NCFGR, 0x0004);
        assert_eq!(register::NSR, 0x0008);
        assert_eq!(register::DMACFG, 0x0010);
        assert_eq!(register::TSR, 0x0014);
        assert_eq!(register::TBQP, 0x001C);
        assert_eq!(register::MAN, 0x0034);
        assert_eq!(register::TBQPH, 0x04C8);
        assert_eq!(register::MID, 0x00FC);
        for offset in [
            register::NCR,
            register::NCFGR,
            register::NSR,
            register::DMACFG,
            register::TSR,
            register::TBQP,
            register::MAN,
            register::TBQPH,
            register::MID,
        ] {
            assert!(offset < crate::board::RP1_GEM_SIZE);
            assert_eq!(offset % 4, 0);
        }
    }
}
