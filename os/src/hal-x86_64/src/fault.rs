//! CPU exception capture: frame layout, error-code decoding, and the entry
//! stubs for `#UD`/`#GP`/`#PF` (`STORY-P1-02-01`).
//!
//! Split the same way [`crate::idt`] and [`crate::interrupts`] are: everything
//! in this module above [`FaultFrame`] is ordinary data manipulation that
//! compiles and runs on any host, and only the `global_asm!` stubs and their
//! `extern "C"` entry point need a real x86_64 CPU. The reason is the same one
//! `idt.rs` gives — a bit-layout error in a fault path is invisible to the type
//! system and corrupts every field downstream, so the layout belongs somewhere
//! a host test can pin it.
//!
//! **One frame shape for all three vectors.** `#UD` pushes no hardware error
//! code while `#GP` and `#PF` do. Rather than three frame types (and three
//! parsers, the third of which would be the one with the bug), the `#UD` stub
//! pushes a synthetic zero so every vector reaches [`fault_common`] with an
//! identical stack layout. `CR2` is read unconditionally and is only
//! *meaningful* for `#PF` — [`FaultFrame::faulting_address`] is what enforces
//! that, rather than a caller remembering.
//!
//! **The error code is evidence, never authority** (`BND-04`, this Feature's
//! containment contract). It arrives from arbitrary, possibly attacker-steered
//! execution. The decoders below name its bits so a report can be read by a
//! human; nothing in the kernel's disposition policy consults them, and this
//! module deliberately exposes no helper that would make it convenient to.
//!
//! **A fault inside this module lands on the IST stack** as of
//! `STORY-P1-02-02`: [`df_fault_stub`] is wired to vector 8 through an
//! IST-bearing gate ([`crate::tss::IstIndex::DOUBLE_FAULT`]), so the escalation
//! that used to end in a silent triple fault now ends in a report. It ends
//! there and nowhere better — a double fault means the primary path above is
//! compromised, so that handler is terminal by design (see
//! [`DOUBLE_FAULT_VECTOR`]).

/// The CPU exception vectors this module captures.
///
/// Deliberately not "every exception": these are the three
/// `STORY-P1-02-01` enumerates. Every other vector keeps
/// `STORY-P0-04-02`'s shared fail-closed default, so this Story narrows what
/// is terminal for the whole system without widening what is ignored (which
/// stays empty).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultVector {
    /// Vector 6, `#UD` — invalid opcode. Pushes no hardware error code.
    InvalidOpcode,
    /// Vector 13, `#GP` — general protection. Pushes a selector error code.
    GeneralProtection,
    /// Vector 14, `#PF` — page fault. Pushes a page-fault error code, and is
    /// the only vector for which `CR2` is meaningful.
    PageFault,
}

impl FaultVector {
    /// Vector 6.
    pub const INVALID_OPCODE: u64 = 6;
    /// Vector 13.
    pub const GENERAL_PROTECTION: u64 = 13;
    /// Vector 14.
    pub const PAGE_FAULT: u64 = 14;

    /// Maps a raw vector number, or `None` for anything this module does not
    /// capture.
    ///
    /// `None` is a fault-handling *failure*, not a fall-through: a vector
    /// reaching the shared entry point that this module never wired means the
    /// IDT and this code disagree, and decoding it as if it were one of the
    /// three would produce a confident, wrong report.
    pub const fn from_raw(vector: u64) -> Option<Self> {
        match vector {
            Self::INVALID_OPCODE => Some(FaultVector::InvalidOpcode),
            Self::GENERAL_PROTECTION => Some(FaultVector::GeneralProtection),
            Self::PAGE_FAULT => Some(FaultVector::PageFault),
            _ => None,
        }
    }

    /// The short mnemonic used in serial reports.
    pub const fn mnemonic(self) -> &'static str {
        match self {
            FaultVector::InvalidOpcode => "#UD",
            FaultVector::GeneralProtection => "#GP",
            FaultVector::PageFault => "#PF",
        }
    }

    /// Whether the CPU itself pushes an error code for this vector.
    ///
    /// Exposed so a reader (and a test) can check the synthetic-zero rule
    /// rather than take the stubs' word for it.
    pub const fn pushes_error_code(self) -> bool {
        match self {
            FaultVector::InvalidOpcode => false,
            FaultVector::GeneralProtection | FaultVector::PageFault => true,
        }
    }
}

