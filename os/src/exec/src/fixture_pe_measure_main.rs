//! `D09` work-unit measurement: PE64 parsing and import validation
//! (`STORY-P0-01-06`, `TEST-P0-01-06-A` clause 2).
//!
//! `D09`'s catalogue rows are titled *"PE64 loading and import validation"* and
//! every one of them is tiered `Host+T0`. Ten of the twenty-five have never had
//! a number behind them, not because the tier was unavailable but because
//! nobody had pointed the shared harness at this domain. This fixture does
//! that, and `STORY-P0-01-06` is the audit of what the resulting numbers can
//! and cannot close.
//!
//! **The subject is the real artifact, not a hand-built image.** It parses the
//! same `blue-sharc.txe` that `blue-sharc-fixture` loads — a genuine PE64 with
//! a genuine import table — for the reason that fixture's own comment gives: a
//! synthetic image measures the parser against input shaped by the person who
//! wrote the parser. `D09`'s title says *import validation*, and an image with
//! no import table would not have measured it.
//!
//! **Two phases, because `G20` is stated against input that fails.**
//!
//! - `pe_parse_blue_sharc_accept` — the whole accept path: DOS header, PE
//!   signature, COFF and optional headers, every section header, and the
//!   import directory walk.
//! - `pe_parse_denied_truncated` — the denial path, against a prefix of the
//!   same image. `PERF-D09-G20` budgets *"denied or malformed work completes
//!   <= 125 us; state changes = 0; allocations = 0"*, and a domain measured
//!   only on well-formed input has not been measured on the input that
//!   guardrail is about. Same reasoning as `fixture_measure`'s separate
//!   `pool_alloc_denied_exhausted` phase.
//!
//! **Why this is not a phase inside `fixture-measure`.** `TEST-P0-01-06-A`
//! clause 3. The gated envelope refuses a measured-but-unbaselined metric
//! (`GateError::MetricNotBaselined`), so adding `D09` there forces a baseline
//! re-record in the same commit — and a re-record taken on a Windows dev host
//! bakes in the confirmed 23-53% cross-host offset (`LE-23`) that `LE-28`
//! warns is one command from a false green. Producing `D09` evidence by
//! corrupting every other domain's baseline would be a net loss, so this
//! follows the `fixture-pool-bench` precedent instead: the shared harness and
//! the same versioned envelope, outside the gated set.
//!
//! **What a number from this fixture is.** Tier 0 QEMU/TCG evidence about the
//! parser and the harness. `LE-09` is untouched: no Tier 0 number is hardware
//! WCET evidence, and this fixture closes no hardware debt.

#![no_std]
#![no_main]
#![allow(static_mut_refs, clippy::deref_addrof)]

use core::fmt::Write;
use exec::pe;
use hal::time::Timebase;
use hal_x86_64::fault::FaultFrame;
use hal_x86_64::qemu_exit::{exit_qemu, QemuExitCode};
use hal_x86_64::serial::SerialPort;
use hal_x86_64::tsc::{self, Tsc};
use kernel::measure::{
    write_result, Calibration, Environment, Metric, MetricLabel, Report, Samples, Stopwatch,
};

/// Every metric this fixture emits, in emission order — `LE-91`'s declaration.
///
/// Both are `D09` — *"PE64 loading and import validation"*, which is exactly
/// and only what the two phases parse — and both serve `STORY-P0-01-06`,
/// whose whole subject is `D09`'s disposition and whose criterion 2 is this
/// fixture.
///
/// **This pair is `LE-91`'s second instance, found by the gate rather than by
/// a reader, and it points the opposite way from the first.** The three spoor
/// metrics were bent *to* a contract; here the label was right all along and
/// the contract simply never selected `D09` — `STORY-P0-01-06`'s row read
/// `D01` alone while the Story's title, its acceptance criteria and its
/// evidence were all about `D09`. Nothing had ever compared the two, because
/// nothing read the emit site against the register at all. The contract now
/// selects `D01,D09`.
static METRIC_LABELS: [MetricLabel; 2] = [
    MetricLabel { domain: "D09", story: "STORY-P0-01-06", name: "pe_parse_blue_sharc_accept" },
    MetricLabel { domain: "D09", story: "STORY-P0-01-06", name: "pe_parse_denied_truncated" },
];

