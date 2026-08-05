//! The board's side of the spoor seam (`STORY-P1-10-02`).
//!
//! This crate cannot import `kernel` — on AArch64 the dependency runs
//! `kernel` → `hal-arm64` — so the boot rungs reach the journal through two
//! `#[no_mangle] extern "C"` symbols, exactly as this crate's boot already
//! reaches `tinyos_arm64_fixture_measure`.
//!
//! # The duplication, and why it is safe
//!
//! [`Rung`] and [`Verdict`] below are a second copy of vocabularies that live
//! authoritatively in `kernel::spoor_stream`. Duplicating a vocabulary across
//! a crate boundary is exactly how one silently drifts from the other, so the
//! copy is not left to good intentions: this crate has a **dev-dependency** on
//! `kernel`, and the tests at the bottom assert value-for-value that the two
//! agree. The duplication is a link-order necessity; the parity test is what
//! makes it honest.
//!
//! # What the board decides
//!
//! Nothing. A call site passes a rung and a verdict; the category, actor and
//! action are chosen on the kernel side where a host test holds them. If a
//! rung has no honest entry here, the answer is to add one — test-first, in
//! both crates, with the parity test failing until they match — never to
//! borrow a neighbouring rung that reads close enough.

/// A rung of the boot or park path, mirroring `kernel::spoor_stream::Rung`.
///
/// Discriminants are wire-visible and therefore **append-only**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum Rung {
    /// The MMU came up with caches enabled (`STORY-P1-07-03`).
    MmuEnabled = 1,
    /// The GIC routed the virtual-timer PPI (`STORY-P1-07-04`).
    GicRouted = 2,
    /// The virtual timer was armed and its control register believed.
    TickArmed = 3,
    /// A beacon frame was handed to the GEM (`FEAT-P1-09`).
    BeaconTransmitted = 4,
    /// The measurement fixture ran to completion (`STORY-P1-07-06`).
    FixtureMeasure = 5,
    /// One pass of the park loop.
    ParkIteration = 6,
    /// A synchronous exception was taken and reported (`STORY-P1-07-02`).
    FaultTaken = 7,
    /// The SoC die temperature was sampled; the cost field carries the AVS
    /// monitor's raw register word, unconverted (`LE-75`).
    ThermalSample = 8,
}

/// What a rung reports, mirroring `kernel::spoor_stream::Verdict`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Verdict {
    /// The rung did what it exists to do.
    Ok = 0,
    /// The rung was refused. As stampable as a success, deliberately.
    Failed = 1,
    /// The rung was not attempted.
    Skipped = 2,
}

#[cfg(target_arch = "aarch64")]
extern "C" {
    /// `kernel::spoor_stream::tinyos_spoor_stamp`.
    fn tinyos_spoor_stamp(rung: u16, verdict: u8, cost: u32);
    /// `kernel::spoor_stream::tinyos_spoor_drain`.
    fn tinyos_spoor_drain(out: *mut u8, cap: usize) -> usize;
    /// `kernel::spoor_stream::tinyos_spoor_seed_epoch`.
    fn tinyos_spoor_seed_epoch(sample: u64);
    /// `kernel::spoor_stream::tinyos_spoor_announce`.
    fn tinyos_spoor_announce(out: *mut u8, cap: usize) -> usize;
}

/// Fixes this boot's epoch from the generic counter (`STORY-P1-10-04`).
///
/// Called once, before the first rung stamps, so every frame this boot emits
/// carries it and a host reading any one of them can tell which boot it is
/// watching.
///
/// **`CNTVCT_EL0` at kernel entry is the only per-boot value this board has.**
/// There is no persistent store and no RTC, and the counter resets with the
/// SoC — so what actually varies between boots is how many ticks firmware
/// spent before reaching here, which does vary, but is the *firmware's*
/// entropy and not ours. That makes the epoch a change detector rather than an
/// identifier, and `LE-74` records the limit rather than letting a field named
/// "epoch" imply a boot count it cannot support.
///
/// It is deliberately not mixed with anything that looks like a seed. Dressing
/// a low-entropy sample up as a nonce would make the field read stronger than
/// it is, which is the failure mode the whole project's ADR discipline exists
/// to prevent.
#[cfg(target_arch = "aarch64")]
pub fn seed_epoch() {
    let sample: u64;
    // SAFETY: side-effect-free read of the always-readable virtual counter —
    // the same register `crate::timer::SystemRegisters` reads, inlined here
    // because this call site runs before any timer object exists.
    unsafe {
        core::arch::asm!("mrs {v}, CNTVCT_EL0", v = out(reg) sample,
            options(nomem, nostack, preserves_flags));
    }
    // SAFETY: the symbol is provided by `kernel`, which `pi5-image` links; the
    // call takes one scalar and the callee is single-core and non-reentrant.
    unsafe { tinyos_spoor_seed_epoch(sample) }
}

