//! AArch64 generic-timer cycle source and timebase (`STORY-P1-01-03`).
//!
//! The second implementor of [`hal::time::CycleSource`], and the reason it
//! exists: `STORY-P1-01-01` designed that trait as the arch seam keeping
//! `kernel::measure` free of any `RDTSC` mention, but a seam with one
//! implementor proves nothing. Everything here is written so the claim can be
//! checked on the x86_64 dev machine today, months before a Raspberry Pi 5
//! runs anything.
//!
//! **Simpler than x86_64 in exactly one respect.** The generic timer reports
//! its own frequency architecturally in `CNTFRQ_EL0`, so [`GenericTimerTimebase`]
//! is one register read and a division — no PIT-style measured calibration of
//! the kind `hal_x86_64::tsc::calibrate_cycles_per_us` needs, and therefore no
//! 10 ms window, no port I/O and no timeout path.
//!
//! **Coarser than x86_64 in one respect that matters more.** `CNTVCT_EL0` is a
//! fixed-frequency *system* counter, not a CPU cycle counter. On a Pi 5 it runs
//! at 54 MHz — one tick is ~18.5 ns, against ~0.43 ns for a 2.3 GHz TSC tick.
//! A ~100 ns context switch is therefore about five ticks on that board, and
//! the smallest measured operations will be quantization-limited rather than
//! noise-limited. That is a real property of the hardware, not a defect to
//! correct here; it is recorded as loose end `LE-15`, whose named alternative
//! (`PMCCNTR_EL0`, the PMU cycle counter) is not architecturally guaranteed to
//! be accessible and must be enabled at EL1 first.
//!
//! **Why the register reads sit behind traits.** The two `mrs` instructions
//! are the only part of this module that cannot run on the host. Putting them
//! behind [`VirtualCounter`] and [`CounterFrequency`] makes them *also* the
//! only untested part: the cycle source, the conversion and the plausibility
//! policy are all driven by host doubles in this module's own tests. The two
//! traits are separate rather than one two-method trait because they serve the
//! already-separate [`CycleSource`]/[`Timebase`] split
//! (`agent/CODING_STANDARDS.md#i--interface-segregation`) — counting code that
//! never converts to microseconds must not depend on a frequency register it
//! never reads.

use hal::time::{CycleSource, Timebase};

/// The lowest counter frequency this backend will derive a timebase from.
///
/// Below 1 MHz a counter cannot produce a nonzero cycles-per-microsecond
/// figure at all, so any smaller value is a bad read rather than slow
/// hardware. Note this floor is deliberately **not** `hal_x86_64::tsc`'s: that
/// backend rejects anything under 10 cycles/µs because a timestamp counter
/// that slow would be broken, whereas a generic timer at 1–2 MHz is an
/// ordinary, conforming implementation. Copying the other backend's bounds
/// would have rejected valid boards.
const MIN_PLAUSIBLE_HZ: u64 = 1_000_000;

/// The highest counter frequency this backend will derive a timebase from —
/// 100 GHz, orders of magnitude above any implemented generic timer, so a
/// larger value indicates a failed read rather than a fast board.
const MAX_PLAUSIBLE_HZ: u64 = 100_000_000_000;

/// Reads the generic timer's virtual count (`CNTVCT_EL0` on hardware).
///
/// One method, deliberately: see this module's documentation on why this is
/// not merged with [`CounterFrequency`].
///
/// # Contract every implementor must honor
///
/// The value returned is the counter's own reading, unmodified — no clamping,
/// no latching, no caching. An implementor that repaired a non-monotonic
/// counter here would hide a real hardware fault behind a
/// [`hal::time::conformance`] pass.
pub trait VirtualCounter {
    /// Reads the current count.
    fn count(&self) -> u64;
}

/// Reads the generic timer's counter frequency in Hz (`CNTFRQ_EL0` on
/// hardware).
///
/// # Contract every implementor must honor
///
/// The value returned is the register's own contents, including zero.
/// `CNTFRQ_EL0` is programmed by firmware and firmware sometimes does not
/// program it; substituting a plausible default here would convert a known
/// firmware defect into an unfalsifiable timing claim. Judging the value is
/// [`plausible_cycles_per_us`]'s job, not the reader's.
pub trait CounterFrequency {
    /// Reads the counter frequency, in Hz.
    fn hertz(&self) -> u64;
}

