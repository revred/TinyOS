//! `TEST-P1-01-01-A`'s Tier 0 measurement fixture: drives the performance
//! domains `FEAT-P1-01` names as its measured targets — context switch
//! (`D04`), ready-queue dispatch (`D05`), pool allocation (`D07`) and fault
//! latency (`D02`, `LE-17`) — through [`kernel::measure`], compiled for the
//! real `x86_64-tinyos` target and run under QEMU so every cycle count comes
//! from the target binary's own cycle-source reads rather than a host
//! process.
//!
//! Since `STORY-P1-07-06` the measured workloads live in
//! [`kernel::measure_phases`], shared verbatim with the AArch64 boot image's
//! fixture — the arch-neutrality claim `STORY-P1-01-03` made, finally
//! load-bearing. What remains here is exactly what is x86_64's own: the COM1
//! sink, the `#UD`-raising D02 phase and its fault entry, and the QEMU exit
//! protocol.
//!
//! **What a number from this fixture is.** Tier 0 evidence about the
//! *mechanism* and the *harness*, calibrating both. QEMU/TCG's TSC and PIT
//! are software models, so neither the cycle counts nor the microseconds
//! derived from them are hardware WCET evidence — the hardware tier is
//! `FEAT-P1-07`'s.
//!
//! Only reachable when the `fixture-measure` feature is enabled — never part
//! of a real boot image.

use core::fmt::Write;
use hal::time::{conformance, CycleSource, Timebase};
use hal_x86_64::fault::FaultFrame;
use hal_x86_64::serial::SerialPort;
use hal_x86_64::tsc::{self, Tsc};
use kernel::context::{self, Context};
use kernel::fault::{Disposition, FaultReport, FaultingContext};
use kernel::measure::{Calibration, Environment, Metric, Report, Samples};
use kernel::measure_phases::{
    phase_context_switch, phase_context_switch_spoored, phase_dispatch_round,
    phase_dispatch_round_spoored, phase_dispatch_select, phase_pool_alloc_free,
    phase_pool_alloc_free_batched, phase_pool_alloc_free_batched_spoored, phase_pool_denial,
    phase_reference_loop, CALIBRATION_SAMPLES, CONFORMANCE_SAMPLES, SAMPLES, STACK_SIZE, WARMUP,
};

static mut SAMPLE_BUFFER: Samples<SAMPLES> = Samples::new();

// D02 (`LE-17`) fault-latency state. A fault handler never resumes the
// context it interrupted (`kernel::fault`'s own doc comment — there is no
// `Resume` arm), so `FAULT_TASK_CTX` is reinitialized every iteration: each
// iteration needs a fresh entry point to fault from.
static mut FAULT_SUPERVISOR_CTX: Context = Context::zeroed();
static mut FAULT_TASK_CTX: Context = Context::zeroed();
/// Where the fault handler saves the faulted iteration's registers. Written
/// once per iteration and never read — a context nothing will ever resume is
/// the honest destination.
static mut FAULT_ABANDONED_CTX: Context = Context::zeroed();
static mut FAULT_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
/// Cycle count read immediately before the victim's `ud2`, so the handler
/// measures exactly the fault-to-disposition-decided span.
static mut FAULT_START_CYCLES: u64 = 0;
/// Set once before the phase begins; read (never written) by the handler.
static mut FAULT_CALIBRATION: Calibration = Calibration::from_overhead_cycles(0);
/// Counts iterations so the handler can skip recording during `WARMUP`.
static mut FAULT_ITERATIONS_RUN: usize = 0;

/// D02's victim: timestamps itself, then raises a real `#UD` — the one
/// vector this kernel can fault from deterministically, with no dependency
/// on GDT/page-table shape.
extern "C" fn fault_latency_victim() -> ! {
    // SAFETY: single-CPU fixture; only this task writes `FAULT_START_CYCLES`,
    // and it is read back only by the handler this fault delivers control to.
    unsafe {
        FAULT_START_CYCLES = Tsc.read_cycles();
        core::arch::asm!("ud2", options(nomem, nostack));
    }
    unreachable!("ud2 always faults")
}

