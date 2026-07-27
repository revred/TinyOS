//! `TEST-P1-01-01-A`'s Tier 0 measurement fixture: drives the three
//! performance domains `FEAT-P1-01` names as its first measured targets —
//! context switch (`D04`), ready-queue dispatch (`D05`), and pool allocation
//! (`D07`) — through [`kernel::measure`], compiled for the real
//! `x86_64-tinyos` target and run under QEMU so every cycle count comes from
//! the target binary's own cycle-source reads rather than a host process.
//!
//! Everything measurement-shaped here comes from the shared harness: the
//! cycle source is [`hal::time::CycleSource`] (this fixture never names
//! `RDTSC` — `hal_x86_64::tsc::Tsc` does), the buffers are
//! [`kernel::measure::Samples`], the percentiles are
//! [`kernel::measure::Samples::summarize`], and the output is the versioned
//! `TINYOS-MEAS/1` envelope `xtask measure` parses. The fixture's own code is
//! therefore *only* the three workloads plus their self-consistency checks —
//! which is the point of the Story: a new domain becomes a phase function,
//! not a new copy of a measurement harness.
//!
//! **Per-phase `#[inline(never)]` functions, not one `run` body.** Identical
//! reason to `fixture_pool_bench`'s own doc comment: this workspace's
//! unoptimized dev-profile build does not reuse stack slots across
//! lexically-separate blocks inside one function, so a monolithic `run`
//! accumulates every phase's locals into one activation record and walks off
//! the boot stack (a real triple fault, hit during that fixture's bring-up).
//!
//! **What a number from this fixture is.** Tier 0 evidence about the
//! *mechanism* and the *harness*, calibrating both. QEMU/TCG's TSC and PIT are
//! software models, so neither the cycle counts nor the microseconds derived
//! from them are hardware WCET evidence — that remains explicit,
//! release-blocking debt until measured on the Raspberry Pi 5 (`LE-09`).
//!
//! Only reachable when the `fixture-measure` feature is enabled — never part
//! of a real boot image.

use core::fmt::Write;
use hal::time::{conformance, CycleSource, Timebase};
use hal_x86_64::fault::FaultFrame;
use hal_x86_64::serial::SerialPort;
use hal_x86_64::tsc::{self, Tsc};
use kernel::context::{self, Context};
use kernel::dispatch;
use kernel::fault::{Disposition, FaultReport, FaultingContext};
use kernel::measure::{Calibration, Environment, Metric, Report, Samples, Stopwatch};
use kernel::mem::Pool;
use kernel::sched::{OverrunPolicy, Priority, Scheduler, TaskState, WcetBudgetTicks};

/// Every phase's sample capacity. One shared buffer in `.bss`, cleared
/// between phases (the reuse pattern [`Samples::clear`] exists for), rather
/// than one buffer per phase — keeps this fixture's static footprint at
/// 8 KiB × 1 instead of × 5.
const SAMPLES: usize = 1_000;

/// Unmeasured iterations before sampling starts, so first-touch page/cache
/// effects land outside the reported percentiles. Reported in the envelope's
/// `warmup=` field rather than left implicit.
const WARMUP: usize = 100;

/// Read pairs used to calibrate the cycle source's own overhead.
const CALIBRATION_SAMPLES: usize = 2_000;

/// Reads taken by the shared [`conformance`] suite against this
/// architecture's [`CycleSource`] — the same suite the host tests run against
/// a test double, executed here against real silicon-or-emulation.
const CONFORMANCE_SAMPLES: usize = 64;

/// Task stack size for the D04/D05 phases. Both phases' tasks do nothing but
/// increment a counter and yield, so this is generous.
const STACK_SIZE: usize = 4_096;

/// Scheduler capacity for the D05 dispatch phase.
const TASKS: usize = 4;

static mut SAMPLE_BUFFER: Samples<SAMPLES> = Samples::new();