/// Reads the generic timer's **physical** count (`CNTPCT_EL0` on hardware).
///
/// The sibling `LE-103` was filed for. A separate trait rather than a second
/// method on [`VirtualCounter`], for the same interface-segregation reason
/// that trait is separate from [`CounterFrequency`] — and because the two
/// registers are not interchangeable: `CNTVCT_EL0` is `CNTPCT_EL0` minus
/// `CNTVOFF_EL2`, so an instrument that must detect a moved offset (`ADR 0005`
/// Q3) needs the physical read *by type*, where handing it the virtual one is
/// a compile error rather than a probe blind to exactly what it measures.
pub trait PhysicalCounter {
    /// Reads the current count. Same contract as [`VirtualCounter::count`]:
    /// the register's own reading, unmodified.
    fn count(&self) -> u64;
}

/// One paired read of the physical and virtual counters.
///
/// The pair is the `Q3` primitive: their difference is `CNTVOFF_EL2` as seen
/// from NS-EL1, the register the architecture forbids NS-EL1 to read
/// directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CounterSplit {
    /// The physical count (`CNTPCT_EL0`).
    pub cntpct: u64,
    /// The virtual count (`CNTVCT_EL0`), read immediately after.
    pub cntvct: u64,
}

impl CounterSplit {
    /// The virtual offset the pair implies: `cntpct - cntvct`, wrapping.
    ///
    /// Wrapping rather than saturating because a virtual counter *ahead* of
    /// the physical one (a negative `CNTVOFF_EL2`) is a real configuration,
    /// and clamping it to zero would be the exact laundering this module's
    /// counter contracts forbid.
    #[must_use]
    pub const fn virtual_offset(&self) -> u64 {
        self.cntpct.wrapping_sub(self.cntvct)
    }
}

/// Reads both counters back to back, physical first.
///
/// The reads are not atomic — the counter advances between them by however
/// long the second `mrs` takes — so the derived offset is exact only to
/// within that skew, which at 54 MHz is well under one tick.
pub fn read_counter_split<P: PhysicalCounter, V: VirtualCounter>(
    physical: &P,
    virtual_counter: &V,
) -> CounterSplit {
    CounterSplit { cntpct: physical.count(), cntvct: virtual_counter.count() }
}

/// What the residency probe observed over one window.
///
/// Three counters over the same interval. `pmccntr_delta` is the work-rate
/// reference, `cntpct_ticks` is the window as the un-offsettable physical
/// counter saw it, and `cntvct_ticks` is the same window as the virtual
/// counter tells it — so `cntpct_ticks - cntvct_ticks` is any `CNTVOFF_EL2`
/// movement that happened *inside the window*, the phenomenon `LE-103`
/// records the previous probe as blind to by construction.
#[derive(Debug, Clone, Copy)]
pub struct ResidencyProbe {
    /// `PMCCNTR_EL0`'s advance across the window.
    pub pmccntr_delta: u64,
    /// The window's width in **physical** counter ticks.
    pub cntpct_ticks: u64,
    /// The same window in **virtual** counter ticks.
    pub cntvct_ticks: u64,
}

/// Spins until the **physical** counter has advanced `window_ticks`, bounded
/// by `max_spins`, and reports all three counters' advances.
///
/// The window is anchored to `CNTPCT_EL0` deliberately: a probe windowed on
/// the virtual counter hands the length of its own window to whatever owns
/// `CNTVOFF_EL2`, which is `LE-103`'s finding stated as code. Generic over
/// the register seams so the anchoring claim is host-tested with scripted
/// doubles before the registers ever answer.
#[must_use]
pub fn probe_residency_window<P: PhysicalCounter, V: VirtualCounter, C: VirtualCounter>(
    physical: &P,
    virtual_counter: &V,
    pmu: &C,
    window_ticks: u64,
    max_spins: u32,
) -> ResidencyProbe {
    let pmu_start = pmu.count();
    let physical_start = physical.count();
    let virtual_start = virtual_counter.count();
    // Bounded: a stuck CNTPCT converts to a short window, not a hang — the
    // same conversion `probe_pmccntr` makes for a stuck CNTVCT.
    let mut spins: u32 = 0;
    while physical.count().wrapping_sub(physical_start) < window_ticks && spins < max_spins {
        spins += 1;
    }
    ResidencyProbe {
        pmccntr_delta: pmu.count().wrapping_sub(pmu_start),
        cntpct_ticks: physical.count().wrapping_sub(physical_start),
        cntvct_ticks: virtual_counter.count().wrapping_sub(virtual_start),
    }
}

