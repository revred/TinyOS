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
use crate::measure::{Calibration, Environment, Metric, MetricLabel, Report, Samples};
use crate::measure_phases::{
    phase_context_switch, phase_context_switch_spoored, phase_dispatch_round,
    phase_dispatch_round_spoored, phase_dispatch_select, phase_pool_alloc_free,
    phase_pool_alloc_free_batched, phase_pool_alloc_free_batched_spoored, phase_pool_denial,
    phase_reference_loop, phase_spoor_announce, phase_spoor_drain, phase_spoor_stamp,
    CALIBRATION_SAMPLES, CONFORMANCE_SAMPLES, SAMPLES, STACK_SIZE, WARMUP,
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

/// `ADR 0005` Q3's positive control (`fixture-qual-control`, `12A` §2 boot 2):
/// sixteen idle residency windows self-calibrate the PMU rate, then one
/// window carries a benign `PSCI_VERSION` SMC at its midpoint — the one
/// documented synchronous EL3 entry this platform has (Q2). The verdict is
/// the trap clause itself: the probe must **see** the round-trip (the control
/// window's unaccounted ticks strictly above every idle window's), or this
/// fixture reports `ok=false` and the campaign is stopped before it starts.
#[cfg(feature = "fixture-qual-control")]
fn qual_smc_control<W: Write>(sink: &mut W) -> bool {
    use crate::qual_campaign::{self, WindowSample};

    const IDLE_WINDOWS: usize = 16;
    let mut idle = [WindowSample::ZERO; IDLE_WINDOWS];
    for slot in idle.iter_mut() {
        let probe = hal_arm64::timer::probe_residency(PMU_PROBE_WINDOW_TICKS);
        *slot = WindowSample {
            cntpct_ticks: probe.cntpct_ticks,
            cntvct_ticks: probe.cntvct_ticks,
            pmccntr_delta: probe.pmccntr_delta,
        };
    }
    let mut scratch = [0u64; IDLE_WINDOWS];
    let summary = match qual_campaign::summarize(&idle, &mut scratch, PMU_PROBE_WINDOW_TICKS) {
        Ok(summary) => summary,
        Err(refusal) => {
            let _ = qual_campaign::write_refusal_line(sink, "smc_control", refusal);
            return false;
        }
    };

    let mut version: u64 = 0;
    let control = hal_arm64::timer::probe_residency_with_event(
        PMU_PROBE_WINDOW_TICKS,
        PMU_PROBE_WINDOW_TICKS / 2,
        || version = hal_arm64::smc::psci_version(),
    );
    let sample = WindowSample {
        cntpct_ticks: control.probe.cntpct_ticks,
        cntvct_ticks: control.probe.cntvct_ticks,
        pmccntr_delta: control.probe.pmccntr_delta,
    };
    let control_unaccounted = qual_campaign::unaccounted_ticks(&sample, summary.pmu_per_1000_ticks);
    let seen = control.event_fired && qual_campaign::control_seen(&summary, control_unaccounted);
    let _ = qual_campaign::write_control_line(
        sink,
        version,
        &summary,
        control_unaccounted,
        control.event_fired,
        seen,
    );
    seen
}

/// `ADR 0005` Q3's campaign (`fixture-qual-campaign`, `12A` §2 boot 3):
/// 6,000 windows of 540,000 physical ticks — 60 seconds of accumulated
/// window time at the proven size, satisfying both of `08A` §5's proposals
/// (≥ 1,000 windows, ≥ 60 s) — summarized to the distribution of unaccounted
/// physical ticks per window and put on the wire as one `TOS64-QUAL/1
/// campaign` line. Environment is whatever the bench is (stated by the
/// capture, not controlled); the bound claim is the *Report's* act, made
/// against `unaccounted_max`, never this fixture's.
#[cfg(feature = "fixture-qual-campaign")]
fn qual_campaign_run<W: Write>(sink: &mut W) -> bool {
    use crate::qual_campaign::{self, WindowSample};

    const CAMPAIGN_WINDOWS: usize = 6_000;
    static mut CAMPAIGN_SAMPLES: [WindowSample; CAMPAIGN_WINDOWS] =
        [WindowSample::ZERO; CAMPAIGN_WINDOWS];
    static mut CAMPAIGN_SCRATCH: [u64; CAMPAIGN_WINDOWS] = [0; CAMPAIGN_WINDOWS];
    // SAFETY: single-core, non-reentrant fixture; this function runs once per
    // boot and is the only reader or writer of these statics.
    let (samples, scratch) = unsafe {
        (
            &mut *core::ptr::addr_of_mut!(CAMPAIGN_SAMPLES),
            &mut *core::ptr::addr_of_mut!(CAMPAIGN_SCRATCH),
        )
    };
    for slot in samples.iter_mut() {
        let probe = hal_arm64::timer::probe_residency(PMU_PROBE_WINDOW_TICKS);
        *slot = WindowSample {
            cntpct_ticks: probe.cntpct_ticks,
            cntvct_ticks: probe.cntvct_ticks,
            pmccntr_delta: probe.pmccntr_delta,
        };
    }
    match qual_campaign::summarize(samples, scratch, PMU_PROBE_WINDOW_TICKS) {
        Ok(summary) => {
            let _ = qual_campaign::write_campaign_line(sink, &summary);
            true
        }
        Err(refusal) => {
            let _ = qual_campaign::write_refusal_line(sink, "campaign", refusal);
            false
        }
    }
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

const METRICS: usize = 14;

/// Every metric this fixture emits, in slot order — `LE-91`'s declaration.
///
/// Each row states the **domain of what is measured** and the **Story whose
/// contract must select it**, together, at one site.
/// `cargo run -p xtask -- check-metric-labels` parses this table out of this
/// file and holds every row against `goals/assurance/story-contracts.tsv`.
///
/// **This is the file the defect was found in.** Slots 8–10 carried `D07`
/// from 2026-08-05 to 2026-08-06 because `STORY-P1-10-02`'s contract selected
/// only `D07` — the metric bent to fit the contract instead of the contract
/// extended to fit its subject — and the cost was that nobody read them
/// against `D11`'s targets, which the stamp misses by 1.9× at the median.
///
/// The Story on each row is the one whose acceptance criteria the metric
/// serves, and for the three `G23` spoor-overhead arms it is the Story
/// `goals/assurance/guardrail-evidence.tsv` already names as the gate's owner
/// — `STORY-P1-10-02` for `PERF-D07-G23`, `STORY-P1-07-06` for `PERF-D04-G23`
/// and `PERF-D05-G23`. Two registers naming the same owner is what makes a
/// disagreement between them findable.
static METRIC_LABELS: [MetricLabel; METRICS] = [
    MetricLabel { domain: "REF", story: "STORY-P1-01-04", name: "fixed_integer_loop" },
    MetricLabel {
        domain: "D07",
        story: "STORY-P1-07-06",
        name: "pool_u64x64_alloc_free_round_trip",
    },
    MetricLabel {
        domain: "D07",
        story: "STORY-P1-07-06",
        name: "pool_u64x4_alloc_denied_exhausted_per_op_of_64",
    },
    MetricLabel {
        domain: "D04",
        story: "STORY-P1-07-06",
        name: "context_switch_yield_roundtrip_2switches",
    },
    MetricLabel {
        domain: "D05",
        story: "STORY-P1-07-06",
        name: "dispatch_select_highest_priority_ready",
    },
    MetricLabel {
        domain: "D05",
        story: "STORY-P1-07-06",
        name: "dispatch_run_once_cooperative_round",
    },
    MetricLabel {
        domain: "D02",
        story: "STORY-P1-07-06",
        name: "fault_brk_capture_escape_kernel_context",
    },
    MetricLabel {
        domain: "D07",
        story: "STORY-P1-07-06",
        name: "pool_u64x64_alloc_free_round_trip_per_op_of_8",
    },
    MetricLabel {
        domain: "D11",
        story: "STORY-P1-10-02",
        name: "spoor_stamp_park_rung_per_op_of_8",
    },
    MetricLabel {
        domain: "D11",
        story: "STORY-P1-10-02",
        name: "spoor_drain_full_ring_frame_of_181",
    },
    MetricLabel {
        domain: "D11",
        story: "STORY-P1-10-02",
        name: "spoor_announce_certificate_frame_of_3",
    },
    MetricLabel {
        domain: "D07",
        story: "STORY-P1-10-02",
        name: "pool_u64x64_alloc_free_round_trip_per_op_of_8_spoored",
    },
    MetricLabel {
        domain: "D04",
        story: "STORY-P1-07-06",
        name: "context_switch_yield_roundtrip_2switches_spoored",
    },
    MetricLabel {
        domain: "D05",
        story: "STORY-P1-07-06",
        name: "dispatch_run_once_cooperative_round_spoored",
    },
];

struct Measured {
    label: &'static MetricLabel,
    summary: crate::measure::Summary,
}

/// Summarizes the current phase into `slot`, taking its identity from
/// [`METRIC_LABELS`] rather than from arguments at the call site — a name and
/// a domain repeated at the call site is a second declaration of something
/// already declared, and the two would be free to disagree.
fn collect(
    collected: &mut [Option<Measured>; METRICS],
    slot: usize,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    let summarized = samples.summarize();
    samples.clear();
    match summarized {
        Some(summary) => {
            collected[slot] = Some(Measured { label: &METRIC_LABELS[slot], summary });
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
        report.metric(&Metric::labelled(measured.label, WARMUP, measured.summary)).ok()?;
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

    // `ADR 0005` Q1 and `LE-103`: the qualification lines, onto the channel
    // that actually gets captured. The entry level reached only the UART
    // until 2026-08-07, and the only Q3-shaped probe read the one counter
    // `CNTVOFF_EL2` can lie through; these three lines are the corrected
    // instrument's first output, interrupt-masked like everything here —
    // exactly the loop condition Q3 states.
    {
        use hal_arm64::exception_level::ExceptionLevel;
        let entered_raw = hal_arm64::boot::entered_current_el();
        let entered = match ExceptionLevel::decode(entered_raw) {
            Some(level) => level.as_str(),
            None => "unknown",
        };
        let now_raw: u64;
        // SAFETY: `CurrentEL` is readable at EL1 with no enablement and no
        // side effect — the same read `hal_arm64::boot::entry` opens with.
        unsafe {
            core::arch::asm!(
                "mrs {value}, CurrentEL",
                value = out(reg) now_raw,
                options(nomem, nostack, preserves_flags),
            );
        }
        let now = match ExceptionLevel::decode(now_raw) {
            Some(level) => level.as_str(),
            None => "unknown",
        };
        // `TOS64-QUAL/1` rather than fixture chatter, deliberately: these
        // lines are the qualification record's wire evidence, `timing.rs`'s
        // SENTINEL (`TOS64-MEAS`) never matches them so the envelope parser
        // is untouched, and ti64dink's harvest carries `TOS64-QUAL/1` into
        // the same text file as the envelope.
        match hal_arm64::boot::firmware_cntvoff() {
            Some(cntvoff) => {
                let _ = writeln!(
                    sink,
                    "TOS64-QUAL/1 boot_entry current_el={entered} raw={entered_raw:#018x} \
                     now_at={now} firmware_cntvoff={cntvoff:#018x}"
                );
            }
            None => {
                // Entry was not at EL2, so the register was never ours to
                // read — said in words rather than as a zero that would read
                // as a measurement.
                let _ = writeln!(
                    sink,
                    "TOS64-QUAL/1 boot_entry current_el={entered} raw={entered_raw:#018x} \
                     now_at={now} firmware_cntvoff=unread"
                );
            }
        }

        let split = hal_arm64::timer::read_counter_split(
            &hal_arm64::timer::SystemRegisters,
            &hal_arm64::timer::SystemRegisters,
        );
        let _ = writeln!(
            sink,
            "TOS64-QUAL/1 counter_split cntpct={} cntvct={} virtual_offset={}",
            split.cntpct,
            split.cntvct,
            split.virtual_offset()
        );

        let residency = hal_arm64::timer::probe_residency(PMU_PROBE_WINDOW_TICKS);
        let _ = writeln!(
            sink,
            "TOS64-QUAL/1 residency window_ticks={PMU_PROBE_WINDOW_TICKS} \
             cntpct_ticks={} cntvct_ticks={} pmccntr_delta={}",
            residency.cntpct_ticks, residency.cntvct_ticks, residency.pmccntr_delta
        );
    }

    let calibration = Calibration::measure(&source, CALIBRATION_SAMPLES);

    // SAFETY: single-threaded, non-reentrant fixture; every phase below
    // borrows this buffer for its own duration only.
    let samples: &mut Samples<SAMPLES> = unsafe { &mut *core::ptr::addr_of_mut!(SAMPLE_BUFFER) };

    let mut ok = conformance_ok;

    // `12A` §0's two Q3 arms, riding the same interrupt-masked region as the
    // three QUAL lines above — the campaign's own stated loop condition —
    // and strictly before the measure phases, so a campaign boot's
    // qualification evidence is on the transcript even if a later phase
    // fails. Each folds its verdict into this run's `TOS64-RESULT/1`.
    #[cfg(feature = "fixture-qual-control")]
    {
        ok &= qual_smc_control(&mut sink);
    }
    #[cfg(feature = "fixture-qual-campaign")]
    {
        ok &= qual_campaign_run(&mut sink);
    }

    let mut collected: [Option<Measured>; METRICS] = [const { None }; METRICS];

    ok &= phase_reference_loop(&source, &calibration, samples);
    ok &= collect(&mut collected, 0, samples);

    ok &= phase_pool_alloc_free(&source, &calibration, samples);
    ok &= collect(&mut collected, 1, samples);

    ok &= phase_pool_denial(&source, &calibration, samples);
    ok &= collect(&mut collected, 2, samples);

    ok &= phase_context_switch(&source, &calibration, samples);
    ok &= collect(&mut collected, 3, samples);

    ok &= phase_dispatch_select(&source, &calibration, samples);
    ok &= collect(&mut collected, 4, samples);

    ok &= phase_dispatch_round(&source, &calibration, samples);
    ok &= collect(&mut collected, 5, samples);

    ok &= phase_fault_latency(source, &calibration);
    ok &= collect(&mut collected, 6, samples);

    ok &= phase_pool_alloc_free_batched(&source, &calibration, samples);
    ok &= collect(&mut collected, 7, samples);

    // `STORY-P1-10-02` criterion 6: the observability substrate's own cost,
    // through the same harness as everything it observes. The timed regions
    // stop at the RAM buffer — the GEM transmit is not in any of them.
    //
    // **`D11`, and they were labelled `D07` until 2026-08-06.** `D07` is pool
    // allocation; `D11` is *"spoor stamp and journal"* and these three measure
    // nothing else. The label was `D07` because `STORY-P1-10-02`'s contract
    // selected only `D07`, so the metric was bent to fit the contract rather
    // than the contract extended to fit the subject — and the consequence was
    // not cosmetic: **nobody ever compared these numbers to `D11`'s targets**,
    // which the stamp arm misses by 1.9x at the median. The contract now
    // selects `D07,D11` and `PERF-D11-G01`/`G02`/`G03` are filed from this
    // metric. A domain label is what decides which targets a number is read
    // against, so a wrong one is not a naming defect, it is an unread gate.
    //
    // Since `LE-91` the label lives in [`METRIC_LABELS`] beside the Story that
    // must select it, and `check-metric-labels` fails the build if it does
    // not — correcting these three by hand left the next one free to be wrong
    // the same way.
    ok &= phase_spoor_stamp(&source, &calibration, samples);
    ok &= collect(&mut collected, 8, samples);

    ok &= phase_spoor_drain(&source, &calibration, samples);
    ok &= collect(&mut collected, 9, samples);

    ok &= phase_spoor_announce(&source, &calibration, samples);
    ok &= collect(&mut collected, 10, samples);

    // `PERF-D07-G23`'s spoor-ENABLED arm, and the reason it is here at all:
    // the three costs above are **absolute**, and `G23`'s target is a
    // **ratio** — `spoor enabled adds <= 2% p99 and <= 2% CPU cycles`. No
    // percentage can be computed from an absolute cost, so every spoor number
    // this board has ever produced was in the wrong units for the gate they
    // were taken for (`09A` §5). This arm is slot 7's twin with one stamp per
    // round trip inside the timed region and nothing else changed, which
    // `kernel::measure_phases`'s source-level test holds it to.
    //
    // Deliberately in the SAME run as slot 7 rather than a separate boot:
    // `BOARD VERDICT 9` measured ~3% build-to-build movement on untouched code
    // paths, which is larger than the 2% the gate allows, so two arms from two
    // runs could not answer the question at all.
    ok &= phase_pool_alloc_free_batched_spoored(&source, &calibration, samples);
    ok &= collect(&mut collected, 11, samples);

    // `PERF-D04-G23` and `PERF-D05-G23`, the same paired-arm method applied to
    // the two domains that already have a committed disabled arm in this run.
    // Slot 3 is D04's disabled arm and slot 5 is D05's; these are those two
    // functions with one stamp inside the timed region and nothing else
    // changed, which `kernel::measure_phases`' source-level test holds them to
    // for all three pairs at once.
    //
    // Same run as their twins, for `G23`'s reason above: ~3% build-to-build
    // movement against a 2% allowance means two boots cannot answer a ratio.
    ok &= phase_context_switch_spoored(&source, &calibration, samples);
    ok &= collect(&mut collected, 12, samples);

    ok &= phase_dispatch_round_spoored(&source, &calibration, samples);
    ok &= collect(&mut collected, 13, samples);

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
