//! The AArch64 boot path (`STORY-P1-07-01`).
//!
//! Two halves, and the split is the point.
//!
//! The **reporting** half — what the board says, and in what order — is pure,
//! generic over the [`Mmio`] seam, and host-tested against a double that
//! accumulates the wire. So the claim "`CurrentEL` is printed before anything
//! else" is checked on the x86_64 dev machine rather than argued from a code
//! reading.
//!
//! The **entry** half is assembly and register writes, compiled only for
//! AArch64. `agent/CODING_STANDARDS.md`'s language policy admits exactly this
//! ("the earliest boot stub ... may be written in a small amount of
//! hand-written assembly, wrapped by a Rust `extern "C"` boundary as thin as
//! possible"), and it is the reason `TEST-P1-07-01-A` clause 5's "only
//! `cfg(target_arch = "aarch64")` item, only `unsafe`" is read as scoped to the
//! PL011 driver: a Story whose deliverable includes a stack and a zeroed `.bss`
//! cannot have zero assembly, and the Test document specifies both. That
//! reading is recorded rather than assumed — see
//! `session/hand-2026-07-28/23-bcm2712-divergence-record.md`.
//!
//! **None of the entry half has executed.** It is compiled and reviewed, the
//! same state `timer::SystemRegisters` has been in since
//! `STORY-P1-01-03`, and `STORY-P1-07-01` is not Verified until a capture
//! exists.

use crate::exception_level::ExceptionLevel;
use crate::pl011::{hex_u64, Mmio, Pl011, Pl011Error};

/// Every line the stub emits carries this prefix, so a capture can be
/// separated from the firmware's own console output on the same wire.
const TAG: &str = "TOS64-BOOT/1 ";

/// The known byte sequence `TEST-P1-07-01-A` clause 4 looks for in the capture.
///
/// Fixed, so evidence is diffed rather than eyeballed, and containing no bare
/// line feed so that framing (see [`crate::pl011::framed`]) cannot make the
/// bytes on the wire depend on which write method the stub happened to call.
pub const KNOWN_BYTE_SEQUENCE: &[u8] = b"TOS64-BOOT/1 READY 0123456789ABCDEF\r\n";

/// What the firmware left in the first four argument registers, plus the
/// exception level it handed over at.
///
/// Read and reported; **never retained as authority** (`BND-02`, `PD-14`).
/// Nothing downstream of the report consumes any of these values, and `x0` in
/// particular — the device-tree blob pointer — is printed and dropped, never
/// walked (`BND-03`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Handoff {
    /// `x0` at entry: the device-tree blob pointer, per the Linux AArch64 boot
    /// protocol the Raspberry Pi firmware follows.
    pub x0: u64,
    /// `x1` at entry. Reserved-zero by that protocol; reported, not trusted.
    pub x1: u64,
    /// `x2` at entry. Reserved-zero by that protocol; reported, not trusted.
    pub x2: u64,
    /// `x3` at entry. Reserved-zero by that protocol; reported, not trusted.
    pub x3: u64,
    /// The raw `CurrentEL` register, undecoded.
    pub current_el: u64,
}

impl Handoff {
    /// The decoded entry level.
    pub const fn level(&self) -> Option<ExceptionLevel> {
        ExceptionLevel::decode(self.current_el)
    }
}

/// Writes the entry report: the exception level first, then the handoff.
///
/// The leading CRLF exists only to break out of a partial line the firmware may
/// have left on the wire. It carries no information, which is why clause 3's
/// "before anything else" is read as being about the first *content* — and why
/// a test pins that reading rather than leaving it to a reviewer.
pub fn report_entry<M: Mmio>(uart: &Pl011<M>, handoff: &Handoff) -> Result<(), Pl011Error> {
    uart.write_str("\n")?;

    uart.write_str(TAG)?;
    uart.write_str("current_el=")?;
    match handoff.level() {
        Some(level) => {
            uart.write_str(level.as_str())?;
            if !level.is_plausible_firmware_entry() {
                // Firmware cannot hand over below EL1. Saying so on the wire
                // costs one word and saves a session spent trusting the number.
                uart.write_str("(implausible)")?;
            }
        }
        None => uart.write_str("unknown")?,
    }
    uart.write_str(" raw=")?;
    write_hex(uart, handoff.current_el)?;
    uart.write_str("\n")?;

    uart.write_str(TAG)?;
    uart.write_str("handoff")?;
    for (name, value) in
        [(" x0=", handoff.x0), (" x1=", handoff.x1), (" x2=", handoff.x2), (" x3=", handoff.x3)]
    {
        uart.write_str(name)?;
        write_hex(uart, value)?;
    }
    uart.write_str("\n")?;

    uart.write_str(TAG)?;
    uart.write_str("dtb=")?;
    write_hex(uart, handoff.x0)?;
    uart.write_str(" parsed=no\n")
}