/// The generic timer's virtual counter as a [`CycleSource`].
///
/// Generic over the register seam so the same code that runs against `mrs` on
/// a board runs against a scripted double on the host — which is how this
/// backend is tested at all before a board exists.
#[derive(Debug, Clone, Copy, Default)]
pub struct Cntvct<R> {
    counter: R,
}

impl<R> Cntvct<R> {
    /// The name this source reports in a measurement envelope's
    /// `cycle_source=` field, so an artifact says which counter produced it
    /// rather than leaving a reader to infer it from the architecture.
    pub const NAME: &'static str = "cntvct_el0";

    /// The name this backend reports in an envelope's `arch=` field.
    pub const ARCH: &'static str = "aarch64";

    /// Wraps a virtual-counter register seam.
    pub const fn new(counter: R) -> Self {
        Cntvct { counter }
    }
}

impl<R: VirtualCounter> CycleSource for Cntvct<R> {
    fn read_cycles(&self) -> u64 {
        self.counter.count()
    }
}

/// A timebase derived from the counter frequency register, or the honest
/// absence of one.
///
/// Constructed by [`GenericTimerTimebase::from_register`]; carries `None`
/// whenever the frequency could not be trusted, and every consumer downstream
/// then reports cycles only (the `TOS64-MEAS/2` envelope emits
/// `cycles_per_us=unknown`).
#[derive(Debug, Clone, Copy, Default)]
pub struct GenericTimerTimebase {
    cycles_per_us: Option<u32>,
}

impl GenericTimerTimebase {
    /// Wraps an already-known factor (or its absence) — used by tests and by a
    /// caller reproducing a previously-reported run.
    pub const fn from_cycles_per_us(cycles_per_us: Option<u32>) -> Self {
        GenericTimerTimebase { cycles_per_us }
    }

    /// Derives the factor from a counter-frequency register.
    ///
    /// This is the whole calibration procedure on AArch64: one read, one
    /// division, one plausibility judgement.
    pub fn from_register<F: CounterFrequency>(frequency: &F) -> Self {
        GenericTimerTimebase { cycles_per_us: plausible_cycles_per_us(frequency.hertz()) }
    }
}

impl Timebase for GenericTimerTimebase {
    fn cycles_per_us(&self) -> Option<u32> {
        self.cycles_per_us
    }
}

/// Converts a counter frequency in Hz to cycles per microsecond, or `None`
/// when the frequency is outside [`MIN_PLAUSIBLE_HZ`]..=[`MAX_PLAUSIBLE_HZ`].
///
/// **Rounds to nearest**, not toward zero. A 24.576 MHz counter is 24.576
/// cycles/µs; truncating to 24 would scale every derived microsecond figure on
/// that board by 2.3% in the flattering direction, and `Timebase`'s `u32`
/// return leaves no way to express the fraction, so nearest is the most honest
/// representation available.
///
/// Zero is rejected explicitly by the lower bound, because firmware that never
/// programmed `CNTFRQ_EL0` is a real ARM condition and a truncating divide
/// would turn it into a silent timebase of 0 — a factor that reports every
/// duration as infinite rather than as unknown.
pub const fn plausible_cycles_per_us(hertz: u64) -> Option<u32> {
    if hertz < MIN_PLAUSIBLE_HZ || hertz > MAX_PLAUSIBLE_HZ {
        return None;
    }
    Some(((hertz + 500_000) / 1_000_000) as u32)
}

/// The real `CNTVCT_EL0`/`CNTFRQ_EL0` reads, compiled only when targeting
/// AArch64.
///
/// Zero-sized: both registers are readable at EL0/EL1 with no setup and no
/// state, so there is nothing to initialize and nothing that can fail — the
/// judgement about whether `CNTFRQ_EL0`'s *contents* can be trusted belongs to
/// [`plausible_cycles_per_us`], not here.
///
/// First executed on silicon by `STORY-P1-07-03`'s cache probe and
/// `STORY-P1-07-04`'s conformance run — the run `LE-27` closes on. Until
/// that capture is quoted in `TEST-P1-07-04-A`, they remain compiled and
/// reviewed rather than verified, exactly as `STORY-P1-01-03`'s Report said.
#[cfg(target_arch = "aarch64")]
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemRegisters;

