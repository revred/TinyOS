//! `TEST-P0-04-02-A`'s fail-closed-default QEMU fixture: brings up
//! [`hal_x86_64::interrupts`]'s IDT, then deliberately triggers a vector
//! this kernel never explicitly routes — proving `STORY-P0-04-02`
//! acceptance criterion 2 ("spurious/unrouted interrupts are handled
//! explicitly ... never silently ignored") is actually reached under real
//! target hardware, not just structurally true of the built `Idt` (which
//! `idt.rs`'s own host tests already cover).
//!
//! **This fixture's correct result is a QEMU isa-debug-exit *Failure*
//! code** — reaching `hal_x86_64::interrupts`' fail-closed default handler
//! *is* the pass condition, mirroring `fixture-broken-boot`'s own
//! established precedent (`main.rs`'s doc comment on that feature) rather
//! than being a new pattern. If this fixture's own `kernel_main` ever
//! resumes and reaches its own trailing `exit_qemu(Success)`, the default
//! handler failed to divert control there — the actual bug this fixture
//! exists to catch.
//!
//! Only reachable when the `fixture-idt-apic-unrouted` feature is enabled —
//! never part of a real boot image.

use hal_x86_64::interrupts;

/// A vector distinct from [`interrupts::TIMER_VECTOR`] and
/// [`interrupts::SPURIOUS_VECTOR`], and not one of the CPU's own reserved
/// exception vectors (`0`..`31`) — chosen so this fixture's `int`
/// instruction unambiguously exercises the catch-all default path, not one
/// of the two vectors this kernel does explicitly service.
const UNROUTED_VECTOR: u8 = 0x21;

/// A deliberately huge local-APIC timer reload value: this fixture never
/// waits for a tick, but `init` unconditionally arms the timer as part of
/// bringing up the rest of interrupt handling — an initial count anywhere
/// near this fixture's own runtime would race a real timer interrupt
/// against the [`UNROUTED_VECTOR`] delivery below (an earlier version of
/// this fixture used `1`, which fires near-continuously and starved the
/// CPU of forward progress before it ever reached the deliberate `int`).
/// `u32::MAX` guarantees the timer cannot fire before this fixture's own
/// `int` diverges.
const NEVER_FIRES_IN_TIME: u32 = u32::MAX;

/// Arms the IDT (with a timer count chosen to never fire before this
/// fixture's own `int`, see [`NEVER_FIRES_IN_TIME`]), then executes a
/// software interrupt on [`UNROUTED_VECTOR`]. Never expected to return: see
/// this module's own doc comment for why reaching `hal_x86_64::interrupts`'s
/// fail-closed default handler — which itself calls `exit_qemu(Failure)`
/// and never returns — is this fixture's actual pass condition.
pub fn run() -> ! {
    // SAFETY: this fixture is the only code running (single-CPU boot path);
    // `init` is called exactly once, per its own documented contract.
    unsafe {
        interrupts::init(NEVER_FIRES_IN_TIME);
    }

    // SAFETY: `int {UNROUTED_VECTOR}` is a software interrupt targeting a
    // fixed, in-range IDT vector this kernel's own `init` has already wired
    // (to the fail-closed default handler, per this module's own doc
    // comment) — the same CPU dispatch mechanism a real hardware-generated
    // interrupt uses, deterministically reproducible rather than depending
    // on real hardware timing/flakiness to provoke an unrouted delivery.
    unsafe {
        core::arch::asm!("int {vector}", vector = const UNROUTED_VECTOR, options(nomem, nostack));
    }

    // Unreachable if the default handler correctly diverged — see this
    // module's own doc comment.
    unreachable!("the fail-closed default handler should have already diverged")
}
