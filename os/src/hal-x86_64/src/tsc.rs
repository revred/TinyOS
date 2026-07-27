//! x86_64 cycle source (`RDTSC`) and its PIT-calibrated timebase
//! (`STORY-P1-01-01`).
//!
//! This is the x86_64 implementor of [`hal::time::CycleSource`] — the arch
//! seam that keeps `kernel::measure` free of any `RDTSC` mention, so the
//! ARM64/Pi 5 slice tracked as loose end `LE-09` can supply a `CNTVCT_EL0`
//! implementor later without touching the harness or any fixture.
//!
//! [`calibrate_cycles_per_us`] exists to close a specific, named evidence gap
//! rather than to be generally useful: `REPORT-2026-07-27-01` found that most
//! of `PERF-D07`'s latency guardrails are denominated in **microseconds**
//! while every number `STORY-P0-03-01` had gathered was denominated in
//! **cycles**, with no documented conversion — so those guardrails could not
//! be scored at all, in either direction. Measuring the timestamp counter
//! against the one always-present fixed-frequency reference on a PC-compatible
//! machine (the 8254 PIT, 1.193182 MHz by construction) supplies that
//! conversion from inside the guest, with no host-side assumption about how
//! fast QEMU happens to be running.
//!
//! **What this does and does not prove.** It establishes what one guest-side
//! microsecond *is*, in that guest's own cycles, on that run. Under QEMU/TCG
//! both the TSC and the PIT are software models, so the resulting factor
//! describes the emulation, not silicon: a microsecond figure derived from it
//! is still Tier 0 evidence and never hardware WCET evidence. On real hardware
//! (including the Pi 5, whose ARM64 generic timer reports its own frequency
//! directly and needs no such calibration) the same factor is a real one.

use hal::time::{CycleSource, Timebase};

/// The 8254 PIT's fixed input frequency in Hz — 1.193182 MHz, a hardware
/// constant on every PC-compatible machine including QEMU's `q35` model.
const PIT_FREQUENCY_HZ: u64 = 1_193_182;

/// PIT channel-2 data port.
const PIT_CHANNEL2_DATA: u16 = 0x42;
/// PIT mode/command port.
const PIT_COMMAND: u16 = 0x43;
/// NMI status and control port: bit 0 gates PIT channel 2, bit 1 enables the
/// speaker (kept **off** — this calibration must not make noise on real
/// hardware), and bit 5 mirrors channel 2's OUT pin, which is how this code
/// observes terminal count without an interrupt.
const NMI_STATUS_AND_CONTROL: u16 = 0x61;
/// `NMI_STATUS_AND_CONTROL` bit 5: channel 2's OUT pin state.
const CHANNEL2_OUT: u8 = 0x20;
/// `NMI_STATUS_AND_CONTROL` bit 0: channel 2's gate.
const CHANNEL2_GATE: u8 = 0x01;
/// `NMI_STATUS_AND_CONTROL` bit 1: speaker enable.
const SPEAKER_ENABLE: u8 = 0x02;

/// Channel 2, lobyte/hibyte access, mode 0 (interrupt on terminal count),
/// binary counting — mode 0 is the one-shot mode whose OUT pin goes high
/// exactly once, at terminal count.
const CHANNEL2_ONE_SHOT: u8 = 0xB0;

/// Calibration interval: 10 ms, the shortest window that still averages the
/// polling granularity below down to well under a percent, and short enough
/// that a fixture pays it once per boot without noticing.
const CALIBRATION_US: u64 = 10_000;

/// PIT ticks in [`CALIBRATION_US`], derived from the frequency constant rather
/// than written as a literal so the two can never drift apart. Truncation
/// costs under a microsecond of window, which [`ticks_to_us`] then reports
/// exactly — the conversion always uses the *real* window length, never the
/// nominal one.
const CALIBRATION_TICKS: u16 = (CALIBRATION_US * PIT_FREQUENCY_HZ / 1_000_000) as u16;

/// Upper bound on OUT-pin polls before this calibration gives up and reports
/// no timebase. The wait is ~10 ms of real time; a single port read costs far
/// more than a nanosecond even under emulation, so this bound is generous by
/// orders of magnitude while still making the loop provably terminating — no
/// unbounded loop on any code path, per
/// `agent/CODING_STANDARDS.md#real-time-discipline-kernel-and-driver-code`.
const MAX_POLLS: u32 = 5_000_000;