#[cfg(target_arch = "aarch64")]
impl VirtualCounter for SystemRegisters {
    fn count(&self) -> u64 {
        let value: u64;
        // SAFETY: `CNTVCT_EL0` is readable at EL0 and EL1 on every AArch64
        // implementation of the architected generic timer, needs no
        // enablement, and reading it has no side effect on any state the
        // caller owns — which is why `CycleSource::read_cycles` is a safe
        // method. The `isb` is the architecture's own requirement for an
        // ordered read: without it the `mrs` may be reordered against the
        // instructions being timed, which for a *measurement* counter is the
        // one error that would silently corrupt every sample. `nomem` is
        // deliberately not asserted, so the barrier is not optimized across.
        unsafe {
            core::arch::asm!(
                "isb",
                "mrs {value}, cntvct_el0",
                value = out(reg) value,
                options(nostack, preserves_flags),
            );
        }
        value
    }
}

#[cfg(target_arch = "aarch64")]
impl PhysicalCounter for SystemRegisters {
    fn count(&self) -> u64 {
        let value: u64;
        // SAFETY: `CNTPCT_EL0` is readable at EL1 once `CNTHCTL_EL2.EL1PCTEN`
        // is set, which the boot stub's drop does (`hal_arm64::boot`,
        // `orr x0, x0, #3` on `cnthctl_el2`); a refused read is a synchronous
        // exception through `STORY-P1-07-02`'s handler, not a hang. The `isb`
        // orders the read exactly as `CNTVCT_EL0`'s does — for the register
        // `ADR 0005` Q3 is stated against, an unordered read would be the
        // same class of silent corruption.
        unsafe {
            core::arch::asm!(
                "isb",
                "mrs {value}, cntpct_el0",
                value = out(reg) value,
                options(nostack, preserves_flags),
            );
        }
        value
    }
}

/// Enables the PMU and runs [`probe_residency_window`] against the real
/// registers, windowed on the physical counter — the `Q3` instrument
/// `LE-103` asked for, as `probe_pmccntr`'s sibling.
#[cfg(target_arch = "aarch64")]
#[must_use]
pub fn probe_residency(window_ticks: u64) -> ResidencyProbe {
    enable_pmu_cycle_counter();
    probe_residency_window(
        &SystemRegisters,
        &SystemRegisters,
        &PmuRegisters,
        window_ticks,
        2_000_000_000,
    )
}

#[cfg(target_arch = "aarch64")]
impl CounterFrequency for SystemRegisters {
    fn hertz(&self) -> u64 {
        let value: u64;
        // SAFETY: `CNTFRQ_EL0` is readable at EL0 and EL1, is a plain
        // firmware-programmed constant, and reading it has no side effect. No
        // barrier is needed: unlike the count, its value does not change, so
        // read ordering cannot affect the result.
        unsafe {
            core::arch::asm!(
                "mrs {value}, cntfrq_el0",
                value = out(reg) value,
                options(nomem, nostack, preserves_flags),
            );
        }
        value
    }
}

/// The PMU cycle counter as a [`CycleSource`] — the recorded `LE-15`
/// decision (`STORY-P1-07-04`, Handover 19 §7.5): `PMCCNTR_EL0` is the ARM64
/// microbenchmark cycle source; [`Cntvct`] stays the `Timebase`/wall-clock
/// source. Generic over the same register-seam pattern as [`Cntvct`], so the
/// selection logic is host-testable before the register ever answers.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pmccntr<R> {
    counter: R,
}

impl<R> Pmccntr<R> {
    /// The `cycle_source=` name this source reports in a measurement
    /// envelope.
    pub const NAME: &'static str = "pmccntr_el0";

    /// Wraps a cycle-counter register seam.
    pub const fn new(counter: R) -> Self {
        Pmccntr { counter }
    }
}

impl<R: VirtualCounter> CycleSource for Pmccntr<R> {
    fn read_cycles(&self) -> u64 {
        self.counter.count()
    }
}

// --- aarch64 glue: the PMU and the virtual timer (`STORY-P1-07-04`) ----------

/// The real `PMCCNTR_EL0` read. A separate zero-sized seam rather than a
/// second method on [`SystemRegisters`], because this register — unlike the
/// generic timer's — must be *enabled* first and may honestly read zero
/// forever ([`enable_pmu_cycle_counter`], `TEST-P1-07-04-A` clause 3).
#[cfg(target_arch = "aarch64")]
#[derive(Debug, Clone, Copy, Default)]
pub struct PmuRegisters;

#[cfg(target_arch = "aarch64")]
impl VirtualCounter for PmuRegisters {
    fn count(&self) -> u64 {
        let value: u64;
        // SAFETY: `PMCCNTR_EL0` is readable at EL1 once `MDCR_EL2` traps are
        // clear (the drop writes it to zero) — and if the implementation
        // still refuses, the refusal is a *synchronous exception*, which
        // reports through `STORY-P1-07-02`'s handler rather than hanging.
        // The `isb` orders the read for the same reason `CNTVCT_EL0`'s does.
        unsafe {
            core::arch::asm!(
                "isb",
                "mrs {value}, pmccntr_el0",
                value = out(reg) value,
                options(nostack, preserves_flags),
            );
        }
        value
    }
}