// D04/D05 task state. Statics rather than locals for the same reason
// `context_switch_fixture.rs` uses statics: a `Context` and its stack must
// never move once a `switch` has been taken into them.
static mut MEASURE_CTX: Context = Context::zeroed();
static mut TASK_CTX: Context = Context::zeroed();
static mut TASK_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
static mut YIELDS: u64 = 0;

static mut DISPATCHER_CTX: Context = Context::zeroed();
static mut DISPATCH_CONTEXTS: [Context; TASKS] = [Context::zeroed(); TASKS];
static mut DISPATCH_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

// D02 (`LE-17`) fault-latency state. A fault handler never resumes the
// context it interrupted (`kernel::fault`'s own doc comment — there is no
// `Resume` arm), so unlike `TASK_CTX` above, `FAULT_TASK_CTX` is
// reinitialized every iteration rather than looped inside one long-lived
// task: each iteration needs a fresh entry point to fault from.
static mut FAULT_SUPERVISOR_CTX: Context = Context::zeroed();
static mut FAULT_TASK_CTX: Context = Context::zeroed();
/// Where the fault handler saves the faulted iteration's registers. Written
/// once per iteration and never read — same rationale as
/// `fixture_fault::ABANDONED_CTX`: a context nothing will ever resume is the
/// honest destination.
static mut FAULT_ABANDONED_CTX: Context = Context::zeroed();
static mut FAULT_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
/// Cycle count read immediately before the victim's `ud2`, so the handler
/// measures exactly the fault-to-disposition-decided span.
static mut FAULT_START_CYCLES: u64 = 0;
/// Set once before the phase begins; read (never written) by the handler on
/// every fault.
static mut FAULT_CALIBRATION: Calibration = Calibration::from_overhead_cycles(0);
/// Counts iterations so the handler can skip recording during `WARMUP`,
/// exactly like every other phase's driver loop does for itself.
static mut FAULT_ITERATIONS_RUN: usize = 0;

/// D04's measured task: yields straight back to whoever resumed it, so one
/// timed region is exactly one switch out and one switch back — the smallest
/// unit that can be measured without a timer interrupt to preempt it
/// (`FEAT-P1-04`'s charge, not this Story's).
extern "C" fn yield_forever() -> ! {
    loop {
        // SAFETY: single-CPU boot fixture; only this task writes `YIELDS`,
        // and `TASK_CTX`/`MEASURE_CTX` are the two slots this phase's
        // measurement loop switches between, never concurrently.
        unsafe {
            YIELDS += 1;
            context::switch(&raw mut TASK_CTX, &raw mut MEASURE_CTX);
        }
    }
}

/// D05's dispatched task: yields back to the dispatcher context, which is
/// what `dispatch::run_once` switched away from.
extern "C" fn dispatch_yield_forever() -> ! {
    loop {
        // SAFETY: see `yield_forever`; slot 0 is the only context this phase
        // ever initializes or switches into.
        unsafe {
            context::switch(&raw mut DISPATCH_CONTEXTS[0], &raw mut DISPATCHER_CTX);
        }
    }
}

/// D07 (`PERF-D07-G01`..`G07`): `Pool<u64, 64>` alloc/free round trip — the
/// same operation shape `fixture_pool_bench` measures, here as the harness's
/// canonical D07 metric so `xtask measure` reports one comparable number for
/// it across runs.
#[inline(never)]
fn phase_pool_alloc_free<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    let mut pool: Pool<u64, 64> = Pool::new();
    let mut ok = true;
    for index in 0..(WARMUP + SAMPLES) {
        let value = index as u64;
        let watch = Stopwatch::start(source);
        let handle = match pool.alloc(value) {
            Ok(handle) => handle,
            Err(_) => {
                ok = false;
                continue;
            }
        };
        let freed = pool.free(handle);
        let cycles = watch.stop(calibration);
        if freed != Ok(value) {
            ok = false;
        }
        if index >= WARMUP {
            samples.record(cycles);
        }
    }
    ok
}

