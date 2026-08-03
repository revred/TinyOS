//! RP1 bank-1 GPIO, at exactly the size `STORY-P1-09-04` needs: releasing the
//! Ethernet PHY's active-low reset on GPIO 32.
//!
//! `TEST-P1-09-04-A`. Sources, retrieved 2026-08-03: Raspberry Pi Linux
//! `rpi-6.12.y` `arch/arm64/boot/dts/broadcom/bcm2712-rpi-5-b.dts`
//! (`phy-reset-gpios = <&rp1_gpio 32 GPIO_ACTIVE_LOW>`,
//! `phy-reset-duration = <5>`) and the RP1 GPIO register layout as carried by
//! the u-boot RP1 pinctrl RFC and the community bare-metal register maps:
//! three banks of 28/6/20 pins at a `0x4000` stride from `io_bank0`
//! `0x0d0000` / `sys_rio0` `0x0e0000` / `pads_bank0` `0x0f0000`, per-pin
//! STATUS+CTRL pairs eight bytes apart, RP2040-style atomic XOR/SET/CLR
//! aliases at `+0x1000`/`+0x2000`/`+0x3000`, function-select code 5 for the
//! registered-IO peripheral, pad output-disable at bit 7.
//!
//! **Nothing here has been observed on silicon.** The sequence is
//! glitch-ordered by construction — level and direction are staged through
//! the RIO registers *before* the pin's function select hands them the pin —
//! so the reset line's first driven value is the assertion, never a float.

use crate::pl011::Mmio;

/// Offsets inside the RP1 peripheral window (the window `STORY-P1-09-01`
/// validates before any use). Bank 1 carries GPIOs 28..=33.
pub mod register {
    /// `io_bank1` base: `io_bank0` (`0x0d0000`) plus one `0x4000` bank stride.
    pub const IO_BANK1: usize = 0x0D4000;
    /// `sys_rio1` base: `sys_rio0` (`0x0e0000`) plus one bank stride.
    pub const SYS_RIO1: usize = 0x0E4000;
    /// `pads_bank1` base: `pads_bank0` (`0x0f0000`) plus one bank stride.
    pub const PADS_BANK1: usize = 0x0F4000;
    /// Atomic set alias offset (RP2040 convention, carried by RP1).
    pub const ATOMIC_SET: usize = 0x2000;
    /// Atomic clear alias offset.
    pub const ATOMIC_CLEAR: usize = 0x3000;
}

/// The PHY reset line: GPIO 32, which is pin 4 of bank 1 (banks are 28/6/20).
pub const PHY_RESET_GPIO: u32 = 32;
/// GPIO 32's index within bank 1.
pub const PHY_RESET_BANK_PIN: usize = (PHY_RESET_GPIO - 28) as usize;
/// The bank-relative bit the RIO registers use for GPIO 32.
pub const PHY_RESET_BIT: u32 = 1 << PHY_RESET_BANK_PIN;

/// GPIO 32's CTRL register: bank base + pin × 8 (STATUS, CTRL pairs) + 4.
pub const PHY_RESET_CTRL: usize = register::IO_BANK1 + PHY_RESET_BANK_PIN * 8 + 4;
/// GPIO 32's pad register: bank base + 4 (past VOLTAGE_SELECT) + pin × 4.
pub const PHY_RESET_PAD: usize = register::PADS_BANK1 + 4 + PHY_RESET_BANK_PIN * 4;
/// `sys_rio1` OUT, atomic-clear alias (drive low).
pub const RIO_OUT_CLEAR: usize = register::SYS_RIO1 + register::ATOMIC_CLEAR;
/// `sys_rio1` OUT, atomic-set alias (drive high).
pub const RIO_OUT_SET: usize = register::SYS_RIO1 + register::ATOMIC_SET;
/// `sys_rio1` OE, atomic-set alias (output-enable).
pub const RIO_OE_SET: usize = register::SYS_RIO1 + register::ATOMIC_SET + 4;

/// CTRL function-select code for the registered-IO peripheral.
pub const FUNCSEL_RIO: u32 = 5;
/// CTRL fields this sequence owns: FUNCSEL `[4:0]`, OUTOVER `[13:12]`,
/// OEOVER `[15:14]` — overrides forced to "follow the peripheral".
pub const CTRL_OWNED_MASK: u32 = 0x1F | (0b11 << 12) | (0b11 << 14);
/// Pad bit 7: output disable. Cleared so the pin can drive; every other pad
/// bit (input enable, pulls, drive strength) is preserved as found.
pub const PAD_OUTPUT_DISABLE: u32 = 1 << 7;

/// The reset hold, from the device tree's `phy-reset-duration`.
pub const RESET_HOLD_MS: u32 = 5;
/// Post-release settle before the first MDIO frame. A named engineering
/// margin, not a datasheet transcription — revisited on board evidence.
pub const RESET_SETTLE_MS: u32 = 10;

/// Why the release aborted. Fail-safe: an aborted release leaves the line
/// asserted and the caller skips the scan — a PHY held in reset is the state
/// the board was already surviving in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseError {
    /// The bounded wait's counter never advanced far enough.
    StuckCounter,
}

