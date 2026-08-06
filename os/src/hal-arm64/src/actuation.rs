//! The AArch64 [`OutputLine`] backend: eight RP1 bank-0 GPIOs driven as one
//! byte-wide command bus (`STORY-P1-06-02`).
//!
//! **Why this module exists at all.** `FEAT-P1-06` is the `G-PA-1` flagship
//! path and every line of its evidence was Tier 0 QEMU `x86_64` until
//! 2026-08-06. [`ADR 0004`] designates ARM64 the real-time tier;
//! [`hal::actuation::OutputLine`] was written arch-neutral *so that* a Pi 5
//! backend could slot in without the kernel path, the fixtures or the
//! measurement harness changing a line — and for a week nothing had. The
//! mechanism this project measures its flagship latency through had never run
//! on the architecture it calls real-time, and nothing anywhere said so
//! (`LE-85`).
//!
//! # The output boundary
//!
//! **GPIO 20..27 of bank 0, driven through `sys_rio0`'s XOR alias.** Three
//! choices, each with an alternative that is worse in a specific way:
//!
//! - **Eight contiguous pins**, because the trait writes a byte and the
//!   `x86_64` stand-in is a byte-wide ISA port write. A one-bit backend would
//!   be measuring a narrower path than the one every Tier 0 number describes.
//! - **20..27 rather than 0..7**, because GPIO 0 and 1 are the HAT ID EEPROM's
//!   `ID_SD`/`ID_SC`. Driving them would be writing a bus the board uses for
//!   something else — the same argument `hal_x86_64::actuation` makes for
//!   choosing port `0x80` over the DMA page-register file.
//! - **The XOR alias rather than SET-then-CLEAR.** A byte written as two
//!   stores presents a transient in which some bits have changed and others
//!   have not, and on a command bus that transient *is* an intermediate
//!   command. XOR against a shadow of the last value updates exactly the
//!   changed bits in exactly one store and touches no other pin in the bank.
//!   RP1 has no masked-write alias, so the shadow is what makes one store
//!   possible; it is owned state rather than a cache of the hardware's, and it
//!   is never read back.
//!
//! **One named limitation, stated rather than discovered later.** Two
//! identical consecutive commands produce two bus stores and no change in pin
//! state, so a downstream device cannot distinguish them without a strobe line
//! or an edge-encoded protocol. That is a property of a parallel command bus,
//! not a coalescing bug: this backend does not coalesce, buffer or replay —
//! the trait's actual prohibitions — and every call performs exactly one
//! unconditional store. A real actuator interface will need a strobe;
//! inventing one now, with no implementor that could honour it, is the
//! speculative-consumer trap `agent/CODING_STANDARDS.md` names.
//!
//! **Nothing here has been observed on silicon**, and the register derivation
//! is [`crate::rp1_gpio`]'s, re-applied to bank 0 — same layout, same strides,
//! bank base without the `0x4000` offset. `STORY-P1-06-02` criterion 4 is the
//! board run and it is open.
//!
//! [`ADR 0004`]: ../../../../docs/adr/0004-arm64-is-the-real-time-tier.md

use hal::actuation::OutputLine;

use crate::pl011::Mmio;

/// Offsets inside the RP1 peripheral window for bank 0. Bank 0 carries
/// GPIOs 0..=27; [`crate::rp1_gpio::register`] carries the bank-1 twins, one
/// `0x4000` stride higher.
pub mod register {
    /// `io_bank0` base.
    pub const IO_BANK0: usize = 0x0D0000;
    /// `sys_rio0` base.
    pub const SYS_RIO0: usize = 0x0E0000;
    /// `pads_bank0` base.
    pub const PADS_BANK0: usize = 0x0F0000;
    /// Atomic XOR alias offset (RP2040 convention, carried by RP1).
    pub const ATOMIC_XOR: usize = 0x1000;
    /// Atomic set alias offset.
    pub const ATOMIC_SET: usize = 0x2000;
    /// Atomic clear alias offset.
    pub const ATOMIC_CLEAR: usize = 0x3000;
}

/// The lowest GPIO of the command byte. See the module doc for why not 0.
pub const COMMAND_BASE_GPIO: usize = 20;
/// How many lines the command occupies — one per bit of the trait's `u8`.
pub const COMMAND_WIDTH: usize = 8;
/// The bank-relative bits this backend owns, and the only bits any store here
/// may ever set.
pub const COMMAND_MASK: u32 = 0xFF << COMMAND_BASE_GPIO;

