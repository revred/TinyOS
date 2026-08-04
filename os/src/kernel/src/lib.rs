//! TinyOS kernel library surface: the parts of the kernel testable on the
//! host toolchain (no QEMU/boot dependency), split out of the `no_std`/
//! `no_main` binary entry point in `main.rs` so `cargo test -p kernel --lib`
//! can run them without a target-specific boot environment.
//!
//! `#![no_std]` is suppressed under `cfg(test)` so `cargo test` links the
//! host's `std` test harness — the crate's own code (outside `#[cfg(test)]`
//! modules) never uses anything beyond `core`.

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

// Arch-neutral core — what the AArch64 boot image links (`STORY-P1-07-06`):
// the measurement harness, the pools, the scheduler, the dispatcher and the
// context switch (whose assembly is arch-split inside the module).
pub mod context;
pub mod dispatch;
pub mod measure;
#[cfg(feature = "fixture-measure")]
pub mod measure_phases;
pub mod mem;
pub mod sched;

/// The AArch64 measurement fixture (`STORY-P1-07-06`) — the board-side twin
/// of the x86_64 binary's `fixture_measure`, linked into `pi5-image` when
/// its `fixture-measure` feature is on and never part of a real boot image.
#[cfg(all(target_arch = "aarch64", feature = "fixture-measure"))]
pub mod fixture_measure_arm64;

// x86_64-coupled modules: each names `hal_x86_64` types in its public
// surface (IST capacities, fault vectors, paging). Their AArch64
// counterparts are owned by `FEAT-P1-07`'s follow-on Features, not gated in
// by cfg — absence is honest here.
#[cfg(target_arch = "x86_64")]
pub mod actuation;
#[cfg(target_arch = "x86_64")]
pub mod capacities;
#[cfg(target_arch = "x86_64")]
pub mod fault;
#[cfg(target_arch = "x86_64")]
pub mod ipc;
#[cfg(target_arch = "x86_64")]
pub mod lock;
#[cfg(target_arch = "x86_64")]
pub mod preempt;
#[cfg(target_arch = "x86_64")]
pub mod spoor;
#[cfg(target_arch = "x86_64")]
pub mod spoor_journal;
#[cfg(target_arch = "x86_64")]
pub mod wcet;