/// Releases the PHY's active-low reset. `wait_ms` is the bounded counter
/// wait; it returns `false` if the counter is stuck, which aborts the
/// sequence between the hold and the release — the line stays asserted.
///
/// Order (`TEST-P1-09-04-A` clause 2): pad enable → level low + direction
/// out via RIO → function select hands RIO the pin (reset now *driven*
/// asserted) → hold → release high → settle.
pub fn release_phy_reset<M: Mmio>(
    window: &M,
    mut wait_ms: impl FnMut(u32) -> bool,
) -> Result<(), ReleaseError> {
    let pad = window.read_u32(PHY_RESET_PAD);
    window.write_u32(PHY_RESET_PAD, pad & !PAD_OUTPUT_DISABLE);
    window.write_u32(RIO_OUT_CLEAR, PHY_RESET_BIT);
    window.write_u32(RIO_OE_SET, PHY_RESET_BIT);
    let ctrl = window.read_u32(PHY_RESET_CTRL);
    window.write_u32(PHY_RESET_CTRL, (ctrl & !CTRL_OWNED_MASK) | FUNCSEL_RIO);
    if !wait_ms(RESET_HOLD_MS) {
        return Err(ReleaseError::StuckCounter);
    }
    window.write_u32(RIO_OUT_SET, PHY_RESET_BIT);
    if !wait_ms(RESET_SETTLE_MS) {
        return Err(ReleaseError::StuckCounter);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::RefCell;

    /// Records every access in order; scripts the two RMW reads.
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
    }

    impl Mmio for RecordingWindow {
        fn read_u32(&self, offset: usize) -> u32 {
            self.log.borrow_mut().push(('r', offset, 0));
            match offset {
                PHY_RESET_PAD => self.pad_readback,
                PHY_RESET_CTRL => self.ctrl_readback,
                other => panic!("unexpected read of {other:#x} — the release owns two reads"),
            }
        }

        fn write_u32(&self, offset: usize, value: u32) {
            self.log.borrow_mut().push(('w', offset, value));
        }
    }

    // TEST-P1-09-04-A clause 1: transcriptions pinned with their arithmetic.

    #[test]
    fn gpio_32_is_pin_4_of_bank_1_and_every_address_derives_from_that() {
        assert_eq!(PHY_RESET_GPIO, 32);
        assert_eq!(PHY_RESET_BANK_PIN, 4);
        assert_eq!(PHY_RESET_BIT, 0b1_0000);
        assert_eq!(register::IO_BANK1, 0x0D0000 + 0x4000);
        assert_eq!(register::SYS_RIO1, 0x0E0000 + 0x4000);
        assert_eq!(register::PADS_BANK1, 0x0F0000 + 0x4000);
        assert_eq!(PHY_RESET_CTRL, 0x0D4024);
        assert_eq!(PHY_RESET_PAD, 0x0F4014);
        assert_eq!(RIO_OUT_CLEAR, 0x0E7000);
        assert_eq!(RIO_OUT_SET, 0x0E6000);
        assert_eq!(RIO_OE_SET, 0x0E6004);
        // Everything stays inside the window the probe validates.
        for offset in [PHY_RESET_CTRL, PHY_RESET_PAD, RIO_OUT_CLEAR, RIO_OUT_SET, RIO_OE_SET] {
            assert!((offset as u64) < crate::board::RP1_WINDOW_MIN_SPAN);
            assert_eq!(offset % 4, 0);
        }
        assert_eq!(RESET_HOLD_MS, 5, "the device tree's phy-reset-duration");
    }

    // TEST-P1-09-04-A clause 2: exact, glitch-ordered, nothing else touched.

    #[test]
    fn the_release_sequence_is_exact_and_glitch_ordered() {
        // Pad arrives with output disabled, input enabled, a pull configured;
        // CTRL arrives on a foreign funcsel with interrupt bits set high up.
        let window = RecordingWindow::new(0b1101_0110, 0xA000_0011);
        let mut waits = Vec::new();
        release_phy_reset(&window, |ms| {
            waits.push(ms);
            true
        })
        .expect("a live counter releases");
        assert_eq!(
            window.writes(),
            vec![
                (PHY_RESET_PAD, 0b0101_0110),   // OD cleared, IE and pulls preserved
                (RIO_OUT_CLEAR, PHY_RESET_BIT), // level staged low first
                (RIO_OE_SET, PHY_RESET_BIT),    // direction staged out
                (PHY_RESET_CTRL, 0xA000_0005),  // funcsel→RIO, overrides cleared, irq bits kept
                (RIO_OUT_SET, PHY_RESET_BIT),   // the release, after the hold
            ]
        );
        assert_eq!(waits, vec![RESET_HOLD_MS, RESET_SETTLE_MS]);
    }

    // TEST-P1-09-04-A clause 3: a stuck counter aborts with the line asserted.

    #[test]
    fn a_stuck_counter_aborts_before_the_release_and_the_line_stays_asserted() {
        let window = RecordingWindow::new(0x80, 0x11);
        assert_eq!(release_phy_reset(&window, |_| false), Err(ReleaseError::StuckCounter));
        let writes = window.writes();
        assert!(
            !writes.iter().any(|(offset, _)| *offset == RIO_OUT_SET),
            "an aborted hold must never drive the release edge"
        );
    }

    #[test]
    fn a_counter_that_sticks_during_settle_still_reports() {
        let window = RecordingWindow::new(0x80, 0x11);
        let mut calls = 0;
        let outcome = release_phy_reset(&window, |_| {
            calls += 1;
            calls == 1 // hold succeeds, settle sticks
        });
        assert_eq!(outcome, Err(ReleaseError::StuckCounter));
        assert!(window.writes().iter().any(|(offset, _)| *offset == RIO_OUT_SET));
    }
}
