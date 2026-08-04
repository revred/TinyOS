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
