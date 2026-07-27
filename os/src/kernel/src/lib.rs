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

pub mod capacities;
pub mod context;
pub mod dispatch;
pub mod ipc;
pub mod lock;
pub mod measure;
pub mod mem;
pub mod sched;
pub mod spoor;
pub mod spoor_journal;
pub mod wcet;
