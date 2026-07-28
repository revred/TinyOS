//! `D04` same-space vs cross-space dispatch measurement (`STORY-P1-03-03`).
//!
//! The one exit criterion `FEAT-P1-03` never met. `STORY-P1-03-01` deferred
//! it because nothing in the dispatch path installed a per-task address
//! space, so any number would have been fixture overhead misreported as a
//! scheduling cost; `STORY-P1-03-02` deferred it again while building the
//! integration that makes it measurable. It is measurable now, and this
//! fixture measures it.
//!
//! **What is timed.** Two whole dispatch rounds through the *production*
//! `kernel::dispatch::run_once_in_space` — select the highest-priority Ready
//! task, transfer into it, let it yield, book-keep it back to Ready — that
//! differ in exactly one respect:
//!
//! - `dispatch_round_same_space`: the selected task's `CR3` equals the one
//!   already loaded, so `cr3_reload_needed` is false and the register is
//!   never written. This is the cost every task pays once per-task address
//!   spaces exist at all, even when no switch is needed.
//! - `dispatch_round_cross_space`: the selected task's `CR3` is a genuinely
//!   different tree, so the round performs a real `mov cr3` — and therefore
//!   a full non-global TLB flush, which is the part that actually costs.
//!
//! The difference between them is the number `FEAT-P1-03` asks for: what
//! address-space isolation costs per scheduling decision, isolated from
//! everything else a dispatch round does.
//!
//! **Why each cross-space iteration reloads the supervisor tree first.**
//! `run_once_in_space` installs the *incoming* task's address space and does
//! not restore the dispatcher's on the way back — deliberately, since the
//! shared kernel directories mean the supervisor remains correctly mapped
//! under any task's tree (that is what `STORY-P1-03-02`'s sharing bought).
//! So without an explicit reload between iterations the second round would
//! already be in the task's space and measure the same-space path while
//! claiming to measure the other. The reload sits **outside** the stopwatch,
//! where it belongs: it is this fixture's own setup, not part of the
//! dispatch round under test.
//!
//! Reports through the same `TINYOS-MEAS/2` envelope and `TINYOS-RESULT/1`
//! sentinel every other measured fixture uses, so `xtask measure` and the
//! `check-timing-regression` gate consume it with no special case.

#![no_std]
#![no_main]
#![allow(static_mut_refs, clippy::deref_addrof)]

use exec::address_space::AddressSpace;
use exec::kernel_map::{self, KernelLayout};
use exec::pe::{Permissions, SectionDescriptor};
use hal::time::Timebase;
#[allow(unused_imports)]
// linked for its `global_asm!` side effect only, per its own doc comment
use hal_x86_64::boot as _;
use hal_x86_64::fault::FaultFrame;
use hal_x86_64::paging::{self, FrameAllocator, PageTable, PAGE_SIZE};
use hal_x86_64::qemu_exit::{exit_qemu, QemuExitCode};
use hal_x86_64::serial::SerialPort;
use hal_x86_64::tsc::{self, Tsc};
use kernel::context::{self, Context};
use kernel::dispatch;
use kernel::measure::{write_result, Calibration, Environment, Metric, Report, Samples, Stopwatch};
use kernel::mem::Pool;
use kernel::sched::{OverrunPolicy, Priority, Scheduler, TaskState, WcetBudgetTicks};

unsafe extern "C" {
    static __kernel_exec_start: u8;
    static __kernel_exec_end: u8;
    static __kernel_rodata_start: u8;
    static __kernel_rodata_end: u8;
    static __kernel_image_end: u8;
}

const APIC_PAGE: u64 = 0xFEE0_0000;
const KERNEL_MAP_FRAMES: usize = 64;
const IMAGE_FRAMES: usize = 32;
const STACK_SIZE: usize = 8_192;
const TASKS: usize = 2;
const IMAGE_BASE: u64 = 0x1_4000_0000;

/// Sample and warmup counts, matching `kernel::fixture_measure`'s so the two
/// fixtures' numbers are directly comparable.
const SAMPLES: usize = 1_000;
const WARMUP: usize = 100;
/// Calibration samples for the cycle-read overhead subtracted from every
/// figure below.
const CALIBRATION_SAMPLES: usize = 1_000;

const RW: Permissions = Permissions { read: true, write: true, execute: false };

#[repr(C, align(4096))]
struct AlignedPage([u8; 4096]);

static IMAGE_BYTES: AlignedPage = AlignedPage([0; 4096]);
static mut STAGING: AlignedPage = AlignedPage([0; 4096]);
static mut IMAGE_PML4: PageTable = PageTable::new();
static mut IMAGE_FRAME_POOL: Pool<PageTable, IMAGE_FRAMES> = Pool::new();

static mut KERNEL_MAP_POOL: Pool<PageTable, KERNEL_MAP_FRAMES> = Pool::new();
static mut SUPERVISOR_PML4: PageTable = PageTable::new();
static mut SUPERVISOR_FRAMES: [PageTable; 4] = [const { PageTable::new() }; 4];
static mut SUPERVISOR_FRAMES_USED: usize = 0;

