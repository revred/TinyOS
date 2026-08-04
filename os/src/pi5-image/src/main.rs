//! The bootable Raspberry Pi 5 image (`STORY-P1-07-05`).
//!
//! Packaging only, and deliberately so: the whole boot flow — entry report,
//! conditional `EL2 → EL1` drop, the READY sequence, the vector install and
//! the `TOS64-RESULT/1` verdict — lives in `hal-arm64`, host-tested behind
//! its MMIO seam. This crate contributes an AArch64 binary for `xtask pi5` to
//! flatten into `kernel8.img`, a panic handler, and nothing else; board logic
//! accumulating here would be board logic escaping its tests.
//!
//! On any architecture other than AArch64 this compiles to an inert stub so
//! workspace-wide host builds, clippy and rustdoc keep working — the real
//! image is built only via `cargo run -p xtask -- pi5`.

#![cfg_attr(target_arch = "aarch64", no_std)]
#![cfg_attr(target_arch = "aarch64", no_main)]
#![forbid(unsafe_code)]
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
#[cfg(target_arch = "aarch64")]
#[used]
static SPOOR_SEAM: (
    extern "C" fn(u16, u8, u32),
    unsafe extern "C" fn(*mut u8, usize) -> usize,
    extern "C" fn(u64),
    unsafe extern "C" fn(*mut u8, usize) -> usize,
) = (
    kernel::spoor_stream::tinyos_spoor_stamp,
    kernel::spoor_stream::tinyos_spoor_drain,
    kernel::spoor_stream::tinyos_spoor_seed_epoch,
    kernel::spoor_stream::tinyos_spoor_announce,
);

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
