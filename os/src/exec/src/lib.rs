//! Executable loading and process compatibility (`FEAT-P0-05`, `SeedMVP.md`
//! §3.7's `G-PC-1`..`G-PC-4`).
//!
//! `pe` (`STORY-P0-05-01`) parses a PE64 image into a validated
//! [`pe::LoadDescriptor`]. `address_space` (`STORY-P0-05-02`) maps a
//! `LoadDescriptor`'s sections into a real x86_64 page-table tree.
//! `win32_shim` (`STORY-P0-05-03`) is the capability-scoped Win32 API
//! compatibility shim mediating every call a loaded image can make.
//!
//! `#![no_std]` is suppressed under `cfg(test)` so `cargo test` links the
//! host's `std` test harness, matching `hal-x86_64`'s `lib.rs` split.

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

pub mod address_space;
pub mod iat;
pub mod kernel_map;
pub mod pe;
pub mod shared_memory;
pub mod win32_shim;