static mut DISPATCHER_CTX: Context = Context::zeroed();
static mut TASK_CTX: [Context; TASKS] = [Context::zeroed(); TASKS];
static mut TASK_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
static mut SAMPLE_BUFFER: Samples<SAMPLES> = Samples::new();

struct SupervisorFrames;
impl FrameAllocator for SupervisorFrames {
    fn allocate_frame(&mut self) -> Option<u64> {
        // SAFETY: single-CPU fixture; only this allocator touches these.
        unsafe {
            let used = SUPERVISOR_FRAMES_USED;
            if used >= SUPERVISOR_FRAMES.len() {
                return None;
            }
            SUPERVISOR_FRAMES_USED = used + 1;
            Some(&raw mut SUPERVISOR_FRAMES[used] as u64)
        }
    }
}

/// The measured task: yields straight back to the dispatcher, so a timed
/// round contains a dispatch round and nothing else.
extern "C" fn yield_forever() -> ! {
    loop {
        // SAFETY: single-CPU fixture; slot 0 is the only context this task
        // is ever switched into, and `DISPATCHER_CTX` is the dispatcher's
        // own suspended slot.
        unsafe {
            context::switch(&raw mut TASK_CTX[0], &raw mut DISPATCHER_CTX);
        }
    }
}

/// This fixture expects no faults at all; any fault means the measurement is
/// not measuring what it claims, so it is terminal and loud.
///
/// # Safety
/// Called only by the fault stubs, with `frame` pointing at a
/// fully-initialized [`FaultFrame`] on the faulting stack.
#[no_mangle]
extern "C" fn tinyos_fault_entry(frame: *const FaultFrame) -> ! {
    use core::fmt::Write;
    // SAFETY: the stubs pass a valid `FaultFrame` pointer, live for this call.
    let frame = unsafe { *frame };
    // SAFETY: never returns; no concurrent COM1 user on this single-CPU path.
    let mut serial = unsafe { SerialPort::init() };
    let _ = writeln!(
        serial,
        "dispatch-measure unexpected fault vector={} rip={:#x} cr2={:#x}",
        frame.vector,
        frame.rip,
        frame.faulting_address().unwrap_or(0)
    );
    let _ = write_result(&mut serial, "dispatch-measure", false);
    exit_qemu(QemuExitCode::Failure)
}

/// Required `#DF` entry — never expected to be reached.
///
/// # Safety
/// Called only by `df_fault_stub` with a valid IST-stack `FaultFrame`.
#[no_mangle]
extern "C" fn tinyos_double_fault_entry(_frame: *const FaultFrame) -> ! {
    use core::fmt::Write;
    // SAFETY: never returns; see `tinyos_fault_entry`.
    let mut serial = unsafe { SerialPort::init() };
    let _ = writeln!(serial, "dispatch-measure unexpected #DF");
    let _ = write_result(&mut serial, "dispatch-measure", false);
    exit_qemu(QemuExitCode::Failure)
}