/// Writes what the stub decided to do about the entry level.
///
/// Announced whether or not a transition happened, because "no drop was needed"
/// and "the drop did not run" are the same silence otherwise.
pub fn report_drop<M: Mmio>(uart: &Pl011<M>, level: ExceptionLevel) -> Result<(), Pl011Error> {
    uart.write_str(TAG)?;
    if level.needs_drop_to_el1() {
        return uart.write_str("dropped_to=EL1\n");
    }
    uart.write_str("dropped_to=none reason=")?;
    match level {
        ExceptionLevel::El1 => uart.write_str("already-el1\n"),
        ExceptionLevel::El3 => uart.write_str("el3-unhandled\n"),
        // EL0 is impossible as an entry level and EL2 was handled above.
        _ => uart.write_str("no-transition-defined\n"),
    }
}

/// Writes [`KNOWN_BYTE_SEQUENCE`] — clause 4's evidence.
pub fn report_ready<M: Mmio>(uart: &Pl011<M>) -> Result<(), Pl011Error> {
    uart.write_bytes(KNOWN_BYTE_SEQUENCE)
}

/// The fixture name the default boot image reports in its verdict line.
pub const BOOT_FIXTURE_NAME: &str = "boot";

/// Writes the UART pass/fail verdict — `STORY-P1-01-02`'s `TOS64-RESULT/1`
/// protocol, byte-compatible with what every Tier 0 fixture emits, because
/// `STORY-P1-07-05`'s run path drives its exit code with the parser that
/// already exists and no new protocol is invented for hardware. The framer
/// owns the CR; this line supplies LF only, like every other report here.
pub fn report_result<M: Mmio>(uart: &Pl011<M>, fixture: &str, ok: bool) -> Result<(), Pl011Error> {
    uart.write_str("TOS64-RESULT/1 fixture=")?;
    uart.write_str(fixture)?;
    uart.write_str(" ok=")?;
    uart.write_str(if ok { "true\n" } else { "false\n" })
}

/// The boot image's own self-consistency verdict: the two facts
/// `continue_at_el1` can actually observe after the drop and the vector
/// install. Anything less than both is `ok=false` — a wrong execution level
/// or a `VBAR_EL1` write that did not take is a hang deferred, not a pass.
pub fn boot_self_check(current_el_raw: u64, vbar_requested: u64, vbar_readback: u64) -> bool {
    matches!(ExceptionLevel::decode(current_el_raw), Some(ExceptionLevel::El1))
        && vbar_requested == vbar_readback
}

/// Writes a `u64` as sixteen hex digits.
fn write_hex<M: Mmio>(uart: &Pl011<M>, value: u64) -> Result<(), Pl011Error> {
    uart.write_bytes(&hex_u64(value))
}