/// `sys_rio0` OUT, XOR alias — the single store every command performs.
pub const RIO_OUT_XOR: usize = register::SYS_RIO0 + register::ATOMIC_XOR;
/// `sys_rio0` OUT, atomic-clear alias, used once during the claim to stage a
/// deliberate zero before the pins are driven.
pub const RIO_OUT_CLEAR: usize = register::SYS_RIO0 + register::ATOMIC_CLEAR;
/// `sys_rio0` OE, atomic-set alias — output-enable, staged before function
/// select.
pub const RIO_OE_SET: usize = register::SYS_RIO0 + register::ATOMIC_SET + 4;

/// CTRL function-select code for the registered-IO peripheral.
pub const FUNCSEL_RIO: u32 = 5;
/// CTRL fields the claim owns: FUNCSEL `[4:0]`, OUTOVER `[13:12]`, OEOVER
/// `[15:14]`. Every other bit is preserved as found.
pub const CTRL_OWNED_MASK: u32 = 0x1F | (0b11 << 12) | (0b11 << 14);
/// Pad bit 7: output disable. Cleared so the pin can drive; every other pad
/// bit is preserved.
pub const PAD_OUTPUT_DISABLE: u32 = 1 << 7;

/// The CTRL register of bank-0 pin `n`: bank base + n × 8 (STATUS, CTRL pairs)
/// + 4.
pub const fn ctrl_register(bank_pin: usize) -> usize {
    register::IO_BANK0 + bank_pin * 8 + 4
}

/// The pad register of bank-0 pin `n`: bank base + 4 (past VOLTAGE_SELECT) +
/// n × 4.
pub const fn pad_register(bank_pin: usize) -> usize {
    register::PADS_BANK0 + 4 + bank_pin * 4
}

/// Eight RP1 GPIOs claimed as one byte-wide actuation command bus.
///
/// Generic over the MMIO window for the same reason [`crate::rp1_gpio`] is:
/// the host tests drive a recording window and the board drives the real
/// peripheral window, with no `cfg` between them and no second code path that
/// could differ from the one silicon runs.
#[derive(Debug)]
pub struct Rp1CommandLines<M: Mmio> {
    window: M,
    /// The last byte written. Owned state, never read back from hardware —
    /// which is what lets a command be one store instead of a
    /// read-modify-write.
    shadow: u8,
}

impl<M: Mmio> Rp1CommandLines<M> {
    /// Claims the eight pins and leaves them driving a deliberate zero.
    ///
    /// Glitch-ordered by construction, and the order is the contract
    /// (`STORY-P1-06-02` criterion 2): pad output-enable, then level and
    /// direction staged through RIO, then function select hands RIO the pins.
    /// The pins are not RIO-driven until the last step, so the **first driven
    /// value on every line is the zero staged before it** — never a float and
    /// never whatever the pad happened to hold.
    pub fn claim(window: M) -> Self {
        for bank_pin in COMMAND_BASE_GPIO..COMMAND_BASE_GPIO + COMMAND_WIDTH {
            let pad = window.read_u32(pad_register(bank_pin));
            window.write_u32(pad_register(bank_pin), pad & !PAD_OUTPUT_DISABLE);
        }
        window.write_u32(RIO_OUT_CLEAR, COMMAND_MASK);
        window.write_u32(RIO_OE_SET, COMMAND_MASK);
        for bank_pin in COMMAND_BASE_GPIO..COMMAND_BASE_GPIO + COMMAND_WIDTH {
            let ctrl = window.read_u32(ctrl_register(bank_pin));
            window.write_u32(ctrl_register(bank_pin), (ctrl & !CTRL_OWNED_MASK) | FUNCSEL_RIO);
        }
        Rp1CommandLines { window, shadow: 0 }
    }
}

impl<M: Mmio> OutputLine for Rp1CommandLines<M> {
    const NAME: &'static str = "rp1-gpio20-27";