/// The captured state of one fault, in the exact order the stubs push it.
///
/// `#[repr(C)]` and field order are load-bearing: [`fault_common`] hands the
/// Rust entry point a pointer to this structure *as it sits on the stack*, so
/// the correspondence between these fields and the assembly below is a real
/// invariant that no compiler checks. The host tests pin `size_of` and every
/// offset for exactly that reason.
///
/// Stack layout, from the frame pointer upward. The stack grows **down**, so
/// offset 0 is what was pushed *last* and offset 56 is what the CPU pushed
/// first:
///
/// | offset | field | pushed by |
/// |---|---|---|
/// | 0 | `cr2` | `fault_common`, read from the register (pushed last) |
/// | 8 | `vector` | the per-vector stub |
/// | 16 | `error_code` | the CPU, or the `#UD` stub's synthetic zero |
/// | 24 | `rip` | the CPU |
/// | 32 | `cs` | the CPU |
/// | 40 | `rflags` | the CPU |
/// | 48 | `rsp` | the CPU |
/// | 56 | `ss` | the CPU (pushed first) |
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaultFrame {
    /// `CR2` as read on entry — meaningful only for `#PF`; see
    /// [`FaultFrame::faulting_address`].
    pub cr2: u64,
    /// Which exception fired.
    pub vector: u64,
    /// The hardware error code, or a synthetic `0` for vectors that push none.
    pub error_code: u64,
    /// Instruction pointer at the fault.
    pub rip: u64,
    /// Code segment at the fault.
    pub cs: u64,
    /// Flags at the fault.
    pub rflags: u64,
    /// Stack pointer at the fault.
    pub rsp: u64,
    /// Stack segment at the fault.
    pub ss: u64,
}

impl FaultFrame {
    /// The decoded vector, or `None` for a vector this module never wired.
    pub const fn kind(&self) -> Option<FaultVector> {
        FaultVector::from_raw(self.vector)
    }

    /// The faulting linear address — `Some` **only** for `#PF`.
    ///
    /// `CR2` holds whatever the last page fault left there, so reporting it
    /// for a `#GP` or `#UD` would be reporting a stale address from an
    /// unrelated earlier event with total confidence. The type enforces what a
    /// comment would only request.
    pub const fn faulting_address(&self) -> Option<u64> {
        match self.kind() {
            Some(FaultVector::PageFault) => Some(self.cr2),
            _ => None,
        }
    }

    /// The decoded `#PF` error code, or `None` for another vector.
    pub const fn page_fault_cause(&self) -> Option<PageFaultCause> {
        match self.kind() {
            Some(FaultVector::PageFault) => Some(PageFaultCause::decode(self.error_code)),
            _ => None,
        }
    }

    /// The decoded `#GP` selector error code, or `None` for another vector.
    pub const fn selector_error(&self) -> Option<SelectorError> {
        match self.kind() {
            Some(FaultVector::GeneralProtection) => Some(SelectorError::decode(self.error_code)),
            _ => None,
        }
    }
}

/// The `#PF` error code's named bits (Intel SDM Vol 3A §4.7).
///
/// Reported, never consulted for a decision: see this module's doc comment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageFaultCause {
    /// Bit 0: the fault was a protection violation on a *present* page
    /// (`false` means the page was not present at all).
    pub present: bool,
    /// Bit 1: the access was a write.
    pub write: bool,
    /// Bit 2: the access came from user mode (CPL 3).
    pub user: bool,
    /// Bit 3: a reserved bit was set in a page-table entry — always a kernel
    /// bug in the tables themselves, never faulting-code behavior.
    pub reserved_write: bool,
    /// Bit 4: the access was an instruction fetch.
    pub instruction_fetch: bool,
}

impl PageFaultCause {
    /// Decodes the five architecturally-defined bits, ignoring the rest.
    pub const fn decode(error_code: u64) -> Self {
        PageFaultCause {
            present: error_code & 1 != 0,
            write: error_code & (1 << 1) != 0,
            user: error_code & (1 << 2) != 0,
            reserved_write: error_code & (1 << 3) != 0,
            instruction_fetch: error_code & (1 << 4) != 0,
        }
    }
}