// The reset vector. (A `//` comment, not a doc comment: rustdoc does not
// document macro invocations, and `global_asm!` is one.)
//
// Placed in `.text.boot`, which `targets/aarch64-tinyos.ld` puts first in the
// image, because the firmware jumps to the first byte of the loaded file and
// the ELF entry point does not survive `objcopy`.
//
// It does four things and stops: park the secondary cores, establish a stack,
// zero `.bss`, call Rust. `x0`-`x3` are untouched throughout so the firmware
// handoff arrives at `entry` intact — which is why the scratch registers start
// at `x4`.
//
// Unverified. Never executed; see this module's documentation.
#[cfg(target_arch = "aarch64")]
core::arch::global_asm!(
    ".section .text.boot",
    ".global _start",
    "_start:",
    // Cores 1-3 park here forever. `FEAT-P1-07` §6 makes single-core a scope
    // boundary, and a secondary core running this stub would race the first
    // one through .bss zeroing and the UART configuration.
    "    mrs  x4, mpidr_el1",
    "    and  x4, x4, #0xFF",
    "    cbz  x4, 1f",
    "0:  wfe",
    "    b    0b",
    // Stack before anything that could call. There is no guard page until
    // `STORY-P1-07-03`.
    "1:  adrp x4, __stack_top",
    "    add  x4, x4, :lo12:__stack_top",
    "    mov  sp, x4",
    // Zero .bss, sixteen bytes at a time. Both bounds are 16-byte aligned by
    // the linker script, so this can neither overrun nor take the alignment
    // fault that Device-nGnRnE memory raises for an unaligned access — and
    // until `STORY-P1-07-02` there is no vector table to report one.
    "    adrp x5, __bss_start",
    "    add  x5, x5, :lo12:__bss_start",
    "    adrp x6, __bss_end",
    "    add  x6, x6, :lo12:__bss_end",
    "2:  cmp  x5, x6",
    "    b.hs 3f",
    "    stp  xzr, xzr, [x5], #16",
    "    b    2b",
    "3:  bl   {entry}",
    // `entry` is `-> !`, so this is unreachable. It exists because a `ret` off
    // the end of the world on this board is a jump to whatever the firmware
    // left in `x30`, and parking is the only safe state available before
    // fault reporting exists.
    "4:  wfe",
    "    b    4b",
    entry = sym entry,
);

/// The Rust side of the boot stub.
///
/// **Unverified.** Never executed; see this module's documentation.
#[cfg(target_arch = "aarch64")]
#[no_mangle]
pub extern "C" fn entry(x0: u64, x1: u64, x2: u64, x3: u64) -> ! {
    let current_el: u64;
    // SAFETY: `CurrentEL` is readable at EL1 and above with no enablement and
    // no side effect. This is the first instruction that touches system state,
    // deliberately: the plan's second-highest risk is that this value is not
    // what the code assumed, and nothing below may assume it.
    unsafe {
        core::arch::asm!(
            "mrs {value}, CurrentEL",
            value = out(reg) current_el,
            options(nomem, nostack, preserves_flags),
        );
    }

    let handoff = Handoff { x0, x1, x2, x3, current_el };

    // SAFETY: `DEBUG_UART_BASE` is the BCM2712 `uart10` window transcribed in
    // `crate::board`, and cores 1-3 are parked in `_start`, so nothing else on
    // this machine is programming it.
    let uart =
        Pl011::new(unsafe { crate::pl011::VolatileMmio::new(crate::board::DEBUG_UART_BASE) });

    // If this fails there is no way to say so — the failure *is* silence, which
    // is why `TEST-P1-07-01-A` clause 1 makes the adapter the first thing
    // proven. Ignored deliberately rather than by omission: there is no
    // reporting channel other than the device that just refused.
    let _ = uart.configure(crate::board::DEBUG_UART_CLOCK_HZ, crate::board::DEBUG_UART_BAUD);

    let _ = report_entry(&uart, &handoff);

    if let Some(level) = handoff.level() {
        let _ = report_drop(&uart, level);
        if level.needs_drop_to_el1() {
            // Does not return: control resumes at `continue_at_el1`.
            // SAFETY: see `drop_to_el1`'s own contract.
            unsafe { drop_to_el1() }
        }
    }

    continue_at_el1()
}

