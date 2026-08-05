//! The die temperature, read and never interpreted (`LE-75`).
//!
//! `LE-75` was raised when the owner noticed the fan never spins under TinyOS
//! while Pi OS starts it immediately. The capture that followed found something
//! larger than a fan: **TinyOS drives this SoC with no thermal awareness at
//! all** — it could not read the temperature, could not throttle, and did not
//! know whether firmware was managing heat on its behalf. That is a rule 1
//! matter (safety before security before correctness before performance), and
//! it is an absence no Story owned.
//!
//! This module is the **sensing half only**. Nothing here drives a fan, caps a
//! clock, or takes any action whatsoever on what it reads. Reading and acting
//! are separated deliberately: an actuator driven by a sensor nobody has
//! validated is worse than no actuator, because it converts a measurement error
//! into a physical one.
//!
//! # Why the raw register goes on the wire
//!
//! The board emits, it does not interpret — §1 of the spoor transport
//! architecture, and here it earns its keep twice over.
//!
//! The conversion from this register's raw datum to millicelsius is a slope and
//! an offset that **have not been verified on this board**. Reading the
//! register from Pi OS to derive them needed root that the ground-truth session
//! did not have. Converting on the board would mean compiling in an unverified
//! constant and emitting a number that *looks* like a temperature — exactly the
//! shape of `LE-69`, where an assumed constant made the code refuse a
//! conforming device, and exactly what the no-bench-tuned-constants rule
//! exists to stop.
//!
//! So the raw 32-bit word travels unaltered and the host converts. Two
//! consequences, both wanted:
//!
//! - **A wrong offset is visible rather than plausible.** If
//!   [`board::AVS_TEMP_STATUS_OFFSET`](crate::board::AVS_TEMP_STATUS_OFFSET) is
//!   not the temperature register, the wire carries a word that does not drift
//!   the way a die temperature drifts, and the reader can see that. A converted
//!   value would arrive as a confident number that happened to be nonsense.
//! - **The calibration can be corrected without reflashing.** The host owns the
//!   arithmetic, so refining it costs an edit to Ti64Dink rather than a card
//!   swap and a power cycle.

// Only the board path and the tests name these: the host stub returns zero
// without consulting a constant, so an ungated import is dead on a host build.
#[cfg(any(target_arch = "aarch64", test))]
use crate::board;

// `VolatileMmio` exists only on the board: it is the one part of this module
// that cannot run on the host, which is exactly why everything else here is
// arithmetic-free and the conversion lives on the laptop.
#[cfg(target_arch = "aarch64")]
use crate::pl011::{Mmio, VolatileMmio};

/// Reads the AVS monitor's temperature status word, verbatim.
///
/// Returns the raw 32-bit register contents with no masking, no scaling and no
/// validity filtering. The validity bits and the data field are the host's to
/// interpret; masking here would throw away the evidence that says whether the
/// register is the one we think it is.
///
/// One `ldr` from a Device-nGnRnE mapping. No allocation, no branch, nothing
/// that can block — cheap enough to stamp from the park loop without arguing
/// for itself.
///
/// # Safety
///
/// Reads MMIO inside the identity map's Device gigabyte. The AVS monitor's
/// temperature status is documented read-only and this function never writes,
/// so the read is side-effect-free on the device.
#[cfg(target_arch = "aarch64")]
#[must_use]
pub fn read_raw() -> u32 {
    // SAFETY: `AVS_MONITOR_BASE` is transcribed from this board's own device
    // tree and lies in the same Device gigabyte the identity map already
    // covers for `GICD_BASE`; the offset is within the one page
    // `AVS_MONITOR_SIZE` bounds, and this is a read of a read-only register.
    let window = unsafe { VolatileMmio::new(board::AVS_MONITOR_BASE) };
    window.read_u32(board::AVS_TEMP_STATUS_OFFSET)
}

/// Host builds have no AVS monitor to read.
///
/// Zero rather than a plausible sample: a host build must not be able to
/// manufacture a temperature reading that looks like a board's.
#[cfg(not(target_arch = "aarch64"))]
#[must_use]
pub fn read_raw() -> u32 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The window this module reads must be inside the page the board constant
    /// bounds, or the read walks off a mapping that was sized for one register.
    #[test]
    fn the_temperature_register_lies_inside_the_mapped_page() {
        // A property of two constants, so it is checked when they are compiled
        // rather than when this test runs: a read that leaves the mapped page
        // should fail the build, not a test run somebody might not do.
        const { assert!(board::AVS_TEMP_STATUS_OFFSET + 4 <= board::AVS_MONITOR_SIZE) };
    }

    /// The AVS monitor must share the Device gigabyte the identity map already
    /// establishes, or reading it faults on a board where every host test
    /// passes. Same class as the mapping checks `mmu` makes for the GIC.
    #[test]
    fn the_avs_monitor_shares_the_device_gigabyte_with_the_gic() {
        const GIB: u64 = 1 << 30;
        assert_eq!(
            board::AVS_MONITOR_BASE / GIB,
            board::GICD_BASE / GIB,
            "no mapping change is needed only while this holds"
        );
    }

    /// A host build must not be able to invent a temperature.
    #[cfg(not(target_arch = "aarch64"))]
    #[test]
    fn a_host_build_reads_nothing_rather_than_something_plausible() {
        assert_eq!(read_raw(), 0);
    }
}