/// Which descriptor table a `#GP` selector error code refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorTable {
    /// Global descriptor table.
    Gdt,
    /// Interrupt descriptor table.
    Idt,
    /// Local descriptor table.
    Ldt,
}

/// The `#GP` selector error code's named fields (Intel SDM Vol 3A §6.13).
///
/// A `#GP` error code of **zero** is both common and meaningful: it says the
/// fault was not caused by a segment selector at all — a privileged-instruction
/// violation, say. That is why the whole structure is reported rather than a
/// bare index. (`kernel::fixture_fault`'s own `#GP` victim deliberately takes
/// the *other* branch, loading an out-of-range selector, so the non-zero path
/// is the one Tier 0 exercises.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectorError {
    /// Bit 0: the fault originated outside the processor (an external event).
    pub external: bool,
    /// Bits 1–2: which descriptor table the index refers to.
    pub table: DescriptorTable,
    /// Bits 3–15: the descriptor index.
    pub index: u16,
    /// Whether the whole error code was zero, i.e. no selector was involved.
    pub no_selector: bool,
}

impl SelectorError {
    /// Decodes the error code.
    pub const fn decode(error_code: u64) -> Self {
        let table = match (error_code >> 1) & 0b11 {
            0 => DescriptorTable::Gdt,
            1 | 3 => DescriptorTable::Idt,
            _ => DescriptorTable::Ldt,
        };
        SelectorError {
            external: error_code & 1 != 0,
            table,
            index: ((error_code >> 3) & 0x1FFF) as u16,
            no_selector: error_code == 0,
        }
    }
}

/// Vector 8, `#DF` — double fault.
///
/// Deliberately **not** a [`FaultVector`] variant. That enum names the vectors
/// whose faults `kernel::fault`'s disposition policy contains, and a double
/// fault is not contained: it means the primary fault path itself failed, so
/// there is nothing left to decide and no context worth trusting. It gets its
/// own vector constant, its own stub ([`df_fault_stub`]) and its own kernel
/// entry point (`tinyos_double_fault_entry`) precisely so that no code can
/// reach it by accident through the containment path.
///
/// The frame shape is identical, though — `#DF` pushes a hardware error code
/// (architecturally always zero), so no synthetic push is needed, and
/// [`FaultFrame::faulting_address`] already refuses to report `CR2` for it.
pub const DOUBLE_FAULT_VECTOR: u64 = 8;

// The fault entry stubs.
//
// Each pushes its vector (and, for `#UD`, a synthetic error code) and falls
// into `fault_common`, which reads `CR2`, pushes it, and hands the resulting
// [`FaultFrame`] to `tinyos_fault_entry` in `RDI` (System V's first integer
// argument).
//
// **No registers are saved and no `iretq` is executed**, for the same reason
// [`crate::interrupts::unhandled_interrupt_handler`] needs no trampoline:
// `tinyos_fault_entry` never returns. There is no resume path in this Story
// (`TEST-P1-02-01-A` clause 3), so there is nothing to restore — and if a
// resume arm is ever added, it arrives with the save/restore code it needs,
// rather than that code sitting here unused and unexercised in the meantime.
#[cfg(not(target_os = "windows"))]
core::arch::global_asm!(
    r#"
    .section .text

    .global ud_fault_stub
ud_fault_stub:
        push 0
        push 6
        jmp fault_common

    .global gp_fault_stub
gp_fault_stub:
        push 13
        jmp fault_common

    .global pf_fault_stub
pf_fault_stub:
        push 14
        jmp fault_common

fault_common:
        mov rax, cr2
        push rax
        mov rdi, rsp
        call tinyos_fault_entry
        ud2

    // `STORY-P1-02-02`. Four instructions duplicated from `fault_common`
    // rather than shared, because the one difference is the whole point: this
    // path calls `tinyos_double_fault_entry`, not `tinyos_fault_entry`. A
    // shared tail with a branch on the vector would put a decision inside the
    // stub that runs when the primary fault path has already failed.
    //
    // Reached only through an IST-bearing gate, so `RSP` here is the
    // known-good `#DF` stack the CPU loaded from the TSS — not the stack that
    // caused the escalation. `#DF` pushes a hardware error code (always zero),
    // so there is no synthetic push.
    .global df_fault_stub
df_fault_stub:
        push 8
        mov rax, cr2
        push rax
        mov rdi, rsp
        call tinyos_double_fault_entry
        ud2
    "#
);

