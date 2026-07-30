//! 08C Stage E / 13F Deliverable A — the host-side operator console, core crate.
//!
//! Three seams, each headless-testable without wry:
//!
//! - [`manifest`] — the signed manifest: an ed25519-signed enumeration of exactly the
//!   console's verbs. A [`manifest::VerifiedManifest`] can only be obtained by verifying a
//!   signature; there is no unchecked constructor, so the resolver is fail-closed by type.
//! - [`authority`] — [`authority::ConsoleAuthority`], an [`tauri::ipc::AuthorityResolver`]
//!   backed by the verified manifest: deny-by-default, remote origins refused
//!   unconditionally, identity checked against the console webview's runtime-derived label,
//!   every denial recorded so the UI can show it.
//! - [`harness`] — the QEMU fixture harness: spawns the *same command surface* CI uses
//!   (`cargo run -p xtask -- qemu-x86_64 --fixture=<name> --serial-capture=<path>`), tails
//!   the serial capture live, and maps the exit code to the same PASS/FAIL verdict `xtask`
//!   computes from `isa-debug-exit`.
//!
//! Non-claims (13F §1): this proves the `EPIC-H4` lane's *shape* — Tauri UI ∩ signed
//! manifest ∩ real kernel under QEMU. It proves nothing about on-target isolation,
//! accounting or time (`PD-01/07/08/12`), and it does not advance `EPIC-H3`.

pub mod authority;
pub mod commands;
pub mod harness;
pub mod manifest;
pub mod parity_suite;
pub mod tabs;