/// D07 (`PERF-D07-G20`): the denial path — an exhausted pool's `alloc` must be
/// both fast *and* free of state change. Measured separately from the happy
/// path because the guardrail's budget is separate (and tighter).
#[inline(never)]
fn phase_pool_denial<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    let mut pool: Pool<u64, 4> = Pool::new();
    for index in 0..4 {
        if pool.alloc(index as u64).is_err() {
            return false;
        }
    }
    let mut ok = true;
    for index in 0..(WARMUP + SAMPLES) {
        let watch = Stopwatch::start(source);
        let denied = pool.alloc(0xDEAD);
        let cycles = watch.stop(calibration);
        if denied.is_ok() {
            ok = false;
        }
        if index >= WARMUP {
            samples.record(cycles);
        }
    }
    // Every slot must still hold its original value: a denial that changed
    // observable state is a `PERF-D*-G20` failure regardless of its latency.
    let mut occupied = 0;
    for (handle, value) in pool.iter_occupied() {
        occupied += 1;
        if *value != handle.index() as u64 {
            ok = false;
        }
    }
    ok && occupied == 4
}

/// D04 (`PERF-D04-G01`..`G07`): one `context::switch` out to a task that
/// immediately switches back — two switches per sample, which the metric name
/// states explicitly so no reader mistakes it for a single-switch figure.
#[inline(never)]
fn phase_context_switch<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    // SAFETY: this fixture is the only code running; `TASK_STACK` is used by
    // exactly one `Context` for the whole phase and never moves (it is a
    // `static`), and `TASK_CTX`/`MEASURE_CTX` are switched between strictly
    // alternately below, never concurrently.
    unsafe {
        let stack = core::slice::from_raw_parts_mut((&raw mut TASK_STACK).cast::<u8>(), STACK_SIZE);
        let Ok(task) = Context::new(stack, yield_forever) else {
            return false;
        };
        TASK_CTX = task;
        YIELDS = 0;
    }

    for index in 0..(WARMUP + SAMPLES) {
        let watch = Stopwatch::start(source);
        // SAFETY: `TASK_CTX` was initialized above and is suspended at either
        // its entry point (first iteration) or its own `switch` call site
        // (every later one); `MEASURE_CTX` is this context's own slot, which
        // only the task's matching `switch` resumes. Exactly `switch`'s
        // documented contract.
        unsafe { context::switch(&raw mut MEASURE_CTX, &raw mut TASK_CTX) };
        let cycles = watch.stop(calibration);
        if index >= WARMUP {
            samples.record(cycles);
        }
    }

    // SAFETY: read after every switch above has returned; single-CPU.
    let yields = unsafe { YIELDS };
    yields == (WARMUP + SAMPLES) as u64
}

/// D05 (`PERF-D05-G01`..`G07`): ready-queue selection alone —
/// `Scheduler::highest_priority_ready` over a populated scheduler, with no
/// switch in the timed region, because this is the metric `D05`'s budget is
/// actually about.
#[inline(never)]
fn phase_dispatch_select<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    let mut scheduler: Scheduler<TASKS> = Scheduler::new();
    let mut created = 0;
    for priority in [3u8, 9, 17, 25] {
        let Ok(priority) = Priority::try_new(priority) else {
            return false;
        };
        if scheduler
            .create_task(
                priority,
                WcetBudgetTicks(1_000),
                OverrunPolicy::TripToSafeState,
                dispatch_yield_forever,
            )
            .is_ok()
        {
            created += 1;
        }
    }
    if created != TASKS {
        return false;
    }

    let mut ok = true;
    for index in 0..(WARMUP + SAMPLES) {
        let watch = Stopwatch::start(source);
        let selected = scheduler.highest_priority_ready();
        // The deliberate regression (`fixture-measure-regression`, never
        // enabled in a real image): seven extra selections inside the timed
        // region, making this phase ~8x slower so the gate has something real
        // to catch. A doctored baseline file would prove only that the
        // comparison arithmetic works; this proves the whole path — build,
        // boot, measure, compare, fail — does.
        #[cfg(feature = "fixture-measure-regression")]
        let selected = {
            let mut last = selected;
            for _ in 0..7 {
                last = scheduler.highest_priority_ready();
            }
            last
        };
        let cycles = watch.stop(calibration);
        // The highest-priority Ready task is the last one created (priority
        // 25, slot 3) and nothing below changes any task's state, so every
        // selection must return it.
        if selected.map(|task| task.index()) != Some(3) {
            ok = false;
        }
        if index >= WARMUP {
            samples.record(cycles);
        }
    }
    ok
}

