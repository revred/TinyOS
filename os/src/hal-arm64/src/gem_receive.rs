//! GEM receive: one bounded frame, counted, and nothing else.
//!
//! `STORY-P1-09-16` — `TEST-P1-09-16-A`. This is the first code in the
//! repository that lets bytes chosen by something outside the image reach the
//! board's RAM, and the module is shaped around that rather than around
//! "receive works".
//!
//! It lives beside [`crate::gem`] rather than inside it deliberately. That
//! module's scripted double asserts, on every test in it, that the receive
//! enable bit is never set — the absence `TEST-P1-09-03-A` clause 4 contracts
//! for the transmit path. **That assertion is still true and still enforced**;
//! the transmit path does not enable receive, and keeping the two modules
//! apart is what keeps the claim checkable instead of quietly widened.
//!
//! The containment argument is written down in `STORY-P1-09-16`, not here, and
//! its load-bearing sentence is still worth repeating at the code: **nothing in
//! this module interprets the bytes.** [`admit`] compares a destination, an
//! EtherType and six payload bytes, and the caller increments a counter. No
//! value taken from a frame selects a branch, an address, an offset or a size
//! anywhere in *this* module.
//!
//! **That is no longer the whole containment, and the date it stopped being so
//! is 2026-08-07.** `STORY-P1-09-16` said its argument expires the moment a
//! received frame is allowed to mean something, and `STORY-P1-09-17` is that
//! moment: an admitted payload is now handed to [`crate::tos64_cmd`], whose
//! fixed-width classifier and two-row deny-by-default table re-make the
//! argument rather than citing it. The four parts this module owns — the
//! separate pinned region, the hardware address filter, the MAC-enforced size
//! bound and the total classifier — are unchanged and still load-bearing on a
//! path with no IOMMU (`LE-67`); the fifth part, "and nothing means anything",
//! moved next door and is argued there.
//!
//! Everything here is pure over the [`crate::pl011::Mmio`] seam and
//! host-tested; the aarch64 glue (the second pinned region, cache maintenance
//! in the device-writes-to-CPU direction, the once-per-beat poll) lives in
//! `ethernet.rs`. Register offsets and bit positions are transcribed from
//! Raspberry Pi Linux `rpi-6.12.y` `drivers/net/ethernet/cadence/macb.h`
//! (retrieved 2026-08-03) and pinned by this module's tests.

use crate::pl011::Mmio;

/// GEM receive-side register offsets, with the macb driver's names.
pub mod register {
    /// `RBQP` — receive buffer queue base, low 32 bits.
    pub const RBQP: usize = 0x0018;
    /// `RSR` — receive status.
    pub const RSR: usize = 0x0020;
    /// `SA1B` — specific address 1, bottom (octets 0..4).
    pub const SA1B: usize = 0x0088;
    /// `SA1T` — specific address 1, top (octets 4..6). Writing this register
    /// is what *arms* the filter, which is why it is written second.
    pub const SA1T: usize = 0x008C;
    /// `RBQPH` — receive buffer queue base, high 32 bits (`GEM_RBQPH`).
    pub const RBQPH: usize = 0x04D4;
}

/// Bits inside [`register::RSR`].
pub mod rsr {
    /// `BNA` — buffer not available: a frame arrived and the descriptor was
    /// still owned by software. A **counted drop** since 2026-08-08, not
    /// terminal — see [`super::read_status`] for why the two error bits are
    /// different failures wearing one word (`LE-118`).
    pub const BUFFER_NOT_AVAILABLE: u32 = 1 << 0;
    /// `REC` — a frame was received.
    pub const FRAME_RECEIVED: u32 = 1 << 1;
    /// `OVR` — receive overrun. Terminal here.
    pub const OVERRUN: u32 = 1 << 2;
}

/// Receive-relevant bits inside `NCFGR`, named so their *absence* is testable.
pub mod ncfgr_receive {
    /// `CAF` — copy all frames, i.e. promiscuous. No path may ever set it:
    /// the hardware address filter is part of this Story's containment
    /// argument, so turning it off is exactly as much a defect as pointing
    /// the ring at the wrong address.
    pub const COPY_ALL_FRAMES: u32 = 1 << 4;
    /// `NBC` — no broadcast. Left clear: the host's first frame is expected
    /// to be broadcast, exactly as the board's own beacon is.
    ///
    /// It is also the next lever if the single descriptor proves too
    /// contended to hold a conversation (`LE-118`'s disposition 2): setting
    /// it keeps ambient broadcast out of the ring entirely, at the cost of
    /// `STORY-P1-09-16` criterion 4's *broadcast* `ping` arm, which would
    /// then be dropped by hardware and the unicast arm become primary. Not
    /// set today, because the drop count now measures the contention rather
    /// than leaving it to be guessed at.
    pub const NO_BROADCAST: u32 = 1 << 5;
    /// `DRFCS` — remove the frame check sequence. Transcribed from `macb.h`:
    /// `#define MACB_DRFCS_OFFSET 17 /* FCS remove */`.
    ///
    /// **Set, and that is `LE-122`'s fix.** The descriptor's frame length
    /// counts every octet the MAC wrote into the buffer, and with this bit
    /// clear that includes the four-octet FCS — so a host frame of `14 + 46`
    /// octets, built at exactly the Ethernet minimum so no sending NIC's
    /// padding can exist, arrived reporting 64 and offered a 50-octet payload
    /// to a fixed-width classifier that wants 46. Measured on silicon
    /// 2026-08-08 (`19A`), not inferred: the board's own `lastlen=50`.
    ///
    /// It is fixed **here rather than by subtracting four in the glue**,
    /// because this is where every other bound on this path lives: the MAC
    /// enforces and software does not compensate (`STORY-P1-09-16`). A
    /// software subtraction would encode a hardware behaviour as arithmetic,
    /// and on the day the bit *is* set by anything else the frame would read
    /// four octets **short** and every command would refuse as `Undersize`
    /// instead — the same defect wearing the opposite name.
    pub const DISCARD_RX_FCS: u32 = 1 << 17;
}

/// The four octets of frame check sequence the wire carries and
/// [`ncfgr_receive::DISCARD_RX_FCS`] keeps out of the reported length.
///
/// Named rather than written as a literal `4` so that the one number
/// `LE-122` measured has somewhere to be stated, and so a reader who finds a
/// four-octet disagreement anywhere on this path has a name to search for.
/// **Nothing subtracts it**: it exists to make the MAC's job legible, not to
/// do that job in software.
pub const FRAME_CHECK_SEQUENCE_BYTES: usize = 4;

/// Receive-relevant fields inside `DMACFG`.
pub mod dmacfg_receive {
    /// `RXBS` — receive buffer size, bits `[23:16]`, in units of 64 bytes.
    /// This is the bound the *MAC itself* enforces on a DMA write, which is
    /// why it is programmed before the enable bit and never rounded up.
    pub const RX_BUFFER_SIZE_SHIFT: u32 = 16;
    /// Mask of the [`RX_BUFFER_SIZE_SHIFT`] field.
    pub const RX_BUFFER_SIZE_MASK: u32 = 0xFF << RX_BUFFER_SIZE_SHIFT;
}

/// Bits of a GEM receive descriptor.
pub mod rx_descriptor {
    /// Ownership, bit 0 of the address word: **set means software owns it**,
    /// the opposite polarity from the transmit descriptor's `USED`. The MAC
    /// sets it when it has written a frame; software clears it to re-arm.
    pub const OWNED_BY_SOFTWARE: u32 = 1 << 0;
    /// `WRAP`, bit 1 of the address word — last descriptor in the ring.
    pub const WRAP: u32 = 1 << 1;
    /// The address bits of the address word: everything above the two flags.
    pub const ADDRESS_MASK: u32 = !0b11;
    /// Frame length, bits `[12:0]` of the control word (`GEM_RX_FRMLEN`).
    pub const FRAME_LENGTH_MASK: u32 = 0x1FFF;
    /// `SOF` — this buffer starts the frame.
    pub const START_OF_FRAME: u32 = 1 << 14;
    /// `EOF` — this buffer ends the frame.
    pub const END_OF_FRAME: u32 = 1 << 15;
}

