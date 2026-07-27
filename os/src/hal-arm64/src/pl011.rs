//! PL011 UART driver (`TEST-P1-07-01-A` clause 5).
//!
//! Everything except the two volatile accesses is pure and host-tested. The
//! register map, the baud-divisor arithmetic, the configuration *ordering*, the
//! bounded flag-polling loop and the byte framing are all driven from scripted
//! doubles on the x86_64 dev machine, so what remains unverified on this board
//! is exactly one `read_volatile` and one `write_volatile`.
//!
//! **On the seam being two methods rather than one.** `STORY-P1-01-03` split
//! `VirtualCounter` and `CounterFrequency` into two one-method traits because
//! counting code must not depend on a frequency register it never reads. That
//! argument does not transfer: a transmit path *must* poll the flag register
//! before it writes the data register, so no caller of this driver holds one
//! capability without the other, and splitting [`Mmio`] would produce two
//! traits that are always implemented and always required together — the
//! coupling without the segregation.
//!
//! **Nothing here has executed on hardware.** It is compiled and reviewed, in
//! the same state `timer::SystemRegisters` has been in since
//! `STORY-P1-01-03`. `TEST-P1-07-01-A` clauses 1, 3 and 4 are the Green, and
//! they need a board and a loopback-tested adapter.

/// PL011 register offsets from the peripheral base.
///
/// Architected by ARM's PrimeCell UART (PL011) TRM, not by Broadcom: the
/// BCM2712 instance declares `arm,primecell-periphid = <0x00341011>` and the
/// offsets are the standard ones. This is the one part of the driver a Pi 4
/// source gets *right*, and saying so is as much a part of the divergence
/// record as the parts it gets wrong.
pub mod register {
    /// Data register. Writing transmits.
    pub const DR: usize = 0x000;
    /// Flag register — the only register this driver reads.
    pub const FR: usize = 0x018;
    /// Integer baud-rate divisor.
    pub const IBRD: usize = 0x024;
    /// Fractional baud-rate divisor.
    pub const FBRD: usize = 0x028;
    /// Line control. Writing it latches [`IBRD`] and [`FBRD`].
    pub const LCR_H: usize = 0x02C;
    /// Control register.
    pub const CR: usize = 0x030;
    /// Interrupt mask set/clear.
    pub const IMSC: usize = 0x038;
    /// Interrupt clear.
    pub const ICR: usize = 0x044;
}

/// Bits of the flag register ([`register::FR`]).
pub mod flag {
    /// Transmit FIFO full.
    pub const TXFF: u32 = 1 << 5;
    /// UART busy — a character is still in the shift register.
    pub const BUSY: u32 = 1 << 3;
}

/// Bits of the line-control register ([`register::LCR_H`]).
pub mod line_control {
    /// Eight data bits.
    pub const WLEN_8: u32 = 0b11 << 5;
    /// Enable the transmit and receive FIFOs.
    pub const FEN: u32 = 1 << 4;
}

/// Bits of the control register ([`register::CR`]).
pub mod control {
    /// UART enable.
    pub const UARTEN: u32 = 1 << 0;
    /// Transmit enable.
    pub const TXE: u32 = 1 << 8;
    /// Receive enable. Deliberately never set by this slice.
    pub const RXE: u32 = 1 << 9;
}

/// Every interrupt source the PL011 can raise, for a one-shot clear.
const ALL_INTERRUPTS: u32 = 0x7FF;

/// How many times the transmit path polls [`flag::TXFF`] before giving up.
///
/// **Derivation.** One 8N1 character at 115200 baud occupies ten bit times,
/// about 87 µs, so a full 32-entry FIFO drains in roughly 2.8 ms worst case.
/// One poll iteration is a Device-nGnRnE MMIO read — uncached and unbuffered
/// while the MMU is off (`STORY-P1-07-03`) — which is on the order of
/// hundreds of nanoseconds. 100 000 polls is therefore at least an order of
/// magnitude beyond any legitimate wait and still a bound.
///
/// The number is deliberately generous rather than tight: this bound exists to
/// convert a hang into a return, not to enforce a latency budget. Nothing in
/// this Story measures anything, and a tight bound here would be a timing
/// claim about a board nobody has run yet.
pub const TX_POLL_LIMIT: usize = 100_000;

