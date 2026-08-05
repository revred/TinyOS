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
pub mod board_dispatch;
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
// `spoor` and `spoor_journal` are **not** x86_64-coupled and never were: the
// first names no type outside itself, and the second names only the first.
// They were swept into the block above with the modules that genuinely do name
// `hal_x86_64` types, and the sweep is why the AArch64 boot image could not
// have journalled a spoor even if a rung had tried to stamp one — `LE-56`'s
// board half was unreachable by construction, not merely unimplemented
// (`FEAT-P1-10`, 2026-08-04).
pub mod spoor;
pub mod spoor_journal;

pub mod spoor_stream;
pub mod spoor_wire;
pub mod udp_wire;
#[cfg(target_arch = "x86_64")]
pub mod wcet;