/// D05 (`PERF-D05-G13`/`G14`): a whole cooperative dispatch round —
/// select, switch in, the task yields, book-keep back to Ready. The
/// enqueue-to-service figure `D05`'s queue guardrails are stated against, and
/// the metric `FEAT-P1-04`'s preemptive version will be compared to.
#[inline(never)]
fn phase_dispatch_round<S: CycleSource>(
    source: &S,
    calibration: &Calibration,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    let mut scheduler: Scheduler<TASKS> = Scheduler::new();
    let Ok(priority) = Priority::try_new(11) else {
        return false;
    };
    let Ok(task) = scheduler.create_task(
        priority,
        WcetBudgetTicks(1_000),
        OverrunPolicy::TripToSafeState,
        dispatch_yield_forever,
    ) else {
        return false;
    };
    if task.index() != 0 {
        return false;
    }

    // SAFETY: slot 0 is the only context this phase initializes or switches
    // into; `DISPATCH_STACK` is a never-moving static owned solely by it.
    unsafe {
        let stack =
            core::slice::from_raw_parts_mut((&raw mut DISPATCH_STACK).cast::<u8>(), STACK_SIZE);
        let Ok(context) = Context::new(stack, dispatch_yield_forever) else {
            return false;
        };
        DISPATCH_CONTEXTS[0] = context;
    }

    let mut ok = true;
    for index in 0..(WARMUP + SAMPLES) {
        let watch = Stopwatch::start(source);
        // SAFETY: `DISPATCH_CONTEXTS[0]` was initialized above and is
        // suspended at its entry point or its own `switch` call site;
        // `DISPATCHER_CTX` is this context's own slot. `run_once`'s
        // documented contract, met exactly as `dispatch.rs`'s own test meets
        // it.
        let ran = unsafe {
            dispatch::run_once(&mut scheduler, &raw mut DISPATCHER_CTX, &raw mut DISPATCH_CONTEXTS)
        };
        let cycles = watch.stop(calibration);
        if ran != Some(task) || scheduler.state_of(task) != Some(TaskState::Ready) {
            ok = false;
        }
        if index >= WARMUP {
            samples.record(cycles);
        }
    }
    ok
}