/// Implausibility bounds on the result. A factor outside this range means the
/// PIT, the TSC, or the emulation of either is not behaving as assumed, and
/// reporting no timebase is the only honest outcome: a fabricated factor would
/// silently turn every derived microsecond figure into an unfalsifiable claim.
const MIN_PLAUSIBLE_CYCLES_PER_US: u32 = 10;
/// See [`MIN_PLAUSIBLE_CYCLES_PER_US`] — 100 GHz, comfortably above any real
/// or emulated clock.
const MAX_PLAUSIBLE_CYCLES_PER_US: u32 = 100_000;

/// Reads a byte from `port`.
///
/// # Safety
/// `port` must name an I/O port whose read has no side effect the caller
/// hasn't accounted for — true for the two ports this module reads (the PIT's
/// own command/data ports and `0x61`'s status bits).
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    // SAFETY: port I/O is unconditionally available on x86_64; the caller's
    // contract covers the only hazard (a port whose read has a side effect).
    unsafe {
        core::arch::asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags));
    }
    value
}

/// Writes `value` to `port`.
///
/// # Safety
/// `port` must name an I/O port whose write has no side effect the caller
/// hasn't accounted for. Every write this module performs targets PIT channel
/// 2 or `0x61`'s gate/speaker bits, and the sequence below always restores the
/// gate it changed.
unsafe fn outb(port: u16, value: u8) {
    // SAFETY: same rationale as `inb`.
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
    }
}

/// The x86_64 timestamp counter as a [`CycleSource`].
///
/// Zero-sized: `RDTSC` needs no state and is unconditionally available on
/// every x86_64 CPU, so there is nothing to initialize and nothing that can
/// fail — unlike the [`Timebase`] beside it, which genuinely can.
#[derive(Debug, Clone, Copy, Default)]
pub struct Tsc;

impl Tsc {
    /// The name this source reports in a measurement envelope's
    /// `cycle_source=` field, so an artifact says which counter produced it
    /// instead of leaving a reader to infer it from the architecture.
    pub const NAME: &'static str = "rdtsc";
}

impl CycleSource for Tsc {
    fn read_cycles(&self) -> u64 {
        // SAFETY: `RDTSC` is unconditionally available on every x86_64 CPU
        // (including QEMU's TCG model) and has no memory or control-flow
        // side effect — reading it is sound anywhere, which is exactly why
        // `CycleSource::read_cycles` is a safe method.
        unsafe { core::arch::x86_64::_rdtsc() }
    }
}

/// A timebase established by measuring the TSC against the PIT, or the honest
/// absence of one.
///
/// Constructed by [`calibrate_cycles_per_us`]; carries `None` whenever that
/// measurement could not be trusted, and every consumer downstream then
/// reports cycles only.
#[derive(Debug, Clone, Copy, Default)]
pub struct PitTimebase {
    cycles_per_us: Option<u32>,
}

impl PitTimebase {
    /// Wraps an already-known factor (or its absence) — used by tests and by
    /// a caller reproducing a previously-reported run.
    pub const fn from_cycles_per_us(cycles_per_us: Option<u32>) -> Self {
        PitTimebase { cycles_per_us }
    }
}

impl Timebase for PitTimebase {
    fn cycles_per_us(&self) -> Option<u32> {
        self.cycles_per_us
    }
}

