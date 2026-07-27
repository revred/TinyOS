//! Raspberry Pi 5 / BCM2712 board constants, hardcoded-and-verified.
//!
//! `BND-03` and `FEAT-P1-07` §6 forbid a device-tree parser in this slice, so
//! the addresses are constants. That makes *where they came from* part of the
//! contract, which is why every value here carries its source revision.
//!
//! **Source of record.** Raspberry Pi Linux `rpi-6.12.y`,
//! `arch/arm64/boot/dts/broadcom/bcm2712.dtsi` — the `uart10` node, the
//! `clk_uart` fixed-clock node, and the `soc` node's `ranges` property.
//! Retrieved 2026-07-28. The full Pi 4 → Pi 5 divergence record, which is the
//! most reusable output of `STORY-P1-07-01`, is
//! `session/hand-2026-07-28/23-bcm2712-divergence-record.md`.
//!
//! **Nothing here has been observed on silicon.** These are transcriptions,
//! and a transcription that is wrong fails as silence on a board with no fault
//! reporting. `TEST-P1-07-01-A` clause 4 — a capture — is what turns them into
//! facts.

/// The smallest DRAM aperture any Raspberry Pi 5 SKU presents (2 GiB).
///
/// DRAM starts at zero on this SoC, so this doubles as the aperture's upper
/// bound; there is no `RAM_BASE` constant because a constant that is always
/// zero and only ever added is noise a reader has to check.
///
/// Used only to state a fact about *misuse*: see this module's tests. Nothing
/// in this slice sizes memory, because nothing in this slice allocates.
pub const MIN_RAM_SIZE: u64 = 2 * 1024 * 1024 * 1024;

/// Where the Raspberry Pi firmware places and enters a bare-metal AArch64
/// image.
///
/// **This requires `os_check=0` in `config.txt`.** Without it the Pi 5 firmware
/// concludes the image is a Linux kernel and loads it at `0x20_0000` instead —
/// a divergence from every Pi 4 tutorial, and one that presents as total
/// silence rather than as an error. The linker script
/// (`os/targets/aarch64-tinyos.ld`) hardcodes the same value; the two must
/// agree with `config.txt` or the entry point is not where the firmware jumps.
pub const KERNEL_LOAD_ADDRESS: u64 = 0x0008_0000;

/// CPU-physical base of `uart10`, the PL011 behind the Pi 5's dedicated 3-pin
/// debug connector.
///
/// The device tree expresses this as `serial@7d001000` inside a `soc` node
/// whose `ranges` is `<0x00000000 0x10 0x00000000 0x80000000>` — a child
/// address of `0x7d00_1000` at CPU-physical `0x10_0000_0000 + 0x7d00_1000`.
/// **The `0x10` is not decoration**: this address does not fit in 32 bits, and
/// the truncation is silent. See this module's tests.
pub const DEBUG_UART_BASE: u64 = 0x0000_0010_7D00_1000;

/// Size of the `uart10` register window, from the device tree's `reg`.
pub const DEBUG_UART_SIZE: usize = 0x200;

/// The `uart10` reference clock, in Hz — `clk_uart`, a fixed 9.216 MHz clock.
///
/// Unlike the Pi 4's `init_uart_clock`, this is not derived from a PLL and is
/// not configurable from `config.txt`: it is a fixed child of the 54 MHz
/// oscillator. 9.216 MHz divides *exactly* at 115200 baud
/// (9_216_000 / (16 × 115_200) = 5), which is almost certainly why it was
/// chosen, and which is why `IBRD = 5, FBRD = 0` on this board.
pub const DEBUG_UART_CLOCK_HZ: u32 = 9_216_000;

/// The baud the Raspberry Pi firmware's own console uses on this connector, and
/// therefore the rate a capture is expected at: `115200n8`.
///
/// Confirmed against the firmware's documented `earlycon=pl011,0x107d001000,115200n8`
/// parameter, which names the same base address as [`DEBUG_UART_BASE`].
pub const DEBUG_UART_BAUD: u32 = 115_200;

#[cfg(test)]
mod tests {
    use super::*;

    // `TEST-P1-07-01-A` clause 6: hardcoded-and-verified, with the source
    // revision recorded. These are assertions about what was read out of the
    // documentation, so that a later edit that "cleans up" an address has to
    // argue with a test rather than with a comment.
    #[test]
    fn the_debug_uart_is_uart10_at_the_bcm2712_cpu_physical_address() {
        assert_eq!(DEBUG_UART_BASE, 0x0000_0010_7D00_1000);
        assert_eq!(DEBUG_UART_SIZE, 0x200);
    }

    #[test]
    fn the_debug_uart_reference_clock_is_the_fixed_9_216_mhz_uart_clock() {
        assert_eq!(DEBUG_UART_CLOCK_HZ, 9_216_000);
    }

    #[test]
    fn the_expected_baud_is_the_one_the_firmware_console_uses() {
        assert_eq!(DEBUG_UART_BAUD, 115_200);
    }

    // The single most dangerous Pi 4 → Pi 5 divergence, pinned as a test
    // because it fails *silently* rather than faulting.
    #[test]
    fn the_debug_uart_address_does_not_fit_in_32_bits_and_truncation_lands_in_ram() {
        assert!(DEBUG_UART_BASE > u64::from(u32::MAX), "Pi 5 MMIO is above 4 GiB");

        // Every Pi 4 source writes a 32-bit peripheral address. Truncating the
        // Pi 5 address to 32 bits does not produce a fault, an abort, or a wild
        // pointer: it produces 0x7D00_1000, which is ordinary DRAM on every
        // Pi 5 SKU. A Pi 4 habit here corrupts memory and reports success.
        let truncated = u64::from(DEBUG_UART_BASE as u32);
        assert_eq!(truncated, 0x7D00_1000);
        assert!(truncated < MIN_RAM_SIZE, "truncation lands inside RAM, silently");
    }

    #[test]
    fn the_kernel_load_address_is_the_bare_metal_one_not_the_linux_one() {
        // `os_check=0` in config.txt is what keeps this at 0x80000: without it
        // the Pi 5 firmware assumes a Linux image and loads at 0x200000
        // instead. The linker script and this constant must agree with
        // config.txt or the image is entered at the wrong offset.
        assert_eq!(KERNEL_LOAD_ADDRESS, 0x0008_0000);
        // Both operands are constants, so this is a compile-time claim rather
        // than a runtime one. `const { .. }` says so — and makes a future SKU
        // whose `MIN_RAM_SIZE` dropped below the load address fail to build
        // rather than fail a test run.
        const { assert!(KERNEL_LOAD_ADDRESS < MIN_RAM_SIZE, "the image loads inside DRAM on every SKU") };
    }
}
