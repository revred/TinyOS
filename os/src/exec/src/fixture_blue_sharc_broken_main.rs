//! `TEST-P0-05-04-A`'s deliberately-corrupted-copy Tier 0 QEMU fixture
//! binary (`STORY-P0-05-04` acceptance criterion 3) — mirrors
//! `kernel/src/main.rs`'s own `fixture-broken-boot` pattern (a
//! distinguishable failure path must actually be reachable, not just
//! assumed), applied to `exec::pe::parse` instead of kernel boot.
//!
//! Corrupts a runtime copy of the same real, page-aligned `blue-sharc.txe`
//! bytes `fixture_blue_sharc_main.rs` loads successfully (flips the DOS
//! header's `"MZ"` signature) and asserts `pe::parse` fails closed with
//! [`exec::pe::PeError::InvalidDosSignature`], not a panic or a
//! best-effort partial parse.

#![no_std]
#![no_main]

use exec::pe::{self, PeError};
#[allow(unused_imports)]
// linked for its `global_asm!` side effect only, per its own doc comment
use hal_x86_64::boot as _;
use hal_x86_64::qemu_exit::{exit_qemu, QemuExitCode};

const SECTIONS: usize = 8;
const IMPORTS: usize = 256;

#[repr(C, align(4096))]
struct AlignedImage([u8; 8_269_824]);

static IMAGE_BYTES: AlignedImage = AlignedImage(*include_bytes!("../fixtures/blue-sharc.txe"));
static mut CORRUPTED: AlignedImage = AlignedImage([0; 8_269_824]);

/// Runs the fixture, returning whether the corruption was correctly
/// rejected.
///
/// `&raw mut` + deref mirrors `exec-fixture`'s own precedent for the
/// identical `static_mut_refs`/`deref_addrof` lint concern.
#[allow(static_mut_refs, clippy::deref_addrof)]
fn run() -> bool {
    // SAFETY: this fixture is the only code running (single-CPU boot
    // path); `CORRUPTED` is written exactly once, before it's ever read.
    unsafe {
        (*&raw mut CORRUPTED).0.copy_from_slice(&IMAGE_BYTES.0);
        (*&raw mut CORRUPTED).0[0] = b'X'; // was 'M' — breaks the "MZ" DOS signature
    }
    // SAFETY: see above; the write above happened-before this read.
    let corrupted_bytes: &[u8] = unsafe { &(*&raw const CORRUPTED).0 };
    pe::parse::<SECTIONS, IMPORTS>(corrupted_bytes).err() == Some(PeError::InvalidDosSignature)
        // A well-formed image (the same bytes, unmodified) must still
        // parse — proving the rejection above is specific to the
        // corruption, not a fixture-wide parsing regression.
        && pe::parse::<SECTIONS, IMPORTS>(&IMAGE_BYTES.0).is_ok()
}

#[no_mangle]
extern "C" fn kernel_main(_start_info_paddr: u64) -> ! {
    if run() {
        exit_qemu(QemuExitCode::Success)
    } else {
        exit_qemu(QemuExitCode::Failure)
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    exit_qemu(QemuExitCode::Failure)
}
