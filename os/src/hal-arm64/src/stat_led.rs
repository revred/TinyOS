//! The BCM2712 status LED (`STORY-P1-07-08`): execution as a naked-eye fact.
//!
//! `TEST-P1-07-08-A`. Sources: the on-silicon capture
//! `goals/reports/pios-ground-truth-2026-08-03.txt` — the `rpi-gpiomem`
//! window line (`base 0x107d517c00 size 0x40`) and the
//! `/sys/kernel/debug/gpio` line naming pin 9 of that block
//! `2712_STAT_LED`/`ACT`, output (the listing said active-low; the
//! 2026-08-03 confession boot measured active-HIGH at this pin, and the
//! measurement governs) — and the `gpio-brcmstb`
//! register layout (Linux `drivers/gpio/gpio-brcmstb.c`, retrieved
//! 2026-08-03): 0x20-stride banks of eight word registers, `DATA` at `0x04`,
//! `IODIR` at `0x08` with a set bit meaning *input*.
//!
//! This is the one observable on the SoC side of every suspect peripheral:
//! no serial adapter, no PCIe gate, no RP1 window, no mailbox negotiation.
//! It is an instrument, never evidence — no capture or timing claim may cite
//! it, and nothing here waits, retries, or reports.

use crate::pl011::Mmio;

/// Register offsets inside bank 0 of the `gpio-brcmstb` block.
pub mod register {
    /// `GIO_DATA` — output level, one bit per pin.
    pub const DATA: usize = 0x04;
    /// `GIO_IODIR` — direction, one bit per pin; **set means input**.
    pub const IODIR: usize = 0x08;
}

/// The ACT LED: pin 9 of bank 0, per the on-silicon debug-gpio listing.
pub const ACT_PIN: u32 = 9;
/// The bank-relative bit for [`ACT_PIN`].
pub const ACT_BIT: u32 = 1 << ACT_PIN;

/// Makes the LED pin an output: clears exactly its `IODIR` bit, preserving
/// every other pin's direction as found.
pub fn make_output<M: Mmio>(gpio: &M) {
    let direction = gpio.read_u32(register::IODIR);
    gpio.write_u32(register::IODIR, direction & !ACT_BIT);
}

/// Drives the LED. `on` sets the data bit: the Pi OS debug listing called
/// the line active-low, but the 2026-08-03 confession boot measured
/// otherwise — the pattern's dark gap read bright-steady on the board, so
/// at this pin a high level lights the lamp. The measurement wins; pinned
/// as arithmetic by the tests, with the observation as the cited source.
pub fn drive<M: Mmio>(gpio: &M, on: bool) {
    let data = gpio.read_u32(register::DATA);
    gpio.write_u32(register::DATA, if on { data | ACT_BIT } else { data & !ACT_BIT });
}

/// Flips the LED, whatever state it was in — the park loop's 1 Hz heartbeat
/// is a toggle precisely so no assumption about the inherited state exists.
pub fn toggle<M: Mmio>(gpio: &M) {
    let data = gpio.read_u32(register::DATA);
    gpio.write_u32(register::DATA, data ^ ACT_BIT);
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::RefCell;

    /// Records every access; scripts the readbacks hostile — every unowned
    /// bit set to a mix the operations must preserve exactly.
    struct RecordingGpio {
        data_readback: u32,
        iodir_readback: u32,
        log: RefCell<Vec<(char, usize, u32)>>,
    }

    impl RecordingGpio {
        fn new(data_readback: u32, iodir_readback: u32) -> Self {
            RecordingGpio { data_readback, iodir_readback, log: RefCell::new(Vec::new()) }
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

    impl Mmio for RecordingGpio {
        fn read_u32(&self, offset: usize) -> u32 {
            self.log.borrow_mut().push(('r', offset, 0));
            match offset {
                register::DATA => self.data_readback,
                register::IODIR => self.iodir_readback,
                other => panic!("unexpected read of {other:#x} — the lamp owns two registers"),
            }
        }

        fn write_u32(&self, offset: usize, value: u32) {
            self.log.borrow_mut().push(('w', offset, value));
        }
    }

    // TEST-P1-07-08-A clause 1: transcriptions pinned with their arithmetic.

    #[test]
    fn the_act_led_is_pin_9_and_every_offset_is_the_brcmstb_layout() {
        assert_eq!(ACT_PIN, 9, "the debug-gpio listing's line for 2712_STAT_LED");
        assert_eq!(ACT_BIT, 0b10_0000_0000);
        assert_eq!(register::DATA, 0x04);
        assert_eq!(register::IODIR, 0x08);
        // Both registers sit inside the window the board itself reported.
        for offset in [register::DATA, register::IODIR] {
            assert!(offset < crate::board::STAT_GPIO_SIZE);
            assert_eq!(offset % 4, 0);
        }
    }

    // TEST-P1-07-08-A clause 2: exact RMWs against hostile readbacks.

    #[test]
    fn make_output_clears_exactly_the_lamp_bit_of_iodir() {
        let gpio = RecordingGpio::new(0, 0xFFFF_FFFF);
        make_output(&gpio);
        assert_eq!(gpio.writes(), vec![(register::IODIR, 0xFFFF_FDFF)]);
    }

    // Polarity is a measurement, not a transcription: the 2026-08-03
    // confession boot's dark gap read bright-steady on the board, so a HIGH
    // level lights this lamp — the debug listing's "active low" lost to the
    // observation.
    #[test]
    fn on_drives_the_bit_high_because_the_board_said_so() {
        let gpio = RecordingGpio::new(0x5555_5155, 0);
        drive(&gpio, true);
        assert_eq!(gpio.writes(), vec![(register::DATA, 0x5555_5355)]);
    }

    #[test]
    fn off_drives_the_bit_low_and_disturbs_nothing_else() {
        let gpio = RecordingGpio::new(0xFFFF_FFFF, 0);
        drive(&gpio, false);
        assert_eq!(gpio.writes(), vec![(register::DATA, 0xFFFF_FDFF)]);
    }

    #[test]
    fn toggle_flips_exactly_one_bit_from_either_state() {
        let lit = RecordingGpio::new(0xAAAA_A8AA, 0);
        toggle(&lit);
        assert_eq!(lit.writes(), vec![(register::DATA, 0xAAAA_AAAA)]);
        let dark = RecordingGpio::new(0xAAAA_AAAA, 0);
        toggle(&dark);
        assert_eq!(dark.writes(), vec![(register::DATA, 0xAAAA_A8AA)]);
    }

    #[test]
    fn no_operation_touches_the_register_it_does_not_own() {
        let gpio = RecordingGpio::new(0, 0);
        drive(&gpio, true);
        toggle(&gpio);
        assert!(
            gpio.log.borrow().iter().all(|(_, offset, _)| *offset == register::DATA),
            "drive and toggle own DATA only"
        );
        let gpio = RecordingGpio::new(0, 0);
        make_output(&gpio);
        assert!(
            gpio.log.borrow().iter().all(|(_, offset, _)| *offset == register::IODIR),
            "make_output owns IODIR only"
        );
    }
}