/// Takes `EL2 → EL1` and resumes at [`continue_at_el1`].
///
/// Called only when [`ExceptionLevel::needs_drop_to_el1`] said so — the level
/// is an input, never a constant.
///
/// **Unverified.** Never executed; see this module's documentation.
///
/// # Safety
///
/// Must be called at `EL2`, on the boot core, with a valid stack. It does not
/// return: `eret` transfers to [`continue_at_el1`] at `EL1h` with `DAIF`
/// masked, and there is no vector table until `STORY-P1-07-02`, so any fault
/// raised between here and there is a silent hang.
#[cfg(target_arch = "aarch64")]
unsafe fn drop_to_el1() -> ! {
    // SAFETY: the caller established EL2 and a stack; every register written
    // below is EL2-owned and this is the only writer.
    unsafe {
        core::arch::asm!(
            // EL1 is AArch64 (HCR_EL2.RW), and nothing else is set: no
            // virtualisation, no trapping. This slice hosts nothing.
            "mov  x0, #(1 << 31)",
            "msr  hcr_el2, x0",
            // Let EL1 read the physical/virtual counters, with zero offset, so
            // `STORY-P1-01-03`'s `CNTVCT_EL0` reads mean what they claim when
            // `STORY-P1-07-04` finally runs them.
            "mrs  x0, cnthctl_el2",
            "orr  x0, x0, #3",
            "msr  cnthctl_el2, x0",
            "msr  cntvoff_el2, xzr",
            // SCTLR_EL1 to its reserved-one pattern with M, C, I and A clear:
            // MMU off, caches off, alignment checking off. `STORY-P1-07-03`
            // owns turning those on, and doing it here would be the "just get
            // it booting" shortcut that Story exists to prevent.
            "mov  x0, #0x0800",
            "movk x0, #0x30d0, lsl #16",
            "msr  sctlr_el1, x0",
            // EL1h, DAIF masked. Interrupts must stay masked: there is no
            // vector table.
            "mov  x0, #0x3c5",
            "msr  spsr_el2, x0",
            // EL1 inherits this core's stack. One core, one stack, no tasks.
            "mov  x0, sp",
            "msr  sp_el1, x0",
            "adr  x0, {resume}",
            "msr  elr_el2, x0",
            "isb",
            "eret",
            resume = sym continue_at_el1,
            options(noreturn),
        )
    }
}

/// Where execution lands after the drop — and where it lands anyway if no drop
/// was needed.
///
/// Re-reads `CurrentEL` rather than trusting that the `eret` did what it was
/// told. A transition that silently did not happen is indistinguishable from
/// one that did, in every respect except this line of text.
///
/// **Unverified.** Never executed; see this module's documentation.
#[cfg(target_arch = "aarch64")]
extern "C" fn continue_at_el1() -> ! {
    let current_el: u64;
    // SAFETY: as in `entry` — a side-effect-free read of an always-readable
    // system register.
    unsafe {
        core::arch::asm!(
            "mrs {value}, CurrentEL",
            value = out(reg) current_el,
            options(nomem, nostack, preserves_flags),
        );
    }

    // SAFETY: as in `entry`.
    let uart =
        Pl011::new(unsafe { crate::pl011::VolatileMmio::new(crate::board::DEBUG_UART_BASE) });

    let _ = uart.write_str(TAG);
    let _ = uart.write_str("now_at=");
    let _ = match ExceptionLevel::decode(current_el) {
        Some(level) => uart.write_str(level.as_str()),
        None => uart.write_str("unknown"),
    };
    let _ = uart.write_str("\n");

    let _ = report_ready(&uart);

    // `STORY-P1-07-02`: the vector table, installed **after**
    // `TEST-P1-07-01-A` clause 4's known byte sequence rather than before it.
    //
    // The order is deliberate and it is about evidence, not about correctness:
    // clause 4's capture is `STORY-P1-07-01`'s Green, it has not been taken
    // yet, and it must be produced by the same code path that was specified
    // for it. Installing vectors first would put two new lines ahead of the
    // sequence and make a pending piece of another Story's evidence depend on
    // this one. Everything between `_start` and here therefore still runs with
    // no fault reporting — which is exactly the state this Story exists to
    // end, and cannot end for the code that precedes its own installation.
    //
    // SAFETY: `continue_at_el1` runs at `EL1` on the boot core with the stack
    // `_start` established, and this is the only caller.
    let (requested, readback) = unsafe { crate::fault::install() };
    let _ = crate::fault::report_vbar(&uart, requested, readback);

    // `STORY-P1-07-05`: the verdict line the host run path's exit code is
    // driven by, emitted last so it vouches for every claim above it. This is
    // the line that turns a capture into a pass/fail rather than a transcript.
    let _ =
        report_result(&uart, BOOT_FIXTURE_NAME, boot_self_check(current_el, requested, readback));

    park()
}

