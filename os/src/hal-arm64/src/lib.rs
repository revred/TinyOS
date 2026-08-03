//! AArch64 HAL backend.
//!
//! Deliberately narrow. `STORY-P1-01-03` is piece 3 of the five-piece minimal
//! Raspberry Pi 5 slice scoped in
//! `session/hand-2026-07-27/03-le-09-arm64-pi5-slice-proposal.md`, and the
//! only piece that needs no board: the generic-timer cycle source and its
//! timebase. Boot and target spec (piece 1), the PL011 UART (piece 2) and the
//! host-side SD-card/serial run path (piece 5) wait for `FEAT-P1-02` under the
//! user's recorded **Option B with the carve-out** decision — on a board with
//! no `isa-debug-exit` and no fault handling, a fault is a silent hang with no
//! output at all.
//!
//! So this crate is not a HAL port and must not grow into one here: it exists
//! to put a *second* implementor behind `hal::time::CycleSource`, because a
//! trait seam nobody has ever crossed is a guess rather than a design.
//!
//! `#![no_std]` is suppressed under `cfg(test)` so `cargo test` links the
//! host's `std` test harness, matching `hal`'s and `kernel`'s own split — the
//! crate's own code (outside `#[cfg(test)]` modules) never uses anything
//! beyond `core`.

#![cfg_attr(not(test), no_std)]
#![deny(missing_docs)]

pub mod board;
pub mod boot;
pub mod esr;
pub mod exception_level;
pub mod fault;
pub mod hdmi;
pub mod pl011;
pub mod timer;
pub mod vectors;