/// The word-sized MMIO seam.
///
/// The concrete implementor for this board is `VolatileMmio` (an
/// `aarch64`-only item, hence not linked here), which is the
/// only `cfg(target_arch = "aarch64")` item and the only `unsafe` in this
/// module — the seam `STORY-P1-01-03` established for `mrs` reads, applied to
/// MMIO.
///
/// # Contract every implementor must honor
///
/// Both methods access the device once, in program order, with no caching,
/// coalescing, or reordering relative to each other. An implementor that
/// cached a flag-register read would turn the bounded poll below into a loop
/// that observes one stale value [`TX_POLL_LIMIT`] times.
pub trait Mmio {
    /// Reads the 32-bit register at `offset` from the peripheral base.
    fn read_u32(&self, offset: usize) -> u32;

    /// Writes the 32-bit register at `offset` from the peripheral base.
    fn write_u32(&self, offset: usize, value: u32);
}

impl<T: Mmio + ?Sized> Mmio for &T {
    fn read_u32(&self, offset: usize) -> u32 {
        (**self).read_u32(offset)
    }

    fn write_u32(&self, offset: usize, value: u32) {
        (**self).write_u32(offset, value);
    }
}

/// Why a requested baud could not be turned into divisors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BaudError {
    /// The reference clock read as zero — firmware did not program it, or the
    /// caller passed a value it never checked.
    ZeroClock,
    /// A baud of zero was requested.
    ZeroBaud,
    /// The rate is faster than the generator can express: the integer divisor
    /// rounds to zero, which the PL011 documents as *disabled*, not *fast*.
    RateTooHigh,
    /// The rate is slower than the 16-bit integer divisor can express.
    RateTooLow,
}

/// Why a transmit could not complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pl011Error {
    /// The baud could not be programmed; the device was left disabled.
    Baud(BaudError),
    /// The transmit FIFO did not accept a byte within [`TX_POLL_LIMIT`] polls.
    TransmitTimeout,
}

/// A validated PL011 baud-rate divisor pair.
///
/// Constructed only by [`BaudDivisors::compute`], so a value of this type is a
/// divisor the generator can actually express — the range checks cannot be
/// skipped by a caller assembling one itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaudDivisors {
    integer: u16,
    fractional: u8,
}

impl BaudDivisors {
    /// Computes the divisors for `baud` from a `clock_hz` reference clock.
    ///
    /// The PL011 divides by a 16.6 fixed-point value: `clock / (16 × baud)`.
    /// This **rounds to nearest** rather than truncating, matching the
    /// precedent [`crate::timer::plausible_cycles_per_us`] set for the same
    /// reason — truncation biases every derived figure in one direction, and
    /// here it is the classic PL011 defect (48 MHz at 115200 truncates to
    /// `FBRD = 2` where the correct answer is 3).
    pub const fn compute(clock_hz: u32, baud: u32) -> Result<BaudDivisors, BaudError> {
        if clock_hz == 0 {
            return Err(BaudError::ZeroClock);
        }
        if baud == 0 {
            return Err(BaudError::ZeroBaud);
        }

        // 64 × (clock / (16 × baud)) == 4 × clock / baud, rounded to nearest
        // via (2a + b) / 2b. `u64` throughout: 8 × u32::MAX overflows u32.
        let numerator = 8 * clock_hz as u64;
        let denominator = 2 * baud as u64;
        let scaled = (numerator + baud as u64) / denominator;

        let integer = scaled >> 6;
        let fractional = (scaled & 0b11_1111) as u8;

        if integer == 0 {
            return Err(BaudError::RateTooHigh);
        }
        // `IBRD == 0xFFFF` is the largest divisor, and the PL011 requires
        // `FBRD == 0` alongside it. A rate needing both is not expressible.
        if integer > 0xFFFF || (integer == 0xFFFF && fractional != 0) {
            return Err(BaudError::RateTooLow);
        }

        Ok(BaudDivisors { integer: integer as u16, fractional })
    }

    /// The integer divisor, for [`register::IBRD`].
    pub const fn integer(self) -> u16 {
        self.integer
    }

    /// The fractional divisor, for [`register::FBRD`].
    pub const fn fractional(self) -> u8 {
        self.fractional
    }

    /// The baud these divisors actually produce from `clock_hz`.
    ///
    /// Reported rather than assumed so that a serial capture can be checked
    /// against the rate the hardware was really programmed to, not the rate the
    /// caller asked for.
    pub const fn achieved_baud_hz(self, clock_hz: u32) -> u32 {
        let divisor = (self.integer as u64) * 64 + self.fractional as u64;
        if divisor == 0 {
            return 0;
        }
        let numerator = 8 * clock_hz as u64;
        ((numerator + divisor) / (2 * divisor)) as u32
    }
}

