//! `fixture_measure` on silicon (`STORY-P1-07-06`, `TEST-P1-07-06-A`): the
//! same shared phases the x86_64 fixture drives, run on the Raspberry Pi 5
//! through the same harness, emitting the same `TOS64-MEAS/2` envelope —
//! parsed by the existing `xtask` parser with **no changes to the parser**,
//! which is the whole Feature's arch-neutrality claim meeting its first real
//! target.
//!
//! What is this architecture's own, mirroring what `fixture_measure.rs`
//! keeps for x86_64:
//!
//! - **The cycle source.** `PMCCNTR_EL0` per the recorded `LE-15` decision,
//!   probed against the generic timer first; a PMU that did not advance
//!   takes the `CNTVCT_EL0` fallback and says so in the envelope's
//!   `cycle_source=` field rather than failing the run.
//! - **The D02 phase.** The victim raises a `BRK` (the AArch64 sibling of
//!   `ud2`: architecturally guaranteed, writes nothing); the fault arrives
//!   through `STORY-P1-07-02`'s real vector table, and the fixture's escape
//!   hook records the span and context-switches away — the same
//!   escape-switch pattern, on the second architecture. `kernel::fault`'s
//!   disposition/audit half is x86_64-gated, so the metric name says
//!   `capture_escape`, not `capture_terminate`: what is timed here is
//!   fault-to-handler-to-decision-recorded, without the audit call that does
//!   not exist on this architecture yet.
//! - **The sink.** The PL011 debug UART — and, because five zero-byte
//!   captures demoted serial to a convenience (`LE-47`), every byte is also
//!   recorded into [`hal_arm64::transcript`], which the park loop paints on
//!   the canvas and transmits line-by-line as `TOS64-*` Ethernet frames for
//!   the host's packet capture.
//!
//! Interrupts are masked for the fixture's whole duration: Tier 0 measures
//! interrupt-free, and a board run whose samples silently included tick
//! handlers would not be the same measurement.
//!
//! Only reachable when the `fixture-measure` feature is enabled — never part
//! of a real boot image.

use crate::context::{self, Context};
use crate::measure::{Calibration, Environment, Metric, Report, Samples};
use crate::measure_phases::{
    phase_context_switch, phase_dispatch_round, phase_dispatch_select, phase_pool_alloc_free,
    phase_pool_alloc_free_batched, phase_pool_denial, phase_reference_loop, CALIBRATION_SAMPLES,
    CONFORMANCE_SAMPLES, SAMPLES, STACK_SIZE, WARMUP,
};
use core::fmt::Write;
use hal::time::{conformance, CycleSource, Timebase};
use hal_arm64::pl011::{Pl011, VolatileMmio};
use hal_arm64::timer::{Cntvct, GenericTimerTimebase, Pmccntr, PmuRegisters, SystemRegisters};

/// Generic-timer ticks the PMU probe window spans: ~10 ms at 54 MHz.
const PMU_PROBE_WINDOW_TICKS: u64 = 540_000;

/// The board's cycle source: the `LE-15` decision, or its recorded fallback.
/// One concrete type so every phase's generic instantiation is decided once,
/// by the probe, rather than per call site.
#[derive(Clone, Copy)]
enum BoardSource {
    /// `PMCCNTR_EL0` advanced across the probe window: the decision holds.
    Pmu(Pmccntr<PmuRegisters>),
    /// The PMU read zero deltas: the fallback, with `LE-15` narrowed in the
    /// Report rather than the run failed.
    Timer(Cntvct<SystemRegisters>),
}

impl CycleSource for BoardSource {
    fn read_cycles(&self) -> u64 {
        match self {
            BoardSource::Pmu(source) => source.read_cycles(),
            BoardSource::Timer(source) => source.read_cycles(),
        }
    }
}

static mut SAMPLE_BUFFER: Samples<SAMPLES> = Samples::new();

// D02 fault-latency state — the AArch64 twin of the x86_64 fixture's.
static mut FAULT_SUPERVISOR_CTX: Context = Context::zeroed();
static mut FAULT_TASK_CTX: Context = Context::zeroed();
static mut FAULT_ABANDONED_CTX: Context = Context::zeroed();
static mut FAULT_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
static mut FAULT_START_CYCLES: u64 = 0;
static mut FAULT_CALIBRATION: Calibration = Calibration::from_overhead_cycles(0);
static mut FAULT_ITERATIONS_RUN: usize = 0;
/// The source the hook reads its stop timestamp from — set once per run,
/// before the phase, so the hook needs no generic parameter.
static mut FAULT_SOURCE: Option<BoardSource> = None;

/// D02's victim: timestamps itself, then raises a `BRK` — architecturally
/// guaranteed to trap synchronously to EL1, writing nothing.
extern "C" fn fault_latency_victim() -> ! {
    // SAFETY: single-core fixture; only this task writes
    // `FAULT_START_CYCLES`, read back only by the escape hook.
    unsafe {
        if let Some(source) = (*core::ptr::addr_of!(FAULT_SOURCE)).as_ref() {
            FAULT_START_CYCLES = source.read_cycles();
        }
        core::arch::asm!("brk #0", options(nomem, nostack));
    }
    unreachable!("brk always faults")
}