/// Parks the core in `wfe` forever.
///
/// Not a halt loop out of laziness: with no scheduler and no resume path, a
/// safe state is the only correct terminal state, and
/// `agent/CODING_STANDARDS.md` resolves this as fail-safe over keep-trying.
/// Public so `pi5-image`'s panic handler lands in the same state instead of
/// duplicating the `unsafe`.
#[cfg(target_arch = "aarch64")]
pub fn park() -> ! {
    loop {
        // SAFETY: `wfe` is architecturally permitted at every exception level
        // and has no effect on any state this code owns.
        unsafe { core::arch::asm!("wfe", options(nomem, nostack, preserves_flags)) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pl011::{register, Mmio, Pl011};
    use core::cell::RefCell;

    /// An always-ready MMIO double that accumulates what reached the wire.
    struct Wire {
        bytes: RefCell<Vec<u8>>,
    }

    impl Wire {
        fn new() -> Self {
            Wire { bytes: RefCell::new(Vec::new()) }
        }

        fn captured(&self) -> String {
            String::from_utf8(self.bytes.borrow().clone()).expect("the report is ASCII")
        }
    }

    impl Mmio for Wire {
        fn read_u32(&self, _offset: usize) -> u32 {
            0
        }

        fn write_u32(&self, offset: usize, value: u32) {
            if offset == register::DR {
                self.bytes.borrow_mut().push(value as u8);
            }
        }
    }

    fn entered_at(current_el: u64) -> Handoff {
        Handoff { x0: 0x1F00_0000, x1: 0, x2: 0, x3: 0, current_el }
    }

    // Clause 3: `CurrentEL` is printed *before anything else*.
    #[test]
    fn the_exception_level_is_the_first_thing_reported() {
        let wire = Wire::new();
        let uart = Pl011::new(&wire);
        report_entry(&uart, &entered_at(0b1000)).expect("a ready device");

        let captured = wire.captured();
        // A leading CRLF only, to break out of a partial firmware line — it
        // carries no information, so it does not count as "anything else".
        let body = captured.strip_prefix("\r\n").expect("a leading line break");
        assert!(
            body.starts_with("TOS64-BOOT/1 current_el="),
            "the level must lead the capture, got: {body:?}"
        );
    }

    #[test]
    fn the_report_names_the_level_and_quotes_the_raw_register() {
        // Both, deliberately. The decoded name is what a human reads; the raw
        // value is what makes a *wrong decode* diagnosable from the capture
        // alone, without a second session on the board.
        let wire = Wire::new();
        let uart = Pl011::new(&wire);
        report_entry(&uart, &entered_at(0b1000)).expect("ready");
        assert!(wire.captured().contains("current_el=EL2 raw=0000000000000008"));
    }

    #[test]
    fn every_entry_level_the_firmware_could_hand_over_at_reports_distinctly() {
        for (raw, name) in [(0b0100u64, "EL1"), (0b1000, "EL2"), (0b1100, "EL3")] {
            let wire = Wire::new();
            let uart = Pl011::new(&wire);
            report_entry(&uart, &entered_at(raw)).expect("ready");
            assert!(wire.captured().contains(&format!("current_el={name}")), "for {raw:#b}");
        }
    }

    #[test]
    fn an_impossible_entry_level_is_flagged_rather_than_reported_as_ordinary() {
        // Firmware cannot hand over at EL0. If the capture says EL0 the read is
        // wrong, and the capture should say so rather than leaving a reader to
        // know that fact independently.
        let wire = Wire::new();
        let uart = Pl011::new(&wire);
        report_entry(&uart, &entered_at(0b0000)).expect("ready");
        let captured = wire.captured();
        assert!(captured.contains("current_el=EL0"));
        assert!(captured.contains("implausible"), "got: {captured:?}");
    }

    // Clause 6: the handoff is read, reported, and not retained as authority.
    #[test]
    fn the_firmware_handoff_registers_are_reported_verbatim() {
        let wire = Wire::new();
        let uart = Pl011::new(&wire);
        let handoff = Handoff {
            x0: 0x1F00_0000,
            x1: 0xDEAD,
            x2: 0,
            x3: 0xFFFF_FFFF_FFFF_FFFF,
            current_el: 0b1000,
        };
        report_entry(&uart, &handoff).expect("ready");
        let captured = wire.captured();
        assert!(captured.contains("x0=000000001F000000"));
        assert!(captured.contains("x1=000000000000DEAD"));
        assert!(captured.contains("x2=0000000000000000"));
        assert!(captured.contains("x3=FFFFFFFFFFFFFFFF"));
    }

    #[test]
    fn the_device_tree_pointer_is_reported_and_declared_unparsed() {
        // `BND-03`: reading the pointer is acceptable, walking the structure is
        // a hostile-format parser in C1. Saying `parsed=no` on the wire makes
        // that a claim the capture itself carries, rather than a claim only the
        // Test document makes.
        let wire = Wire::new();
        let uart = Pl011::new(&wire);
        report_entry(&uart, &entered_at(0x1F00_0000)).expect("ready");
        assert!(wire.captured().contains("dtb=000000001F000000 parsed=no"));
    }

    // Clause 3: the drop is conditional, and what was decided is on the wire.
    #[test]
    fn a_drop_from_el2_is_announced() {
        let wire = Wire::new();
        let uart = Pl011::new(&wire);
        report_drop(&uart, ExceptionLevel::El2).expect("ready");
        assert!(wire.captured().contains("dropped_to=EL1"));
    }

    #[test]
    fn entry_already_at_el1_announces_that_no_drop_was_needed() {
        let wire = Wire::new();
        let uart = Pl011::new(&wire);
        report_drop(&uart, ExceptionLevel::El1).expect("ready");
        let captured = wire.captured();
        assert!(captured.contains("dropped_to=none"));
        assert!(captured.contains("already-el1"));
    }

    #[test]
    fn entry_at_el3_announces_that_this_slice_does_not_handle_it() {
        // Not a drop, not a silent continue: EL3 entry is outside what a Story
        // with no exception vectors can attempt, and the capture says so.
        let wire = Wire::new();
        let uart = Pl011::new(&wire);
        report_drop(&uart, ExceptionLevel::El3).expect("ready");
        assert!(wire.captured().contains("el3-unhandled"));
    }

    // Clause 4: the known byte sequence.
    #[test]
    fn the_known_byte_sequence_is_fixed_and_reaches_the_wire_unchanged() {
        // Fixed so that a capture can be diffed rather than eyeballed, and
        // written with `write_bytes` rather than `write_str` so nothing frames
        // it on the way out — the evidence is the bytes, in order.
        let wire = Wire::new();
        let uart = Pl011::new(&wire);
        report_ready(&uart).expect("ready");
        let captured = wire.captured();
        assert!(captured.ends_with("TOS64-BOOT/1 READY 0123456789ABCDEF\r\n"));
        assert_eq!(KNOWN_BYTE_SEQUENCE, b"TOS64-BOOT/1 READY 0123456789ABCDEF\r\n");
    }

    #[test]
    fn the_known_byte_sequence_carries_its_own_crlf_because_nothing_frames_it() {
        // `write_bytes` does not frame (clause 4's evidence must reach the wire
        // unchanged), so the sequence supplies the CR itself. Every LF in it is
        // CRLF-paired, which is what makes the quoted capture render on a
        // terminal without the transmit path having rewritten it.
        for (index, byte) in KNOWN_BYTE_SEQUENCE.iter().enumerate() {
            if *byte == b'\n' {
                assert_eq!(
                    index.checked_sub(1).map(|before| KNOWN_BYTE_SEQUENCE[before]),
                    Some(b'\r'),
                    "bare LF at {index} would staircase on a terminal"
                );
            }
        }
    }

    #[test]
    fn no_reported_line_contains_a_carriage_return_the_framer_would_double() {
        // Found by this test, which is why it exists: `write_str("\r\n")`
        // frames the LF and emits CR-CR-LF, so a source string that spells its
        // own line ending puts a stray CR on the wire. The framer owns the CR;
        // the report supplies LF only. A capture with doubled CRs is not
        // *wrong* on most terminals, which is exactly why it would have
        // survived review and landed in a quoted piece of evidence.
        let wire = Wire::new();
        let uart = Pl011::new(&wire);
        report_entry(&uart, &entered_at(0b1000)).expect("ready");
        report_drop(&uart, ExceptionLevel::El2).expect("ready");
        assert!(!wire.captured().contains("\r\r"), "got: {:?}", wire.captured());
    }

    // `STORY-P1-07-05` clause 2: the host run path's exit code is driven by
    // the *existing* UART pass/fail protocol — `TOS64-RESULT/1`, exactly as
    // `STORY-P1-01-02` shipped it and exactly as `xtask`'s `parse_result`
    // consumes it. No new protocol is invented for hardware, so the line this
    // board emits must be byte-compatible with the Tier 0 one.
    #[test]
    fn the_boot_verdict_line_is_the_tier_0_result_protocol_verbatim() {
        let wire = Wire::new();
        let uart = Pl011::new(&wire);
        report_result(&uart, BOOT_FIXTURE_NAME, true).expect("ready");
        // The framer owns the CR (see the no-doubled-CR test above), so the
        // wire carries CRLF while the source supplies LF only.
        assert_eq!(wire.captured(), "TOS64-RESULT/1 fixture=boot ok=true\r\n");
    }

    #[test]
    fn a_failed_self_check_reports_ok_false_rather_than_staying_silent() {
        // A board whose self-check fails and says nothing is indistinguishable
        // from a board that hung — and `TEST-P1-07-05-A` clause 3 spends its
        // whole third paragraph on why that ambiguity wastes a session.
        let wire = Wire::new();
        let uart = Pl011::new(&wire);
        report_result(&uart, BOOT_FIXTURE_NAME, false).expect("ready");
        assert_eq!(wire.captured(), "TOS64-RESULT/1 fixture=boot ok=false\r\n");
    }

    #[test]
    fn the_verdict_passes_only_at_el1_with_the_vbar_it_asked_for() {
        const EL1: u64 = 0b0100;
        const EL2: u64 = 0b1000;
        const VBAR: u64 = 0x8_0800;
        // The two facts `continue_at_el1` can actually observe: the re-read
        // exception level, and the `VBAR_EL1` readback. Both right is the only
        // pass.
        assert!(boot_self_check(EL1, VBAR, VBAR));
        // Still at EL2: the `eret` did not do what it was told.
        assert!(!boot_self_check(EL2, VBAR, VBAR));
        // A `VBAR_EL1` readback that disagrees: the vector install silently
        // did not take, which is a hang wearing a success banner.
        assert!(!boot_self_check(EL1, VBAR, VBAR + 0x800));
        // An undecodable level is a wrong read, never a pass.
        assert!(!boot_self_check(0xFFFF, VBAR, VBAR));
        // EL0 is impossible as an execution level here; a capture claiming it
        // means the read is wrong.
        assert!(!boot_self_check(0b0000, VBAR, VBAR));
    }

    // The report is one transmit path, and it stops on the first failure like
    // every other one — an unbounded-retry banner on a wedged UART is the hang
    // this Story exists to eliminate, wearing a different hat.
    #[test]
    fn a_wedged_uart_makes_the_report_fail_rather_than_stall() {
        struct Wedged;
        impl Mmio for Wedged {
            fn read_u32(&self, _offset: usize) -> u32 {
                crate::pl011::flag::TXFF
            }
            fn write_u32(&self, _offset: usize, _value: u32) {}
        }
        let uart = Pl011::new(Wedged);
        assert_eq!(
            report_entry(&uart, &entered_at(0b1000)),
            Err(crate::pl011::Pl011Error::TransmitTimeout)
        );
    }
}