#[cfg(not(target_os = "windows"))]
unsafe extern "C" {
    /// `#UD` entry point — install as vector 6's handler.
    pub fn ud_fault_stub();
    /// `#GP` entry point — install as vector 13's handler.
    pub fn gp_fault_stub();
    /// `#PF` entry point — install as vector 14's handler.
    pub fn pf_fault_stub();
    /// `#DF` entry point — install as vector 8's handler, through an
    /// **IST-bearing** gate ([`crate::idt::Idt::set_handler_with_ist`]).
    /// Installing it without an IST would leave the escalation exactly where it
    /// was: pushing a frame onto the stack that already failed.
    pub fn df_fault_stub();
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, size_of};

    // Clause 1: the frame's layout is the assembly stub's push order, and
    // nothing in the type system checks that correspondence.
    #[test]
    fn the_fault_frame_matches_the_stubs_push_order_exactly() {
        assert_eq!(size_of::<FaultFrame>(), 64, "eight 8-byte pushes");
        assert_eq!(align_of::<FaultFrame>(), 8);
        let frame = FaultFrame {
            cr2: 0,
            vector: 0,
            error_code: 0,
            rip: 0,
            cs: 0,
            rflags: 0,
            rsp: 0,
            ss: 0,
        };
        let base = &frame as *const FaultFrame as usize;
        let offset = |field: *const u64| field as usize - base;
        // The stack grows down, so offset 0 is the *last* push (`cr2`) and
        // offset 56 is the CPU's first (`ss`).
        assert_eq!(offset(&frame.cr2), 0);
        assert_eq!(offset(&frame.vector), 8);
        assert_eq!(offset(&frame.error_code), 16);
        assert_eq!(offset(&frame.rip), 24);
        assert_eq!(offset(&frame.cs), 32);
        assert_eq!(offset(&frame.rflags), 40);
        assert_eq!(offset(&frame.rsp), 48);
        assert_eq!(offset(&frame.ss), 56);
    }

    #[test]
    fn only_the_three_wired_vectors_decode() {
        assert_eq!(FaultVector::from_raw(6), Some(FaultVector::InvalidOpcode));
        assert_eq!(FaultVector::from_raw(13), Some(FaultVector::GeneralProtection));
        assert_eq!(FaultVector::from_raw(14), Some(FaultVector::PageFault));
        // Vectors this enum never names. Vector 8 is in this list *after*
        // `STORY-P1-02-02` too, and that is deliberate (`TEST-P1-02-02-A`
        // clause 4): the double fault now has a handler, but `FaultVector`
        // enumerates the vectors the *disposition policy* contains, and a
        // double fault is never contained.
        for vector in [0, 1, 3, 8, 12, 18, 0x30, 0xFF] {
            assert_eq!(FaultVector::from_raw(vector), None, "vector {vector} must not decode");
        }
        assert_eq!(FaultVector::from_raw(DOUBLE_FAULT_VECTOR), None);
    }

    // Clause 4: the `#DF` stub reads `CR2` unconditionally like every other
    // stub, so the type is again what stops a stale address being reported
    // against a vector it means nothing for.
    #[test]
    fn a_double_fault_frame_reports_no_faulting_address_and_no_decoded_error() {
        let frame = FaultFrame {
            cr2: 0xdead_beef,
            vector: DOUBLE_FAULT_VECTOR,
            error_code: 0,
            rip: 0x1000,
            cs: 8,
            rflags: 2,
            rsp: 0x2000,
            ss: 0x10,
        };
        assert_eq!(frame.kind(), None);
        assert_eq!(frame.faulting_address(), None);
        assert_eq!(frame.page_fault_cause(), None);
        assert_eq!(frame.selector_error(), None);
    }

    #[test]
    fn only_invalid_opcode_lacks_a_hardware_error_code() {
        assert!(!FaultVector::InvalidOpcode.pushes_error_code());
        assert!(FaultVector::GeneralProtection.pushes_error_code());
        assert!(FaultVector::PageFault.pushes_error_code());
    }

    // Clause 1: CR2 is read unconditionally, so the *type* must be what stops
    // a stale address from being reported against an unrelated vector.
    #[test]
    fn a_faulting_address_is_reported_only_for_a_page_fault() {
        let stale = 0xdead_beef;
        let frame = |vector| FaultFrame {
            cr2: stale,
            vector,
            error_code: 0,
            rip: 0x1000,
            cs: 8,
            rflags: 2,
            rsp: 0x2000,
            ss: 0x10,
        };
        assert_eq!(frame(14).faulting_address(), Some(stale));
        assert_eq!(frame(6).faulting_address(), None);
        assert_eq!(frame(13).faulting_address(), None);
        assert_eq!(frame(255).faulting_address(), None);
    }

    // Clause 2: error codes decode into named bits.
    #[test]
    fn a_page_fault_error_code_decodes_into_its_named_bits() {
        // Not-present, read, kernel-mode, data access.
        let cause = PageFaultCause::decode(0);
        assert!(!cause.present && !cause.write && !cause.user);
        assert!(!cause.reserved_write && !cause.instruction_fetch);
        // Present + write + user: a protection violation on a mapped page.
        let cause = PageFaultCause::decode(0b0111);
        assert!(cause.present && cause.write && cause.user);
        // Reserved-bit and instruction-fetch bits, independently.
        assert!(PageFaultCause::decode(1 << 3).reserved_write);
        assert!(PageFaultCause::decode(1 << 4).instruction_fetch);
        // Bits above the architectural five are ignored, not misread.
        assert_eq!(PageFaultCause::decode(0xFFFF_FFFF_FFFF_FFE0), PageFaultCause::decode(0));
    }

    #[test]
    fn a_zero_general_protection_error_code_means_no_selector_was_involved() {
        // The shape the fixture's `wrmsr` to a reserved MSR actually produces:
        // reporting it as "GDT index 0" would be a confident fiction.
        let error = SelectorError::decode(0);
        assert!(error.no_selector);
        assert_eq!(error.index, 0);
        assert!(!error.external);
    }

    #[test]
    fn a_selector_error_code_decodes_its_table_and_index() {
        // Index 4 in the GDT: (4 << 3) = 0x20.
        let error = SelectorError::decode(0x20);
        assert!(!error.no_selector);
        assert_eq!(error.table, DescriptorTable::Gdt);
        assert_eq!(error.index, 4);
        // Table bits 01 and 11 both mean the IDT.
        assert_eq!(SelectorError::decode(0b010).table, DescriptorTable::Idt);
        assert_eq!(SelectorError::decode(0b110).table, DescriptorTable::Idt);
        // Table bits 10 mean the LDT.
        assert_eq!(SelectorError::decode(0b100).table, DescriptorTable::Ldt);
        // The external bit is bit 0.
        assert!(SelectorError::decode(0x21).external);
    }

    #[test]
    fn decoders_are_reported_per_vector_and_never_cross_wired() {
        let page_fault = FaultFrame {
            cr2: 0x4000,
            vector: 14,
            error_code: 0b0111,
            rip: 0,
            cs: 0,
            rflags: 0,
            rsp: 0,
            ss: 0,
        };
        assert!(page_fault.page_fault_cause().is_some());
        assert!(page_fault.selector_error().is_none());

        let protection = FaultFrame { vector: 13, ..page_fault };
        assert!(protection.selector_error().is_some());
        assert!(protection.page_fault_cause().is_none());

        let invalid_opcode = FaultFrame { vector: 6, ..page_fault };
        assert!(invalid_opcode.selector_error().is_none());
        assert!(invalid_opcode.page_fault_cause().is_none());
    }

    #[test]
    fn mnemonics_are_the_ones_a_reader_expects() {
        assert_eq!(FaultVector::InvalidOpcode.mnemonic(), "#UD");
        assert_eq!(FaultVector::GeneralProtection.mnemonic(), "#GP");
        assert_eq!(FaultVector::PageFault.mnemonic(), "#PF");
    }
}