/// Re-announces the boot certificate into `out`, returning the payload length
/// or `0` when the announcement is not due (`STORY-P1-10-04`).
///
/// The park loop calls this every pass and the kernel decides when a frame
/// comes out, so the period is one constant a host test can read rather than a
/// cadence spread across a loop this crate owns.
#[cfg(target_arch = "aarch64")]
#[must_use]
pub fn announce(out: &mut [u8]) -> usize {
    // SAFETY: `out` is a live slice, so the pointer and length are valid; the
    // callee is single-core and non-reentrant, which the park loop satisfies
    // because it is the only caller.
    unsafe { tinyos_spoor_announce(out.as_mut_ptr(), out.len()) }
}

/// Stamps one rung. Never fails, never blocks, and costs one call plus a
/// `u64` store — cheap enough that no rung has to justify itself.
///
/// `cost` is whatever the call site measured; zero is an honest "not
/// measured" rather than a claim that the rung was free.
#[cfg(target_arch = "aarch64")]
pub fn stamp(rung: Rung, verdict: Verdict, cost: u32) {
    // SAFETY: the symbol is provided by `kernel`, which `pi5-image` links; the
    // call takes only scalars and the callee is documented single-core and
    // non-reentrant, which this crate's boot path satisfies.
    unsafe { tinyos_spoor_stamp(rung as u16, verdict as u8, cost) }
}

/// Drains the stream into `out`, returning the payload length or `0` when
/// there was nothing to send.
///
/// Zero means "no frame", not "empty frame": transmitting an empty frame every
/// park pass would fill the wire with silence that looks like data.
#[cfg(target_arch = "aarch64")]
#[must_use]
pub fn drain(out: &mut [u8]) -> usize {
    // SAFETY: `out` is a live slice, so the pointer and length are valid; the
    // callee's contract is single-core and non-reentrant, which the park loop
    // satisfies because it is the only caller.
    unsafe { tinyos_spoor_drain(out.as_mut_ptr(), out.len()) }
}

/// Host builds have no `kernel` symbol to call and no board to stamp on.
#[cfg(not(target_arch = "aarch64"))]
pub fn stamp(_rung: Rung, _verdict: Verdict, _cost: u32) {}

/// Host builds drain nothing.
#[cfg(not(target_arch = "aarch64"))]
#[must_use]
pub fn drain(_out: &mut [u8]) -> usize {
    0
}

/// Host builds have no counter to seed from and no boot to identify.
#[cfg(not(target_arch = "aarch64"))]
pub fn seed_epoch() {}

/// Host builds announce nothing.
#[cfg(not(target_arch = "aarch64"))]
#[must_use]
pub fn announce(_out: &mut [u8]) -> usize {
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel::spoor_stream::{Rung as KernelRung, Verdict as KernelVerdict};

    /// The parity test the duplication above exists behind.
    ///
    /// If either crate gains, loses or renumbers a rung without the other,
    /// this fails — which is the whole reason a second copy of the vocabulary
    /// is tolerable at all.
    #[test]
    fn every_rung_agrees_with_the_kernel_vocabulary() {
        let pairs = [
            (Rung::MmuEnabled, KernelRung::MmuEnabled),
            (Rung::GicRouted, KernelRung::GicRouted),
            (Rung::TickArmed, KernelRung::TickArmed),
            (Rung::BeaconTransmitted, KernelRung::BeaconTransmitted),
            (Rung::FixtureMeasure, KernelRung::FixtureMeasure),
            (Rung::ParkIteration, KernelRung::ParkIteration),
            (Rung::FaultTaken, KernelRung::FaultTaken),
            (Rung::ThermalSample, KernelRung::ThermalSample),
        ];
        for (ours, theirs) in pairs {
            assert_eq!(
                ours as u16,
                theirs.to_bits(),
                "{ours:?} and {theirs:?} must carry the same wire value"
            );
        }
        assert_eq!(pairs.len(), 8, "a rung added on either side must be added here too");
    }

    #[test]
    fn every_verdict_agrees_with_the_kernel_vocabulary() {
        let pairs = [
            (Verdict::Ok, KernelVerdict::Ok),
            (Verdict::Failed, KernelVerdict::Failed),
            (Verdict::Skipped, KernelVerdict::Skipped),
        ];
        for (ours, theirs) in pairs {
            assert_eq!(ours as u8, theirs.to_bits(), "{ours:?} and {theirs:?} must agree");
        }
        assert_eq!(pairs.len(), 3, "a verdict added on either side must be added here too");
    }

    /// Every value this crate can send must be one the kernel side accepts.
    /// A rung we can stamp but they cannot decode would be dropped silently.
    #[test]
    fn nothing_this_crate_can_send_is_undecodable_on_the_other_side() {
        for rung in [
            Rung::MmuEnabled,
            Rung::GicRouted,
            Rung::TickArmed,
            Rung::BeaconTransmitted,
            Rung::FixtureMeasure,
            Rung::ParkIteration,
            Rung::FaultTaken,
            Rung::ThermalSample,
        ] {
            assert!(
                KernelRung::from_bits(rung as u16).is_some(),
                "{rung:?} would be dropped by the seam"
            );
        }
        for verdict in [Verdict::Ok, Verdict::Failed, Verdict::Skipped] {
            assert!(
                KernelVerdict::from_bits(verdict as u8).is_some(),
                "{verdict:?} would be dropped by the seam"
            );
        }
    }
}