/// Size of the single pinned receive region, bytes. One standard 1518-byte
/// frame with room for the MAC's own alignment, rounded to the 64-byte unit
/// the `RXBS` field counts in — 24 units exactly, so nothing is rounded *up*
/// past what the argument in `STORY-P1-09-16` grants.
pub const RECEIVE_BUFFER_BYTES: usize = 1536;

/// The unit [`dmacfg_receive::RX_BUFFER_SIZE_SHIFT`] counts in.
pub const BUFFER_SIZE_UNIT: usize = 64;

/// The Ethernet header this module skips and never parses.
pub const HEADER_BYTES: usize = 14;

/// The only payload prefix [`admit`] accepts. Six bytes, compared and then
/// forgotten — this is the "bound on what is accepted" and not a protocol.
pub const ENVELOPE_PREFIX: &[u8] = b"TOS64-";

// --- programming the receive path (criterion 1) ------------------------------

/// Encodes `bytes` into the `RXBS` field, or [`None`] when it cannot be
/// expressed. **Refused rather than rounded**: a rounded-up bound is a grant
/// the containment argument did not make, and a rounded-down one silently
/// truncates every frame.
pub const fn buffer_size_code(bytes: usize) -> Option<u32> {
    if bytes == 0 || !bytes.is_multiple_of(BUFFER_SIZE_UNIT) {
        return None;
    }
    let units = bytes / BUFFER_SIZE_UNIT;
    if units > 0xFF {
        return None;
    }
    Some(units as u32)
}

/// The `DMACFG` value that programs 64-bit addressing and the receive
/// buffer-size bound, preserving everything else — a pure read-modify-write
/// plan so the field placement is a pinned test rather than a field guess.
pub const fn dmacfg_for_receive(readback: u32, code: u32) -> u32 {
    let cleared = readback & !dmacfg_receive::RX_BUFFER_SIZE_MASK;
    cleared | crate::gem::dmacfg::ADDR64 | (code << dmacfg_receive::RX_BUFFER_SIZE_SHIFT)
}

/// Splits a MAC address into the `SA1B`/`SA1T` pair, little-endian per the
/// macb layout: bottom carries octets 0..4, top carries octets 4..6.
pub const fn address_filter_words(mac: [u8; 6]) -> (u32, u32) {
    let bottom = (mac[0] as u32)
        | ((mac[1] as u32) << 8)
        | ((mac[2] as u32) << 16)
        | ((mac[3] as u32) << 24);
    let top = (mac[4] as u32) | ((mac[5] as u32) << 8);
    (bottom, top)
}

/// Builds the one-descriptor receive ring in 64-bit (`ADDR64`) layout, or
/// [`None`] when `buffer_dma_address` is not four-byte aligned.
///
/// The alignment refusal is not tidiness. The low two bits of the address word
/// *are* the ownership and wrap flags, so an unaligned address is silently a
/// different address **and** a different ownership state — the one class of
/// mistake here that produces no symptom at all until a device writes
/// somewhere nobody granted.
///
/// One descriptor with `WRAP` set is the smallest ring the MAC can walk: it
/// cannot reach a second address, and it cannot place a second frame until
/// software has explicitly handed this descriptor back ([`rearm`]).
pub const fn receive_ring(buffer_dma_address: u64) -> Option<[u32; 4]> {
    if !buffer_dma_address.is_multiple_of(4) {
        return None;
    }
    Some([
        (buffer_dma_address as u32 & rx_descriptor::ADDRESS_MASK) | rx_descriptor::WRAP,
        0,
        (buffer_dma_address >> 32) as u32,
        0,
    ])
}

/// Hands the single descriptor back to the MAC — the same words
/// [`receive_ring`] builds, because re-arming *is* rebuilding the ring. There
/// is no partial re-arm and no second descriptor to advance to.
pub const fn rearm(buffer_dma_address: u64) -> Option<[u32; 4]> {
    receive_ring(buffer_dma_address)
}

/// Why receive could not be enabled. Every arm leaves the device untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnableError {
    /// The region's size cannot be expressed in the `RXBS` field.
    UnencodableBufferSize,
    /// The ring's DMA address is not four-byte aligned.
    MisalignedRing,
    /// The transmit staging region and the receive region overlap.
    AliasedGrants,
}

/// Whether two granted regions touch at any octet.
///
/// Stated as arithmetic and tested at the edges because "the two statics are
/// obviously separate" is exactly the kind of claim that survives a refactor
/// it stopped being true for, and the symptom on this path is a device write
/// landing in the frame the board is about to transmit.
pub const fn regions_disjoint(a: u64, a_len: usize, b: u64, b_len: usize) -> bool {
    let a_end = a.saturating_add(a_len as u64);
    let b_end = b.saturating_add(b_len as u64);
    a_end <= b || b_end <= a
}

/// Refuses a pair of grants that alias — `LE-67` re-read for the answer path
/// (`STORY-P1-09-17`: "the answer buffer must not alias `RECEIVE_MEMORY`,
/// stated and tested").
///
/// The two directions have opposite writers: the CPU writes the transmit
/// staging region and the device reads it; the device writes the receive
/// region. Aliasing them would let a confused inbound write corrupt the frame
/// the board is about to transmit, turning an inbound fault into an **outbound
/// lie** — and every piece of evidence this project holds is an outbound
/// frame. The answer this Story adds transmits from the *transmit* region, so
/// the claim is one line of arithmetic and it is checked rather than asserted.
pub const fn check_grants(
    transmit: (u64, usize),
    receive: (u64, usize),
) -> Result<(), EnableError> {
    if regions_disjoint(transmit.0, transmit.1, receive.0, receive.1) {
        Ok(())
    } else {
        Err(EnableError::AliasedGrants)
    }
}

/// Arms the receive path, in the one order that is safe.
///
/// Address filter bottom then top; `DMACFG` with 64-bit addressing and the
/// size bound; queue base low then high; stale status cleared; **`NCR.RE`
/// strictly last**.
///
/// The order is the containment, not a style. `RE` set before `RBQP` hands the
/// MAC whatever address that register held at reset and lets it write there;
/// `RE` set before the size field lets it write past the end of a region that
/// is correctly addressed. Both are single-write mistakes with no symptom on a
/// bench where the register happens to read zero, which is why the order is a
/// test (`TEST-P1-09-16-A` clause 1) and not a comment.
///
/// Every refusal happens **before** the first write, so a refused enable
/// leaves the device exactly as it was.
pub fn enable_receive<M: Mmio>(
    device: &M,
    mac: [u8; 6],
    ring_dma_address: u64,
    buffer_bytes: usize,
) -> Result<(), EnableError> {
    let code = match buffer_size_code(buffer_bytes) {
        Some(code) => code,
        None => return Err(EnableError::UnencodableBufferSize),
    };
    if !ring_dma_address.is_multiple_of(4) {
        return Err(EnableError::MisalignedRing);
    }

    // The filter first, and armed by its own second write, so the window in
    // which the MAC is enabled with no address to match never exists.
    let (bottom, top) = address_filter_words(mac);
    device.write_u32(register::SA1B, bottom);
    device.write_u32(register::SA1T, top);

    // The bound the MAC enforces, and 64-bit addressing: system RAM is above
    // any 32-bit address on RP1's bus. Promiscuous is cleared explicitly
    // rather than assumed clear — the filter above is only containment if
    // this bit is down. The FCS strip is set for the same reason the bound is
    // programmed here: the MAC enforces the widths this path believes, and
    // software does not compensate for it afterwards (`LE-122`).
    let ncfgr = device.read_u32(crate::gem::register::NCFGR);
    device.write_u32(
        crate::gem::register::NCFGR,
        (ncfgr & !ncfgr_receive::COPY_ALL_FRAMES & !ncfgr_receive::NO_BROADCAST)
            | ncfgr_receive::DISCARD_RX_FCS,
    );
    let dmacfg = device.read_u32(crate::gem::register::DMACFG);
    device.write_u32(crate::gem::register::DMACFG, dmacfg_for_receive(dmacfg, code));

    device.write_u32(register::RBQP, ring_dma_address as u32);
    device.write_u32(register::RBQPH, (ring_dma_address >> 32) as u32);

    // Write-one-to-clear, so the first poll reads this session's outcome.
    let stale = device.read_u32(register::RSR);
    device.write_u32(register::RSR, stale);

    let control = device.read_u32(crate::gem::register::NCR);
    device.write_u32(crate::gem::register::NCR, control | crate::gem::ncr::RECEIVE_ENABLE);
    Ok(())
}