/// D02's victim: timestamps itself, then raises a real `#UD` — the same
/// architecturally-guaranteed instruction `fixture_fault::victim_invalid_opcode`
/// uses, chosen here for the same reason: it is the one vector this kernel
/// can fault from deterministically, with no dependency on GDT/page-table
/// shape (unlike `#GP`/`#PF`, whose fixtures both note reasons a chosen
/// address or selector could drift under a future change).
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
/// latency — the baseline `FEAT-P1-02`'s exit criteria name and
/// `TEST-P1-02-01-A` clause 8 named as follow-on work rather than quietly
/// skipping it.
///
/// Measures the real hardware path: a real `#UD` through this fixture's own
/// `tinyos_fault_entry` (below), which runs the same `kernel::fault::of`/
/// `audit` calls the production and `fixture_fault` entry points run, timed
/// from immediately before the faulting instruction to immediately after the
/// disposition and its audit pair are computed — the same span `PERF-D02-G21`
/// ("fault decision and containment") states its budget against.
///
/// Uses `FaultingContext::Kernel` rather than a real scheduled task: this
/// fixture measures the capture-decide-audit cost, which
/// `kernel::fault::audit` computes identically regardless of which context
/// faulted (it branches only on the *outcome* value stamped, never on cost),
/// so no `Scheduler`/`TaskId` machinery is needed to measure it honestly. The
/// containment behavior itself — a real task actually being terminated while
/// its siblings keep running — is `fixture_fault`'s charge, already Verified;
/// this fixture's only job is the timing.
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
        // abandoned by the handler before this loop resumed, so reusing the
        // stack memory is safe. `FAULT_SUPERVISOR_CTX`/`FAULT_TASK_CTX` are
        // switched between strictly alternately, matching `switch`'s
        // documented contract.
        unsafe {
            let stack =
                core::slice::from_raw_parts_mut((&raw mut FAULT_STACK).cast::<u8>(), STACK_SIZE);
            let Ok(task) = Context::new(stack, fault_latency_victim) else {
                return false;
            };
            FAULT_TASK_CTX = task;
            context::switch(&raw mut FAULT_SUPERVISOR_CTX, &raw mut FAULT_TASK_CTX);
            // Control returns here only via the handler's escape switch,
            // after the fault has been captured, decided and recorded.
        }
    }

    // SAFETY: read after every switch above has returned.
    unsafe { FAULT_ITERATIONS_RUN == WARMUP + SAMPLES }
}

/// The fixture-measure fault entry point (`LE-17`) — installed in place of
/// `main.rs`'s default `tinyos_fault_entry` only under this feature, exactly
/// as `fixture_fault` and `fixture_double_fault` already install their own.
///
/// Unlike the default handler, this one does not halt: it times the
/// disposition path, records the sample, and switches back to the
/// supervisor so `phase_fault_latency`'s loop can continue — the same
/// escape-switch pattern `fixture_fault::tinyos_fault_entry` uses to survive
/// past a fault it deliberately caused.
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
        // functions the production and `fixture_fault` entry points call —
        // so this measures the actual cost, not a hand-rolled stand-in for it.
        let report = FaultReport { vector: frame.vector, context: FaultingContext::Kernel };
        let disposition = Disposition::of(&report);
        let _ = kernel::fault::audit(&report, disposition);

        context::switch(&raw mut FAULT_ABANDONED_CTX, &raw mut FAULT_SUPERVISOR_CTX);
    }
    unreachable!("a measured fault-latency iteration is never switched back into")
}

/// How many metrics this fixture measures — the fixed capacity of the
/// collected-summary array below.
const METRICS: usize = 6;

