//! Motion-group synchronisation contracts (`STORY-P1-08-01`, `FEAT-P1-08`).
//!
//! [`ADR 0010`](../../../../docs/adr/0010-the-motion-group-is-the-unit-of-control.md)
//! governs this crate: **the motion group is the unit of control, and EtherCAT
//! is a transport implementation**, never the architecture. The public
//! boundary here takes and produces whole-group frames — one coherent
//! feedback epoch in ([`feedback::FeedbackFrame`]), one atomic time-tagged
//! command commit out ([`command::ActuationFrame`]) — accepted completely or
//! rejected completely, with typed reasons. There is deliberately no per-axis
//! feedback callback and no per-axis "write now" escape path anywhere in this
//! crate, and adding one is an ADR-level change, not a convenience patch.
//!
//! The epoch rule (delivery contract §3): `sample N → validate N → calculate
//! from N → stage N+1 → apply N+1`. Every record names its epoch; a command
//! is never silently relabelled for a later epoch; epoch wrap is an explicit
//! protocol event that must never make an old frame appear current.
//!
//! Scope is `MFS-01` plus the minimal `MFS-03` conformance double from
//! `work/case-motion-controller/foundational-motion-synchronisation-delivery.md`:
//! no EtherCAT, no interpolation, no scheduler binding, no physical I/O, no
//! timing claim of any kind (`LE-62`). Fixed capacity everywhere — 16 axes,
//! 32 feedback channels, compile-time bounds, no allocator.
//!
//! `#![no_std]` is suppressed under `cfg(test)` so `cargo test` links the
//! host's `std` test harness, matching `hal`'s and `kernel`'s split — the
//! crate's own code (outside `#[cfg(test)]` modules) never uses anything
//! beyond `core`.

#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod command;
pub mod double;
pub mod feedback;
pub mod ident;
pub mod profile;
pub mod transport;
pub mod units;
pub mod validate;