/// Measures the timestamp counter against one 10 ms PIT channel-2 one-shot and
/// returns the resulting cycles-per-microsecond factor, or a [`PitTimebase`]
/// carrying `None` when the result cannot be trusted.
///
/// The speaker is explicitly kept disabled throughout (bit 1 of `0x61` is
/// cleared before the gate is raised), so this is silent on real hardware, and
/// the gate is restored to its pre-call state before returning.
///
/// # Safety
/// The caller must have exclusive use of PIT channel 2 and of port `0x61`'s
/// gate/speaker bits for the duration of the call, and must not be relying on
/// channel 2 for anything else (nothing in this codebase does — `interrupts.rs`
/// uses the local APIC timer, not the PIT). Interrupts may be enabled: an
/// interrupt landing inside the measured window inflates the observed cycle
/// count and therefore the factor, so a caller wanting the tightest factor
/// calls this before arming any timer, exactly as the Tier 0 measurement
/// fixture does.
pub unsafe fn calibrate_cycles_per_us() -> PitTimebase {
    let source = Tsc;

    // SAFETY: every port access below targets PIT channel 2 or `0x61`'s
    // gate/speaker bits, which this function's own contract gives it
    // exclusive use of. The sequence is the standard 8254 one-shot protocol:
    // drop the gate (and the speaker) so the channel is quiescent, program
    // mode 0 with the tick count, then raise the gate to start counting.
    let baseline = unsafe { inb(NMI_STATUS_AND_CONTROL) } & !(CHANNEL2_GATE | SPEAKER_ENABLE);
    // SAFETY: as above.
    unsafe {
        outb(NMI_STATUS_AND_CONTROL, baseline);
        outb(PIT_COMMAND, CHANNEL2_ONE_SHOT);
        outb(PIT_CHANNEL2_DATA, (CALIBRATION_TICKS & 0xFF) as u8);
        outb(PIT_CHANNEL2_DATA, (CALIBRATION_TICKS >> 8) as u8);
    }

    let start = source.read_cycles();
    // SAFETY: as above — raising the gate starts channel 2 counting down.
    unsafe { outb(NMI_STATUS_AND_CONTROL, baseline | CHANNEL2_GATE) };

    let mut polls = 0u32;
    let end = loop {
        // SAFETY: reading `0x61` has no side effect beyond reporting status.
        if unsafe { inb(NMI_STATUS_AND_CONTROL) } & CHANNEL2_OUT != 0 {
            break source.read_cycles();
        }
        polls += 1;
        if polls >= MAX_POLLS {
            // Terminal count never observed: restore the gate and report no
            // timebase rather than a factor derived from a timed-out wait.
            // SAFETY: as above.
            unsafe { outb(NMI_STATUS_AND_CONTROL, baseline) };
            return PitTimebase { cycles_per_us: None };
        }
    };

    // SAFETY: as above — leave the channel quiescent, as found.
    unsafe { outb(NMI_STATUS_AND_CONTROL, baseline) };

    let elapsed = end.saturating_sub(start);
    let elapsed_us = ticks_to_us(CALIBRATION_TICKS);
    PitTimebase { cycles_per_us: plausible_factor(elapsed, elapsed_us) }
}

/// The real length of a `ticks`-long channel-2 one-shot, in microseconds.
/// Split out (and unit-tested on the host) because it is the one place the
/// PIT's frequency constant turns into the denominator of every derived
/// microsecond figure — an error here would scale every result silently.
const fn ticks_to_us(ticks: u16) -> u64 {
    (ticks as u64) * 1_000_000 / PIT_FREQUENCY_HZ
}

/// Divides `elapsed` cycles by `elapsed_us` microseconds, returning the factor
/// only when it is inside [`MIN_PLAUSIBLE_CYCLES_PER_US`]..=[`MAX_PLAUSIBLE_CYCLES_PER_US`].
const fn plausible_factor(elapsed: u64, elapsed_us: u64) -> Option<u32> {
    if elapsed_us == 0 || elapsed == 0 {
        return None;
    }
    let factor = elapsed / elapsed_us;
    if factor < MIN_PLAUSIBLE_CYCLES_PER_US as u64 || factor > MAX_PLAUSIBLE_CYCLES_PER_US as u64 {
        return None;
    }
    Some(factor as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The PIT arithmetic is host-testable even though the port I/O around it
    // is not: `CALIBRATION_TICKS` must really be ~10 ms at the PIT's fixed
    // frequency, or every derived microsecond figure is scaled wrongly.
    #[test]
    fn the_calibration_window_really_is_ten_milliseconds() {
        // The derived tick count truncates, so the real window is within one
        // microsecond of the nominal 10 ms — and `ticks_to_us` reports the
        // real one, which is what every derived figure divides by.
        let real = ticks_to_us(CALIBRATION_TICKS);
        assert!(
            real.abs_diff(CALIBRATION_US) <= 1,
            "{CALIBRATION_TICKS} ticks is {real} us, not ~{CALIBRATION_US} us"
        );
    }

    #[test]
    fn a_plausible_measurement_yields_its_factor() {
        // 25,000,000 cycles across 10,000 us = 2,500 cycles/us (a 2.5 GHz
        // machine).
        assert_eq!(plausible_factor(25_000_000, 10_000), Some(2_500));
    }

    #[test]
    fn an_implausible_measurement_yields_no_timebase_rather_than_a_guess() {
        // Far too slow to be a cycle counter (1 cycle/us).
        assert_eq!(plausible_factor(10_000, 10_000), None);
        // Far too fast (1,000,000 cycles/us = 1 PHz).
        assert_eq!(plausible_factor(10_000_000_000, 10_000), None);
        // Degenerate inputs.
        assert_eq!(plausible_factor(0, 10_000), None);
        assert_eq!(plausible_factor(25_000_000, 0), None);
    }

    #[test]
    fn an_absent_timebase_reports_absent_not_zero() {
        assert_eq!(PitTimebase::from_cycles_per_us(None).cycles_per_us(), None);
        assert_eq!(PitTimebase::from_cycles_per_us(Some(1_000)).cycles_per_us(), Some(1_000));
    }
}
