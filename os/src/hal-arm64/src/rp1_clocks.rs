//! RP1's clock generator block: the current switched on before the GEM is
//! asked to speak (`STORY-P1-09-12`).
//!
//! The identity rung read `0xDEAD` — RP1's fabric poison for a block whose
//! clock is not running — while the same silicon under Pi OS read
//! `0x00070109` through the same window with two enable bits set. The live
//! capture (`goals/reports/pios-ground-truth-2026-08-03.txt`) is the whole
//! design: the GEM's register-bus clock (`clk_sys`) is critical-always-on,
//! the two gateable consumers are `clk_eth` (tx) and `clk_eth_tsu`, and a
//! working system holds both at `CTRL = 0x10000800` — `ENABLE` (bit 11)
//! held, running-status (bit 28) answering, `AUXSRC = 0`, `DIV_INT = 1`.
//!
//! Nothing here programs a PLL, chooses a rate, or tunes a divider: the
//! values written are the architectural defaults transcribed from the
//! `rpi-6.12.y` driver source and confirmed live, every write is believed
//! only from its readback, and the block itself is read before it is
//! believed — a poisoned answer is a gate, not a mystery.

use crate::pl011::Mmio;

/// Register offsets inside the clocks block (RP1 bus `0x4001_8000`;
/// CPU [`crate::board::RP1_WINDOW_BASE`]` + `[`crate::board::RP1_CLOCKS_OFFSET`]).
/// Values from `clk-rp1.c`, confirmed by the live capture.
pub mod register {
    /// `CLK_SYS_SEL` — one-hot readback of `clk_sys`'s selected parent.
    /// The pre-flight gate: a credible block answers exactly one bit here
    /// (the capture read `0x4`, parent 2, `pll_sys`).
    pub const SYS_SEL: usize = 0x0020;
    /// `CLK_ETH_CTRL` — the GEM `tx_clk` (125 MHz from `pll_sys_sec`).
    pub const ETH_CTRL: usize = 0x0064;
    /// `CLK_ETH_DIV_INT`.
    pub const ETH_DIV_INT: usize = 0x0068;
    /// `CLK_ETH_TSU_CTRL` — the GEM `tsu_clk` (50 MHz from `xosc`).
    pub const ETH_TSU_CTRL: usize = 0x0134;
    /// `CLK_ETH_TSU_DIV_INT`.
    pub const ETH_TSU_DIV_INT: usize = 0x0138;
}

/// `CLK_CTRL_ENABLE` — the request bit a write may set.
pub const CTRL_ENABLE: u32 = 1 << 11;
/// The running-status readback bit — hardware's answer, never written.
pub const CTRL_RUNNING: u32 = 1 << 28;
/// The pinned enable write: `ENABLE` set, `AUXSRC = 0` — for `clk_eth`
/// aux parent 0 is `pll_sys_sec`, for `clk_eth_tsu` it is `xosc`, both the
/// architectural defaults the capture confirmed live.
pub const CTRL_ENABLE_VALUE: u32 = CTRL_ENABLE;
/// The pinned divider: unity, per the capture (`DIV_INT = 1` on both).
pub const DIV_INT_UNITY: u32 = 1;

/// How many times the running-status poll reads before concluding the clock
/// will never run. The glitch-free mux settles in a handful of parent
/// cycles; this bound exists to convert a hang into a return, not to
/// enforce a latency budget — the same rationale as
/// [`crate::gem::MDIO_POLL_LIMIT`], and no time constant anywhere.
pub const RUN_POLL_LIMIT: u32 = 100_000;

/// Why the clock rung refused. Each arm is a distinct confession code and
/// carries the readback that convicts it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockRefused {
    /// The pre-flight `CLK_SYS_SEL` read was not a credible one-hot answer —
    /// all-zeros, all-ones, or fabric poison. The block was never written.
    BlockSilent {
        /// The refused readback; its high half is the decisive detail
        /// (poison spells `0xDEAD` = 57005).
        sel: u32,
    },
    /// The enable write did not hold: the readback came back without the
    /// enable bit.
    EnableNotHeld {
        /// The post-write readback; its low half is the decisive detail.
        ctrl: u32,
    },
    /// The enable held but the running status never answered within the
    /// attempt budget.
    NeverRan {
        /// The final readback; its high half is the decisive detail.
        ctrl: u32,
    },
}

/// Switches the two gateable Ethernet clocks on, believing only readbacks:
/// the block pre-flight first, then per clock an exactly-once pinned write
/// pair (divider, then control) and a bounded running poll. A clock already
/// enabled and running is left untouched — zero writes on the happy
/// re-probe pass, so the rung is idempotent from the park loop.
pub fn enable_ethernet_clocks<M: Mmio>(clocks: &M) -> Result<(), ClockRefused> {
    let sel = clocks.read_u32(register::SYS_SEL);
    if sel.count_ones() != 1 {
        return Err(ClockRefused::BlockSilent { sel });
    }
    enable_one(clocks, register::ETH_CTRL, register::ETH_DIV_INT)?;
    enable_one(clocks, register::ETH_TSU_CTRL, register::ETH_TSU_DIV_INT)
}