/// Enables the PMU cycle counter: `PMCR_EL0.E` plus cycle-counter reset,
/// `PMCCFILTR_EL0` cleared so EL1 is counted, `PMCNTENSET_EL0` bit 31.
///
/// No readback here on purpose — the *counter advancing* is the only
/// readback that matters, and [`tick::cycle_source_decision`] judges exactly
/// that ([`crate::tick`]).
#[cfg(target_arch = "aarch64")]
pub fn enable_pmu_cycle_counter() {
    // SAFETY: all three registers are architected PMUv3 EL1-writable state;
    // single core, sole writer; the `isb` makes the enable visible before
    // the first read.
    unsafe {
        core::arch::asm!(
            "mrs  {scratch}, pmcr_el0",
            // E (enable) then C (cycle reset) — two `orr`s because 0b101 is
            // not an encodable AArch64 logical immediate.
            "orr  {scratch}, {scratch}, #0x1",
            "orr  {scratch}, {scratch}, #0x4",
            "msr  pmcr_el0, {scratch}",
            "msr  pmccfiltr_el0, xzr",         // count at EL1, no filtering
            "mov  {scratch}, #(1 << 31)",
            "msr  pmcntenset_el0, {scratch}",
            "isb",
            scratch = out(reg) _,
            options(nostack, preserves_flags),
        );
    }
}

/// What the PMU probe observed: the cycle-counter advance across a window
/// whose width the generic timer measured.
#[cfg(target_arch = "aarch64")]
#[derive(Debug, Clone, Copy)]
pub struct PmuProbe {
    /// `PMCCNTR_EL0`'s advance across the window — zero convicts the PMU
    /// and takes the recorded fallback, never the Story.
    pub pmccntr_delta: u64,
    /// The window's width in generic-timer ticks.
    pub window_ticks: u64,
}

/// Enables the PMU and measures its counter against the generic timer for
/// `window_ticks` (bounded busy wait). Clause 4's measurement: the rate is
/// derived from the *other* counter, never quoted from the manual.
#[cfg(target_arch = "aarch64")]
#[must_use]
pub fn probe_pmccntr(window_ticks: u64) -> PmuProbe {
    enable_pmu_cycle_counter();
    let timer = SystemRegisters;
    let pmu = PmuRegisters;
    // Disambiguated by trait since `SystemRegisters` reads the physical
    // counter too (`LE-103`): this probe's window is the VIRTUAL counter's,
    // which is correct *here* — it serves `LE-15`'s rate decision, not Q3.
    let pmu_start = pmu.count();
    let started = VirtualCounter::count(&timer);
    // Bounded: the loop ends when the window closes or the iteration bound
    // trips (a stuck CNTVCT converts to a short window, not a hang).
    let mut spins: u32 = 0;
    while VirtualCounter::count(&timer).wrapping_sub(started) < window_ticks
        && spins < 2_000_000_000
    {
        spins += 1;
    }
    let window = VirtualCounter::count(&timer).wrapping_sub(started);
    PmuProbe { pmccntr_delta: pmu.count().wrapping_sub(pmu_start), window_ticks: window }
}

/// Arms the EL1 virtual timer to fire in `interval` ticks and enables it,
/// returning the `CNTV_CTL_EL0` readback (bit 0 must be set, bit 1 clear).
#[cfg(target_arch = "aarch64")]
pub fn start_virtual_timer(interval: u32) -> u64 {
    let readback: u64;
    // SAFETY: `CNTV_TVAL_EL0`/`CNTV_CTL_EL0` are EL1-writable generic-timer
    // state; `CNTHCTL_EL2` was configured at the drop; single core.
    unsafe {
        core::arch::asm!(
            "msr  cntv_tval_el0, {interval}",
            "mov  {ctl}, #1",
            "msr  cntv_ctl_el0, {ctl}",
            "isb",
            "mrs  {ctl}, cntv_ctl_el0",
            interval = in(reg) u64::from(interval),
            ctl = out(reg) readback,
            options(nostack, preserves_flags),
        );
    }
    readback
}