    fn write_command(&mut self, command: u8) {
        // One store, unconditionally, whatever the command is. No branch on
        // `command == self.shadow`: a conditional store would make the RT
        // path's cost depend on the data flowing through it, which is the
        // thing this whole Feature exists to measure the absence of.
        let delta = u32::from(self.shadow ^ command) << COMMAND_BASE_GPIO;
        self.window.write_u32(RIO_OUT_XOR, delta);
        self.shadow = command;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::RefCell;

    /// Records every access in order and scripts the read-modify-write
    /// readbacks, so a test can assert the *sequence* rather than a set.
    struct RecordingWindow {
        pad_readback: u32,
        ctrl_readback: u32,
        log: RefCell<Vec<(char, usize, u32)>>,
    }

    impl RecordingWindow {
        fn new(pad_readback: u32, ctrl_readback: u32) -> Self {
            RecordingWindow { pad_readback, ctrl_readback, log: RefCell::new(Vec::new()) }
        }

        fn writes(&self) -> Vec<(usize, u32)> {
            self.log
                .borrow()
                .iter()
                .filter(|(kind, ..)| *kind == 'w')
                .map(|(_, offset, value)| (*offset, *value))
                .collect()
        }

        fn clear(&self) {
            self.log.borrow_mut().clear();
        }
    }

    impl Mmio for RecordingWindow {
        fn read_u32(&self, offset: usize) -> u32 {
            self.log.borrow_mut().push(('r', offset, 0));
            if (COMMAND_BASE_GPIO..COMMAND_BASE_GPIO + COMMAND_WIDTH)
                .any(|pin| pad_register(pin) == offset)
            {
                return self.pad_readback;
            }
            if (COMMAND_BASE_GPIO..COMMAND_BASE_GPIO + COMMAND_WIDTH)
                .any(|pin| ctrl_register(pin) == offset)
            {
                return self.ctrl_readback;
            }
            panic!("unexpected read of {offset:#x} — the claim owns two reads per owned pin")
        }

        fn write_u32(&self, offset: usize, value: u32) {
            self.log.borrow_mut().push(('w', offset, value));
        }
    }

    /// The pins are the module doc's argument, pinned. "Which pins" is a
    /// hardware-safety decision, and a silent edit to it drives whatever the
    /// board has wired to the new ones.
    #[test]
    fn the_command_bus_is_gpio_20_through_27_of_bank_0() {
        assert_eq!(COMMAND_BASE_GPIO, 20);
        assert_eq!(COMMAND_WIDTH, 8);
        assert_eq!(COMMAND_MASK, 0x0FF0_0000);
        assert_eq!(
            COMMAND_MASK & 0b11,
            0,
            "GPIO 0 and 1 are the HAT ID EEPROM and must never be in the mask"
        );
        assert_eq!(<Rp1CommandLines<RecordingWindow> as OutputLine>::NAME, "rp1-gpio20-27");
    }

    /// `STORY-P1-06-02` criterion 2, asserted as an **ordered** trace.
    ///
    /// A set of writes would pass whatever order they happened in, and the
    /// order is the entire safety property: if function select ran before the
    /// level and direction were staged, the first driven value on each line
    /// would be whatever the pad held, which is a command nobody chose.
    #[test]
    fn claim_stages_level_and_direction_before_handing_rio_the_pins() {
        let window = RecordingWindow::new(PAD_OUTPUT_DISABLE, 0);
        let _lines = Rp1CommandLines::claim(&window);
        let writes = window.writes();

        let out_clear = writes
            .iter()
            .position(|(offset, _)| *offset == RIO_OUT_CLEAR)
            .expect("the claim stages a zero level");
        let oe_set = writes
            .iter()
            .position(|(offset, _)| *offset == RIO_OE_SET)
            .expect("the claim stages the output direction");
        let first_funcsel = writes
            .iter()
            .position(|(offset, _)| *offset == ctrl_register(COMMAND_BASE_GPIO))
            .expect("the claim hands RIO the pins");

        assert!(out_clear < first_funcsel, "level must be staged before the pin is driven");
        assert!(oe_set < first_funcsel, "direction must be staged before the pin is driven");
        assert_eq!(writes[out_clear].1, COMMAND_MASK, "the staged level is a deliberate zero");
        assert_eq!(writes[oe_set].1, COMMAND_MASK);
    }

    /// The claim must not disturb a bit it does not own — the pad's input
    /// enable, pulls and drive strength, and every CTRL field outside FUNCSEL
    /// and the two overrides.
    #[test]
    fn claim_preserves_every_bit_it_does_not_own() {
        // A pad with output-disable set plus unrelated bits, and a CTRL with a
        // foreign function selected plus unrelated high bits.
        let window = RecordingWindow::new(PAD_OUTPUT_DISABLE | 0b0101_0110, 0x00A0_0002);
        let _lines = Rp1CommandLines::claim(&window);
        let writes = window.writes();

        let pad_write = writes
            .iter()
            .find(|(offset, _)| *offset == pad_register(COMMAND_BASE_GPIO))
            .expect("the pad is written");
        assert_eq!(pad_write.1, 0b0101_0110, "only output-disable is cleared");

        let ctrl_write = writes
            .iter()
            .find(|(offset, _)| *offset == ctrl_register(COMMAND_BASE_GPIO))
            .expect("the CTRL is written");
        assert_eq!(ctrl_write.1, 0x00A0_0000 | FUNCSEL_RIO, "high bits survive, FUNCSEL replaced");
    }

    /// `STORY-P1-06-02` criterion 3. One call, one store — and the same for a
    /// repeat of the identical command, which is the case a conditional store
    /// would silently optimise away and make the RT path data-dependent.
    #[test]
    fn every_command_is_exactly_one_store() {
        let window = RecordingWindow::new(PAD_OUTPUT_DISABLE, 0);
        let mut lines = Rp1CommandLines::claim(&window);
        window.clear();

        lines.write_command(0xA5);
        assert_eq!(window.writes().len(), 1, "one command is one store");

        window.clear();
        lines.write_command(0xA5);
        assert_eq!(
            window.writes().len(),
            1,
            "a repeated command is still one store; the cost may not depend on the data"
        );

        window.clear();
        lines.write_command(0x00);
        assert_eq!(window.writes().len(), 1);
    }

    /// The XOR is against the shadow, so the pins end up holding the command
    /// and not the difference. Checked by replaying the deltas.
    #[test]
    fn successive_commands_leave_the_pins_holding_the_last_one() {
        let window = RecordingWindow::new(PAD_OUTPUT_DISABLE, 0);
        let mut lines = Rp1CommandLines::claim(&window);
        window.clear();

        let mut pins: u32 = 0;
        for command in [0xA5u8, 0x5A, 0xFF, 0x00, 0x01, 0x01] {
            lines.write_command(command);
            let writes = window.writes();
            let (offset, delta) = *writes.last().expect("a command stores");
            assert_eq!(offset, RIO_OUT_XOR);
            pins ^= delta;
            assert_eq!(
                pins,
                u32::from(command) << COMMAND_BASE_GPIO,
                "after {command:#04x} the owned pins must hold that command"
            );
        }
    }

    /// No store from this module may touch a pin outside the owned byte.
    /// The XOR alias applies every set bit, so a stray bit here is a write to
    /// whatever the board has on that line.
    #[test]
    fn a_command_touches_no_pin_outside_the_owned_byte() {
        let window = RecordingWindow::new(PAD_OUTPUT_DISABLE, 0);
        let mut lines = Rp1CommandLines::claim(&window);
        window.clear();

        for command in [0x00u8, 0x01, 0x80, 0xFF, 0xA5] {
            lines.write_command(command);
        }
        for (offset, value) in window.writes() {
            assert_eq!(offset, RIO_OUT_XOR);
            assert_eq!(value & !COMMAND_MASK, 0, "{value:#010x} reaches outside the owned pins");
        }
    }

    /// The bank-0 register derivation, against `rp1_gpio`'s bank-1 twins.
    ///
    /// The two banks are one `0x4000` stride apart by construction, so this
    /// catches a transcription slip in either module rather than trusting both
    /// independently.
    #[test]
    fn bank_0_registers_are_one_bank_stride_below_the_bank_1_twins() {
        use crate::rp1_gpio::register as bank1;
        const STRIDE: usize = 0x4000;
        assert_eq!(register::IO_BANK0 + STRIDE, bank1::IO_BANK1);
        assert_eq!(register::SYS_RIO0 + STRIDE, bank1::SYS_RIO1);
        assert_eq!(register::PADS_BANK0 + STRIDE, bank1::PADS_BANK1);
        assert_eq!(register::ATOMIC_SET, bank1::ATOMIC_SET);
        assert_eq!(register::ATOMIC_CLEAR, bank1::ATOMIC_CLEAR);
    }
}
