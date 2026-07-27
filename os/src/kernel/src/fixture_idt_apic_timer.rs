//! `TEST-P0-04-02-A`'s bounded-interval QEMU fixture: brings up
//! [`hal_x86_64::interrupts`]'s IDT/local-APIC timer, waits for real
//! hardware-delivered ticks, and measures the elapsed CPU-cycle interval
//! between them — proving `STORY-P0-04-02` acceptance criterion 1 ("a timer
//! interrupt configured through the local APIC fires at a bounded, measured
//! interval under QEMU — verified by a Tier 0 test, not assumed from
//! datasheet timing alone") against real target hardware/QEMU timing, not a
//! host-side simulation.
//!
//! Only reachable when the `fixture-idt-apic-timer` feature is enabled —
//! never part of a real boot image.

use hal_x86_64::interrupts;

/// Local-APIC timer reload value (see `interrupts::configure_timer`'s own
/// doc comment for what this counts down against): chosen to be small
/// enough that several ticks land comfortably inside `xtask`'s own 15-second
/// QEMU boot-timeout budget, without being so small that QEMU's own
/// interrupt-delivery/emulation overhead dominates and starves this
/// fixture's `hlt` loop of real CPU time to advance in.
const INITIAL_COUNT: u32 = 500_000;

/// How many real ticks to observe before judging the measured intervals.
const TICKS_TO_OBSERVE: usize = 5;

/// Upper bound on `hlt`-wake iterations this fixture spins through waiting
/// for `TICKS_TO_OBSERVE` real ticks — defense in depth alongside `xtask`'s
/// own external boot-timeout kill, per `agent/CODING_STANDARDS.md`'s
/// fail-safe-over-keep-trying discipline: this fixture does not rely solely
/// on an external harness to bound its own loop.
const MAX_WAIT_ITERATIONS: u64 = 200_000_000;

/// A measured interval is accepted if it is nonzero (ticks actually
/// advanced real time, not a fluke double-read) and no single interval
/// exceeds `MAX_INTERVAL_RATIO` times the smallest observed interval — a
/// self-consistency bound rather than a fixed microsecond figure, since
/// QEMU's own APIC-timer-to-wall-clock relationship under software
/// emulation is not itself a stable absolute number this fixture should
/// depend on; what must hold is that delivery is genuinely periodic and
/// bounded relative to itself, which this ratio directly tests.
const MAX_INTERVAL_RATIO: u64 = 20;

fn rdtsc() -> u64 {
    // SAFETY: `RDTSC` is unconditionally available on every x86_64 CPU;
    // reading the timestamp counter has no memory or control-flow side
    // effect this fixture needs to account for.
    unsafe { core::arch::x86_64::_rdtsc() }
}

/// Runs the fixture: arms the timer, waits for `TICKS_TO_OBSERVE` real
/// ticks, and reports whether the measured inter-tick intervals are
/// self-consistently bounded.
pub fn run() -> bool {
    // SAFETY: this fixture is the only code running (single-CPU boot path),
    // and `init` is called exactly once, before anything else in this
    // fixture depends on interrupts being armed — per `init`'s own
    // documented contract.
    unsafe {
        interrupts::init(INITIAL_COUNT);
    }

    let mut timestamps = [0u64; TICKS_TO_OBSERVE + 1];
    timestamps[0] = rdtsc();
    let mut observed = 0usize;
    let mut last_tick = interrupts::tick_count();

    for _ in 0..MAX_WAIT_ITERATIONS {
        if observed == TICKS_TO_OBSERVE {
            break;
        }
        // SAFETY: `hlt` with interrupts enabled (`init` already executed
        // `sti`) simply parks the CPU until the next interrupt — the
        // standard idle-wait primitive, not a privileged or unsafe-by-effect
        // instruction beyond needing inline asm to issue at all.
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack));
        }
        let now = interrupts::tick_count();
        if now != last_tick {
            last_tick = now;
            observed += 1;
            timestamps[observed] = rdtsc();
        }
    }

    if observed < TICKS_TO_OBSERVE {
        return false;
    }

    let mut min_delta = u64::MAX;
    let mut max_delta = 0u64;
    for i in 0..TICKS_TO_OBSERVE {
        let delta = timestamps[i + 1] - timestamps[i];
        if delta == 0 {
            return false;
        }
        min_delta = min_delta.min(delta);
        max_delta = max_delta.max(delta);
    }

    max_delta <= min_delta.saturating_mul(MAX_INTERVAL_RATIO)
}
