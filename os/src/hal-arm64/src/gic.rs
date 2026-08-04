//! The GIC-400 half of `STORY-P1-07-04`: route exactly one interrupt — the
//! EL1 virtual timer's PPI — and believe every enable from its readback.
//!
//! `TEST-P1-07-04-A`. The BCM2712 carries a GICv2 GIC-400 (distributor and
//! CPU interface bases in [`crate::board`], from the device tree the
//! ground-truth session captured). This module is deliberately not a GIC
//! driver: no SPIs, no targeting, no priorities beyond "unmasked", no second
//! core — device-IRQ routing beyond the timer is named debt (the Story's
//! `LE-08` analogue) and `FEAT-P1-07` §6 is the boundary.
//!
//! House split: the enable sequence is generic over the [`Mmio`] seam and
//! host-tested against latching doubles (`SEC-19`); the only board-side code
//! is the two `VolatileMmio` constructions in `boot` and the IRQ entry.
//! House rule since `STORY-P1-09-09`: a write nobody read back is a wish —
//! every enable here returns its readback or a named refusal.

use crate::pl011::Mmio;

/// Distributor register offsets (GICv2).
pub mod gicd {
    /// Distributor control: bit 0 enables group-1 forwarding.
    pub const CTLR: usize = 0x000;
    /// Interrupt set-enable, bank 0 (INTIDs 0–31: SGIs and PPIs).
    pub const ISENABLER0: usize = 0x100;
}

/// CPU-interface register offsets (GICv2).
pub mod gicc {
    /// CPU interface control: bit 0 enables signalling to this core.
    pub const CTLR: usize = 0x000;
    /// Priority mask: interrupts with priority below this value are
    /// forwarded. GIC-400 implements the upper five bits.
    pub const PMR: usize = 0x004;
    /// Interrupt acknowledge: reading it claims the highest pending INTID.
    pub const IAR: usize = 0x00C;
    /// End of interrupt: writing the claimed value back retires it.
    pub const EOIR: usize = 0x010;
}

/// The EL1 virtual timer's private peripheral interrupt, architecturally
/// INTID 27 on every GIC the generic timer integrates with.
pub const VIRTUAL_TIMER_INTID: u32 = 27;

/// The INTID `GICC_IAR` returns when nothing was actually pending — a claim
/// that must **not** be retired with an `EOIR` write.
pub const SPURIOUS_INTID: u32 = 1023;

/// The priority mask value written: everything unmasked. GIC-400 implements
/// 32 priority levels, so the low three bits read as zero.
pub const PRIORITY_MASK_ALL: u32 = 0xF8;

/// Why the tick interrupt could not be routed. Each variant carries the
/// readback that convicted the register, for the report line — the decisive
/// half convention (`etherrors`) applied to a new peripheral.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GicRefused {
    /// `GICD_CTLR`'s enable bit did not hold.
    DistributorNotHeld(u32),
    /// `GICD_ISENABLER0` did not latch the timer PPI's bit.
    EnableNotHeld(u32),
    /// `GICC_PMR` did not hold the all-unmasked priority mask.
    MaskNotHeld(u32),
    /// `GICC_CTLR`'s enable bit did not hold.
    InterfaceNotHeld(u32),
    /// `CNTV_CTL_EL0` read back other than enabled-and-unmasked after the
    /// timer was armed — not a GIC register, but the same question ("why is
    /// there no tick?") and the same discipline (the readback convicts).
    TimerNotHeld(u32),
}

impl GicRefused {
    /// A short name for the report line.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            GicRefused::DistributorNotHeld(_) => "gicd-ctlr",
            GicRefused::EnableNotHeld(_) => "gicd-isenabler",
            GicRefused::MaskNotHeld(_) => "gicc-pmr",
            GicRefused::InterfaceNotHeld(_) => "gicc-ctlr",
            GicRefused::TimerNotHeld(_) => "cntv-ctl",
        }
    }

    /// The readback that convicted the register.
    #[must_use]
    pub const fn readback(self) -> u32 {
        match self {
            GicRefused::DistributorNotHeld(value)
            | GicRefused::EnableNotHeld(value)
            | GicRefused::MaskNotHeld(value)
            | GicRefused::InterfaceNotHeld(value)
            | GicRefused::TimerNotHeld(value) => value,
        }
    }
}