/// D02 (`LE-17`, `PERF-D02-G01`..`G07`): fault-to-disposition-decided
/// latency, measured through this fixture's own `tinyos_fault_entry` (below)
/// running the same `kernel::fault::of`/`audit` calls the production entry
/// points run.
#[inline(never)]
fn phase_fault_latency(calibration: &Calibration) -> bool {
    // SAFETY: single-CPU fixture, run once per phase before any iteration.
    unsafe {
        FAULT_CALIBRATION = *calibration;
        FAULT_ITERATIONS_RUN = 0;
    }

    for _ in 0..(WARMUP + SAMPLES) {
        // SAFETY: `FAULT_STACK` is a never-moving static used by exactly one
        // `Context` per iteration; the previous iteration's context was
        // abandoned by the handler before this loop resumed.
        // `FAULT_SUPERVISOR_CTX`/`FAULT_TASK_CTX` are switched strictly
        // alternately, matching `switch`'s documented contract.
        unsafe {
            let stack =
                core::slice::from_raw_parts_mut((&raw mut FAULT_STACK).cast::<u8>(), STACK_SIZE);
            let Ok(task) = Context::new(stack, fault_latency_victim) else {
                return false;
            };
            FAULT_TASK_CTX = task;
            context::switch(&raw mut FAULT_SUPERVISOR_CTX, &raw mut FAULT_TASK_CTX);
            // Control returns here only via the handler's escape switch.
        }
    }

    // SAFETY: read after every switch above has returned.
    unsafe { FAULT_ITERATIONS_RUN == WARMUP + SAMPLES }
}

/// The fixture-measure fault entry point (`LE-17`) — installed in place of
/// `main.rs`'s default `tinyos_fault_entry` only under this feature. Unlike
/// the default handler, it does not halt: it times the disposition path,
/// records the sample, and switches back to the supervisor.
///
/// # Safety
/// Called only by the `hal_x86_64::fault` stubs, with `frame` pointing at a
/// fully-initialized [`FaultFrame`] on the faulting stack. Runs with `IF`
/// clear (interrupt gates), so it cannot be re-entered by an interrupt.
#[no_mangle]
extern "C" fn tinyos_fault_entry(frame: *const FaultFrame) -> ! {
    // SAFETY: the stubs pass a pointer to a fully-initialized `FaultFrame` on
    // the current stack, live for this call.
    let frame = unsafe { *frame };
    // Read as early as possible: every instruction before this one inflates
    // the reported latency by its own cost.
    let stop = Tsc.read_cycles();

    // SAFETY: single-CPU fixture; only this handler and `phase_fault_latency`
    // touch these statics, never concurrently (the handler runs to
    // completion, via the escape switch, before the driver loop resumes).
    unsafe {
        let calibration = (&raw const FAULT_CALIBRATION).read();
        let started = (&raw const FAULT_START_CYCLES).read();
        let corrected = calibration.correct(stop.saturating_sub(started));
        FAULT_ITERATIONS_RUN += 1;
        if FAULT_ITERATIONS_RUN > WARMUP {
            let samples: &mut Samples<SAMPLES> = &mut *core::ptr::addr_of_mut!(SAMPLE_BUFFER);
            samples.record(corrected);
        }

        // The real disposition/audit path — the same `kernel::fault`
        // functions the production entry points call — so this measures the
        // actual cost, not a hand-rolled stand-in for it.
        let report = FaultReport { vector: frame.vector, context: FaultingContext::Kernel };
        let disposition = Disposition::of(&report);
        let _ = kernel::fault::audit(&report, disposition);

        context::switch(&raw mut FAULT_ABANDONED_CTX, &raw mut FAULT_SUPERVISOR_CTX);
    }
    unreachable!("a measured fault-latency iteration is never switched back into")
}

/// How many metrics this fixture measures — the fixed capacity of the
/// collected-summary array below.
const METRICS: usize = 11;

/// One measured phase, held until every phase has run. Phases are measured
/// first and the envelope emitted once at the end, so a fixture that dies
/// mid-measurement produces no envelope at all rather than a half-open one —
/// which `xtask` rejects as truncated, the fail-closed outcome
/// `TEST-P1-01-01-A` clause 6 requires.
struct Measured {
    domain: &'static str,
    name: &'static str,
    summary: kernel::measure::Summary,
}

/// Summarizes the current phase's samples into `collected` and clears the
/// buffer for the next phase. A phase that recorded nothing collects nothing
/// and fails the run: silence is not a fast pass.
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