/// Times `SAMPLES` dispatch rounds against a task whose address space is
/// `task_cr3`, reloading `reset_cr3` outside the timed region before each
/// iteration when `reset` is set (the cross-space case — see this module's
/// own doc comment for why).
#[inline(never)]
fn phase_dispatch(
    source: &Tsc,
    calibration: &Calibration,
    samples: &mut Samples<SAMPLES>,
    task_cr3: u64,
    reset_cr3: Option<u64>,
) -> bool {
    let mut scheduler: Scheduler<TASKS> = Scheduler::new();
    let Ok(priority) = Priority::try_new(11) else { return false };
    let Ok(task) = scheduler.create_task(
        priority,
        WcetBudgetTicks(1_000),
        OverrunPolicy::TripToSafeState,
        yield_forever,
    ) else {
        return false;
    };
    if task.index() != 0 || scheduler.set_address_space(task, task_cr3).is_none() {
        return false;
    }

    // SAFETY: slot 0 is the only context this phase initializes or switches
    // into; `TASK_STACK` is a never-moving static owned solely by it.
    unsafe {
        let stack = core::slice::from_raw_parts_mut((&raw mut TASK_STACK).cast::<u8>(), STACK_SIZE);
        let Ok(context) = Context::new(stack, yield_forever) else { return false };
        TASK_CTX[0] = context;
    }

    let mut ok = true;
    for index in 0..(WARMUP + SAMPLES) {
        if let Some(reset) = reset_cr3 {
            // Outside the stopwatch: this is the fixture's setup, not part
            // of the round under test.
            // SAFETY: `reset` is the supervisor tree, which maps this code,
            // this stack, and the IDT — `write_cr3`'s contract.
            unsafe { paging::write_cr3(reset) };
        }
        let watch = Stopwatch::start(source);
        // SAFETY: `TASK_CTX[0]` was initialized above and is suspended at
        // its entry point or its own `switch` call site; `DISPATCHER_CTX` is
        // this context's own slot; the task's `CR3` is a fully-populated
        // tree sharing the kernel directories — `run_once_in_space`'s
        // documented contract.
        let ran = unsafe {
            dispatch::run_once_in_space(&mut scheduler, &raw mut DISPATCHER_CTX, &raw mut TASK_CTX)
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

fn run() -> bool {
    // SAFETY: single-CPU fixture with no concurrent COM1 user.
    let mut serial = unsafe { SerialPort::init() };

    // SAFETY: called once, before any fault could occur.
    unsafe { hal_x86_64::interrupts::init_faults_only() };
    // SAFETY: still on the bring-up map (no NX bits); every mapping written
    // through hereafter is genuinely writable in the W^X trees below.
    unsafe { paging::enable_nx_and_wp() };

    let layout = KernelLayout {
        exec_start: (&raw const __kernel_exec_start) as u64,
        exec_end: (&raw const __kernel_exec_end) as u64,
        rodata_start: (&raw const __kernel_rodata_start) as u64,
        rodata_end: (&raw const __kernel_rodata_end) as u64,
        image_end: (&raw const __kernel_image_end) as u64,
    };
    // SAFETY: the pool static is borrowed once and never moves.
    let dirs = unsafe {
        match kernel_map::build_shared_directories(
            &mut *(&raw mut KERNEL_MAP_POOL),
            layout,
            APIC_PAGE,
        ) {
            Ok(dirs) => dirs,
            Err(_) => return false,
        }
    };
    // SAFETY: the supervisor tree maps this binary's whole linked extent
    // before it is loaded — `write_cr3`'s contract.
    let supervisor_cr3 = unsafe {
        let supervisor = &mut *(&raw mut SUPERVISOR_PML4);
        if paging::install_shared_pd(supervisor, &mut SupervisorFrames, 0, dirs.low_pd).is_err()
            || paging::install_shared_pd(
                supervisor,
                &mut SupervisorFrames,
                dirs.apic_base,
                dirs.apic_pd,
            )
            .is_err()
        {
            return false;
        }
        let cr3 = (&raw const SUPERVISOR_PML4) as u64;
        paging::write_cr3(cr3);
        cr3
    };

    // A second, genuinely distinct tree: one private page of its own plus
    // the same shared kernel directories, so a task running under it has
    // everything the dispatcher and the fault path need.
    // SAFETY: each static is borrowed once; the space lives past this block
    // via `forget`, exactly as `STORY-P1-03-01`'s fixture documents.
    let image_cr3 = unsafe {
        let sections = [SectionDescriptor {
            virtual_address: 0,
            virtual_size: PAGE_SIZE as u32,
            file_offset: 0,
            file_size: PAGE_SIZE as u32,
            permissions: RW,
        }];
        let mut space = match AddressSpace::create(
            &mut *(&raw mut IMAGE_PML4),
            &mut *(&raw mut IMAGE_FRAME_POOL),
            &sections,
            IMAGE_BASE,
            &IMAGE_BYTES.0,
            &mut *(&raw mut STAGING.0),
        ) {
            Ok(space) => space,
            Err(_) => return false,
        };
        if space.attach_shared_pd(0, dirs.low_pd).is_err()
            || space.attach_shared_pd(dirs.apic_base, dirs.apic_pd).is_err()
        {
            return false;
        }
        core::mem::forget(space);
        (&raw const IMAGE_PML4) as u64
    };
    if supervisor_cr3 == image_cr3 {
        return false;
    }

    let source = Tsc;
    let calibration = Calibration::measure(&source, CALIBRATION_SAMPLES);
    // SAFETY: nothing else in this fixture uses PIT channel 2 or port 0x61,
    // and no timer is armed on this boot path (`init_faults_only` never
    // arms one) — `calibrate_cycles_per_us`'s documented contract, met
    // before any measurement starts so an interrupt cannot inflate it.
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

    // SAFETY: single-CPU fixture; the shared sample buffer is used by one
    // phase at a time and cleared between them.
    let samples = unsafe { &mut *(&raw mut SAMPLE_BUFFER) };

    samples.clear();
    ok &= phase_dispatch(&source, &calibration, samples, supervisor_cr3, None);
    match samples.summarize() {
        Some(summary) => {
            ok &= report
                .metric(&Metric {
                    domain: "D04",
                    name: "dispatch_round_same_space",
                    warmup: WARMUP,
                    summary,
                })
                .is_ok();
        }
        None => ok = false,
    }

    samples.clear();
    ok &= phase_dispatch(&source, &calibration, samples, image_cr3, Some(supervisor_cr3));
    match samples.summarize() {
        Some(summary) => {
            ok &= report
                .metric(&Metric {
                    domain: "D04",
                    name: "dispatch_round_cross_space",
                    warmup: WARMUP,
                    summary,
                })
                .is_ok();
        }
        None => ok = false,
    }

    ok &= matches!(report.end(), Ok(2));

    // Leave the supervisor tree live, so nothing after this runs under a
    // task's address space by accident.
    // SAFETY: the supervisor tree maps this code and stack.
    unsafe { paging::write_cr3(supervisor_cr3) };
    let _ = write_result(&mut serial, "dispatch-measure", ok);
    ok
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