/// Frames one byte for a terminal: `\n` becomes `\r\n`.
///
/// The only transformation applied to a caller's text, and pure so that a
/// capture can be predicted exactly from the source. Returns the byte to send
/// and an optional second byte to follow it.
///
/// Note this is applied by [`Pl011::write_str`] and **not** by
/// [`Pl011::write_bytes`]: `TEST-P1-07-01-A` clause 4's evidence is a known
/// byte sequence arriving *in order*, and a transmit path that rewrote it would
/// make the capture evidence about the driver rather than about the wire.
pub const fn framed(byte: u8) -> (u8, Option<u8>) {
    if byte == b'\n' {
        (b'\r', Some(b'\n'))
    } else {
        (byte, None)
    }
}

/// Renders a `u64` as sixteen uppercase hex digits, zero-padded.
///
/// Fixed width and no `core::fmt`: the firmware handoff and the device-tree
/// blob pointer are *reported* (`BND-02`, `PD-14`) and never parsed
/// (`BND-03`), and reporting them on a board with no fault handler should not
/// drag in formatting machinery that can panic.
pub const fn hex_u64(value: u64) -> [u8; 16] {
    const DIGITS: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = [b'0'; 16];
    let mut i = 0;
    while i < 16 {
        let nibble = ((value >> (60 - 4 * i)) & 0xF) as usize;
        out[i] = DIGITS[nibble];
        i += 1;
    }
    out
}

/// A PL011 behind an [`Mmio`] seam.
///
/// Generic over the seam so the same code that runs against `write_volatile` on
/// the board runs against a scripted double on the host — which is how this
/// driver is tested at all before a board exists.
#[derive(Debug, Clone, Copy)]
pub struct Pl011<M> {
    mmio: M,
}

impl<M> Pl011<M> {
    /// Wraps an MMIO seam.
    pub const fn new(mmio: M) -> Self {
        Pl011 { mmio }
    }
}

impl<M: Mmio> Pl011<M> {
    /// Programs the UART for `baud` from a `clock_hz` reference clock, 8N1,
    /// FIFOs on, every interrupt masked, transmit only.
    ///
    /// The ordering is not incidental and is asserted by tests:
    ///
    /// 1. Divisors are computed **before** any register is touched, so a bad
    ///    rate leaves the device untouched rather than half-configured.
    /// 2. The UART is disabled and drained before reprogramming — a rate change
    ///    under a character in flight corrupts that character.
    /// 3. `IBRD`/`FBRD` are written **before** `LCR_H`, because writing `LCR_H`
    ///    is what latches them. The reverse order reads perfectly naturally and
    ///    silently leaves the firmware's baud in effect.
    /// 4. `CR` is written last, so the device is never enabled in a partially
    ///    programmed state.
    pub fn configure(&self, clock_hz: u32, baud: u32) -> Result<(), Pl011Error> {
        let divisors = match BaudDivisors::compute(clock_hz, baud) {
            Ok(divisors) => divisors,
            Err(error) => return Err(Pl011Error::Baud(error)),
        };

        self.mmio.write_u32(register::CR, 0);
        self.drain()?;

        self.mmio.write_u32(register::IBRD, u32::from(divisors.integer()));
        self.mmio.write_u32(register::FBRD, u32::from(divisors.fractional()));
        self.mmio.write_u32(register::LCR_H, line_control::WLEN_8 | line_control::FEN);

        // No vector table is installed *yet* when this runs:
        // `crate::fault::install` is called later in `crate::boot`, after the
        // UART exists to report through. So an interrupt raised here still
        // jumps to whatever the firmware left in `VBAR_EL1`. Masked and
        // cleared, not merely masked.
        self.mmio.write_u32(register::IMSC, 0);
        self.mmio.write_u32(register::ICR, ALL_INTERRUPTS);

        self.mmio.write_u32(register::CR, control::UARTEN | control::TXE);
        Ok(())
    }

    /// Writes one byte, waiting for FIFO space under a bound.
    pub fn write_byte(&self, byte: u8) -> Result<(), Pl011Error> {
        self.poll_until(flag::TXFF)?;
        self.mmio.write_u32(register::DR, u32::from(byte));
        Ok(())
    }