/// Writes the whole envelope from the collected summaries.
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

/// Runs every phase and reports whether each one's self-consistency check
/// held — the pass/fail bit `xtask` reads back through isa-debug-exit. The
/// percentiles themselves are evidence this fixture reports, never
/// thresholds it enforces: gating is `STORY-P1-01-02`'s charge.
pub fn run() -> bool {
    // SAFETY: this fixture is the only code running (single-CPU boot path, no
    // other UART user) and `init` is called exactly once, before any other
    // `SerialPort` method — `init`'s own documented contract.
    let mut serial = unsafe { SerialPort::init() };

    // Retire the legacy PIC and install fault-only handling before anything
    // is measured — load-bearing, learned the hard way (`LE-03`/`LE-04`/
    // `LE-11`): `Context::new` seeds `IF` set, and a legacy IRQ0 against an
    // empty IDT is a triple fault. `init_faults_only` routes `#UD` to this
    // file's own `tinyos_fault_entry` and arms no timer.
    //
    // SAFETY: called once, here, before any other code depends on interrupts
    // being masked or on a fault handler existing.
    unsafe { hal_x86_64::interrupts::init_faults_only() };

    let source = Tsc;
    // The shared `CycleSource` conformance suite, run against the real
    // x86_64 backend before any number derived from it is reported.
    let conformance = conformance::check(&source, CONFORMANCE_SAMPLES);
    let conformance_ok = match conformance {
        Ok(span) => {
            let _ = writeln!(serial, "fixture-measure cycle_source_conformance ok span={span}");
            true
        }
        Err(failure) => {
            let _ = writeln!(serial, "fixture-measure cycle_source_conformance FAILED {failure:?}");
            false
        }
    };

    // SAFETY: nothing else in this fixture uses PIT channel 2 or port 0x61,
    // and no timer is armed on this boot path — `calibrate_cycles_per_us`'s
    // documented contract, met before any measurement starts.
    let timebase = unsafe { tsc::calibrate_cycles_per_us() };
    let calibration = Calibration::measure(&source, CALIBRATION_SAMPLES);

    // SAFETY: single-threaded, non-reentrant fixture; every phase below
    // borrows this buffer for its own duration only and returns before the
    // next one starts, and this is the only code in the binary that touches
    // it.
    let samples: &mut Samples<SAMPLES> = unsafe { &mut *core::ptr::addr_of_mut!(SAMPLE_BUFFER) };

    let mut ok = conformance_ok;
    // Sized FROM `METRICS` rather than spelled out: a hand-written run of
    // `None`s is a second declaration of the metric count that nothing keeps
    // in step with the first. Adding the ninth metric (`PERF-D07-G23`'s
    // spoor-enabled arm) bumped `METRICS` and left eight `None`s here, which
    // no local gate compiles — this file is built only for the x86_64 kernel
    // binary — so it reached CI as a type error. The AArch64 fixture beside it
    // already did this.
    let mut collected: [Option<Measured>; METRICS] = [const { None }; METRICS];

    // The reference goes first: it is the denominator of every ratio the gate
    // compares, so a run in which it did not happen is not a run with one
    // metric missing — it is a run with no gated evidence at all.
    ok &= phase_reference_loop(&source, &calibration, samples);
    ok &= collect(&mut collected, 0, "REF", "fixed_integer_loop", samples);
    let _ = writeln!(serial, "fixture-measure phase 1/11 done (REF gate reference)");

    ok &= phase_pool_alloc_free(&source, &calibration, samples);
    ok &= collect(&mut collected, 1, "D07", "pool_u64x64_alloc_free_round_trip", samples);
    let _ = writeln!(serial, "fixture-measure phase 2/11 done (D07 alloc/free)");

    ok &= phase_pool_denial(&source, &calibration, samples);
    ok &= collect(
        &mut collected,
        2,
        "D07",
        "pool_u64x4_alloc_denied_exhausted_per_op_of_64",
        samples,
    );
    let _ = writeln!(serial, "fixture-measure phase 3/11 done (D07 denial)");

    ok &= phase_context_switch(&source, &calibration, samples);
    ok &= collect(&mut collected, 3, "D04", "context_switch_yield_roundtrip_2switches", samples);
    let _ = writeln!(serial, "fixture-measure phase 4/11 done (D04 context switch)");

    ok &= phase_dispatch_select(&source, &calibration, samples);
    ok &= collect(&mut collected, 4, "D05", "dispatch_select_highest_priority_ready", samples);
    let _ = writeln!(serial, "fixture-measure phase 5/11 done (D05 selection)");

    ok &= phase_dispatch_round(&source, &calibration, samples);
    ok &= collect(&mut collected, 5, "D05", "dispatch_run_once_cooperative_round", samples);
    let _ = writeln!(serial, "fixture-measure phase 6/11 done (D05 dispatch round)");

    ok &= phase_fault_latency(&calibration);
    ok &= collect(&mut collected, 6, "D02", "fault_ud2_capture_terminate_kernel_context", samples);
    let _ = writeln!(serial, "fixture-measure phase 7/11 done (D02 fault latency)");

    // Last rather than beside its unbatched twin, so the six pre-existing
    // phases keep their order and their chatter unchanged.
    ok &= phase_pool_alloc_free_batched(&source, &calibration, samples);
    ok &=
        collect(&mut collected, 7, "D07", "pool_u64x64_alloc_free_round_trip_per_op_of_8", samples);
    let _ = writeln!(serial, "fixture-measure phase 8/11 done (D07 batched round trip, LE-24)");

    // `PERF-D07-G23`'s spoor-ENABLED arm. Immediately after its twin and
    // sharing the same sample buffer, so the pair is measured under the same
    // thermal and cache conditions in the same run: two arms taken minutes
    // apart, or in different runs, would carry the ~3% build-to-build movement
    // BOARD VERDICT 9 observed as if it were spoor overhead. The ratio the
    // gate asks for is (this p99 - slot 7's p99) / slot 7's p99, and it is
    // computed by a reader from these two rows rather than asserted here --
    // the fixture emits measurements, never verdicts.
    ok &= phase_pool_alloc_free_batched_spoored(&source, &calibration, samples);
    ok &= collect(
        &mut collected,
        8,
        "D07",
        "pool_u64x64_alloc_free_round_trip_per_op_of_8_spoored",
        samples,
    );
    let _ = writeln!(serial, "fixture-measure phase 9/11 done (D07 G23 spoor-enabled arm)");

    // `PERF-D04-G23` and `PERF-D05-G23`: the same paired-arm method on the two
    // domains whose disabled arms are slots 3 and 5 of this same run. Their
    // twins are minutes earlier in the run rather than immediately before them,
    // which is a weaker pairing than the D07 one and is stated rather than
    // hidden -- on this host tier the alternative is re-running the disabled
    // arms and having two D04 rows that disagree.
    ok &= phase_context_switch_spoored(&source, &calibration, samples);
    ok &= collect(
        &mut collected,
        9,
        "D04",
        "context_switch_yield_roundtrip_2switches_spoored",
        samples,
    );
    let _ = writeln!(serial, "fixture-measure phase 10/11 done (D04 G23 spoor-enabled arm)");

    ok &= phase_dispatch_round_spoored(&source, &calibration, samples);
    ok &=
        collect(&mut collected, 10, "D05", "dispatch_run_once_cooperative_round_spoored", samples);
    let _ = writeln!(serial, "fixture-measure phase 11/11 done (D05 G23 spoor-enabled arm)");

    let environment = Environment {
        tier: "T0",
        arch: "x86_64",
        platform: "qemu-tcg-x86_64",
        qualification: kernel::measure::UNQUALIFIED,
        cycle_source: Tsc::NAME,
        overhead_cycles: calibration.overhead_cycles(),
        cycles_per_us: timebase.cycles_per_us(),
    };
    let Some(metrics) = emit_all(&mut serial, &environment, &collected) else {
        return false;
    };
    let _ = writeln!(serial, "fixture-measure metrics={metrics}");
    // The verdict travels as its own sentinel line (`STORY-P1-01-02`), not as
    // prose: on Tier 0 the host cross-checks it against the isa-debug-exit
    // code, and on a Raspberry Pi 5 — which has no such port (`LE-09`) — it is
    // the only pass/fail bit that exists.
    let verdict = ok && metrics == METRICS;
    let _ = kernel::measure::write_result(&mut serial, "measure", verdict);
    verdict
}