/// Section capacity, matching `blue-sharc-fixture`'s own figure for the same
/// artifact.
const SECTIONS: usize = 8;

/// Import capacity, matching `blue-sharc-fixture`. The real image's import
/// table is what makes this domain's title honest.
const IMPORTS: usize = 256;

/// Samples per phase. Lower than `fixture_measure`'s 1 000 because one sample
/// here is a whole multi-megabyte image parse rather than a pool round trip,
/// and the run has to finish inside the fixture timeout.
const SAMPLES: usize = 200;

/// Unmeasured iterations before sampling, so first-touch cache effects land
/// outside the reported percentiles. Reported in the envelope's `warmup=`
/// field rather than left implicit.
const WARMUP: usize = 20;

/// Read pairs used to calibrate the cycle source's own overhead.
const CALIBRATION_SAMPLES: usize = 1_000;

/// How much of the image the denial phase feeds the parser.
///
/// Past the DOS header and the PE signature, so the rejection happens *inside*
/// the header walk rather than on the first two bytes — a denial measured at
/// `bytes[0..2] != "MZ"` would be measuring nothing but a comparison, and
/// `G20`'s budget is about work that gets some way in before failing closed.
const TRUNCATED_LEN: usize = 512;

#[repr(C, align(4096))]
struct AlignedImage([u8; 8_269_824]);

static IMAGE_BYTES: AlignedImage = AlignedImage(*include_bytes!("../fixtures/blue-sharc.txe"));
static mut SAMPLE_BUFFER: Samples<SAMPLES> = Samples::new();

/// This fixture expects no faults at all; a fault means the measurement is not
/// measuring what it claims, so it is terminal and loud.
///
/// # Safety
/// Called only by the fault stubs, with `frame` pointing at a
/// fully-initialized [`FaultFrame`] on the faulting stack.
#[no_mangle]
extern "C" fn tinyos_fault_entry(frame: *const FaultFrame) -> ! {
    // SAFETY: the stubs pass a valid `FaultFrame` pointer, live for this call.
    let frame = unsafe { *frame };
    // SAFETY: never returns; no concurrent COM1 user on this single-CPU path.
    let mut serial = unsafe { SerialPort::init() };
    let _ = writeln!(
        serial,
        "pe-measure unexpected fault vector={} rip={:#x} cr2={:#x}",
        frame.vector,
        frame.rip,
        frame.faulting_address().unwrap_or(0)
    );
    let _ = write_result(&mut serial, "pe-measure", false);
    exit_qemu(QemuExitCode::Failure)
}

/// Required `#DF` entry — never expected to be reached.
///
/// # Safety
/// Called only by `df_fault_stub` with a valid IST-stack [`FaultFrame`].
#[no_mangle]
extern "C" fn tinyos_double_fault_entry(_frame: *const FaultFrame) -> ! {
    // SAFETY: never returns; see `tinyos_fault_entry`.
    let mut serial = unsafe { SerialPort::init() };
    let _ = writeln!(serial, "pe-measure unexpected #DF");
    let _ = write_result(&mut serial, "pe-measure", false);
    exit_qemu(QemuExitCode::Failure)
}

/// `D09` accept path: the whole parse, including the import directory walk.
///
/// The self-consistency check is deliberately stronger than "it returned
/// `Ok`": every iteration must produce the *same* entry point, section count
/// and import count. `pe::parse` is documented as pure and deterministic, and
/// a measurement loop is the cheapest place that claim is ever exercised
/// hundreds of times in a row.
#[inline(never)]
fn phase_parse_accept(
    source: &Tsc,
    calibration: &Calibration,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    let mut expected: Option<(u32, usize, usize)> = None;
    for index in 0..(WARMUP + SAMPLES) {
        let watch = Stopwatch::start(source);
        let parsed = pe::parse::<SECTIONS, IMPORTS>(&IMAGE_BYTES.0);
        let cycles = watch.stop(calibration);

        let Ok(descriptor) = parsed else { return false };
        let observed = (
            descriptor.entry_point_rva(),
            descriptor.sections().count(),
            descriptor.imports().count(),
        );
        match expected {
            None => expected = Some(observed),
            Some(first) if first != observed => return false,
            Some(_) => {}
        }

        if index >= WARMUP {
            samples.record(cycles);
        }
    }
    // An image that parsed to zero imports would make this domain's "import
    // validation" title untrue for this measurement, so it is a failure here
    // rather than a footnote in the report.
    matches!(expected, Some((_, sections, imports)) if sections > 0 && imports > 0)
}