/// Clears `NCR.RE`. The terminal state of every error arm and the safe state
/// for an input path: **deaf, not retrying**.
pub fn disable_receive<M: Mmio>(device: &M) {
    let control = device.read_u32(crate::gem::register::NCR);
    device.write_u32(crate::gem::register::NCR, control & !crate::gem::ncr::RECEIVE_ENABLE);
}

/// Why the receive path stopped. Each arm is permanent — fail-closed over
/// keep-trying, and unlike the beacon this one also stops *listening*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiveError {
    /// `OVR` — the MAC's receive FIFO overflowed.
    ///
    /// The only terminal receive error, and deliberately the only one. `BNA`
    /// was the second until 2026-08-08; it is now [`ReceiveStatus::dropped`],
    /// a counted drop, because it describes a frame that never entered the
    /// ring rather than a device whose accounting broke (`LE-118`). The
    /// variant is **gone rather than unreachable**: a terminal error that
    /// cannot fire is a taxonomy that lies, and the canvas would advertise a
    /// `stopped reason=nobuffer` state the board can no longer enter.
    Overrun,
}

/// What one status read says: a frame may be waiting, frames may have been
/// dropped for want of a descriptor, or the MAC's own accounting broke.
///
/// Three outcomes rather than two, and the third is the `LE-118` fix. `BNA`
/// used to collapse into [`ReceiveError`] *before* `REC` was even read, so a
/// status of `REC|BNA` — one frame safely in the ring, plus a second the MAC
/// had nowhere to put — discarded the good frame and killed the channel for
/// the boot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReceiveStatus {
    /// `REC`: the MAC signalled a frame this pass.
    pub frame_waiting: bool,
    /// `BNA`: at least one frame arrived with no descriptor free and was
    /// dropped by the MAC. **Counted, not terminal** — see [`read_status`].
    pub dropped: bool,
}

/// Reads and clears the receive status.
///
/// # Why `BNA` is a counted drop and `OVR` is not (`LE-118`, 2026-08-08)
///
/// They are different failures wearing one word. `OVR` is the MAC's receive
/// FIFO overflowing: a frame was torn, the device's own accounting broke, and
/// a descriptor that looks like a whole frame underneath it is exactly the
/// frame least worth believing. That stays terminal, and the safe state for
/// an input path that cannot be trusted is deaf.
///
/// `BNA` is **backpressure**, and it says nothing about the frame in the
/// ring. It means a frame arrived while the single descriptor was still
/// software-owned — which on a one-descriptor ring polled once per park beat
/// is the *designed* consequence of two frames inside one second, not a
/// malfunction. `STORY-P1-09-16` made it terminal believing it pathological;
/// the first capture that could read the row (2026-08-08, once `LE-119` put
/// it on the wire) showed a freshly netbooted board already
/// `stopped reason=nobuffer accepted=0 refused=0` with no host frame sent,
/// because ordinary Windows broadcast puts two frames in a beat routinely.
/// Terminal-on-`BNA` was therefore not a fail-safe posture but a guarantee
/// that the board can never be spoken to on any real segment.
///
/// The drop is **counted and spoken**, never swallowed: a dropped frame that
/// left no trace would be the silent loss this project refuses everywhere
/// else, and the count is also the only measure of how contended the single
/// slot is.
///
/// # Errors
///
/// [`ReceiveError::Overrun`] only. Its status bit is already cleared, and the
/// caller must disable receive permanently.
pub fn read_status<M: Mmio>(device: &M) -> Result<ReceiveStatus, ReceiveError> {
    let status = device.read_u32(register::RSR);
    device.write_u32(register::RSR, status);
    if status & rsr::OVERRUN != 0 {
        return Err(ReceiveError::Overrun);
    }
    Ok(ReceiveStatus {
        frame_waiting: status & rsr::FRAME_RECEIVED != 0,
        dropped: status & rsr::BUFFER_NOT_AVAILABLE != 0,
    })
}

// --- believing the descriptor (criterion 2) ----------------------------------

/// Why a descriptor that claims a frame is not one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorRefusal {
    /// Start-of-frame or end-of-frame is missing: a fragment, and this ring
    /// has no second buffer to continue into.
    NotWholeFrame,
    /// A zero-length frame. Not a thing the wire can carry; a device saying
    /// so is a device to disbelieve.
    ZeroLength,
    /// The reported length exceeds the region the `RXBS` field bounds.
    OverLength,
}

/// What the single descriptor says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorState {
    /// Ownership bit clear: the MAC still owns it, nothing has arrived.
    MacOwns,
    /// A whole frame of `length` bytes is in the buffer.
    Frame {
        /// Bytes the MAC wrote, already bounded by [`RECEIVE_BUFFER_BYTES`].
        length: usize,
    },
    /// The descriptor claims a frame that cannot be one.
    Refused(DescriptorRefusal),
}

/// Classifies the descriptor — total over both words, so there is no input a
/// device can present that this function does not have an answer for.
///
/// [`DescriptorRefusal::OverLength`] is kept even though [`enable_receive`]
/// programs a bound that should make it unreachable. That is the point: it is
/// the assertion that this code does not trust the device to have obeyed the
/// bound it was given, and a classifier that believes the length word is a
/// classifier that indexes out of the buffer the day the device is wrong.
pub const fn classify_descriptor(word0: u32, word1: u32) -> DescriptorState {
    if word0 & rx_descriptor::OWNED_BY_SOFTWARE == 0 {
        return DescriptorState::MacOwns;
    }
    let whole = rx_descriptor::START_OF_FRAME | rx_descriptor::END_OF_FRAME;
    if word1 & whole != whole {
        return DescriptorState::Refused(DescriptorRefusal::NotWholeFrame);
    }
    let length = (word1 & rx_descriptor::FRAME_LENGTH_MASK) as usize;
    if length == 0 {
        return DescriptorState::Refused(DescriptorRefusal::ZeroLength);
    }
    if length > RECEIVE_BUFFER_BYTES {
        return DescriptorState::Refused(DescriptorRefusal::OverLength);
    }
    DescriptorState::Frame { length }
}

// --- admitting the frame (criterion 3) ---------------------------------------

/// Why a frame that arrived was not counted as one of ours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameRefusal {
    /// Shorter than a header plus the envelope prefix.
    TooShort,
    /// The destination is neither broadcast nor this board's own address.
    /// Reachable only if the hardware filter is wrong, and checked anyway.
    NotAddressedHere,
    /// Not the local-experimental EtherType this project uses.
    WrongEtherType,
    /// The payload does not begin [`ENVELOPE_PREFIX`].
    NotAnEnvelope,
}

/// The verdict on one frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// Count it. Nothing else happens to it.
    Accepted,
    /// Count the refusal, by kind.
    Refused(FrameRefusal),
}

/// Decides whether a frame is counted, reading a destination, an EtherType and
/// six payload bytes — **and nothing else, ever**.
///
/// The conditions are checked in a fixed order so the refusal a reader sees is
/// the first thing wrong rather than whichever check happened to run: too
/// short, then not addressed here, then wrong EtherType, then not an envelope.
///
/// What this function deliberately does not do is the whole safety argument of
/// `STORY-P1-09-16`: no field beyond those is read, no length inside the
/// payload is believed, and no value from the frame selects a branch, an
/// address, an offset or a size anywhere in the image.
pub fn admit(frame: &[u8], own_mac: [u8; 6]) -> Admission {
    if frame.len() < HEADER_BYTES + ENVELOPE_PREFIX.len() {
        return Admission::Refused(FrameRefusal::TooShort);
    }
    let destination = &frame[0..6];
    if destination != &[0xFFu8; 6][..] && destination != &own_mac[..] {
        return Admission::Refused(FrameRefusal::NotAddressedHere);
    }
    let ethertype = [(crate::gem::BEACON_ETHERTYPE >> 8) as u8, crate::gem::BEACON_ETHERTYPE as u8];
    if frame[12..14] != ethertype {
        return Admission::Refused(FrameRefusal::WrongEtherType);
    }
    if &frame[HEADER_BYTES..HEADER_BYTES + ENVELOPE_PREFIX.len()] != ENVELOPE_PREFIX {
        return Admission::Refused(FrameRefusal::NotAnEnvelope);
    }
    Admission::Accepted
}