/// One measured phase, held until every phase has run.
///
/// Phases are measured first and the envelope is emitted once at the end,
/// rather than a `METRIC` line being written between phases, for two reasons:
/// the fixture stays free to print its own progress/self-check chatter on the
/// same UART (a `Report` borrows the sink for its whole lifetime), and a
/// fixture that dies mid-measurement produces no envelope at all rather than a
/// half-open one — which `xtask` then rejects as truncated, exactly the
/// fail-closed outcome `TEST-P1-01-01-A` clause 6 requires.
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
/// percentiles themselves are evidence this fixture reports, never thresholds
/// it enforces: gating is `STORY-P1-01-02`'s charge.
pub fn run() -> bool {
    // SAFETY: this fixture is the only code running (single-CPU boot path, no
    // other UART user) and `init` is called exactly once, before any other
    // `SerialPort` method — `init`'s own documented contract.
    let mut serial = unsafe { SerialPort::init() };

    // Retire the legacy PIC and install fault-only handling before anything
    // is measured. This is not housekeeping — it is load-bearing, and finding
    // out why cost this fixture its first bring-up failure: `Context::new`
    // seeds a task's initial `rflags` with `IF` set, so the first
    // `context::switch` into a measured task enables interrupts. With the PIC
    // left in its power-on state, a legacy IRQ0 that accumulated during the
    // ~10 ms PIT calibration below then fired the instant `IF` went high,
    // against an empty IDT: triple fault, QEMU shutdown, and a truncated
    // envelope (loose ends `LE-03`/`LE-04`, plus `LE-11` for the
    // `IF`-set-with-no-IDT seam this uncovered).
    //
    // `init_faults_only` (not the bare PIC remap this fixture called before
    // `LE-17`) is what `phase_fault_latency` needs: a real IDT routing `#UD`
    // to this file's own `tinyos_fault_entry`, below. It arms no APIC timer
    // and never executes `sti` itself, so it changes nothing about the four
    // earlier phases' own interrupt-free measurement — only a deliberate
    // fault reaches this IDT at all.
    //
    // SAFETY: called once, here, before any other code depends on interrupts
    // being masked or on a fault handler existing — `init_faults_only`'s
    // documented contract.
    unsafe { hal_x86_64::interrupts::init_faults_only() };

    let source = Tsc;
    // The shared `CycleSource` conformance suite, run here against the real
    // x86_64 backend: the same checks the host tests run against a test
    // double, so a backend that violates the trait contract fails before any
    // number derived from it is reported.
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
    // documented contract, met before any measurement starts so an interrupt
    // cannot inflate the factor.
    let timebase = unsafe { tsc::calibrate_cycles_per_us() };
    let calibration = Calibration::measure(&source, CALIBRATION_SAMPLES);

    // SAFETY: single-threaded, non-reentrant fixture; every phase below
    // borrows this buffer for its own duration only and returns before the
    // next one starts, and this is the only code in the binary that touches
    // it.
    let samples: &mut Samples<SAMPLES> = unsafe { &mut *core::ptr::addr_of_mut!(SAMPLE_BUFFER) };

    let mut ok = conformance_ok;
    let mut collected: [Option<Measured>; METRICS] = [None, None, None, None, None, None];

    ok &= phase_pool_alloc_free(&source, &calibration, samples);
    ok &= collect(&mut collected, 0, "D07", "pool_u64x64_alloc_free_round_trip", samples);
    let _ = writeln!(serial, "fixture-measure phase 1/6 done (D07 alloc/free)");

    ok &= phase_pool_denial(&source, &calibration, samples);
    ok &= collect(&mut collected, 1, "D07", "pool_u64x4_alloc_denied_exhausted", samples);
    let _ = writeln!(serial, "fixture-measure phase 2/6 done (D07 denial)");

    ok &= phase_context_switch(&source, &calibration, samples);
    ok &= collect(&mut collected, 2, "D04", "context_switch_yield_roundtrip_2switches", samples);
    let _ = writeln!(serial, "fixture-measure phase 3/6 done (D04 context switch)");

    ok &= phase_dispatch_select(&source, &calibration, samples);
    ok &= collect(&mut collected, 3, "D05", "dispatch_select_highest_priority_ready", samples);
    let _ = writeln!(serial, "fixture-measure phase 4/6 done (D05 selection)");

    ok &= phase_dispatch_round(&source, &calibration, samples);
    ok &= collect(&mut collected, 4, "D05", "dispatch_run_once_cooperative_round", samples);
    let _ = writeln!(serial, "fixture-measure phase 5/6 done (D05 dispatch round)");

    ok &= phase_fault_latency(&calibration);
    ok &= collect(&mut collected, 5, "D02", "fault_ud2_capture_terminate_kernel_context", samples);
    let _ = writeln!(serial, "fixture-measure phase 6/6 done (D02 fault latency)");

    let environment = Environment {
        tier: "T0",
        arch: "x86_64",
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
