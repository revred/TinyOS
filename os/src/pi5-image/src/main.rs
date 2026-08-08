//! The bootable Raspberry Pi 5 image (`STORY-P1-07-05`), and the composition
//! root that decides what the board's wire shell may reach
//! (`STORY-P1-09-18`).
//!
//! Packaging first, and deliberately so: the whole boot flow — entry report,
//! conditional `EL2 → EL1` drop, the READY sequence, the vector install and
//! the `TOS64-RESULT/1` verdict — lives in `hal-arm64`, host-tested behind
//! its MMIO seam. Board logic accumulating here would be board logic escaping
//! its tests, and that rule has not been relaxed.
//!
//! **What did change on 2026-08-08:** this crate is now also the one place
//! that sees the whole dependency graph, because `shell` depends on `kernel`
//! and on AArch64 the dependency runs `kernel` → `hal-arm64`. A HAL that named
//! `shell` would be a cycle, so wiring `TINYCMD`'s verb core to the command
//! channel can only happen at the root. It happens in [`wire_shell`], which is
//! compiled and unit-tested on **every** architecture — the composition is
//! host-tested even though the image is not, which is the whole point of
//! keeping the `extern "C"` shim below down to a clamp and a call.
//!
//! On any architecture other than AArch64 this compiles to an inert stub so
//! workspace-wide host builds, clippy and rustdoc keep working — the real
//! image is built only via `cargo run -p xtask -- pi5`.

#![cfg_attr(target_arch = "aarch64", no_std)]
#![cfg_attr(target_arch = "aarch64", no_main)]
// `deny` rather than `forbid` since 2026-08-08, for exactly one item and for a
// reason worth stating rather than hiding behind a lint name.
//
// `#[no_mangle]` is classed as unsafe, and rustc says precisely why: *"the
// linker's behavior with multiple libraries exporting duplicate symbol names
// is undefined"*. That is a **symbol-naming** hazard, not a memory one. There
// is no `unsafe` block anywhere in this crate, no raw pointer, no
// transmute and no FFI type that is not a reference to a sized array — the
// single allowance below is on an attribute, and the function it decorates is
// ordinary safe Rust.
//
// The hazard it does carry is real and is mitigated by construction: the
// symbol is `tinyos_wire_shell_run`, project-prefixed and defined exactly once
// in the workspace, in the final binary, referenced by exactly one caller.
// `forbid` was preferred while this crate defined no symbols; now that it
// defines one, `deny` plus a named allowance says more to a reviewer than a
// `forbid` that had to be worked around somewhere else would have.
#![deny(unsafe_code)]
#![deny(missing_docs)]

#[cfg(target_arch = "aarch64")]
#[allow(unused_imports)]
// Linked for its `global_asm!` `_start`/`entry` side effect only, exactly as
// exec's Tier 0 fixtures link `hal_x86_64::boot`.
use hal_arm64::boot as _;

#[cfg(all(target_arch = "aarch64", feature = "fixture-measure"))]
#[allow(unused_imports)]
// Linked for `tinyos_arm64_fixture_measure`'s `#[no_mangle]` side effect —
// the symbol `hal_arm64::boot` calls under the same feature.
use kernel::fixture_measure_arm64 as _;

