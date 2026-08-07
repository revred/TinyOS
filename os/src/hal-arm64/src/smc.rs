//! The one SMC this image ever issues: `PSCI_VERSION`, the Q3 positive
//! control's injected perturbation (`12A` §0, `ADR 0005`'s trap clause).
//!
//! `REPORT-2026-08-07-01` Q2 determined that stock Pi 5 EL3 hosts a resident
//! TF-A BL31 with PSCI live over SMC (`psci { method = "smc" }` in the
//! platform's own device tree), and that an SMC is the **one documented
//! synchronous entry into EL3 on this platform** — which is exactly what
//! makes it the injectable perturbation the residency campaign must first be
//! shown to detect: a real EL3 round-trip, on demand, with no side effect.
//!
//! `PSCI_VERSION` (function id `0x8400_0000`) is chosen because the PSCI
//! specification defines it as a pure query — no state change, no arguments,
//! always implemented — so the excursion it produces is the *entry cost*,
//! not the cost of work this kernel asked EL3 to do.
//!
//! This module is deliberately one function and no seam: an SMC cannot be
//! meaningfully doubled on a host (the *probe logic* around it is what gets
//! host-tested, over `timer::probe_residency_window_with_event`'s scripted
//! counters), and a mock EL3 would test the mock. Board-only, reviewed
//! rather than host-verified, exercised by `fixture-qual-control`.

/// The SMCCC function id of `PSCI_VERSION` (SMC32 fast call, PSCI service).
pub const PSCI_VERSION: u32 = 0x8400_0000;

/// Issues `PSCI_VERSION` over SMC and returns `x0` — the version word
/// (major in bits \[31:16\], minor in \[15:0\]).
///
/// Only reachable on the board; the fixture prints the returned word onto
/// the wire so the capture carries its own Q2 corroboration.
#[cfg(target_arch = "aarch64")]
#[must_use]
pub fn psci_version() -> u64 {
    let mut x0: u64 = u64::from(PSCI_VERSION);
    // SAFETY: `smc #0` with a PSCI query id is the platform's own documented
    // conduit (Q2: TF-A BL31 resident, PSCI over SMC); `PSCI_VERSION` is
    // defined side-effect-free and always implemented. Per SMCCC, a fast
    // call may clobber x0–x17 and preserves x18+; every register in the
    // clobberable range is declared to the compiler, so no live value can
    // sit in one across the call. Single core, interrupts masked by the
    // fixture's own region — the SMC returns to the next instruction.
    unsafe {
        core::arch::asm!(
            "smc #0",
            inout("x0") x0,
            out("x1") _, out("x2") _, out("x3") _, out("x4") _, out("x5") _,
            out("x6") _, out("x7") _, out("x8") _, out("x9") _, out("x10") _,
            out("x11") _, out("x12") _, out("x13") _, out("x14") _, out("x15") _,
            out("x16") _, out("x17") _,
            options(nostack),
        );
    }
    x0
}