/// Routes the virtual-timer PPI to this core, believing each step from its
/// readback: distributor on, PPI 27 enabled, priority mask open, CPU
/// interface on — in that order, distributor before interface, so no
/// interrupt can be signalled before the source that raises it is routed.
///
/// # Errors
///
/// The first register whose readback disagrees, as a [`GicRefused`] carrying
/// the readback; nothing after a refusal is written.
pub fn enable_tick_interrupt<D: Mmio, C: Mmio>(gicd: &D, gicc: &C) -> Result<(), GicRefused> {
    gicd.write_u32(gicd::CTLR, 1);
    let ctlr = gicd.read_u32(gicd::CTLR);
    if ctlr & 1 == 0 {
        return Err(GicRefused::DistributorNotHeld(ctlr));
    }

    let timer_bit = 1u32 << VIRTUAL_TIMER_INTID;
    gicd.write_u32(gicd::ISENABLER0, timer_bit);
    let enabled = gicd.read_u32(gicd::ISENABLER0);
    if enabled & timer_bit == 0 {
        return Err(GicRefused::EnableNotHeld(enabled));
    }

    gicc.write_u32(gicc::PMR, PRIORITY_MASK_ALL);
    let mask = gicc.read_u32(gicc::PMR);
    if mask & PRIORITY_MASK_ALL != PRIORITY_MASK_ALL {
        return Err(GicRefused::MaskNotHeld(mask));
    }

    gicc.write_u32(gicc::CTLR, 1);
    let interface = gicc.read_u32(gicc::CTLR);
    if interface & 1 == 0 {
        return Err(GicRefused::InterfaceNotHeld(interface));
    }
    Ok(())
}

/// Claims the highest pending interrupt — one read of `GICC_IAR`.
#[must_use]
pub fn acknowledge<C: Mmio>(gicc: &C) -> u32 {
    gicc.read_u32(gicc::IAR)
}