/// Forces `kernel`'s spoor seam into the link (`STORY-P1-10-02`,
/// `STORY-P1-10-04`).
///
/// `hal-arm64`'s boot rungs call `tinyos_spoor_stamp`/`tinyos_spoor_drain` by
/// symbol, because on AArch64 the dependency runs `kernel` → `hal-arm64` and
/// the call cannot be a direct one. But nothing else in this image references
/// `kernel` unless `fixture-measure` is on, and an rlib nothing references is
/// dropped from the link entirely — taking the `#[no_mangle]` definitions with
/// it and leaving those two symbols undefined.
///
/// That is exactly how this image failed to link on the runner while building
/// fine locally: every local build passed `--fixture=measure`, which pulls
/// `kernel` in for its own reasons and masked the missing reference. The
/// spoor stream is not a fixture, so the reference is unconditional.
///
/// `#[used]` rather than a plain `use`, because a mere import of a function
/// item is not a reference the linker must honour.
///
/// **Every symbol the seam exports is named here, not a representative one.**
/// The whole `kernel` rlib comes in as soon as one is referenced, so listing
/// only `tinyos_spoor_stamp` would work — right up until someone moves the
/// stream into a crate of its own and discovers which symbols were load-
/// bearing by watching the link fail on the runner. That is exactly the
/// discovery this static exists to have already made.
/// The seam's shape, named rather than a tuple.
///
/// It became a six-tuple when dispatch joined the spoor entry points and
/// clippy refused it as a very complex type — correctly. A struct also makes
/// the static self-describing: a reader sees which symbols the image is
/// pinning and why, instead of counting positions in a tuple.
///
/// **The fields are never read, and that is the whole point.** This static
/// exists so the linker keeps `kernel` and its `#[no_mangle]` definitions in
/// the image; reading a field would be the one thing that makes it look
/// justified to a lint and changes nothing about why it is here.
#[cfg(target_arch = "aarch64")]
#[allow(dead_code)]
struct KernelSeam {
    stamp: extern "C" fn(u16, u8, u32),
    drain: unsafe extern "C" fn(*mut u8, usize) -> usize,
    seed_epoch: extern "C" fn(u64),
    announce: unsafe extern "C" fn(*mut u8, usize) -> usize,
    dispatch_init: extern "C" fn() -> u8,
    dispatch_round: extern "C" fn() -> u16,
}

#[cfg(target_arch = "aarch64")]
#[used]
static SPOOR_SEAM: KernelSeam = KernelSeam {
    stamp: kernel::spoor_stream::tinyos_spoor_stamp,
    drain: kernel::spoor_stream::tinyos_spoor_drain,
    seed_epoch: kernel::spoor_stream::tinyos_spoor_seed_epoch,
    announce: kernel::spoor_stream::tinyos_spoor_announce,
    dispatch_init: kernel::board_dispatch::tinyos_dispatch_init,
    dispatch_round: kernel::board_dispatch::tinyos_dispatch_round,
};
pub mod wire_shell;

/// `TOS64-CMD/1`'s `SHELL` row, resolved (`STORY-P1-09-18`).
///
/// The seam `hal_arm64::wire_shell` declares, implemented by the composition
/// root because it is the only crate that can see both ends: `hal-arm64` holds
/// the command channel and cannot name `shell` without a dependency cycle, and
/// `shell` holds the verb core and knows nothing about a board.
///
/// **Three lines and no decisions.** Every choice — the grant set, the seeded
/// volume, the session name, the bound on output — is in
/// [`wire_shell`], compiled and unit-tested on the host on every
/// architecture. A shim that decided anything would be a decision no host test
/// could reach, which is exactly the class of hole `LE-66` was raised over.
///
/// No `unsafe` **operation**: `&[u8; N]` and `&mut [u8; N]` are FFI-safe thin
/// pointers, so the body is ordinary safe Rust and the compiler-enforced "a
/// wire verb cannot reach a register" claim survives the crate boundary. The
/// one allowance is the `#[no_mangle]` attribute itself — see the crate
/// header for what that hazard actually is and why it is contained.
/// `line_len` is clamped rather than trusted: a seam whose safety depends on
/// its caller is a seam nobody re-reads.
#[cfg(target_arch = "aarch64")]
#[allow(unsafe_code)]
#[no_mangle]
extern "C" fn tinyos_wire_shell_run(
    line: &[u8; hal_arm64::tos64_cmd::ARGUMENT_BYTES],
    line_len: usize,
    out: &mut [u8; hal_arm64::tos64_cmd::SHELL_OUTPUT_CAPACITY],
) -> usize {
    wire_shell::run(&line[..line_len.min(line.len())], out)
}

/// A panic in this image has no reporting channel of its own — the UART may
/// be the very thing that failed — so the fail-safe terminal state is the
/// same park the boot path ends in.
#[cfg(target_arch = "aarch64")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    hal_arm64::boot::park()
}

/// Host-side stand-in so `cargo test`/`cargo clippy --workspace` on the dev
/// machine can build this crate.
#[cfg(not(target_arch = "aarch64"))]
fn main() {
    eprintln!("pi5-image is an AArch64 boot image; build it with `cargo run -p xtask -- pi5`");
}