/// The escape hook `hal_arm64::fault` calls instead of reporting-and-parking
/// while this fixture has it installed: read the stop timestamp first,
/// record the corrected span, switch back to the supervisor.
extern "C" fn fault_escape_hook() -> ! {
    // SAFETY: single core; only this hook and `phase_fault_latency` touch
    // these statics, never concurrently — the hook runs to completion (via
    // the escape switch) before the driver loop resumes.
    unsafe {
        let stop = match (*core::ptr::addr_of!(FAULT_SOURCE)).as_ref() {
            Some(source) => source.read_cycles(),
            None => 0,
        };
        let calibration = (&raw const FAULT_CALIBRATION).read();
        let started = (&raw const FAULT_START_CYCLES).read();
        let corrected = calibration.correct(stop.saturating_sub(started));
        FAULT_ITERATIONS_RUN += 1;
        if FAULT_ITERATIONS_RUN > WARMUP {
            let samples: &mut Samples<SAMPLES> = &mut *core::ptr::addr_of_mut!(SAMPLE_BUFFER);
            samples.record(corrected);
        }
        context::switch(&raw mut FAULT_ABANDONED_CTX, &raw mut FAULT_SUPERVISOR_CTX);
    }
    unreachable!("a measured fault-latency iteration is never switched back into")
}

/// D02 on this architecture: fault-to-escape-decided latency through the
/// real vector table.
#[inline(never)]
fn phase_fault_latency(source: BoardSource, calibration: &Calibration) -> bool {
    // SAFETY: single-core fixture, run once per phase before any iteration.
    unsafe {
        FAULT_SOURCE = Some(source);
        FAULT_CALIBRATION = *calibration;
        FAULT_ITERATIONS_RUN = 0;
    }
    hal_arm64::fault::install_measure_hook(Some(fault_escape_hook));

    for _ in 0..(WARMUP + SAMPLES) {
        // SAFETY: `FAULT_STACK` is a never-moving static used by exactly one
        // `Context` per iteration; the previous iteration's context was
        // abandoned by the hook before this loop resumed. The two contexts
        // are switched strictly alternately — `switch`'s contract.
        unsafe {
            let stack =
                core::slice::from_raw_parts_mut((&raw mut FAULT_STACK).cast::<u8>(), STACK_SIZE);
            let Ok(task) = Context::new(stack, fault_latency_victim) else {
                hal_arm64::fault::install_measure_hook(None);
                return false;
            };
            FAULT_TASK_CTX = task;
            context::switch(&raw mut FAULT_SUPERVISOR_CTX, &raw mut FAULT_TASK_CTX);
            // Control returns here only via the hook's escape switch.
        }
    }

    hal_arm64::fault::install_measure_hook(None);
    // SAFETY: read after every switch above has returned.
    unsafe { FAULT_ITERATIONS_RUN == WARMUP + SAMPLES }
}

/// The envelope sink: every byte to the PL011 (CR-framed by the driver) and,
/// raw, into the transcript the park loop paints and transmits.
struct BoardSink {
    uart: Pl011<VolatileMmio>,
}

impl Write for BoardSink {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        hal_arm64::transcript::record(text.as_bytes());
        self.uart.write_str(text).map_err(|_| core::fmt::Error)
    }
}

const METRICS: usize = 8;

struct Measured {
    domain: &'static str,
    name: &'static str,
    summary: crate::measure::Summary,
}

fn collect(
    collected: &mut [Option<Measured>; METRICS],
    slot: usize,
    domain: &'static str,
    name: &'static str,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    let summarized = samples.summarize();
    samples.clear();
    match summarized {
        Some(summary) => {
            collected[slot] = Some(Measured { domain, name, summary });
            true
        }
        None => false,
    }
}

fn emit_all<W: Write>(
    sink: &mut W,
    environment: &Environment<'_>,
    collected: &[Option<Measured>; METRICS],
) -> Option<usize> {
    let mut report = Report::begin(sink, environment).ok()?;
    for measured in collected.iter().flatten() {
        report
            .metric(&Metric {
                domain: measured.domain,
                name: measured.name,
                warmup: WARMUP,
                summary: measured.summary,
            })
            .ok()?;
    }
    report.end().ok()
}

/// The entry `hal_arm64::boot` calls between the counter bring-up and the
/// park (`STORY-P1-07-06`). Returns the run's verdict; the caller folds it
/// into the `TOS64-RESULT/1 fixture=measure` line.
///
/// Single core, vector table installed, MMU on — the caller's established
/// state, which is exactly this fixture's required contract.
#[no_mangle]
pub extern "C" fn tinyos_arm64_fixture_measure() -> bool {
    // Interrupt-free for the whole run, like Tier 0 — and only for the run.
    //
    // `LE-71`: this mask used to be one-way, on the stated reasoning that the
    // tick line would have accumulated its intervals before the fixture
    // started. On silicon that pre-fixture window admitted exactly one tick
    // (`BOARD VERDICT 6`), and one tick is one timestamp, which is zero
    // intervals — so the ratio `STORY-P1-07-04` criterion 1 depends on could
    // never form, on any board. The region is a scope now: the park loop gets
    // its tick back and the ratio accumulates live, where it is observable.
    hal::interrupts::with_interrupts_masked(&hal_arm64::boot::PstateInterrupts, measure_run)
}