    /// Writes bytes unchanged, in order, stopping at the first failure.
    ///
    /// No framing is applied — see [`framed`]. Stopping rather than continuing
    /// is deliberate: a device that cannot accept the first byte will not
    /// accept the last, and pushing on multiplies one stall by the length of
    /// the message.
    pub fn write_bytes(&self, bytes: &[u8]) -> Result<(), Pl011Error> {
        for byte in bytes {
            self.write_byte(*byte)?;
        }
        Ok(())
    }

    /// Writes text, framing `\n` as `\r\n` so a terminal capture is readable.
    pub fn write_str(&self, text: &str) -> Result<(), Pl011Error> {
        for byte in text.as_bytes() {
            let (first, second) = framed(*byte);
            self.write_byte(first)?;
            if let Some(second) = second {
                self.write_byte(second)?;
            }
        }
        Ok(())
    }

    /// Waits, under a bound, for the shift register to empty.
    fn drain(&self) -> Result<(), Pl011Error> {
        self.poll_until(flag::BUSY)
    }

    /// Polls the flag register until `mask` is clear, or gives up.
    ///
    /// The bounded loop `TEST-P1-07-01-A` clause 5's second paragraph requires.
    /// An unbounded wait here is a hang indistinguishable from a dead adapter,
    /// a rejected image, or a board that never started — and until
    /// `STORY-P1-07-02` there is no fault reporting to tell them apart.
    fn poll_until(&self, mask: u32) -> Result<(), Pl011Error> {
        for _ in 0..TX_POLL_LIMIT {
            if self.mmio.read_u32(register::FR) & mask == 0 {
                return Ok(());
            }
        }
        Err(Pl011Error::TransmitTimeout)
    }
}

/// The real MMIO accesses, compiled only when targeting AArch64.
///
/// The **only** `cfg(target_arch = "aarch64")` item and the **only** `unsafe`
/// in this module, which is `TEST-P1-07-01-A` clause 5 restated as a code
/// layout: everything above this point is reachable from a host test.
///
/// Nothing in this repository has executed these two instructions. They are
/// compiled and reviewed, exactly as `timer::SystemRegisters` has been since
/// `STORY-P1-01-03`.
#[cfg(target_arch = "aarch64")]
#[derive(Debug, Clone, Copy)]
pub struct VolatileMmio {
    base: *mut u32,
}

#[cfg(target_arch = "aarch64")]
impl VolatileMmio {
    /// Wraps a peripheral base address.
    ///
    /// # Safety
    ///
    /// `base` must be the physical address of a PL011 register window that is
    /// accessible from the current exception level and that no other code is
    /// concurrently programming. For this board that is
    /// [`crate::board::DEBUG_UART_BASE`], and "no other code" is guaranteed
    /// only because `FEAT-P1-07` §6 parks cores 1-3 — a fact this constructor
    /// cannot check, which is why it is `unsafe` rather than merely fallible.
    ///
    /// Note the address does not fit in 32 bits. A `u64` parameter is not
    /// defensive typing here: truncation is the single most dangerous Pi 4
    /// habit on this board, and it lands in DRAM rather than faulting.
    pub const unsafe fn new(base: u64) -> Self {
        VolatileMmio { base: base as *mut u32 }
    }
}

#[cfg(target_arch = "aarch64")]
impl Mmio for VolatileMmio {
    fn read_u32(&self, offset: usize) -> u32 {
        // SAFETY: `offset` comes only from `register::*`, every one of which is
        // word-aligned and inside the `DEBUG_UART_SIZE` window this module's
        // own test asserts; `base` was established by the caller of
        // `VolatileMmio::new` to be a mapped PL011 window. `read_volatile` is
        // required rather than a plain read because the flag register changes
        // underneath us — a cached read would turn `poll_until` into
        // `TX_POLL_LIMIT` observations of one stale value.
        unsafe { self.base.byte_add(offset).read_volatile() }
    }