/// One clock's enable: skip if already running, otherwise write the pinned
/// pair, believe the enable only from readback, then poll running under the
/// attempt budget.
fn enable_one<M: Mmio>(clocks: &M, ctrl: usize, div: usize) -> Result<(), ClockRefused> {
    let before = clocks.read_u32(ctrl);
    if before & CTRL_ENABLE != 0 && before & CTRL_RUNNING != 0 {
        return Ok(());
    }
    if before & CTRL_ENABLE == 0 {
        clocks.write_u32(div, DIV_INT_UNITY);
        clocks.write_u32(ctrl, CTRL_ENABLE_VALUE);
        let held = clocks.read_u32(ctrl);
        if held & CTRL_ENABLE == 0 {
            return Err(ClockRefused::EnableNotHeld { ctrl: held });
        }
        if held & CTRL_RUNNING != 0 {
            return Ok(());
        }
    }
    let mut last = 0;
    let mut attempts = 0;
    while attempts < RUN_POLL_LIMIT {
        attempts += 1;
        last = clocks.read_u32(ctrl);
        if last & CTRL_RUNNING != 0 {
            return Ok(());
        }
    }
    Err(ClockRefused::NeverRan { ctrl: last })
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;

    /// A clocks-block double: scripted `SYS_SEL`, per-clock control
    /// behaviour, and a full write log. Panics on any write when
    /// `writable` is false — the pre-flight clause's teeth.
    struct ClocksDouble {
        sys_sel: u32,
        writable: bool,
        /// Reads of each ctrl register return this once enabled…
        run_after_reads: Option<u32>,
        /// …counting reads since the enable write per register.
        eth_reads: Cell<u32>,
        tsu_reads: Cell<u32>,
        eth_enabled: Cell<bool>,
        tsu_enabled: Cell<bool>,
        enable_sticks: bool,
        /// Pre-enabled-and-running clocks (the idempotent pass).
        already_running: bool,
        writes: Cell<u32>,
        eth_ctrl_writes: Cell<u32>,
        tsu_ctrl_writes: Cell<u32>,
    }

    impl ClocksDouble {
        fn new(sys_sel: u32) -> Self {
            ClocksDouble {
                sys_sel,
                writable: true,
                run_after_reads: Some(1),
                eth_reads: Cell::new(0),
                tsu_reads: Cell::new(0),
                eth_enabled: Cell::new(false),
                tsu_enabled: Cell::new(false),
                enable_sticks: true,
                already_running: false,
                writes: Cell::new(0),
                eth_ctrl_writes: Cell::new(0),
                tsu_ctrl_writes: Cell::new(0),
            }
        }

        fn read_only(sys_sel: u32) -> Self {
            ClocksDouble { writable: false, ..ClocksDouble::new(sys_sel) }
        }

        fn ctrl_value(&self, enabled: &Cell<bool>, reads: &Cell<u32>) -> u32 {
            if self.already_running {
                return CTRL_ENABLE | CTRL_RUNNING;
            }
            if !enabled.get() {
                return 0;
            }
            if !self.enable_sticks {
                return 0;
            }
            reads.set(reads.get() + 1);
            match self.run_after_reads {
                Some(after) if reads.get() > after => CTRL_ENABLE | CTRL_RUNNING,
                _ => CTRL_ENABLE,
            }
        }
    }

    impl Mmio for ClocksDouble {
        fn read_u32(&self, offset: usize) -> u32 {
            match offset {
                register::SYS_SEL => self.sys_sel,
                register::ETH_CTRL => self.ctrl_value(&self.eth_enabled, &self.eth_reads),
                register::ETH_TSU_CTRL => self.ctrl_value(&self.tsu_enabled, &self.tsu_reads),
                _ => panic!("unscripted read at {offset:#x}"),
            }
        }

        fn write_u32(&self, offset: usize, value: u32) {
            assert!(self.writable, "the block was written before it was believed ({offset:#x})");
            self.writes.set(self.writes.get() + 1);
            match offset {
                register::ETH_DIV_INT | register::ETH_TSU_DIV_INT => {
                    assert_eq!(value, DIV_INT_UNITY, "divider must be the pinned unity value");
                }
                register::ETH_CTRL => {
                    assert_eq!(value, CTRL_ENABLE_VALUE, "ctrl write must be the pinned value");
                    self.eth_ctrl_writes.set(self.eth_ctrl_writes.get() + 1);
                    self.eth_enabled.set(true);
                }
                register::ETH_TSU_CTRL => {
                    assert_eq!(value, CTRL_ENABLE_VALUE, "ctrl write must be the pinned value");
                    self.tsu_ctrl_writes.set(self.tsu_ctrl_writes.get() + 1);
                    self.tsu_enabled.set(true);
                }
                _ => panic!("unscripted write at {offset:#x}"),
            }
        }
    }

    // TEST-P1-09-12-A clause 1: the pre-flight gate refuses poison honestly.
    #[test]
    fn a_silent_block_is_refused_before_any_write() {
        for hostile in [0u32, 0xFFFF_FFFF, 0xDEAD_0000, 0xDEAD_BEEF] {
            let block = ClocksDouble::read_only(hostile);
            assert_eq!(
                enable_ethernet_clocks(&block),
                Err(ClockRefused::BlockSilent { sel: hostile }),
                "sel {hostile:#010x} must refuse as the pre-flight arm",
            );
        }
    }

    #[test]
    fn a_credible_one_hot_readback_passes_the_gate() {
        // The capture's own value, and every other single-bit answer.
        for credible in [0x4u32, 0x1, 0x2, 0x8000_0000] {
            let block = ClocksDouble::new(credible);
            assert_eq!(enable_ethernet_clocks(&block), Ok(()));
        }
    }

    // TEST-P1-09-12-A clause 2: enable is a write believed only by readback.
    #[test]
    fn each_clock_is_enabled_by_exactly_one_pinned_write_pair() {
        let block = ClocksDouble::new(0x4);
        assert_eq!(enable_ethernet_clocks(&block), Ok(()));
        assert_eq!(block.eth_ctrl_writes.get(), 1, "one ctrl write for clk_eth");
        assert_eq!(block.tsu_ctrl_writes.get(), 1, "one ctrl write for clk_eth_tsu");
        assert_eq!(block.writes.get(), 4, "two divider writes + two ctrl writes, nothing else");
    }

    #[test]
    fn an_enable_that_does_not_hold_is_refused_with_the_readback() {
        let mut block = ClocksDouble::new(0x4);
        block.enable_sticks = false;
        assert_eq!(
            enable_ethernet_clocks(&block),
            Err(ClockRefused::EnableNotHeld { ctrl: 0 }),
            "a readback without the enable bit is the enable-refused arm",
        );
    }

    #[test]
    fn a_clock_already_running_sees_zero_writes() {
        let mut block = ClocksDouble::new(0x4);
        block.already_running = true;
        assert_eq!(enable_ethernet_clocks(&block), Ok(()));
        assert_eq!(block.writes.get(), 0, "the happy re-probe pass writes nothing");
    }

    // TEST-P1-09-12-A clause 3: the run poll is bounded and honest.
    #[test]
    fn a_clock_that_never_runs_exhausts_exactly_the_budget_and_refuses() {
        let mut block = ClocksDouble::new(0x4);
        block.run_after_reads = None;
        assert_eq!(
            enable_ethernet_clocks(&block),
            Err(ClockRefused::NeverRan { ctrl: CTRL_ENABLE }),
            "the final readback names the status",
        );
        // One post-write held readback plus exactly the budgeted attempts.
        assert_eq!(block.eth_reads.get(), 1 + RUN_POLL_LIMIT);
        assert_eq!(block.tsu_reads.get(), 0, "the second clock is never touched after a refusal");
    }

    #[test]
    fn a_clock_that_runs_on_attempt_k_is_read_exactly_k_more_times() {
        let mut block = ClocksDouble::new(0x4);
        block.run_after_reads = Some(7);
        assert_eq!(enable_ethernet_clocks(&block), Ok(()));
        // 1 held readback + 7 polls that saw ENABLE only + the 8th that saw
        // RUNNING (ctrl_value counts every post-enable read).
        assert_eq!(block.eth_reads.get(), 8);
    }

    #[test]
    fn the_register_map_is_the_capture() {
        // Pinned against clk-rp1.c and the live dump; a transposed offset
        // writes some other clock and must fail here, not on the board.
        assert_eq!(register::SYS_SEL, 0x20);
        assert_eq!(register::ETH_CTRL, 0x64);
        assert_eq!(register::ETH_DIV_INT, 0x68);
        assert_eq!(register::ETH_TSU_CTRL, 0x134);
        assert_eq!(register::ETH_TSU_DIV_INT, 0x138);
        assert_eq!(CTRL_ENABLE, 0x800);
        assert_eq!(CTRL_RUNNING, 0x1000_0000);
        const {
            assert!(register::ETH_TSU_CTRL < crate::board::RP1_CLOCKS_SIZE);
        }
    }
}