// --- the beat: what one bounded pass does (criterion 6, the re-arm) ----------

/// What one park beat must do with the receive path, decided as a pure
/// function so the two properties that matter are tests rather than readings:
/// **no error arm ever hands the descriptor back**, and the healthy path
/// always does.
///
/// The plan exists because the re-arm is the difference between an ear and a
/// doorbell. A ring of one wrapped descriptor that is never handed back holds
/// exactly one frame for the lifetime of the boot — the MAC has nowhere to put
/// the second one, so an echo that works once is indistinguishable from a
/// fluke (`hand-2026-08-07/10A` §3 S1). It is decided here rather than in the
/// aarch64 glue because the glue is the one part of this path no host test can
/// reach, and "the error arm does not re-arm" is precisely the claim that must
/// not live somewhere unreachable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeatPlan {
    /// Nothing arrived. The descriptor is left exactly as the MAC has it.
    Quiet,
    /// Terminal. Receive is disabled and the descriptor is **never** handed
    /// back — on this pass or any later one.
    Stop(ReceiveError),
    /// The descriptor claims something that cannot be a frame. Count one
    /// refusal by its own name and hand the descriptor back: a device that
    /// wrote nonsense is the premise this Feature contracts, not an error.
    Malformed(DescriptorRefusal),
    /// A whole frame of `length` bytes is in the buffer. Admit it, count the
    /// verdict, hand the descriptor back.
    Classify {
        /// Bytes the MAC wrote, already bounded against [`RECEIVE_BUFFER_BYTES`]
        /// by [`classify_descriptor`] — never the device's own number believed.
        length: usize,
    },
}

impl BeatPlan {
    /// Whether this beat returns the descriptor to the MAC.
    ///
    /// A single predicate rather than a rule spread across the glue's match
    /// arms, so `TEST-P1-09-16-A` clause 10 can state the whole discipline
    /// exhaustively over the plan's four arms.
    pub const fn hands_descriptor_back(self) -> bool {
        match self {
            BeatPlan::Malformed(_) | BeatPlan::Classify { .. } => true,
            BeatPlan::Quiet | BeatPlan::Stop(_) => false,
        }
    }
}

/// Decides one bounded receive pass — total over both inputs.
///
/// An **overrun** outranks the descriptor deliberately: it says the MAC's own
/// accounting broke, and a descriptor that looks like a whole frame
/// underneath one is exactly the frame least worth believing. That refusal is
/// `STORY-P1-09-16` criterion 3's and neither the re-arm nor `LE-118`
/// weakens it.
///
/// A **drop** ([`ReceiveStatus::dropped`]) does not outrank anything, and
/// that is the `LE-118` fix: it is backpressure about a frame that never
/// entered the ring, so it says nothing about the frame that did. The waiting
/// frame is classified and the descriptor handed back exactly as on a quiet
/// beat — which is what keeps the ear alive on a segment where two frames per
/// second is the median rather than a flood.
pub const fn beat_plan(
    status: Result<ReceiveStatus, ReceiveError>,
    descriptor: DescriptorState,
) -> BeatPlan {
    match status {
        Err(error) => BeatPlan::Stop(error),
        Ok(_) => match descriptor {
            DescriptorState::MacOwns => BeatPlan::Quiet,
            DescriptorState::Refused(refusal) => BeatPlan::Malformed(refusal),
            DescriptorState::Frame { length } => BeatPlan::Classify { length },
        },
    }
}

// --- the park loop's view (criterion 3, the reported half) -------------------