/// Retires a claimed interrupt — one write of the claimed value to
/// `GICC_EOIR`. The caller must not pass [`SPURIOUS_INTID`].
pub fn end_of_interrupt<C: Mmio>(gicc: &C, claimed: u32) {
    gicc.write_u32(gicc::EOIR, claimed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::RefCell;

    /// A latching double: writes are remembered, reads replay the latest
    /// matching write — the `pcie::ProgrammableRc` pattern for a new block.
    struct LatchingBlock {
        writes: RefCell<Vec<(usize, u32)>>,
    }

    impl LatchingBlock {
        fn new() -> Self {
            LatchingBlock { writes: RefCell::new(Vec::new()) }
        }

        fn writes(&self) -> Vec<(usize, u32)> {
            self.writes.borrow().clone()
        }
    }

    impl Mmio for LatchingBlock {
        fn read_u32(&self, offset: usize) -> u32 {
            self.writes
                .borrow()
                .iter()
                .rev()
                .find(|(written, _)| *written == offset)
                .map_or(0, |(_, value)| *value)
        }

        fn write_u32(&self, offset: usize, value: u32) {
            self.writes.borrow_mut().push((offset, value));
        }
    }

    /// A block that answers every read with zero and latches nothing — the
    /// "assignment did not hold" case (`pcie::StickyMask`'s sibling).
    struct DeadBlock;

    impl Mmio for DeadBlock {
        fn read_u32(&self, _offset: usize) -> u32 {
            0
        }
        fn write_u32(&self, _offset: usize, _value: u32) {}
    }

    #[test]
    fn the_enable_sequence_writes_the_four_registers_in_routing_order() {
        let gicd = LatchingBlock::new();
        let gicc = LatchingBlock::new();
        assert_eq!(enable_tick_interrupt(&gicd, &gicc), Ok(()));
        // Distributor first — source routed before signalling is enabled.
        assert_eq!(
            gicd.writes(),
            vec![(gicd::CTLR, 1), (gicd::ISENABLER0, 1 << VIRTUAL_TIMER_INTID)]
        );
        assert_eq!(gicc.writes(), vec![(gicc::PMR, PRIORITY_MASK_ALL), (gicc::CTLR, 1)]);
    }

    #[test]
    fn a_dead_distributor_refuses_first_and_nothing_downstream_is_written() {
        let gicd = DeadBlock;
        let gicc = LatchingBlock::new();
        assert_eq!(enable_tick_interrupt(&gicd, &gicc), Err(GicRefused::DistributorNotHeld(0)));
        assert!(gicc.writes().is_empty(), "a refusal stops the sequence");
    }

    /// The set-enable register is write-1-to-set: a readback that kept other
    /// bits set alongside ours still holds our enable.
    struct BusyDistributor {
        inner: LatchingBlock,
    }

    impl Mmio for BusyDistributor {
        fn read_u32(&self, offset: usize) -> u32 {
            let raw = self.inner.read_u32(offset);
            if offset == gicd::ISENABLER0 {
                // SGIs 0–15 permanently enabled, as real GICs report.
                return raw | 0xFFFF;
            }
            raw
        }
        fn write_u32(&self, offset: usize, value: u32) {
            self.inner.write_u32(offset, value);
        }
    }

    #[test]
    fn other_enabled_interrupts_do_not_mask_the_timer_bit_check() {
        let gicd = BusyDistributor { inner: LatchingBlock::new() };
        let gicc = LatchingBlock::new();
        assert_eq!(enable_tick_interrupt(&gicd, &gicc), Ok(()));
    }

    #[test]
    fn a_priority_mask_that_reads_back_masked_is_a_refusal() {
        /// Latches everything except `PMR`, which reads back zero — every
        /// priority masked, so the routed interrupt could never arrive.
        struct MaskedInterface {
            inner: LatchingBlock,
        }
        impl Mmio for MaskedInterface {
            fn read_u32(&self, offset: usize) -> u32 {
                if offset == gicc::PMR {
                    return 0;
                }
                self.inner.read_u32(offset)
            }
            fn write_u32(&self, offset: usize, value: u32) {
                self.inner.write_u32(offset, value);
            }
        }
        let gicd = LatchingBlock::new();
        let gicc = MaskedInterface { inner: LatchingBlock::new() };
        assert_eq!(enable_tick_interrupt(&gicd, &gicc), Err(GicRefused::MaskNotHeld(0)));
    }

    #[test]
    fn acknowledge_reads_iar_and_eoi_writes_the_claim_back() {
        let gicc = LatchingBlock::new();
        gicc.write_u32(gicc::IAR, VIRTUAL_TIMER_INTID);
        assert_eq!(acknowledge(&gicc), VIRTUAL_TIMER_INTID);
        end_of_interrupt(&gicc, VIRTUAL_TIMER_INTID);
        assert_eq!(gicc.writes().last(), Some(&(gicc::EOIR, VIRTUAL_TIMER_INTID)));
    }

    #[test]
    fn every_refusal_names_itself_and_carries_its_readback() {
        let refusals = [
            GicRefused::DistributorNotHeld(0xAAAA_5555),
            GicRefused::EnableNotHeld(0x1234_5678),
            GicRefused::MaskNotHeld(0),
            GicRefused::InterfaceNotHeld(2),
            GicRefused::TimerNotHeld(0b10),
        ];
        let mut names: Vec<&str> = refusals.iter().map(|r| r.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), refusals.len(), "no two refusals share a name");
        assert_eq!(GicRefused::DistributorNotHeld(0xAAAA_5555).readback(), 0xAAAA_5555);
    }
}