/// Re-arms the already-enabled virtual timer for the next interval — the one
/// register write the tick handler performs on the timer.
#[cfg(target_arch = "aarch64")]
pub fn rearm_virtual_timer(interval: u32) {
    // SAFETY: as in `start_virtual_timer`; writing `TVAL` restarts the
    // countdown and drops the pending state, which is the acknowledgement
    // the timer side of the interrupt requires.
    unsafe {
        core::arch::asm!(
            "msr  cntv_tval_el0, {interval}",
            interval = in(reg) u64::from(interval),
            options(nostack, preserves_flags),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::cell::Cell;
    use hal::time::conformance::{check, ConformanceFailure};
    use kernel::measure::{Calibration, Environment, Metric, Report, Samples, Stopwatch};

    /// A scripted virtual-counter double: each read advances by `step`.
    struct StepCounter {
        now: Cell<u64>,
        step: u64,
    }

    impl StepCounter {
        fn new(start: u64, step: u64) -> Self {
            StepCounter { now: Cell::new(start), step }
        }
    }

    impl VirtualCounter for StepCounter {
        fn count(&self) -> u64 {
            let value = self.now.get();
            self.now.set(value + self.step);
            value
        }
    }

    /// A counter whose second read goes backwards — a non-conforming source
    /// the wrapper must not launder into a conforming one.
    struct BackwardsCounter {
        reads: Cell<usize>,
    }

    impl VirtualCounter for BackwardsCounter {
        fn count(&self) -> u64 {
            let n = self.reads.get();
            self.reads.set(n + 1);
            if n == 0 {
                1_000
            } else {
                500
            }
        }
    }

    /// A scripted sequence, for asserting the wrapper is transparent.
    struct ScriptedCounter {
        values: [u64; 4],
        index: Cell<usize>,
    }

    impl VirtualCounter for ScriptedCounter {
        fn count(&self) -> u64 {
            let index = self.index.get();
            self.index.set(index + 1);
            self.values[index.min(self.values.len() - 1)]
        }
    }

    struct FixedFrequency(u64);

    impl CounterFrequency for FixedFrequency {
        fn hertz(&self) -> u64 {
            self.0
        }
    }

    /// A physical-counter double that shares its advance script with a
    /// virtual-counter double, minus a scripted offset — the architecture's
    /// own relation, `CNTVCT_EL0 = CNTPCT_EL0 - CNTVOFF_EL2`, as a test rig.
    /// `offset` can change between reads, which is exactly the move `LE-103`
    /// says a world above NS-EL1 would make.
    struct OffsetPair {
        physical: Cell<u64>,
        /// The physical value most recently *returned* — the instant a
        /// virtual read that follows a physical read observes.
        last: Cell<u64>,
        step: u64,
        offsets: [u64; 8],
        reads: Cell<usize>,
    }

    impl OffsetPair {
        fn new(step: u64, offsets: [u64; 8]) -> Self {
            OffsetPair {
                physical: Cell::new(0),
                last: Cell::new(0),
                step,
                offsets,
                reads: Cell::new(0),
            }
        }
    }

    impl PhysicalCounter for &OffsetPair {
        fn count(&self) -> u64 {
            let value = self.physical.get();
            self.physical.set(value + self.step);
            self.last.set(value);
            value
        }
    }

    impl VirtualCounter for &OffsetPair {
        fn count(&self) -> u64 {
            let index = self.reads.get();
            self.reads.set(index + 1);
            let offset = self.offsets[index.min(self.offsets.len() - 1)];
            // The virtual counter is the physical counter seen through the
            // offset at the same instant as the adjacent physical read; it
            // does not advance the shared clock itself.
            self.last.get().wrapping_sub(offset)
        }
    }

    // `LE-103`: the split is the physical counter minus the virtual one —
    // the quantity `CNTVOFF_EL2` holds, recovered from NS-EL1's own reads.
    #[test]
    fn the_counter_split_reports_the_virtual_offset() {
        let split = CounterSplit { cntpct: 1_000, cntvct: 400 };
        assert_eq!(split.virtual_offset(), 600);
        // A virtual counter AHEAD of the physical one is a negative offset;
        // wrapping arithmetic keeps it faithful rather than clamping to zero.
        let ahead = CounterSplit { cntpct: 400, cntvct: 1_000 };
        assert_eq!(ahead.virtual_offset(), 600u64.wrapping_neg());
    }

    #[test]
    fn reading_the_split_takes_both_registers_unmodified() {
        struct FixedPhysical(u64);
        impl PhysicalCounter for FixedPhysical {
            fn count(&self) -> u64 {
                self.0
            }
        }
        let split = read_counter_split(&FixedPhysical(5_400), &StepCounter::new(5_100, 0));
        assert_eq!(split.cntpct, 5_400);
        assert_eq!(split.cntvct, 5_100);
        assert_eq!(split.virtual_offset(), 300);
    }

    // `LE-103`'s defect, demonstrated in the direction that matters: the
    // residency window is anchored to the PHYSICAL counter, so an offset that
    // moves mid-window cannot shorten the window — it shows up as the virtual
    // counter's advance disagreeing with the physical one's.
    #[test]
    fn a_moving_virtual_offset_is_visible_not_absorbed() {
        // The offset grows by 40 ticks between the probe's two virtual reads:
        // a hidden residency of 40 physical ticks, exactly what `CNTVOFF_EL2`
        // movement looks like from NS-EL1.
        let pair = OffsetPair::new(10, [0, 40, 40, 40, 40, 40, 40, 40]);
        let probe = probe_residency_window(&&pair, &&pair, &StepCounter::new(0, 7), 100, 1_000);
        assert!(probe.cntpct_ticks >= 100, "the window is the physical counter's");
        assert_eq!(
            probe.cntpct_ticks.wrapping_sub(probe.cntvct_ticks),
            40,
            "the moved offset must appear as the two advances disagreeing"
        );
    }

    #[test]
    fn an_honest_pair_of_counters_advances_in_lockstep() {
        let pair = OffsetPair::new(10, [7; 8]);
        let probe = probe_residency_window(&&pair, &&pair, &StepCounter::new(0, 3), 50, 1_000);
        assert_eq!(probe.cntpct_ticks, probe.cntvct_ticks);
        assert!(probe.pmccntr_delta > 0, "the PMU advanced across the window");
    }

    #[test]
    fn a_stuck_physical_counter_trips_the_spin_bound_rather_than_hanging() {
        let stuck = OffsetPair::new(0, [0; 8]);
        let probe = probe_residency_window(&&stuck, &&stuck, &StepCounter::new(0, 1), 100, 16);
        // The window never closed; the bound converted a hang into a short
        // window, the same conversion `probe_pmccntr` already makes.
        assert_eq!(probe.cntpct_ticks, 0);
    }

    // Clause 2: the shared conformance suite, unchanged, against the second
    // implementor of `CycleSource`.
    #[test]
    fn the_generic_timer_source_passes_the_shared_conformance_suite() {
        let source = Cntvct::new(StepCounter::new(0, 7));
        assert_eq!(check(&source, 11), Ok(70));
    }

    #[test]
    fn a_stuck_generic_timer_fails_conformance_rather_than_being_laundered() {
        let source = Cntvct::new(StepCounter::new(42, 0));
        assert_eq!(check(&source, 64), Err(ConformanceFailure::NoForwardProgress { samples: 64 }));
    }

    #[test]
    fn a_backwards_generic_timer_fails_conformance_rather_than_being_clamped() {
        let source = Cntvct::new(BackwardsCounter { reads: Cell::new(0) });
        assert_eq!(
            check(&source, 8),
            Err(ConformanceFailure::WentBackwards { previous: 1_000, observed: 500 })
        );
    }

    // Clause 2, final paragraph: the wrapper adds no arithmetic of its own.
    #[test]
    fn the_wrapper_returns_the_register_value_unmodified() {
        let source = Cntvct::new(ScriptedCounter { values: [5, 9, 9, 40], index: Cell::new(0) });
        assert_eq!(source.read_cycles(), 5);
        assert_eq!(source.read_cycles(), 9);
        assert_eq!(source.read_cycles(), 9);
        assert_eq!(source.read_cycles(), 40);
    }

    // Clause 3: derived, not calibrated — and rounded to nearest.
    #[test]
    fn a_counter_frequency_converts_to_cycles_per_microsecond() {
        // The Raspberry Pi 5's 54 MHz generic timer, the board this slice
        // targets.
        assert_eq!(plausible_cycles_per_us(54_000_000), Some(54));
        // Other frequencies real ARM boards program.
        assert_eq!(plausible_cycles_per_us(19_200_000), Some(19));
        assert_eq!(plausible_cycles_per_us(1_000_000), Some(1));
    }

    #[test]
    fn the_conversion_rounds_to_nearest_rather_than_truncating() {
        // 24.576 MHz is 24.576 cycles/us: truncation would report 24, a 2.3%
        // error scaling every derived microsecond figure on that board.
        assert_eq!(plausible_cycles_per_us(24_576_000), Some(25));
        assert_eq!(plausible_cycles_per_us(1_499_999), Some(1));
        assert_eq!(plausible_cycles_per_us(1_500_000), Some(2));
    }

    // Clause 4: honest absence.
    #[test]
    fn an_unprogrammed_frequency_register_yields_no_timebase_not_zero() {
        // Firmware that never programmed CNTFRQ_EL0 is a real, documented ARM
        // condition — and exactly the input a truncating divide turns into a
        // silent timebase of 0.
        assert_eq!(plausible_cycles_per_us(0), None);
    }

    #[test]
    fn an_implausible_frequency_yields_no_timebase_rather_than_a_guess() {
        // Below 1 MHz cannot produce a nonzero cycles-per-microsecond figure.
        assert_eq!(plausible_cycles_per_us(999_999), None);
        // No generic timer runs at 200 GHz; that is a bad read, not a fast
        // board.
        assert_eq!(plausible_cycles_per_us(200_000_000_000), None);
    }

    #[test]
    fn the_plausibility_floor_is_this_architectures_and_not_the_x86_backends() {
        // `hal_x86_64::tsc` rejects anything under 10 cycles/us because a TSC
        // that slow is broken. A generic timer at 1-2 MHz is ordinary,
        // conforming hardware, so copying that floor would reject valid
        // boards. Both ends of this crate's own range are accepted.
        assert_eq!(plausible_cycles_per_us(1_000_000), Some(1));
        assert_eq!(plausible_cycles_per_us(100_000_000_000), Some(100_000));
    }

    #[test]
    fn the_timebase_reads_its_factor_from_the_frequency_register() {
        let timebase = GenericTimerTimebase::from_register(&FixedFrequency(54_000_000));
        assert_eq!(timebase.cycles_per_us(), Some(54));
        let absent = GenericTimerTimebase::from_register(&FixedFrequency(0));
        assert_eq!(absent.cycles_per_us(), None);
    }

    // Clause 6: a coarse counter is a supported case. At 54 MHz one tick is
    // ~18.5 ns, so a paired-read calibration can legitimately measure zero.
    #[test]
    fn a_zero_calibrated_overhead_is_an_ordinary_reading_not_an_error() {
        let source = Cntvct::new(StepCounter::new(1_000, 0));
        let calibration = Calibration::measure(&source, 32);
        assert_eq!(calibration.overhead_cycles(), 0);
        assert_eq!(calibration.correct(7), 7);
        assert_eq!(calibration.correct(0), 0);
    }

    // Clause 5: the harness consumes this backend unmodified, and the
    // envelope it produces is the one `xtask::timing` already parses.
    #[test]
    fn the_measurement_harness_accepts_this_backend_unmodified() {
        let source = Cntvct::new(StepCounter::new(0, 10));
        let calibration = Calibration::from_overhead_cycles(0);
        let mut samples: Samples<8> = Samples::new();
        for _ in 0..8 {
            let stopwatch = Stopwatch::start(&source);
            samples.record(stopwatch.stop(&calibration));
        }
        let summary = samples.summarize().expect("eight samples were recorded");
        assert_eq!(summary.n, 8);

        let timebase = GenericTimerTimebase::from_register(&FixedFrequency(54_000_000));
        let mut sink = String::new();
        let environment = Environment {
            tier: "T1",
            arch: Cntvct::<StepCounter>::ARCH,
            platform: "rpi5-bcm2712",
            qualification: kernel::measure::UNQUALIFIED,
            cycle_source: Cntvct::<StepCounter>::NAME,
            overhead_cycles: calibration.overhead_cycles(),
            cycles_per_us: timebase.cycles_per_us(),
        };
        let mut report = Report::begin(&mut sink, &environment).expect("begin writes");
        report
            .metric(&Metric { domain: "D04", name: "context_switch", warmup: 0, summary })
            .expect("metric writes");
        assert_eq!(report.end().expect("end writes"), 1);

        let mut lines = sink.lines();
        assert_eq!(
            lines.next(),
            Some(
                "TOS64-MEAS/2 BEGIN tier=T1 arch=aarch64 platform=rpi5-bcm2712 qualification=none cycle_source=cntvct_el0 \
                 overhead_cycles=0 cycles_per_us=54"
            )
        );
        assert_eq!(
            lines.next(),
            Some(
                "TOS64-MEAS/2 METRIC domain=D04 metric=context_switch n=8 dropped=0 warmup=0 \
                 min=10 p50=10 p99=10 p99_9=10 max=10 unit=cycles"
            )
        );
        assert_eq!(lines.next(), Some("TOS64-MEAS/2 END metrics=1"));
        assert_eq!(lines.next(), None);
    }
}
