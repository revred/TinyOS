//! `TEST-P2-07-01-A`'s Tier 0 QEMU fixture: the parity `.TCB` on the real target.
//!
//! Runs the *same* seeded world and script as the host golden test
//! (`shell::parity`), streaming the transcript over COM1. In-guest assertions —
//! exactly one denial (the withheld verb), no truncation — gate the
//! `isa-debug-exit` code, so the harness gets two independent signals: the
//! byte-compared transcript and the exit verdict (`timing.rs`'s discipline).
//!
//! Boot glue on the `exec-fixture` pattern; the `shell` *library* stays
//! `forbid(unsafe_code)` — this binary is its own compilation unit.

#![no_std]
#![no_main]

#[allow(unused_imports)]
// linked for its `global_asm!` side effect only, per its own doc comment
use hal_x86_64::boot as _;
use hal_x86_64::qemu_exit::{exit_qemu, QemuExitCode};
use hal_x86_64::serial::SerialPort;
use shell::{batch, parity};

/// The spoor-journaling decorator (`LE-56`), shared by `#[path]` with the
/// shell library's `#[cfg(test)]`-only include — see `spoor_policy.rs`'s own
/// doc comment for why it lives beside this binary and not in the library.
mod aci {
    pub use shell::policy::{GrantSet, VerbPolicy};
    pub use shell::verbs::{SpoorRow, SpoorView, VerbKind};

    #[path = "../spoor_policy.rs"]
    pub mod spoor_policy;
}

use aci::spoor_policy::{DenialJournal, SpoorPolicy};
use core::fmt::Write;

/// Sized at the batch line budget — the most denials one parity run can
/// produce — so the journal can never drop a record this fixture would count.
const JOURNAL_CAPACITY: usize = shell::capacities::MAX_BATCH_LINES;

/// The run's denial journal: each policy denial lands here as a kernel spoor
/// the moment the verdict is made.
static JOURNAL: DenialJournal<JOURNAL_CAPACITY> = DenialJournal::new();

/// The parity policy, decorated: same verdicts (proven authorisation-neutral
/// by `spoor_policy.rs`'s SP3), plus the audit journal.
static SPOOR_POLICY: SpoorPolicy<'static, JOURNAL_CAPACITY> =
    SpoorPolicy::new(&parity::POLICY, &JOURNAL);

#[no_mangle]
extern "C" fn kernel_main(_start_info_paddr: u64) -> ! {
    // SAFETY-adjacent precondition: COM1 exists under the QEMU q35 fixture
    // machine, the same precondition every serial-using fixture states.
    let mut serial = unsafe { SerialPort::init() };

    // The decorated policy journals denials; the same journal is the `SPOOR`
    // verb's view — so the transcript itself shows the spoors it audited.
    let mut world = parity::world_with(&SPOOR_POLICY, &JOURNAL);
    let outcome = batch::run(&mut world, parity::SCRIPT, &mut serial);

    // The in-guest half of the spoor gate (hand-2026-07-30/04A §1.3): the
    // journal must corroborate the denial counter, which must match the
    // parity expectation — three counts, one fact.
    let ok = match outcome {
        Ok(stats) => {
            // The spoor trailer (`LE-56`, third signal): emitted *after* the
            // transcript so the golden byte-comparison never sees it —
            // `check-shell-parity` splits the capture at this marker.
            let _ =
                writeln!(serial, "TINYOS-SPOOR/1 len={} denials={}", JOURNAL.len(), stats.denials);
            stats.denials == parity::expected_denials()
                && !stats.truncated
                && JOURNAL.len() as u32 == stats.denials
        }
        Err(_) => false,
    };
    exit_qemu(if ok { QemuExitCode::Success } else { QemuExitCode::Failure })
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    hal_x86_64::qemu_exit::panic_report(info)
}