/// `D09` denial path (`PERF-D09-G20`): a truncated image must fail closed,
/// every time, at the same error.
#[inline(never)]
fn phase_parse_denied(
    source: &Tsc,
    calibration: &Calibration,
    samples: &mut Samples<SAMPLES>,
) -> bool {
    let truncated = &IMAGE_BYTES.0[..TRUNCATED_LEN];
    let mut expected: Option<pe::PeError> = None;
    for index in 0..(WARMUP + SAMPLES) {
        let watch = Stopwatch::start(source);
        let parsed = pe::parse::<SECTIONS, IMPORTS>(truncated);
        let cycles = watch.stop(calibration);

        let Err(error) = parsed else { return false };
        match expected {
            None => expected = Some(error),
            // A denial that varies run to run is not a fail-closed path; it is
            // a path whose outcome depends on something nobody declared.
            Some(first) if first != error => return false,
            Some(_) => {}
        }

        if index >= WARMUP {
            samples.record(cycles);
        }
    }
    expected.is_some()
}

fn run() -> bool {
    // SAFETY: this fixture is the only code running (single-CPU boot path, no
    // other UART user) and `init` is called exactly once, before any other
    // `SerialPort` method — `init`'s own documented contract.
    let mut serial = unsafe { SerialPort::init() };

    // A real IDT before anything is measured, so an unexpected fault reaches
    // `tinyos_fault_entry` above and reports, instead of triple-faulting into
    // a truncated envelope. Arms no timer and never executes `sti`, so it
    // changes nothing about the interrupt-free measurement below.
    //
    // SAFETY: called once, here, before anything depends on a fault handler
    // existing — `init_faults_only`'s documented contract.
    unsafe { hal_x86_64::interrupts::init_faults_only() };

    let source = Tsc;
    let calibration = Calibration::measure(&source, CALIBRATION_SAMPLES);
    // SAFETY: nothing else in this fixture uses PIT channel 2 or port 0x61,
    // and no timer is armed on this boot path — `calibrate_cycles_per_us`'s
    // documented contract, met before any measurement starts so an interrupt
    // cannot inflate the factor.
    let timebase = unsafe { tsc::calibrate_cycles_per_us() };

    let environment = Environment {
        tier: "T0",
        arch: "x86_64",
        platform: "qemu-tcg-x86_64",
        qualification: kernel::measure::UNQUALIFIED,
        cycle_source: "Tsc",
        overhead_cycles: calibration.overhead_cycles(),
        cycles_per_us: timebase.cycles_per_us(),
    };

    let mut ok = true;
    let Ok(mut report) = Report::begin(&mut serial, &environment) else { return false };

    // SAFETY: single-threaded, non-reentrant fixture; each phase borrows this
    // buffer for its own duration only and returns before the next starts.
    let samples = unsafe { &mut *(&raw mut SAMPLE_BUFFER) };

    samples.clear();
    ok &= phase_parse_accept(&source, &calibration, samples);
    match samples.summarize() {
        Some(summary) => {
            ok &= report.metric(&Metric::labelled(&METRIC_LABELS[0], WARMUP, summary)).is_ok();
        }
        None => ok = false,
    }

    samples.clear();
    ok &= phase_parse_denied(&source, &calibration, samples);
    match samples.summarize() {
        Some(summary) => {
            ok &= report.metric(&Metric::labelled(&METRIC_LABELS[1], WARMUP, summary)).is_ok();
        }
        None => ok = false,
    }

    let Ok(metrics) = report.end() else { return false };
    let verdict = ok && metrics == 2;
    let _ = write_result(&mut serial, "pe-measure", verdict);
    verdict
}

#[no_mangle]
extern "C" fn kernel_main(_start_info_paddr: u64) -> ! {
    if run() {
        exit_qemu(QemuExitCode::Success)
    } else {
        exit_qemu(QemuExitCode::Failure)
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    hal_x86_64::qemu_exit::panic_report(info)
}