/// What the receive channel is doing, for the canvas row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiveState {
    /// Not armed: no link resolved up, so there is nothing to listen on.
    Idle,
    /// Armed and polling.
    Listening,
    /// Permanently stopped on an error; the reason is spoken every beat.
    Stopped(ReceiveError),
    /// Permanently stopped because arming itself was refused.
    Refused(EnableError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gem::{self, ncr};
    use core::cell::RefCell;

    /// A scripted GEM double for the receive path. Records every access in
    /// order and asserts the one absence this Story contracts: no `NCFGR`
    /// write ever sets copy-all-frames.
    struct ScriptedRx {
        reads: RefCell<std::collections::HashMap<usize, Vec<u32>>>,
        steady: std::collections::HashMap<usize, u32>,
        writes: RefCell<Vec<(usize, u32)>>,
    }

    impl ScriptedRx {
        fn new() -> Self {
            ScriptedRx {
                reads: RefCell::new(std::collections::HashMap::new()),
                steady: std::collections::HashMap::new(),
                writes: RefCell::new(Vec::new()),
            }
        }

        fn steady(mut self, offset: usize, value: u32) -> Self {
            self.steady.insert(offset, value);
            self
        }

        fn writes(&self) -> Vec<(usize, u32)> {
            self.writes.borrow().clone()
        }

        fn written_to(&self, offset: usize) -> Vec<u32> {
            self.writes().iter().filter(|(o, _)| *o == offset).map(|(_, v)| *v).collect()
        }
    }

    impl Mmio for ScriptedRx {
        fn read_u32(&self, offset: usize) -> u32 {
            let named = [
                gem::register::NCR,
                gem::register::NCFGR,
                gem::register::DMACFG,
                register::RBQP,
                register::RBQPH,
                register::RSR,
                register::SA1B,
                register::SA1T,
            ];
            assert!(
                named.contains(&offset),
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
            if offset == gem::register::NCFGR {
                assert_eq!(
                    value & ncfgr_receive::COPY_ALL_FRAMES,
                    0,
                    "promiscuous mode was enabled; the hardware address filter is part of \
                     STORY-P1-09-16's containment argument, so turning it off is exactly as \
                     much a defect as pointing the ring at the wrong address"
                );
            }
            self.writes.borrow_mut().push((offset, value));
        }
    }

    // TEST-P1-09-16-A clause 1: the enable order is the containment.

    #[test]
    fn the_enable_order_is_pinned_and_receive_enable_is_strictly_last() {
        let device = ScriptedRx::new();
        assert_eq!(
            enable_receive(
                &device,
                gem::BEACON_SOURCE_MAC,
                0x0000_0010_0008_3000,
                RECEIVE_BUFFER_BYTES
            ),
            Ok(())
        );
        let offsets: Vec<usize> = device.writes().iter().map(|(o, _)| *o).collect();
        assert_eq!(
            offsets,
            vec![
                register::SA1B,
                register::SA1T,
                gem::register::NCFGR,
                gem::register::DMACFG,
                register::RBQP,
                register::RBQPH,
                register::RSR,
                gem::register::NCR,
            ],
            "filter bottom, filter top, config, addressing+bound, queue low, queue high, \
             stale status, THEN enable — the last write is what makes every grant live"
        );
        let last = device.writes().last().copied().expect("at least one write");
        assert_eq!(last.0, gem::register::NCR);
        assert_eq!(last.1 & ncr::RECEIVE_ENABLE, ncr::RECEIVE_ENABLE);
    }

    #[test]
    fn the_queue_base_and_the_size_bound_are_programmed_before_the_enable_bit() {
        let device = ScriptedRx::new();
        let _ = enable_receive(
            &device,
            gem::BEACON_SOURCE_MAC,
            0x0000_0010_0008_3000,
            RECEIVE_BUFFER_BYTES,
        );
        let offsets: Vec<usize> = device.writes().iter().map(|(o, _)| *o).collect();
        let enable = offsets.iter().position(|o| *o == gem::register::NCR).expect("enable write");
        for required in [register::RBQP, register::RBQPH, gem::register::DMACFG] {
            let at = offsets.iter().position(|o| *o == required).expect("programmed");
            assert!(at < enable, "register {required:#x} must be written before NCR.RE");
        }
        assert_eq!(device.written_to(register::RBQP), vec![0x0008_3000]);
        assert_eq!(device.written_to(register::RBQPH), vec![0x0000_0010]);
        let dmacfg = device.written_to(gem::register::DMACFG);
        assert_eq!(dmacfg[0] & gem::dmacfg::ADDR64, gem::dmacfg::ADDR64);
        assert_eq!(
            (dmacfg[0] & dmacfg_receive::RX_BUFFER_SIZE_MASK)
                >> dmacfg_receive::RX_BUFFER_SIZE_SHIFT,
            24,
            "1536 bytes is 24 units of 64"
        );
    }

    #[test]
    fn the_address_filter_is_written_bottom_then_top_and_carries_this_boards_mac() {
        let device = ScriptedRx::new();
        let _ = enable_receive(
            &device,
            gem::BEACON_SOURCE_MAC,
            0x0000_0010_0008_3000,
            RECEIVE_BUFFER_BYTES,
        );
        // 02:54:4F:53:36:34 — bottom is octets 0..4 little-endian.
        assert_eq!(device.written_to(register::SA1B), vec![0x534F_5402]);
        assert_eq!(device.written_to(register::SA1T), vec![0x0000_3436]);
        let offsets: Vec<usize> = device.writes().iter().map(|(o, _)| *o).collect();
        let bottom = offsets.iter().position(|o| *o == register::SA1B).expect("bottom");
        let top = offsets.iter().position(|o| *o == register::SA1T).expect("top");
        assert!(bottom < top, "the top word arms the filter, so it is written second");
    }

    // TEST-P1-09-16-A clause 7: promiscuous is never enabled.

    #[test]
    fn no_path_in_this_module_ever_enables_promiscuous_mode() {
        // A device whose NCFGR already reads back with copy-all-frames set:
        // the read-modify-write must clear it, not preserve it.
        let device = ScriptedRx::new()
            .steady(gem::register::NCFGR, ncfgr_receive::COPY_ALL_FRAMES | 0x0008_0000);
        let _ = enable_receive(
            &device,
            gem::BEACON_SOURCE_MAC,
            0x0000_0010_0008_3000,
            RECEIVE_BUFFER_BYTES,
        );
        let ncfgr = device.written_to(gem::register::NCFGR);
        assert_eq!(ncfgr.len(), 1, "one configuration write, read-modify-write");
        assert_eq!(ncfgr[0] & ncfgr_receive::COPY_ALL_FRAMES, 0);
        assert_eq!(ncfgr[0] & ncfgr_receive::NO_BROADCAST, 0, "broadcast stays acceptable");
        assert_eq!(ncfgr[0] & 0x0008_0000, 0x0008_0000, "unrelated bits survive");
    }

    // TEST-P1-09-16-A clause 2: bounded before believed.

    #[test]
    fn the_buffer_size_bound_is_refused_rather_than_rounded() {
        assert_eq!(buffer_size_code(RECEIVE_BUFFER_BYTES), Some(24));
        assert_eq!(buffer_size_code(64), Some(1));
        assert_eq!(buffer_size_code(0), None, "a zero-size grant is not a small grant");
        assert_eq!(buffer_size_code(1500), None, "not a multiple of 64 — refused, never rounded");
        assert_eq!(buffer_size_code(63), None);
        assert_eq!(buffer_size_code(0xFF * 64), Some(0xFF), "the largest the field can hold");
        assert_eq!(buffer_size_code(0x100 * 64), None, "one unit past the field — refused");
    }

    #[test]
    fn an_unencodable_size_or_a_misaligned_ring_refuses_before_the_first_write() {
        let device = ScriptedRx::new();
        assert_eq!(
            enable_receive(&device, gem::BEACON_SOURCE_MAC, 0x0000_0010_0008_3000, 1500),
            Err(EnableError::UnencodableBufferSize)
        );
        assert_eq!(
            enable_receive(
                &device,
                gem::BEACON_SOURCE_MAC,
                0x0000_0010_0008_3002,
                RECEIVE_BUFFER_BYTES
            ),
            Err(EnableError::MisalignedRing)
        );
        assert!(device.writes().is_empty(), "a refused enable leaves the device exactly as it was");
    }

    #[test]
    fn the_receive_ring_is_one_wrapped_descriptor_owned_by_the_mac() {
        let ring = receive_ring(0x0000_0010_0008_3000).expect("aligned");
        assert_eq!(
            ring[0],
            0x0008_3000 | rx_descriptor::WRAP,
            "address low with WRAP; ownership clear means the MAC owns it"
        );
        assert_eq!(ring[0] & rx_descriptor::OWNED_BY_SOFTWARE, 0);
        assert_eq!(ring[1], 0, "no status yet");
        assert_eq!(ring[2], 0x0000_0010, "address high — the DMA offset is not optional");
        assert_eq!(ring[3], 0);
        assert_eq!(rearm(0x0000_0010_0008_3000), Some(ring), "re-arming is rebuilding the ring");
    }

    #[test]
    fn a_misaligned_buffer_address_is_refused_because_the_low_bits_are_flags() {
        assert_eq!(receive_ring(0x0000_0010_0008_3001), None);
        assert_eq!(receive_ring(0x0000_0010_0008_3002), None);
        assert_eq!(receive_ring(0x0000_0010_0008_3003), None);
        assert!(receive_ring(0x0000_0010_0008_3004).is_some());
    }

    // TEST-P1-09-16-A clause 4: classification is total, refusals distinct.

    #[test]
    fn a_descriptor_the_mac_still_owns_is_not_a_frame() {
        assert_eq!(classify_descriptor(rx_descriptor::WRAP, 0), DescriptorState::MacOwns);
        // Even with a plausible-looking control word.
        let control = rx_descriptor::START_OF_FRAME | rx_descriptor::END_OF_FRAME | 60;
        assert_eq!(classify_descriptor(rx_descriptor::WRAP, control), DescriptorState::MacOwns);
    }

    #[test]
    fn a_whole_frame_reports_its_length() {
        let owned = rx_descriptor::WRAP | rx_descriptor::OWNED_BY_SOFTWARE;
        let control = rx_descriptor::START_OF_FRAME | rx_descriptor::END_OF_FRAME | 60;
        assert_eq!(classify_descriptor(owned, control), DescriptorState::Frame { length: 60 });
    }

    #[test]
    fn a_fragment_a_zero_length_and_an_over_length_are_three_distinct_refusals() {
        let owned = rx_descriptor::WRAP | rx_descriptor::OWNED_BY_SOFTWARE;
        let whole = rx_descriptor::START_OF_FRAME | rx_descriptor::END_OF_FRAME;
        assert_eq!(
            classify_descriptor(owned, rx_descriptor::START_OF_FRAME | 60),
            DescriptorState::Refused(DescriptorRefusal::NotWholeFrame),
            "start without end is a fragment and this ring has nothing to continue into"
        );
        assert_eq!(
            classify_descriptor(owned, rx_descriptor::END_OF_FRAME | 60),
            DescriptorState::Refused(DescriptorRefusal::NotWholeFrame)
        );
        assert_eq!(
            classify_descriptor(owned, whole),
            DescriptorState::Refused(DescriptorRefusal::ZeroLength)
        );
        assert_eq!(
            classify_descriptor(owned, whole | (RECEIVE_BUFFER_BYTES as u32 + 1)),
            DescriptorState::Refused(DescriptorRefusal::OverLength),
            "the device is not trusted to have obeyed the bound it was given"
        );
        assert_eq!(
            classify_descriptor(owned, whole | rx_descriptor::FRAME_LENGTH_MASK),
            DescriptorState::Refused(DescriptorRefusal::OverLength),
            "the widest length the field can express is still refused"
        );
    }

    #[test]
    fn classification_is_total_over_the_length_field() {
        // Every expressible length classifies, and no Frame arm ever escapes
        // the region — the property the glue indexes on.
        let owned = rx_descriptor::WRAP | rx_descriptor::OWNED_BY_SOFTWARE;
        let whole = rx_descriptor::START_OF_FRAME | rx_descriptor::END_OF_FRAME;
        for length in 0..=rx_descriptor::FRAME_LENGTH_MASK {
            match classify_descriptor(owned, whole | length) {
                DescriptorState::Frame { length: reported } => {
                    assert!(reported > 0 && reported <= RECEIVE_BUFFER_BYTES);
                    assert_eq!(reported, length as usize);
                }
                DescriptorState::Refused(_) => {}
                DescriptorState::MacOwns => panic!("ownership was set"),
            }
        }
    }

    // TEST-P1-09-16-A clause 5: admission reads six bytes and interprets none.

    /// A frame the board should count: broadcast, `0x88B5`, `TOS64-` payload.
    fn admissible() -> Vec<u8> {
        let mut frame = vec![0xFFu8; 6];
        frame.extend_from_slice(&[0x02, 0x11, 0x22, 0x33, 0x44, 0x55]);
        frame.extend_from_slice(&[0x88, 0xB5]);
        frame.extend_from_slice(b"TOS64-PING/1 seq=1\n");
        frame
    }

    #[test]
    fn a_broadcast_tos64_frame_is_admitted() {
        assert_eq!(admit(&admissible(), gem::BEACON_SOURCE_MAC), Admission::Accepted);
    }

    #[test]
    fn a_frame_addressed_to_this_board_is_admitted() {
        let mut frame = admissible();
        frame[0..6].copy_from_slice(&gem::BEACON_SOURCE_MAC);
        assert_eq!(admit(&frame, gem::BEACON_SOURCE_MAC), Admission::Accepted);
    }

    #[test]
    fn each_admission_refusal_is_distinct_and_the_order_is_fixed() {
        let mac = gem::BEACON_SOURCE_MAC;
        let short = admissible()[..HEADER_BYTES + ENVELOPE_PREFIX.len() - 1].to_vec();
        assert_eq!(admit(&short, mac), Admission::Refused(FrameRefusal::TooShort));
        assert_eq!(admit(&[], mac), Admission::Refused(FrameRefusal::TooShort));

        let mut elsewhere = admissible();
        elsewhere[0..6].copy_from_slice(&[0x02, 0xDE, 0xAD, 0xBE, 0xEF, 0x00]);
        assert_eq!(admit(&elsewhere, mac), Admission::Refused(FrameRefusal::NotAddressedHere));

        let mut wrong_type = admissible();
        wrong_type[12] = 0x08;
        wrong_type[13] = 0x00; // IPv4
        assert_eq!(admit(&wrong_type, mac), Admission::Refused(FrameRefusal::WrongEtherType));

        let mut not_ours = admissible();
        not_ours[HEADER_BYTES..HEADER_BYTES + 6].copy_from_slice(b"TINYOS");
        assert_eq!(admit(&not_ours, mac), Admission::Refused(FrameRefusal::NotAnEnvelope));

        // Order: a frame wrong in two ways reports the first check that failed.
        let mut both = elsewhere.clone();
        both[12] = 0x08;
        both[13] = 0x00;
        assert_eq!(admit(&both, mac), Admission::Refused(FrameRefusal::NotAddressedHere));
    }

    #[test]
    fn admission_is_indifferent_to_every_byte_it_does_not_name() {
        // The payload past the prefix is varied across its whole range; the
        // verdict may not move. This is the "interprets nothing" claim as a
        // test rather than as a sentence in a doc comment.
        let base = admissible();
        for fill in [0x00u8, 0x01, 0x7F, 0x80, 0xFF] {
            let mut frame = base.clone();
            for byte in frame.iter_mut().skip(HEADER_BYTES + ENVELOPE_PREFIX.len()) {
                *byte = fill;
            }
            assert_eq!(
                admit(&frame, gem::BEACON_SOURCE_MAC),
                Admission::Accepted,
                "payload fill {fill:#04x} changed the verdict"
            );
        }
        // And the source MAC is never consulted.
        for fill in [0x00u8, 0xFF] {
            let mut frame = base.clone();
            frame[6..12].copy_from_slice(&[fill; 6]);
            assert_eq!(admit(&frame, gem::BEACON_SOURCE_MAC), Admission::Accepted);
        }
    }

    // TEST-P1-09-16-A clause 6: fail-closed, and every error arm terminal.

    const QUIET: ReceiveStatus = ReceiveStatus { frame_waiting: false, dropped: false };
    const FRAME: ReceiveStatus = ReceiveStatus { frame_waiting: true, dropped: false };

    #[test]
    fn a_quiet_pass_reports_no_frame_and_leaves_receive_enabled() {
        let device = ScriptedRx::new().steady(register::RSR, 0);
        assert_eq!(read_status(&device), Ok(QUIET));
        assert!(
            device.written_to(gem::register::NCR).is_empty(),
            "nothing happened, so nothing was disabled"
        );
    }

    #[test]
    fn a_signalled_frame_clears_its_status_bit() {
        let device = ScriptedRx::new().steady(register::RSR, rsr::FRAME_RECEIVED);
        assert_eq!(read_status(&device), Ok(FRAME));
        assert_eq!(
            device.written_to(register::RSR),
            vec![rsr::FRAME_RECEIVED],
            "write-one-to-clear, so the next poll reads the next pass"
        );
    }

    #[test]
    fn an_overrun_is_terminal_and_outranks_a_frame_in_the_buffer() {
        let device = ScriptedRx::new().steady(register::RSR, rsr::OVERRUN);
        assert_eq!(read_status(&device), Err(ReceiveError::Overrun));
        assert_eq!(
            device.written_to(register::RSR),
            vec![rsr::OVERRUN],
            "cleared even on the error"
        );

        // An overrun that arrives alongside a received frame is still an
        // overrun: the frame in the buffer may be the truncated one.
        let device = ScriptedRx::new().steady(register::RSR, rsr::OVERRUN | rsr::FRAME_RECEIVED);
        assert_eq!(read_status(&device), Err(ReceiveError::Overrun));
    }

    // `LE-118`, and the fix's whole point: `BNA` is backpressure about a
    // frame that never entered the ring, so it is counted, and it says
    // nothing about the frame that did.

    #[test]
    fn a_buffer_not_available_is_a_counted_drop_and_not_terminal() {
        let device = ScriptedRx::new().steady(register::RSR, rsr::BUFFER_NOT_AVAILABLE);
        assert_eq!(read_status(&device), Ok(ReceiveStatus { frame_waiting: false, dropped: true }));
        assert!(
            device.written_to(gem::register::NCR).is_empty(),
            "a dropped frame does not disable the ear"
        );
        assert_eq!(
            device.written_to(register::RSR),
            vec![rsr::BUFFER_NOT_AVAILABLE],
            "and the bit is still cleared, so the next beat reads the next pass"
        );
    }

    /// **The status word that killed the channel**, and the reason the ear was
    /// deaf on arrival: one frame safely in the ring plus a second the MAC had
    /// nowhere to put. On this bench that is the median beat, not an edge case
    /// — and it used to discard the good frame and stop receive for the boot.
    #[test]
    fn a_frame_arriving_beside_a_drop_is_kept_and_the_drop_is_counted() {
        let device = ScriptedRx::new()
            .steady(register::RSR, rsr::FRAME_RECEIVED | rsr::BUFFER_NOT_AVAILABLE);
        assert_eq!(read_status(&device), Ok(ReceiveStatus { frame_waiting: true, dropped: true }));
        assert!(device.written_to(gem::register::NCR).is_empty(), "the ear survives");
    }

    /// The plan-level statement of the same property: a drop classifies and
    /// re-arms exactly as a quiet beat does, and only an overrun stops.
    #[test]
    fn a_drop_still_classifies_the_waiting_frame_and_hands_the_descriptor_back() {
        let dropped = ReceiveStatus { frame_waiting: true, dropped: true };
        let plan = beat_plan(Ok(dropped), DescriptorState::Frame { length: 64 });
        assert_eq!(plan, BeatPlan::Classify { length: 64 });
        assert!(plan.hands_descriptor_back(), "the ear is re-armed for the next beat");
        // And the terminal arm is untouched by any of it.
        assert_eq!(
            beat_plan(Err(ReceiveError::Overrun), DescriptorState::Frame { length: 64 }),
            BeatPlan::Stop(ReceiveError::Overrun)
        );
        assert!(!beat_plan(Err(ReceiveError::Overrun), DescriptorState::MacOwns)
            .hands_descriptor_back());
    }

    #[test]
    fn disabling_receive_clears_only_the_receive_bit() {
        let device = ScriptedRx::new()
            .steady(gem::register::NCR, ncr::RECEIVE_ENABLE | ncr::TRANSMIT_ENABLE);
        disable_receive(&device);
        let written = device.written_to(gem::register::NCR);
        assert_eq!(written.len(), 1);
        assert_eq!(written[0] & ncr::RECEIVE_ENABLE, 0, "deaf is the safe state for an input path");
        assert_eq!(
            written[0] & ncr::TRANSMIT_ENABLE,
            ncr::TRANSMIT_ENABLE,
            "the board keeps speaking; only listening stops"
        );
    }

    /// `TEST-P1-09-16-A` clause 8's host half, and the reason criterion 4 can
    /// be *predicted* rather than discovered on a bench: **the exact frames
    /// `ti64dink --send` transmits, each asserted against the verdict its own
    /// arm table claims [`admit`] will return.**
    ///
    /// Without this the sender's expectations live in its prose and the filter
    /// lives here, and the two can drift apart with no symptom until an
    /// operator is standing over a powered board wondering which of them is
    /// wrong. The fixture is generated from the arm table by
    /// `ti64dink --send-frames`, so the bytes that go on the wire and the bytes
    /// this test asserts are one copy — the `LE-80` mirror shape, and the same
    /// trick [`crate::gem::tests`]' captured-beacon test uses in the outbound
    /// direction.
    ///
    /// `notforus` records the **software** verdict. On hardware that frame
    /// should never reach this function at all, because the GEM's address
    /// filter drops it before DMA; the board session asserts that half by both
    /// canvas counters staying still, and no host test can.
    #[test]
    fn every_frame_the_host_sender_transmits_gets_the_verdict_its_arm_predicts() {
        const ARMS: &str = include_str!("../../../../goals/reports/rx-arms-2026-08-06.txt");

        let mut checked = 0;
        for line in ARMS.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut fields = line.split(' ');
            let arm = fields.next().expect("<arm> <verdict> <hex>");
            let verdict = fields.next().expect("<arm> <verdict> <hex>");
            let hex = fields.next().expect("<arm> <verdict> <hex>");
            assert_eq!(hex.len() % 2, 0, "{arm}: a whole number of octets");
            let frame: Vec<u8> = (0..hex.len() / 2)
                .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).expect("hex octet"))
                .collect();

            let expected = match verdict {
                "Accepted" => Admission::Accepted,
                "TooShort" => Admission::Refused(FrameRefusal::TooShort),
                "NotAddressedHere" => Admission::Refused(FrameRefusal::NotAddressedHere),
                "WrongEtherType" => Admission::Refused(FrameRefusal::WrongEtherType),
                "NotAnEnvelope" => Admission::Refused(FrameRefusal::NotAnEnvelope),
                other => {
                    panic!("{arm}: the fixture names a verdict this filter has no arm for: {other}")
                }
            };
            assert_eq!(
                admit(&frame, gem::BEACON_SOURCE_MAC),
                expected,
                "arm `{arm}`: the sender predicts {verdict} and the board's filter disagrees"
            );
            checked += 1;
        }

        assert_eq!(checked, 5, "every arm must be checked, not merely parsed");
        // Both halves of criterion 4 are actually present in the fixture. An
        // all-accepting arm table would pass every assertion above and prove
        // only that the board can hear.
        let accepted = ARMS.lines().filter(|l| l.contains(" Accepted ")).count();
        assert!(accepted >= 1, "no arm exercises the accepting half");
        assert!(checked - accepted >= 1, "no arm exercises the declining half");
    }

    // TEST-P1-09-17-A clause 5: the answer path does not alias the region the
    // device writes (`LE-67`, re-read rather than inherited).

    #[test]
    fn two_grants_that_touch_at_all_are_not_disjoint_and_the_edges_are_the_cases() {
        // Adjacent, in both orders: the commonest real layout, and it is fine.
        assert!(regions_disjoint(0x1000, 0x100, 0x1100, 0x100));
        assert!(regions_disjoint(0x1100, 0x100, 0x1000, 0x100));
        // One octet of overlap, in both orders. This is the case a hand-read
        // of two `static mut`s would miss and a device would find.
        assert!(!regions_disjoint(0x1000, 0x101, 0x1100, 0x100));
        assert!(!regions_disjoint(0x1100, 0x100, 0x1000, 0x101));
        // Containment, identity, and the degenerate empty grant.
        assert!(!regions_disjoint(0x1000, 0x1000, 0x1400, 0x10));
        assert!(!regions_disjoint(0x1000, 0x10, 0x1000, 0x10));
        assert!(regions_disjoint(0x1000, 0, 0x1000, 0x10), "an empty grant reaches nothing");
    }

    #[test]
    fn the_two_pinned_grants_this_feature_holds_are_asserted_disjoint_not_assumed() {
        // The transmit staging region and the receive region have opposite
        // writers, and an inbound write that landed in the outbound frame
        // would turn a device fault into an outbound lie — every piece of
        // evidence this project holds is an outbound frame. So the arming
        // path refuses rather than trusting the linker's placement.
        let receive = 0x8000u64;
        let transmit_below =
            (receive - crate::gem::SPOOR_FRAME_CAPACITY as u64, crate::gem::SPOOR_FRAME_CAPACITY);
        assert!(regions_disjoint(
            transmit_below.0,
            transmit_below.1,
            receive,
            RECEIVE_BUFFER_BYTES
        ));
        assert!(!regions_disjoint(receive + 8, 64, receive, RECEIVE_BUFFER_BYTES));
    }

    #[test]
    fn an_aliased_pair_of_grants_is_a_named_refusal_the_arming_path_can_report() {
        let receive = (0x0008_3000u64, RECEIVE_BUFFER_BYTES);
        assert_eq!(check_grants((0x0008_0000, 0x1000), receive), Ok(()));
        assert_eq!(
            check_grants((receive.0 + 16, 64), receive),
            Err(EnableError::AliasedGrants),
            "an aliased grant is a refusal the canvas can name, not a comment"
        );
    }

    // TEST-P1-09-16-A clause 10: the beat plan — the ear stays armed, and no
    // error arm ever re-arms anything.

    /// Every descriptor state, for the exhaustive arms below.
    fn every_descriptor_state() -> [DescriptorState; 5] {
        [
            DescriptorState::MacOwns,
            DescriptorState::Frame { length: 60 },
            DescriptorState::Refused(DescriptorRefusal::NotWholeFrame),
            DescriptorState::Refused(DescriptorRefusal::ZeroLength),
            DescriptorState::Refused(DescriptorRefusal::OverLength),
        ]
    }

    #[test]
    fn a_quiet_beat_leaves_the_descriptor_exactly_where_the_mac_has_it() {
        for signalled in [false, true] {
            let plan = beat_plan(
                Ok(ReceiveStatus { frame_waiting: signalled, dropped: false }),
                DescriptorState::MacOwns,
            );
            assert_eq!(plan, BeatPlan::Quiet, "nothing arrived, so nothing is handed back");
            assert!(!plan.hands_descriptor_back());
        }
    }

    #[test]
    fn a_whole_frame_is_classified_and_the_descriptor_is_handed_back() {
        let plan = beat_plan(Ok(FRAME), DescriptorState::Frame { length: 60 });
        assert_eq!(
            plan,
            BeatPlan::Classify { length: 60 },
            "the length the classifier bounded, carried through unchanged"
        );
        assert!(
            plan.hands_descriptor_back(),
            "an ear that hears once and never re-arms is a doorbell (hand-2026-08-07/10A S1)"
        );
    }

    #[test]
    fn a_malformed_descriptor_is_counted_by_its_own_name_and_the_ear_stays_open() {
        for refusal in [
            DescriptorRefusal::NotWholeFrame,
            DescriptorRefusal::ZeroLength,
            DescriptorRefusal::OverLength,
        ] {
            let plan = beat_plan(Ok(FRAME), DescriptorState::Refused(refusal));
            assert_eq!(
                plan,
                BeatPlan::Malformed(refusal),
                "the descriptor refusal keeps its own name; it is not relabelled TooShort"
            );
            assert!(
                plan.hands_descriptor_back(),
                "a device that wrote nonsense once is the premise, not an error condition"
            );
        }
    }

    #[test]
    fn no_error_arm_re_arms_anything_on_that_pass_or_any_later_one() {
        // The exhaustive statement of TEST-P1-09-16-A clause 6 against the
        // re-arm added by clause 10: whatever the descriptor happens to say,
        // a terminal status is terminal and hands nothing back.
        for error in [ReceiveError::Overrun] {
            for descriptor in every_descriptor_state() {
                let plan = beat_plan(Err(error), descriptor);
                assert_eq!(
                    plan,
                    BeatPlan::Stop(error),
                    "a terminal error outranks every descriptor state, including a whole frame"
                );
                assert!(
                    !plan.hands_descriptor_back(),
                    "the re-arm is for the healthy path only — deaf is the safe state"
                );
            }
        }
    }

    #[test]
    fn the_beat_plan_is_total_and_exactly_the_healthy_arms_hand_the_descriptor_back() {
        let mut handed_back = 0;
        let mut kept = 0;
        // All four status shapes: quiet, a frame, a drop, a frame beside a
        // drop — plus the one terminal error. `LE-118` is why the drop arms
        // are here and why `BufferUnavailable` is not.
        for status in [
            Ok(QUIET),
            Ok(FRAME),
            Ok(ReceiveStatus { frame_waiting: false, dropped: true }),
            Ok(ReceiveStatus { frame_waiting: true, dropped: true }),
            Err(ReceiveError::Overrun),
        ] {
            for descriptor in every_descriptor_state() {
                let plan = beat_plan(status, descriptor);
                // Totality: every input has an answer, and the answer's
                // hand-back decision is a property of the plan alone.
                match plan {
                    BeatPlan::Quiet | BeatPlan::Stop(_) => {
                        assert!(!plan.hands_descriptor_back());
                        kept += 1;
                    }
                    BeatPlan::Classify { .. } | BeatPlan::Malformed(_) => {
                        assert!(plan.hands_descriptor_back());
                        handed_back += 1;
                    }
                }
            }
        }
        assert_eq!(handed_back + kept, 5 * 5, "every combination classified");
        assert!(handed_back > 0 && kept > 0, "both halves are reachable");
    }

    #[test]
    fn the_hand_back_preserves_the_address_and_the_wrap_bit_and_returns_ownership() {
        let address = 0x0000_0010_0008_3000u64;
        let armed = receive_ring(address).expect("aligned");
        let handed_back = rearm(address).expect("aligned");
        assert_eq!(handed_back, armed, "re-arming is rebuilding the same ring");
        assert_eq!(
            handed_back[0] & rx_descriptor::ADDRESS_MASK,
            armed[0] & rx_descriptor::ADDRESS_MASK,
            "the address is preserved — a hand-back that moved the buffer is a new grant"
        );
        assert_eq!(
            handed_back[0] & rx_descriptor::WRAP,
            rx_descriptor::WRAP,
            "WRAP is kept: without it the MAC walks to a second address nobody granted"
        );
        assert_eq!(
            handed_back[0] & rx_descriptor::OWNED_BY_SOFTWARE,
            0,
            "ownership goes back to the MAC — that is what makes the ear an ear"
        );
        assert_eq!(handed_back[1], 0, "the status word is cleared with the hand-back");
    }

    #[test]
    fn a_beat_hands_back_at_most_one_descriptor_and_the_next_beat_is_quiet() {
        // One frame classified, one descriptor re-armed, per beat. The second
        // frame does not exist until the MAC has somewhere to put it, so the
        // beat immediately after a hand-back reads the descriptor the MAC
        // owns again — a bound rather than a hope.
        let first = beat_plan(Ok(FRAME), DescriptorState::Frame { length: 60 });
        assert!(first.hands_descriptor_back());
        let next =
            beat_plan(Ok(QUIET), classify_descriptor(rearm(0x0008_3000).expect("aligned")[0], 0));
        assert_eq!(next, BeatPlan::Quiet);
    }

    // LE-122: the length the descriptor reports must be the length the host
    // sent, and the MAC is what makes that true.

    #[test]
    fn the_frame_check_sequence_is_stripped_by_the_mac_and_never_reaches_a_length() {
        // A device whose NCFGR reads back with the strip bit clear — which is
        // how this bench's silicon was found on 2026-08-08 — must be written
        // with it set.
        let device = ScriptedRx::new().steady(gem::register::NCFGR, 0x0008_0000);
        let _ = enable_receive(
            &device,
            gem::BEACON_SOURCE_MAC,
            0x0000_0010_0008_3000,
            RECEIVE_BUFFER_BYTES,
        );
        let ncfgr = device.written_to(gem::register::NCFGR);
        assert_eq!(ncfgr.len(), 1, "still one configuration write, read-modify-write");
        assert_eq!(
            ncfgr[0] & ncfgr_receive::DISCARD_RX_FCS,
            ncfgr_receive::DISCARD_RX_FCS,
            "LE-122: with DRFCS clear the descriptor's length includes the four-octet FCS, \
             so a 60-octet host frame offers a 50-octet payload to a classifier that wants 46 \
             and every command refuses as oversize before the verb table is consulted"
        );
        assert_eq!(ncfgr[0] & ncfgr_receive::COPY_ALL_FRAMES, 0, "and the filter still stands");
        assert_eq!(ncfgr[0] & 0x0008_0000, 0x0008_0000, "unrelated bits still survive");
    }

    #[test]
    fn a_minimum_ethernet_frame_offers_exactly_the_command_envelope_after_the_strip() {
        // The arithmetic LE-122 turned from a mystery into a number, stated
        // where a later reader can see both halves at once. The host builds
        // 14 + 46 = 60 octets; the wire adds a four-octet FCS; the MAC strips
        // it, so the descriptor reports 60 and the payload is 46 — not 50.
        let on_the_wire = crate::gem::MINIMUM_FRAME_LEN + FRAME_CHECK_SEQUENCE_BYTES;
        assert_eq!(on_the_wire, 64);
        let reported = on_the_wire - FRAME_CHECK_SEQUENCE_BYTES;
        assert_eq!(reported, crate::gem::MINIMUM_FRAME_LEN);
        assert_eq!(
            reported - HEADER_BYTES,
            crate::tos64_cmd::COMMAND_PAYLOAD_BYTES,
            "the fixed-width envelope is measured against a length that excludes the FCS"
        );
    }

    #[test]
    fn the_register_offsets_are_the_macb_transcriptions() {
        assert_eq!(register::RBQP, 0x0018);
        assert_eq!(register::RSR, 0x0020);
        assert_eq!(register::SA1B, 0x0088);
        assert_eq!(register::SA1T, 0x008C);
        assert_eq!(register::RBQPH, 0x04D4);
        // NCFGR bit positions, transcribed rather than recalled: macb.h gives
        // MACB_CAF_OFFSET 4, MACB_NBC_OFFSET 5, MACB_DRFCS_OFFSET 17.
        assert_eq!(ncfgr_receive::COPY_ALL_FRAMES, 1 << 4);
        assert_eq!(ncfgr_receive::NO_BROADCAST, 1 << 5);
        assert_eq!(ncfgr_receive::DISCARD_RX_FCS, 1 << 17);
        // Between RLCE (16) and the GEM MDC divisor field (18..21), and clear
        // of every bit this path or the transmit path writes.
        assert_eq!(
            ncfgr_receive::DISCARD_RX_FCS & crate::gem::ncfgr::MDC_DIVISOR_MASK,
            0,
            "the strip bit must not land in the MDC divisor field"
        );
        for other in [
            crate::gem::ncfgr::SPEED_100,
            crate::gem::ncfgr::FULL_DUPLEX,
            crate::gem::ncfgr::GIGABIT,
            ncfgr_receive::COPY_ALL_FRAMES,
            ncfgr_receive::NO_BROADCAST,
        ] {
            assert_eq!(ncfgr_receive::DISCARD_RX_FCS & other, 0);
        }
        assert_eq!(FRAME_CHECK_SEQUENCE_BYTES, 4);
        for offset in
            [register::RBQP, register::RSR, register::SA1B, register::SA1T, register::RBQPH]
        {
            assert!(offset < crate::board::RP1_GEM_SIZE);
            assert_eq!(offset % 4, 0);
        }
        // The region is a whole number of the units the field counts in.
        assert_eq!(RECEIVE_BUFFER_BYTES % BUFFER_SIZE_UNIT, 0);
    }
}