/// The measured run itself. Its caller owns the interrupt-free region, so
/// nothing here masks, unmasks, or may assume which it was entered with.
fn measure_run() -> bool {
    // SAFETY: the debug UART base from `hal_arm64::board`; single core, and
    // the boot path configured the device before this fixture ran.
    let uart = Pl011::new(unsafe { VolatileMmio::new(hal_arm64::board::DEBUG_UART_BASE) });
    let mut sink = BoardSink { uart };

    // The `LE-15` decision meets its registers: probe the PMU against the
    // generic timer, take the decision or its recorded fallback.
    let probe = hal_arm64::timer::probe_pmccntr(PMU_PROBE_WINDOW_TICKS);
    let cntfrq = {
        use hal_arm64::timer::CounterFrequency;
        SystemRegisters.hertz()
    };
    let (source, cycle_source_name, cycles_per_us) = if probe.pmccntr_delta != 0 {
        let rate =
            hal_arm64::tick::measured_rate_mhz(probe.pmccntr_delta, probe.window_ticks, cntfrq);
        (
            BoardSource::Pmu(Pmccntr::new(PmuRegisters)),
            Pmccntr::<PmuRegisters>::NAME,
            rate.and_then(|mhz| u32::try_from(mhz).ok()),
        )
    } else {
        (
            BoardSource::Timer(Cntvct::new(SystemRegisters)),
            Cntvct::<SystemRegisters>::NAME,
            GenericTimerTimebase::from_register(&SystemRegisters).cycles_per_us(),
        )
    };

    // The shared conformance suite against the chosen live source — the run
    // `LE-27`'s sibling evidence for the microbenchmark counter.
    let conformance = conformance::check(&source, CONFORMANCE_SAMPLES);
    let conformance_ok = match conformance {
        Ok(span) => {
            let _ = writeln!(sink, "fixture-measure cycle_source_conformance ok span={span}");
            true
        }
        Err(failure) => {
            let _ = writeln!(sink, "fixture-measure cycle_source_conformance FAILED {failure:?}");
            false
        }
    };

    let calibration = Calibration::measure(&source, CALIBRATION_SAMPLES);

    // SAFETY: single-threaded, non-reentrant fixture; every phase below
    // borrows this buffer for its own duration only.
    let samples: &mut Samples<SAMPLES> = unsafe { &mut *core::ptr::addr_of_mut!(SAMPLE_BUFFER) };

    let mut ok = conformance_ok;
    let mut collected: [Option<Measured>; METRICS] =
        [None, None, None, None, None, None, None, None];

    ok &= phase_reference_loop(&source, &calibration, samples);
    ok &= collect(&mut collected, 0, "REF", "fixed_integer_loop", samples);

    ok &= phase_pool_alloc_free(&source, &calibration, samples);
    ok &= collect(&mut collected, 1, "D07", "pool_u64x64_alloc_free_round_trip", samples);

    ok &= phase_pool_denial(&source, &calibration, samples);
    ok &= collect(
        &mut collected,
        2,
        "D07",
        "pool_u64x4_alloc_denied_exhausted_per_op_of_64",
        samples,
    );

    ok &= phase_context_switch(&source, &calibration, samples);
    ok &= collect(&mut collected, 3, "D04", "context_switch_yield_roundtrip_2switches", samples);

    ok &= phase_dispatch_select(&source, &calibration, samples);
    ok &= collect(&mut collected, 4, "D05", "dispatch_select_highest_priority_ready", samples);

    ok &= phase_dispatch_round(&source, &calibration, samples);
    ok &= collect(&mut collected, 5, "D05", "dispatch_run_once_cooperative_round", samples);

    ok &= phase_fault_latency(source, &calibration);
    ok &= collect(&mut collected, 6, "D02", "fault_brk_capture_escape_kernel_context", samples);

    ok &= phase_pool_alloc_free_batched(&source, &calibration, samples);
    ok &=
        collect(&mut collected, 7, "D07", "pool_u64x64_alloc_free_round_trip_per_op_of_8", samples);

    let environment = Environment {
        tier: "T1",
        arch: "aarch64",
        platform: "rpi5-bcm2712",
        qualification: crate::measure::UNQUALIFIED,
        cycle_source: cycle_source_name,
        overhead_cycles: calibration.overhead_cycles(),
        cycles_per_us,
    };
    let Some(metrics) = emit_all(&mut sink, &environment, &collected) else {
        return false;
    };
    let _ = writeln!(sink, "fixture-measure metrics={metrics}");
    ok && metrics == METRICS
}