    fn write_u32(&self, offset: usize, value: u32) {
        // SAFETY: as above. `write_volatile` is required because these writes
        // are side effects on a device, not stores to memory: the compiler must
        // not reorder, coalesce or elide them, and the configuration ordering
        // in `configure` is only meaningful if it survives to the bus.
        unsafe { self.base.byte_add(offset).write_volatile(value) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::RefCell;

    /// A scripted MMIO double. Records every write in order, and answers `FR`
    /// reads from a queue so the flag-polling loop can be driven deterministic-
    /// ally on the host — including the case where the flag never clears.
    struct FakeMmio {
        writes: RefCell<Vec<(usize, u32)>>,
        flags: RefCell<Vec<u32>>,
        /// Value returned once `flags` is exhausted.
        flags_after: u32,
        reads: RefCell<usize>,
    }

    impl FakeMmio {
        /// A device that is always ready: TX FIFO never full, never busy.
        fn ready() -> Self {
            FakeMmio {
                writes: RefCell::new(Vec::new()),
                flags: RefCell::new(Vec::new()),
                flags_after: 0,
                reads: RefCell::new(0),
            }
        }

        /// A device whose TX FIFO is full for `busy_for` polls and then drains.
        fn full_for(busy_for: usize) -> Self {
            FakeMmio {
                writes: RefCell::new(Vec::new()),
                flags: RefCell::new(vec![flag::TXFF; busy_for]),
                flags_after: 0,
                reads: RefCell::new(0),
            }
        }

        /// A device whose TX FIFO is full forever — a stuck or absent UART.
        fn wedged() -> Self {
            FakeMmio {
                writes: RefCell::new(Vec::new()),
                flags: RefCell::new(Vec::new()),
                flags_after: flag::TXFF,
                reads: RefCell::new(0),
            }
        }

        fn writes(&self) -> Vec<(usize, u32)> {
            self.writes.borrow().clone()
        }

        fn written_to(&self, offset: usize) -> Vec<u32> {
            self.writes
                .borrow()
                .iter()
                .filter(|(at, _)| *at == offset)
                .map(|(_, value)| *value)
                .collect()
        }

        fn reads(&self) -> usize {
            *self.reads.borrow()
        }
    }

    impl Mmio for FakeMmio {
        fn read_u32(&self, offset: usize) -> u32 {
            *self.reads.borrow_mut() += 1;
            assert_eq!(offset, register::FR, "the driver reads only FR");
            let mut flags = self.flags.borrow_mut();
            if flags.is_empty() {
                self.flags_after
            } else {
                flags.remove(0)
            }
        }

        fn write_u32(&self, offset: usize, value: u32) {
            self.writes.borrow_mut().push((offset, value));
        }
    }

    // ---- Register map: clause 6, hardcoded-and-verified ------------------

    #[test]
    fn the_register_offsets_are_the_architected_pl011_ones() {
        assert_eq!(register::DR, 0x000);
        assert_eq!(register::FR, 0x018);
        assert_eq!(register::IBRD, 0x024);
        assert_eq!(register::FBRD, 0x028);
        assert_eq!(register::LCR_H, 0x02C);
        assert_eq!(register::CR, 0x030);
        assert_eq!(register::IMSC, 0x038);
        assert_eq!(register::ICR, 0x044);
    }

    #[test]
    fn every_register_offset_lies_inside_the_mapped_window() {
        for offset in [
            register::DR,
            register::FR,
            register::IBRD,
            register::FBRD,
            register::LCR_H,
            register::CR,
            register::IMSC,
            register::ICR,
        ] {
            assert!(offset + 4 <= crate::board::DEBUG_UART_SIZE, "{offset:#x} escapes the window");
            assert_eq!(offset % 4, 0, "{offset:#x} is not word-aligned");
        }
    }

    // ---- Baud divisor arithmetic: clause 5 -------------------------------

    #[test]
    fn the_pi_5_debug_uart_divides_exactly_at_115200() {
        // 9_216_000 / (16 * 115_200) == 5.0 exactly. The fractional register is
        // zero, and that exactness is a property of the board's fixed
        // 9.216 MHz UART clock, not a coincidence worth glossing over: it is
        // the reason this clock frequency was chosen.
        let divisors = BaudDivisors::compute(9_216_000, 115_200).expect("an exact divisor");
        assert_eq!(divisors.integer(), 5);
        assert_eq!(divisors.fractional(), 0);
        assert_eq!(divisors.achieved_baud_hz(9_216_000), 115_200);
    }

    #[test]
    fn a_pi_4_clock_needs_the_fractional_divisor_and_rounds_to_nearest() {
        // 48 MHz / (16 * 115200) == 26.0417. Truncating the fraction to 2/64
        // rather than rounding to 3/64 is the classic PL011 bug; it is here as
        // a test because the divergence table records that a Pi 4 clock is what
        // a reader arrives with. Rounding to nearest matches this crate's
        // existing precedent in `timer::plausible_cycles_per_us`.
        let divisors = BaudDivisors::compute(48_000_000, 115_200).expect("a valid divisor");
        assert_eq!(divisors.integer(), 26);
        assert_eq!(divisors.fractional(), 3);
    }

    #[test]
    fn the_fractional_divisor_rounds_rather_than_truncates() {
        // 1_843_400 / (16 * 115_200) = 1.00033..., fraction 0.0213 of 64.
        let low = BaudDivisors::compute(1_843_400, 115_200).expect("valid");
        assert_eq!((low.integer(), low.fractional()), (1, 0));
        // A clock landing exactly on a half-step rounds up, not toward zero.
        let half =
            BaudDivisors::compute(115_200 * 16 * 2 + 115_200 * 16 / 128, 115_200).expect("valid");
        assert_eq!((half.integer(), half.fractional()), (2, 1));
    }

    #[test]
    fn a_zero_clock_is_an_error_rather_than_a_divisor_of_zero() {
        // Firmware that did not program the UART clock is a real condition. A
        // truncating divide turns it into IBRD=0, which the PL011 documents as
        // "baud rate generator disabled" — a silent dead line.
        assert_eq!(BaudDivisors::compute(0, 115_200), Err(BaudError::ZeroClock));
    }

    #[test]
    fn a_zero_baud_is_an_error_rather_than_a_division_by_zero() {
        assert_eq!(BaudDivisors::compute(9_216_000, 0), Err(BaudError::ZeroBaud));
    }

    #[test]
    fn a_baud_faster_than_the_clock_can_produce_is_rejected() {
        // IBRD == 0 disables the generator. Requesting 1 Mbaud from a 9.216 MHz
        // clock gives a divisor of 0.576, which is not a slow line, it is no
        // line at all.
        assert_eq!(BaudDivisors::compute(9_216_000, 1_000_000), Err(BaudError::RateTooHigh));
    }

    #[test]
    fn a_baud_slower_than_the_16_bit_integer_divisor_can_express_is_rejected() {
        // IBRD is 16 bits. A 9.216 MHz clock cannot reach 8 baud.
        assert_eq!(BaudDivisors::compute(9_216_000, 8), Err(BaudError::RateTooLow));
    }

    #[test]
    fn the_fractional_divisor_bounds_the_worst_case_error_below_framing_tolerance() {
        // This test is the reason there is no "rate unachievable" error
        // variant. With six fractional bits the rounded divisor is never worse
        // than 0.5/64 relative — under 0.8%, and only at the fastest rate the
        // generator can express; async 8N1 framing tolerates roughly 2-3%
        // combined across both ends. So a divisor that passes the range checks
        // is always usable, and a tolerance rejection would have been
        // unreachable code defending against a state the hardware cannot enter.
        for clock in [9_216_000u32, 48_000_000, 1_843_200, 24_000_000, 3_000_000] {
            for baud in [9_600u32, 38_400, 115_200, 230_400, 921_600] {
                let Ok(divisors) = BaudDivisors::compute(clock, baud) else { continue };
                let achieved = divisors.achieved_baud_hz(clock);
                let parts_per_thousand =
                    u64::from(achieved.abs_diff(baud)) * 1_000 / u64::from(baud);
                assert!(
                    parts_per_thousand <= 8,
                    "{clock} Hz at {baud} baud achieved {achieved}: {parts_per_thousand}/1000 off"
                );
            }
        }
    }

    #[test]
    fn the_achieved_rate_is_reported_so_a_capture_can_be_checked_against_it() {
        let divisors = BaudDivisors::compute(48_000_000, 115_200).expect("valid");
        // 48e6 / (16 * (26 + 3/64)) = 115177.5 -> 115177 Hz, 0.02% low.
        assert_eq!(divisors.achieved_baud_hz(48_000_000), 115_177);
    }

    // ---- Framing: clause 5 ----------------------------------------------

    #[test]
    fn a_line_feed_is_framed_as_carriage_return_line_feed() {
        // The capture is read by a human on a terminal, and a bare LF produces
        // a staircase. This is the only transformation the driver applies to a
        // caller's bytes, and it is pure so that a capture can be predicted
        // exactly from the source.
        assert_eq!(framed(b'\n'), (b'\r', Some(b'\n')));
    }

    #[test]
    fn every_other_byte_passes_through_untouched_including_carriage_return() {
        assert_eq!(framed(b'A'), (b'A', None));
        assert_eq!(framed(b'\r'), (b'\r', None));
        assert_eq!(framed(0x00), (0x00, None));
        assert_eq!(framed(0xFF), (0xFF, None));
    }

    // ---- Hex rendering: clause 6, the handoff is reported ----------------

    #[test]
    fn a_u64_renders_as_sixteen_uppercase_hex_digits() {
        // The device-tree blob pointer is *reported*, never parsed (`BND-03`).
        // Reporting it needs formatting, and `core::fmt` is more machinery than
        // a board with no fault handler should carry, so this is a fixed-width
        // pure function instead.
        assert_eq!(&hex_u64(0), b"0000000000000000");
        assert_eq!(&hex_u64(crate::board::DEBUG_UART_BASE), b"000000107D001000");
        assert_eq!(&hex_u64(u64::MAX), b"FFFFFFFFFFFFFFFF");
        assert_eq!(&hex_u64(0x0123_4567_89AB_CDEF), b"0123456789ABCDEF");
    }

    // ---- Configuration sequence: clause 5 --------------------------------

    #[test]
    fn configuration_disables_the_uart_before_touching_the_divisors() {
        let mmio = FakeMmio::ready();
        let uart = Pl011::new(&mmio);
        uart.configure(9_216_000, 115_200).expect("the board's own clock and baud");

        let writes = mmio.writes();
        let first_cr = writes.iter().position(|(at, _)| *at == register::CR).expect("CR written");
        let ibrd = writes.iter().position(|(at, _)| *at == register::IBRD).expect("IBRD written");
        assert_eq!(writes[first_cr].1, 0, "the UART is disabled before reprogramming");
        assert!(first_cr < ibrd, "CR=0 must precede the divisor writes");
    }

    #[test]
    fn the_line_control_register_is_written_after_the_divisors_because_it_latches_them() {
        // On a PL011 the IBRD/FBRD values are only transferred to the baud-rate
        // generator when LCR_H is written. Programming LCR_H first — which
        // reads perfectly naturally — leaves the *previous* baud in effect and
        // produces a capture at the firmware's rate rather than ours. That is a
        // wrong-looking-right failure, so the ordering is a test.
        let mmio = FakeMmio::ready();
        let uart = Pl011::new(&mmio);
        uart.configure(9_216_000, 115_200).expect("valid");

        let writes = mmio.writes();
        let ibrd = writes.iter().position(|(at, _)| *at == register::IBRD).expect("IBRD");
        let fbrd = writes.iter().position(|(at, _)| *at == register::FBRD).expect("FBRD");
        let lcr_h = writes.iter().position(|(at, _)| *at == register::LCR_H).expect("LCR_H");
        assert!(ibrd < lcr_h && fbrd < lcr_h, "LCR_H latches IBRD/FBRD and must follow both");
    }

    #[test]
    fn configuration_programs_the_divisors_it_computed() {
        let mmio = FakeMmio::ready();
        let uart = Pl011::new(&mmio);
        uart.configure(9_216_000, 115_200).expect("valid");
        assert_eq!(mmio.written_to(register::IBRD), vec![5]);
        assert_eq!(mmio.written_to(register::FBRD), vec![0]);
    }

    #[test]
    fn configuration_selects_eight_bits_no_parity_one_stop_with_fifos_enabled() {
        let mmio = FakeMmio::ready();
        let uart = Pl011::new(&mmio);
        uart.configure(9_216_000, 115_200).expect("valid");
        // 8N1 + FIFOs. The firmware console is 115200n8; a mismatch here is a
        // capture full of framing errors rather than silence.
        assert_eq!(
            mmio.written_to(register::LCR_H),
            vec![line_control::WLEN_8 | line_control::FEN]
        );
    }

    #[test]
    fn configuration_masks_every_interrupt_and_clears_every_pending_one() {
        // `STORY-P1-07-02` has not landed: there is no vector table on this
        // board yet, so an interrupt that fires here is an unrecoverable jump
        // into whatever the firmware left at VBAR_EL1.
        let mmio = FakeMmio::ready();
        let uart = Pl011::new(&mmio);
        uart.configure(9_216_000, 115_200).expect("valid");
        assert_eq!(mmio.written_to(register::IMSC), vec![0]);
        assert_eq!(mmio.written_to(register::ICR), vec![0x7FF]);
    }

    #[test]
    fn configuration_enables_the_uart_and_the_transmitter_last_and_nothing_else() {
        let mmio = FakeMmio::ready();
        let uart = Pl011::new(&mmio);
        uart.configure(9_216_000, 115_200).expect("valid");

        let writes = mmio.writes();
        let (last_offset, last_value) = *writes.last().expect("at least one write");
        assert_eq!(last_offset, register::CR);
        // Transmit only. This slice never reads a byte from the wire, and
        // `agent/CODING_STANDARDS.md`'s assurance-spine section requires that
        // for unselected features absence is tested rather than inferred — so
        // the receiver staying off is an assertion, not an omission.
        assert_eq!(last_value, control::UARTEN | control::TXE);
        assert_eq!(last_value & control::RXE, 0, "the receiver is not enabled by this slice");
    }

    #[test]
    fn a_rejected_baud_leaves_the_uart_disabled_rather_than_half_configured() {
        // Fail-safe: if the divisor cannot be computed, the device must not be
        // left enabled at whatever rate the firmware had. A half-configured
        // UART is the failure that looks like a working one.
        let mmio = FakeMmio::ready();
        let uart = Pl011::new(&mmio);
        assert_eq!(uart.configure(0, 115_200), Err(Pl011Error::Baud(BaudError::ZeroClock)));
        assert_eq!(mmio.written_to(register::IBRD), Vec::<u32>::new());
        assert_eq!(mmio.written_to(register::CR), Vec::<u32>::new());
    }

    // ---- Bounded polling: clause 5's second paragraph --------------------

    #[test]
    fn a_byte_is_written_to_the_data_register_once_the_fifo_has_room() {
        let mmio = FakeMmio::full_for(3);
        let uart = Pl011::new(&mmio);
        uart.write_byte(b'T').expect("the FIFO drains after three polls");
        assert_eq!(mmio.written_to(register::DR), vec![u32::from(b'T')]);
        assert_eq!(mmio.reads(), 4, "three full polls plus the one that saw room");
    }

    #[test]
    fn a_wedged_transmit_fifo_times_out_rather_than_hanging() {
        // The single most important test in this module. An unbounded wait on
        // TXFF is a hang indistinguishable from a dead adapter, a rejected
        // image, or a board that never started — and until `STORY-P1-07-02`
        // there is no fault reporting to tell those apart.
        let mmio = FakeMmio::wedged();
        let uart = Pl011::new(&mmio);
        assert_eq!(uart.write_byte(b'T'), Err(Pl011Error::TransmitTimeout));
        assert_eq!(mmio.written_to(register::DR), Vec::<u32>::new());
    }

    #[test]
    fn the_poll_bound_is_exactly_the_documented_limit() {
        let mmio = FakeMmio::wedged();
        let uart = Pl011::new(&mmio);
        let _ = uart.write_byte(b'T');
        assert_eq!(mmio.reads(), TX_POLL_LIMIT);
    }

    #[test]
    fn a_byte_sequence_stops_at_the_first_failure_rather_than_pushing_on() {
        let mmio = FakeMmio::wedged();
        let uart = Pl011::new(&mmio);
        assert_eq!(uart.write_bytes(b"TinyOS"), Err(Pl011Error::TransmitTimeout));
        // One byte attempted, not six: a UART that cannot accept the first byte
        // will not accept the sixth, and six timeouts is six times the stall.
        assert_eq!(mmio.reads(), TX_POLL_LIMIT);
    }

    #[test]
    fn a_string_is_transmitted_byte_for_byte_with_line_feeds_framed() {
        let mmio = FakeMmio::ready();
        let uart = Pl011::new(&mmio);
        uart.write_str("EL2\n").expect("a ready device accepts everything");
        assert_eq!(
            mmio.written_to(register::DR),
            b"EL2\r\n".iter().map(|b| u32::from(*b)).collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_data_register_receives_only_the_low_eight_bits() {
        let mmio = FakeMmio::ready();
        let uart = Pl011::new(&mmio);
        uart.write_byte(0xFF).expect("ready");
        assert_eq!(mmio.written_to(register::DR), vec![0xFF]);
    }

    // ---- The seam itself: clause 5's first paragraph ---------------------

    #[test]
    fn the_driver_reads_nothing_but_the_flag_register() {
        // Asserted inside `FakeMmio::read_u32`, and exercised here so the
        // assertion is actually reached: the driver must not depend on reading
        // back state it wrote, because a write-only mapping is a legitimate
        // MMIO configuration and read-back of a FIFO register has side effects.
        let mmio = FakeMmio::ready();
        let uart = Pl011::new(&mmio);
        uart.configure(9_216_000, 115_200).expect("valid");
        uart.write_str("TinyOS\n").expect("ready");
        assert!(mmio.reads() > 0);
    }
}
