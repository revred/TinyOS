//! Arch-neutral hardware abstraction layer.
//!
//! `topology` (`STORY-P0-04-01`) is the first real surface: the shared
//! output type an ACPI backend (x86_64, `hal-x86_64`) and a future
//! device-tree backend (ARM64, `EPIC-P7`) both produce. `device`
//! (`STORY-P0-04-03`) is its bus-enumeration companion: the shared
//! discovered-device table a PCI configuration-space walk records into.
//! HAL trait definitions beyond those land in later Phases.
//!
//! `#![no_std]` is suppressed under `cfg(test)` so `cargo test` links the
//! host's `std` test harness, matching `kernel`'s `lib.rs` split — the
//! crate's own code (outside `#[cfg(test)]` modules) never uses anything
//! beyond `core`.

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

pub mod device;
pub mod time;
pub mod topology;
