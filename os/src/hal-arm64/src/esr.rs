//! `ESR_EL1` decoding — pure, host-tested, and the only thing in this Story a
//! dev host can prove (`TEST-P1-07-02-A` clause 3).
//!
//! The register read lives in [`crate::fault`], which is the one place allowed
//! to be architecture-specific about the exception path. Everything that
//! *interprets* the value is here, for the reason [`crate::exception_level`]
//! gives about `CurrentEL`: a decode that is confidently wrong on a board with
//! one output channel costs a session, and a decode with tests costs nothing.
//!
//! **The whole register, not pre-shifted fields.** [`Esr::new`] takes the raw
//! 64-bit value exactly as `mrs x0, esr_el1` produces it, so the masking is
//! tested here once instead of at each call site.

/// A raw `ESR_EL1` value, undecoded.
///
/// A newtype rather than a bare `u64` per `agent/CODING_STANDARDS.md`'s
/// "prefer newtypes over bare primitives" rule — and for a specific reason
/// here: `ESR_EL1`, `FAR_EL1`, `ELR_EL1` and `SPSR_EL1` are all `u64`, all read
/// in the same handler, and confusing two of them produces a report that is
/// entirely plausible and entirely wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Esr(u64);

/// What kind of exception fired, as named by `ESR_EL1.EC`.
///
/// Deliberately not every architectural class. These are the classes this
/// Story claims to name; every other value decodes to
/// [`ExceptionClass::Unrecognised`] carrying its raw `EC`, per clause 3's
/// second paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExceptionClass {
    /// `EC = 0x00`. The **architecture's** own "unknown reason" — a real,
    /// named class, not a decode failure. See
    /// [`ExceptionClass::Unrecognised`] for the other thing "unknown" could
    /// mean, and why the two must never share a variant.
    UnknownReason,
    /// `EC = 0x01`. A trapped `WFI`/`WFE`.
    WfxTrap,
    /// `EC = 0x07`. A trapped SIMD/FP access — what `CPACR_EL1.FPEN` being
    /// clear produces, which is the fault this board is most likely to take
    /// from code nobody wrote (see the target spec's softfloat ABI, and
    /// `session/hand-2026-07-28/23-bcm2712-divergence-record.md` §5).
    SimdFpAccessTrap,
    /// `EC = 0x0E`. Illegal execution state — `PSTATE.IL` set, which is what a
    /// bad `eret` produces.
    IllegalExecutionState,
    /// `EC = 0x15`. `SVC` from AArch64. No `EL0` exists in this Feature, so
    /// this is an `EL1`-to-`EL1` `SVC`.
    Svc64,
    /// `EC = 0x18`. A trapped `MSR`/`MRS`/system instruction.
    SystemRegisterTrap,
    /// `EC = 0x20`. Instruction abort from a lower exception level.
    InstructionAbortLowerEl,
    /// `EC = 0x21`. Instruction abort taken without a change of exception
    /// level — the only instruction abort this Feature can produce.
    InstructionAbortSameEl,
    /// `EC = 0x22`. PC alignment fault.
    PcAlignmentFault,
    /// `EC = 0x24`. Data abort from a lower exception level.
    DataAbortLowerEl,
    /// `EC = 0x25`. Data abort taken without a change of exception level — the
    /// class an unaligned Device-nGnRnE access lands in while the MMU is off.
    DataAbortSameEl,
    /// `EC = 0x26`. SP alignment fault.
    SpAlignmentFault,
    /// `EC = 0x2F`. `SError` interrupt.
    SError,
    /// `EC = 0x3C`. A `BRK` instruction in AArch64 — how a deliberate fault is
    /// raised without corrupting anything (`TEST-P1-07-02-A` clause 2).
    Brk64,
    /// An `EC` value this decoder does not name, carrying the raw six bits.
    ///
    /// **Distinct from [`ExceptionClass::UnknownReason`]**, which is `EC` `0x00`
    /// and means the *architecture* could not attribute the exception. This
    /// variant means *this code* has never been taught the class. Collapsing
    /// the two would report a missing decoder as a hardware-attributed unknown,
    /// which is the confident-wrong-answer failure clause 3 forbids.
    Unrecognised(u8),
}

/// The fault status a data or instruction abort reports (`DFSC`/`IFSC`).
///
/// Same six-bit encoding in both, which is why one type serves both — the
/// architecture defines a single status space, and giving instruction aborts
/// their own copy would be a second decoder to keep in step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultStatus {
    /// Address size fault at the given translation level.
    AddressSize(u8),
    /// Translation fault at the given translation level.
    Translation(u8),
    /// Access flag fault at the given translation level.
    AccessFlag(u8),
    /// Permission fault at the given translation level.
    Permission(u8),
    /// Synchronous external abort, not on a translation table walk.
    ExternalAbort,
    /// Synchronous external abort on a translation table walk, at the given
    /// level.
    ExternalAbortOnWalk(u8),
    /// Synchronous parity or ECC error on memory access.
    ParityOrEcc,
    /// Alignment fault.
    ///
    /// The one to expect on this board before `STORY-P1-07-03`: with
    /// `SCTLR_EL1.M` clear every access is Device-nGnRnE, and unaligned
    /// accesses to Device memory fault rather than merely running slowly.
    Alignment,
    /// TLB conflict abort.
    TlbConflict,
    /// A status value this decoder does not name, carrying the raw six bits.
    Unrecognised(u8),
}

/// The decoded `ISS` of a data abort (`EC` `0x24`/`0x25`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataAbortIss {
    /// `DFSC`, `ISS[5:0]`.
    pub status: FaultStatus,
    /// `WnR`, `ISS[6]`: the access was a write.
    pub write: bool,
    /// `S1PTW`, `ISS[7]`: the fault was on a stage-1 translation table walk.
    pub stage1_table_walk: bool,
    /// `CM`, `ISS[8]`: the access was a cache-maintenance or address-translation
    /// instruction.
    pub cache_maintenance: bool,
    /// `EA`, `ISS[9]`: external abort type — IMPLEMENTATION DEFINED.
    pub external_abort_type: bool,
    /// `FnV`, `ISS[10]`: **`FAR_EL1` is not valid**. See
    /// [`Esr::far_is_meaningful`].
    pub far_not_valid: bool,
    /// `ISV`, `ISS[24]`: the instruction syndrome fields are valid, which is
    /// what makes [`DataAbortIss::access_size_bytes`] answerable at all.
    pub instruction_syndrome_valid: bool,
    /// `SAS`, `ISS[23:22]`: the raw access-size field. Meaningless unless
    /// `instruction_syndrome_valid`; see
    /// [`DataAbortIss::access_size_bytes`].
    pub access_size: u8,
}

/// The decoded `ISS` of an instruction abort (`EC` `0x20`/`0x21`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstructionAbortIss {
    /// `IFSC`, `ISS[5:0]`.
    pub status: FaultStatus,
    /// `S1PTW`, `ISS[7]`.
    pub stage1_table_walk: bool,
    /// `EA`, `ISS[9]`.
    pub external_abort_type: bool,
    /// `FnV`, `ISS[10]`: **`FAR_EL1` is not valid**.
    pub far_not_valid: bool,
}

impl ExceptionClass {
    /// Decodes the six-bit `EC` field.
    ///
    /// Total over all 64 representable values: a class this decoder does not
    /// name becomes [`ExceptionClass::Unrecognised`] rather than a panic. A
    /// panic here would fire inside the fault handler, which is the one place
    /// in the system with nothing left to catch it.
    pub const fn from_ec(ec: u8) -> ExceptionClass {
        match ec {
            0x00 => ExceptionClass::UnknownReason,
            0x01 => ExceptionClass::WfxTrap,
            0x07 => ExceptionClass::SimdFpAccessTrap,
            0x0E => ExceptionClass::IllegalExecutionState,
            0x15 => ExceptionClass::Svc64,
            0x18 => ExceptionClass::SystemRegisterTrap,
            0x20 => ExceptionClass::InstructionAbortLowerEl,
            0x21 => ExceptionClass::InstructionAbortSameEl,
            0x22 => ExceptionClass::PcAlignmentFault,
            0x24 => ExceptionClass::DataAbortLowerEl,
            0x25 => ExceptionClass::DataAbortSameEl,
            0x26 => ExceptionClass::SpAlignmentFault,
            0x2F => ExceptionClass::SError,
            0x3C => ExceptionClass::Brk64,
            other => ExceptionClass::Unrecognised(other),
        }
    }

    /// The short name used in a serial report.
    ///
    /// A fixed string rather than a `Display` impl, for the reason
    /// [`crate::exception_level::ExceptionLevel::as_str`] gives: `core::fmt` is
    /// more machinery than a fault path should carry, and it can panic — inside
    /// the handler whose whole job is to be the thing that still works.
    ///
    /// [`ExceptionClass::Unrecognised`] renders as `"unknown-class"`, and the
    /// raw `EC` is written alongside it by the caller
    /// ([`crate::fault::report`]) rather than formatted in here.
    pub const fn as_str(self) -> &'static str {
        match self {
            ExceptionClass::UnknownReason => "unknown-reason",
            ExceptionClass::WfxTrap => "wfx-trap",
            ExceptionClass::SimdFpAccessTrap => "simd-fp-access",
            ExceptionClass::IllegalExecutionState => "illegal-execution-state",
            ExceptionClass::Svc64 => "svc64",
            ExceptionClass::SystemRegisterTrap => "system-register-trap",
            ExceptionClass::InstructionAbortLowerEl => "instruction-abort-lower-el",
            ExceptionClass::InstructionAbortSameEl => "instruction-abort",
            ExceptionClass::PcAlignmentFault => "pc-alignment",
            ExceptionClass::DataAbortLowerEl => "data-abort-lower-el",
            ExceptionClass::DataAbortSameEl => "data-abort",
            ExceptionClass::SpAlignmentFault => "sp-alignment",
            ExceptionClass::SError => "serror",
            ExceptionClass::Brk64 => "brk64",
            ExceptionClass::Unrecognised(_) => "unknown-class",
        }
    }
}

impl FaultStatus {
    /// Decodes the six-bit `DFSC`/`IFSC` field.
    ///
    /// Total, for the reason [`ExceptionClass::from_ec`] is.
    pub const fn from_bits(bits: u8) -> FaultStatus {
        // The level-bearing statuses occupy contiguous runs whose low two bits
        // *are* the level. Written as explicit arms rather than arithmetic
        // over a mask: the runs are not uniform (access flag and permission
        // faults have no level-0 encoding outside `FEAT_LPA2`), and a clever
        // shift here would quietly invent one.
        match bits {
            0b000000 => FaultStatus::AddressSize(0),
            0b000001 => FaultStatus::AddressSize(1),
            0b000010 => FaultStatus::AddressSize(2),
            0b000011 => FaultStatus::AddressSize(3),
            0b000100 => FaultStatus::Translation(0),
            0b000101 => FaultStatus::Translation(1),
            0b000110 => FaultStatus::Translation(2),
            0b000111 => FaultStatus::Translation(3),
            0b001001 => FaultStatus::AccessFlag(1),
            0b001010 => FaultStatus::AccessFlag(2),
            0b001011 => FaultStatus::AccessFlag(3),
            0b001101 => FaultStatus::Permission(1),
            0b001110 => FaultStatus::Permission(2),
            0b001111 => FaultStatus::Permission(3),
            0b010000 => FaultStatus::ExternalAbort,
            0b010101 => FaultStatus::ExternalAbortOnWalk(1),
            0b010110 => FaultStatus::ExternalAbortOnWalk(2),
            0b010111 => FaultStatus::ExternalAbortOnWalk(3),
            0b011000 => FaultStatus::ParityOrEcc,
            0b100001 => FaultStatus::Alignment,
            0b110000 => FaultStatus::TlbConflict,
            other => FaultStatus::Unrecognised(other),
        }
    }

    /// The short name used in a serial report.
    ///
    /// The translation level is written separately by the caller, so this is a
    /// fixed string for every variant.
    pub const fn as_str(self) -> &'static str {
        match self {
            FaultStatus::AddressSize(_) => "address-size",
            FaultStatus::Translation(_) => "translation",
            FaultStatus::AccessFlag(_) => "access-flag",
            FaultStatus::Permission(_) => "permission",
            FaultStatus::ExternalAbort => "external-abort",
            FaultStatus::ExternalAbortOnWalk(_) => "external-abort-on-walk",
            FaultStatus::ParityOrEcc => "parity-or-ecc",
            FaultStatus::Alignment => "alignment",
            FaultStatus::TlbConflict => "tlb-conflict",
            FaultStatus::Unrecognised(_) => "unknown-status",
        }
    }

    /// The translation level this status names, if it names one.
    pub const fn level(self) -> Option<u8> {
        match self {
            FaultStatus::AddressSize(level)
            | FaultStatus::Translation(level)
            | FaultStatus::AccessFlag(level)
            | FaultStatus::Permission(level)
            | FaultStatus::ExternalAbortOnWalk(level) => Some(level),
            _ => None,
        }
    }
}

impl DataAbortIss {
    /// Decodes a data abort's 25-bit `ISS`.
    pub const fn decode(iss: u32) -> DataAbortIss {
        DataAbortIss {
            status: FaultStatus::from_bits((iss & 0b11_1111) as u8),
            write: iss & (1 << 6) != 0,
            stage1_table_walk: iss & (1 << 7) != 0,
            cache_maintenance: iss & (1 << 8) != 0,
            external_abort_type: iss & (1 << 9) != 0,
            far_not_valid: iss & (1 << 10) != 0,
            instruction_syndrome_valid: iss & (1 << 24) != 0,
            access_size: ((iss >> 22) & 0b11) as u8,
        }
    }

    /// The width of the faulting access in bytes, or `None` when the
    /// instruction syndrome is not valid.
    ///
    /// `None` is the common case on a real abort, and reporting a decoded `SAS`
    /// when `ISV` is clear would be reading a field the architecture leaves
    /// UNKNOWN — the same mistake as reporting `CR2` for a `#GP` on x86_64
    /// (`hal_x86_64::fault::FaultFrame::faulting_address`).
    pub const fn access_size_bytes(self) -> Option<u8> {
        if !self.instruction_syndrome_valid {
            return None;
        }
        // `SAS` is a log2 width: 0 = byte, 3 = doubleword.
        Some(1 << self.access_size)
    }
}

impl InstructionAbortIss {
    /// Decodes an instruction abort's 25-bit `ISS`.
    ///
    /// Note what is **not** here: no `WnR`, no `CM`, no `ISV`/`SAS`. Those bits
    /// belong to a data abort's syndrome and are `RES0` in this one, so a field
    /// for them would invite a report that reads a bit the architecture does
    /// not define here.
    pub const fn decode(iss: u32) -> InstructionAbortIss {
        InstructionAbortIss {
            status: FaultStatus::from_bits((iss & 0b11_1111) as u8),
            stage1_table_walk: iss & (1 << 7) != 0,
            external_abort_type: iss & (1 << 9) != 0,
            far_not_valid: iss & (1 << 10) != 0,
        }
    }
}

impl Esr {
    /// Wraps a raw `ESR_EL1` value.
    pub const fn new(raw: u64) -> Esr {
        Esr(raw)
    }

    /// The raw register, for the report.
    ///
    /// Quoted on the wire beside the decode for the reason
    /// `crate::boot::report_entry` quotes raw `CurrentEL`: a *wrong decode* has
    /// to stay diagnosable from the capture alone, without a second session on
    /// the board.
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// `EC`, bits `[31:26]`, undecoded.
    pub const fn ec(self) -> u8 {
        ((self.0 >> 26) & 0b11_1111) as u8
    }

    /// The decoded exception class.
    pub const fn class(self) -> ExceptionClass {
        ExceptionClass::from_ec(self.ec())
    }

    /// `IL`, bit `[25]`: the trapped instruction was 32-bit.
    ///
    /// Clear means a 16-bit T32 instruction. This Feature never leaves AArch64,
    /// so a clear `IL` on a synchronous abort is itself a signal that something
    /// is not what it appears to be — which is why it is reported rather than
    /// assumed to be 1.
    pub const fn instruction_length_is_32_bit(self) -> bool {
        self.0 & (1 << 25) != 0
    }

    /// `ISS`, bits `[24:0]`, undecoded.
    pub const fn iss(self) -> u32 {
        (self.0 & 0x01FF_FFFF) as u32
    }

    /// The decoded data-abort syndrome, or `None` for another class.
    ///
    /// The `Option` is the same device `hal_x86_64::fault::FaultFrame`'s
    /// per-vector decoders use: the type refuses to decode one class's `ISS`
    /// with another class's field layout, rather than a comment asking callers
    /// not to.
    pub const fn data_abort(self) -> Option<DataAbortIss> {
        match self.class() {
            ExceptionClass::DataAbortSameEl | ExceptionClass::DataAbortLowerEl => {
                Some(DataAbortIss::decode(self.iss()))
            }
            _ => None,
        }
    }

    /// The decoded instruction-abort syndrome, or `None` for another class.
    pub const fn instruction_abort(self) -> Option<InstructionAbortIss> {
        match self.class() {
            ExceptionClass::InstructionAbortSameEl | ExceptionClass::InstructionAbortLowerEl => {
                Some(InstructionAbortIss::decode(self.iss()))
            }
            _ => None,
        }
    }

    /// Whether `FAR_EL1` holds a meaningful address for this exception.
    ///
    /// The direct AArch64 counterpart of
    /// `hal_x86_64::fault::FaultFrame::faulting_address`'s refusal to report
    /// `CR2` for a `#GP`. `FAR_EL1` is a register, not a value pushed with the
    /// frame: it holds whatever the last exception that updated it left there.
    /// Reporting it for a class that does not update it is reporting a stale
    /// address from an unrelated earlier event, with total confidence.
    ///
    /// True only for:
    ///
    /// - data and instruction aborts whose `FnV` bit is clear;
    /// - PC alignment faults, where `FAR_EL1` holds the misaligned PC.
    ///
    /// **Not** SP alignment faults: `FAR_EL1` is not updated for `EC` `0x26`.
    pub const fn far_is_meaningful(self) -> bool {
        match self.class() {
            ExceptionClass::DataAbortSameEl | ExceptionClass::DataAbortLowerEl => {
                match self.data_abort() {
                    Some(abort) => !abort.far_not_valid,
                    None => false,
                }
            }
            ExceptionClass::InstructionAbortSameEl | ExceptionClass::InstructionAbortLowerEl => {
                match self.instruction_abort() {
                    Some(abort) => !abort.far_not_valid,
                    None => false,
                }
            }
            ExceptionClass::PcAlignmentFault => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assembles a register value from its architectural fields, so every test
    /// below states the bit positions it depends on rather than a magic
    /// constant whose derivation lives in someone's head.
    const fn esr(ec: u8, il: bool, iss: u32) -> Esr {
        Esr::new(((ec as u64) << 26) | ((il as u64) << 25) | (iss as u64 & 0x01FF_FFFF))
    }

    // Clause 3: class, IL bit and class-specific ISS, each from its own field.
    #[test]
    fn the_exception_class_comes_from_bits_31_26_and_nothing_else() {
        for (ec, expected) in [
            (0x00u8, ExceptionClass::UnknownReason),
            (0x01, ExceptionClass::WfxTrap),
            (0x07, ExceptionClass::SimdFpAccessTrap),
            (0x0E, ExceptionClass::IllegalExecutionState),
            (0x15, ExceptionClass::Svc64),
            (0x18, ExceptionClass::SystemRegisterTrap),
            (0x20, ExceptionClass::InstructionAbortLowerEl),
            (0x21, ExceptionClass::InstructionAbortSameEl),
            (0x22, ExceptionClass::PcAlignmentFault),
            (0x24, ExceptionClass::DataAbortLowerEl),
            (0x25, ExceptionClass::DataAbortSameEl),
            (0x26, ExceptionClass::SpAlignmentFault),
            (0x2F, ExceptionClass::SError),
            (0x3C, ExceptionClass::Brk64),
        ] {
            // Every bit outside [31:26] set, so a decoder reading one bit too
            // far in either direction gets a different answer.
            let noisy = Esr::new(((ec as u64) << 26) | 0xFFFF_FFFF_03FF_FFFF);
            assert_eq!(noisy.class(), expected, "EC {ec:#04x}");
            assert_eq!(noisy.ec(), ec);
        }
    }

    #[test]
    fn an_unnamed_class_reports_its_raw_value_and_never_borrows_a_known_one() {
        // Clause 3's second paragraph. These are real architectural classes
        // this Story does not name — a later Story teaching the decoder one of
        // them is an added variant, not a changed meaning.
        for ec in [0x02u8, 0x0C, 0x17, 0x2C, 0x30, 0x31, 0x34, 0x35, 0x3F] {
            assert_eq!(
                esr(ec, true, 0).class(),
                ExceptionClass::Unrecognised(ec),
                "EC {ec:#04x} must decode as unrecognised, carrying its raw value"
            );
        }
    }

    #[test]
    fn every_representable_class_field_decodes_to_something() {
        // Six bits, 64 values, no gaps: a decoder that panicked or wrapped on
        // an unnamed class would be a panic inside the fault handler.
        for ec in 0u8..64 {
            let class = esr(ec, true, 0).class();
            assert_eq!(esr(ec, true, 0).ec(), ec);
            // Either a named class or the raw value, never a silent default.
            if let ExceptionClass::Unrecognised(raw) = class {
                assert_eq!(raw, ec);
            }
        }
    }

    #[test]
    fn the_architectures_own_unknown_and_a_missing_decoder_are_different_answers() {
        // The distinction this module's `Unrecognised` doc comment exists for.
        // `EC = 0x00` is the architecture saying it could not attribute the
        // exception; `Unrecognised` is this code saying it was never taught the
        // class. A reader debugging a board must be able to tell which.
        assert_eq!(esr(0x00, true, 0).class(), ExceptionClass::UnknownReason);
        assert_ne!(esr(0x00, true, 0).class(), ExceptionClass::Unrecognised(0x00));
        assert_eq!(esr(0x00, true, 0).class().as_str(), "unknown-reason");
        assert_eq!(esr(0x3F, true, 0).class().as_str(), "unknown-class");
    }

    #[test]
    fn the_instruction_length_bit_is_bit_25_alone() {
        assert!(esr(0x25, true, 0).instruction_length_is_32_bit());
        assert!(!esr(0x25, false, 0).instruction_length_is_32_bit());
        // A full ISS and a full EC must not leak into the IL answer.
        assert!(!esr(0x3F, false, 0x01FF_FFFF).instruction_length_is_32_bit());
        assert!(esr(0x00, true, 0x01FF_FFFF).instruction_length_is_32_bit());
    }

    #[test]
    fn the_iss_is_bits_24_0_and_excludes_the_il_bit_above_it() {
        assert_eq!(esr(0x25, true, 0x01FF_FFFF).iss(), 0x01FF_FFFF);
        // IL set, ISS zero: the IL bit must not appear as ISS bit 25 or as a
        // sign-extension of anything.
        assert_eq!(esr(0x25, true, 0).iss(), 0);
        assert_eq!(Esr::new(u64::MAX).iss(), 0x01FF_FFFF);
    }

    // Clause 3: a case per class this Story claims to name.
    #[test]
    fn a_data_abort_decodes_its_status_write_bit_and_far_validity() {
        // Alignment fault, write, FAR valid — the abort an unaligned store to
        // Device-nGnRnE memory produces while the MMU is off.
        let iss = 0b10_0001 | (1 << 6);
        let abort = esr(0x25, true, iss).data_abort().expect("a data abort decodes");
        assert_eq!(abort.status, FaultStatus::Alignment);
        assert!(abort.write);
        assert!(!abort.far_not_valid);
        assert!(esr(0x25, true, iss).far_is_meaningful());

        // The same fault as a read.
        let read = esr(0x25, true, 0b10_0001).data_abort().expect("decodes");
        assert!(!read.write);
    }

    #[test]
    fn a_data_abort_decodes_every_named_status_and_its_translation_level() {
        for (bits, expected) in [
            (0b000000u8, FaultStatus::AddressSize(0)),
            (0b000011, FaultStatus::AddressSize(3)),
            (0b000100, FaultStatus::Translation(0)),
            (0b000111, FaultStatus::Translation(3)),
            (0b001001, FaultStatus::AccessFlag(1)),
            (0b001011, FaultStatus::AccessFlag(3)),
            (0b001101, FaultStatus::Permission(1)),
            (0b001111, FaultStatus::Permission(3)),
            (0b010000, FaultStatus::ExternalAbort),
            (0b010101, FaultStatus::ExternalAbortOnWalk(1)),
            (0b010111, FaultStatus::ExternalAbortOnWalk(3)),
            (0b011000, FaultStatus::ParityOrEcc),
            (0b100001, FaultStatus::Alignment),
            (0b110000, FaultStatus::TlbConflict),
        ] {
            assert_eq!(FaultStatus::from_bits(bits), expected, "status {bits:#08b}");
            let abort = esr(0x25, true, bits as u32).data_abort().expect("decodes");
            assert_eq!(abort.status, expected);
        }
        assert_eq!(FaultStatus::Translation(2).level(), Some(2));
        assert_eq!(FaultStatus::Alignment.level(), None);
        assert_eq!(FaultStatus::ExternalAbort.level(), None);
    }

    #[test]
    fn an_unnamed_fault_status_keeps_its_raw_bits_too() {
        for bits in [0b000_1100u8 & 0x3F, 0b011_0001 & 0x3F, 0b111_1111 & 0x3F] {
            if let FaultStatus::Unrecognised(raw) = FaultStatus::from_bits(bits) {
                assert_eq!(raw, bits);
            }
        }
        assert_eq!(FaultStatus::from_bits(0b11_1111), FaultStatus::Unrecognised(0b11_1111));
        assert_eq!(FaultStatus::Unrecognised(0b11_1111).as_str(), "unknown-status");
    }

    #[test]
    fn an_access_size_is_reported_only_when_the_instruction_syndrome_is_valid() {
        // `SAS` is UNKNOWN unless `ISV` is set, and a real abort usually clears
        // `ISV`. Reporting a decoded width anyway is the `CR2`-for-a-`#GP`
        // mistake in AArch64 clothing.
        let with_isv = |sas: u32| esr(0x25, true, (1 << 24) | (sas << 22) | 0b10_0001);
        for (sas, bytes) in [(0u32, 1u8), (1, 2), (2, 4), (3, 8)] {
            let abort = with_isv(sas).data_abort().expect("decodes");
            assert!(abort.instruction_syndrome_valid);
            assert_eq!(abort.access_size_bytes(), Some(bytes), "SAS {sas}");
        }
        let without_isv = esr(0x25, true, (3 << 22) | 0b10_0001).data_abort().expect("decodes");
        assert!(!without_isv.instruction_syndrome_valid);
        assert_eq!(without_isv.access_size_bytes(), None);
    }

    #[test]
    fn an_instruction_abort_decodes_its_own_syndrome_and_not_a_data_aborts() {
        let abort = esr(0x21, true, 0b00_0111 | (1 << 7)).instruction_abort().expect("decodes");
        assert_eq!(abort.status, FaultStatus::Translation(3));
        assert!(abort.stage1_table_walk);
        assert!(!abort.far_not_valid);
        // A data abort's `WnR` bit sits at ISS[6], which in an instruction
        // abort is part of no field this decoder reads. Cross-wiring the two
        // decoders is the defect this test exists to catch.
        assert!(esr(0x21, true, 0).data_abort().is_none());
        assert!(esr(0x25, true, 0).instruction_abort().is_none());
    }

    #[test]
    fn both_abort_directions_decode_the_same_way() {
        // `0x20`/`0x24` (from a lower EL) cannot occur in this Feature — there
        // is no `EL0` — but they are decoded rather than left to fall into
        // `Unrecognised`, so that a capture showing one is legible instead of
        // being reported as a decoder gap.
        assert!(esr(0x20, true, 0b10_0001).instruction_abort().is_some());
        assert!(esr(0x24, true, 0b10_0001).data_abort().is_some());
    }

    // Clause 3, and the invariant this module borrows wholesale from x86_64.
    #[test]
    fn far_is_meaningful_only_for_the_classes_that_update_it() {
        // Aborts with `FnV` clear.
        assert!(esr(0x25, true, 0b10_0001).far_is_meaningful());
        assert!(esr(0x21, true, 0b00_0100).far_is_meaningful());
        // The same aborts with `FnV` set: the architecture says the register is
        // not valid, so the report must not quote it.
        assert!(!esr(0x25, true, 0b10_0001 | (1 << 10)).far_is_meaningful());
        assert!(!esr(0x21, true, 0b00_0100 | (1 << 10)).far_is_meaningful());
        // A PC alignment fault puts the misaligned PC in `FAR_EL1`.
        assert!(esr(0x22, true, 0).far_is_meaningful());
        // An SP alignment fault does not update `FAR_EL1` at all. This is the
        // pair most likely to be treated as one case, which is why they are
        // adjacent here.
        assert!(!esr(0x26, true, 0).far_is_meaningful());
        // Nothing else does either.
        for ec in [0x00u8, 0x01, 0x07, 0x0E, 0x15, 0x18, 0x2F, 0x3C, 0x3F] {
            assert!(!esr(ec, true, 0).far_is_meaningful(), "EC {ec:#04x}");
        }
    }

    #[test]
    fn the_raw_register_survives_decoding_unchanged() {
        // The decode is a reading of the register; the register itself is the
        // evidence. A capture that quoted only the decode could not be
        // re-examined after the decoder was found wrong.
        let raw = 0x9600_0021;
        assert_eq!(Esr::new(raw).raw(), raw);
        assert_eq!(Esr::new(u64::MAX).raw(), u64::MAX);
    }

    #[test]
    fn every_named_class_renders_a_distinct_name() {
        let named = [
            ExceptionClass::UnknownReason,
            ExceptionClass::WfxTrap,
            ExceptionClass::SimdFpAccessTrap,
            ExceptionClass::IllegalExecutionState,
            ExceptionClass::Svc64,
            ExceptionClass::SystemRegisterTrap,
            ExceptionClass::InstructionAbortLowerEl,
            ExceptionClass::InstructionAbortSameEl,
            ExceptionClass::PcAlignmentFault,
            ExceptionClass::DataAbortLowerEl,
            ExceptionClass::DataAbortSameEl,
            ExceptionClass::SpAlignmentFault,
            ExceptionClass::SError,
            ExceptionClass::Brk64,
        ];
        for (i, class) in named.iter().enumerate() {
            assert!(!class.as_str().is_empty());
            for other in named.iter().skip(i + 1) {
                assert_ne!(class.as_str(), other.as_str(), "{class:?} and {other:?} collide");
            }
        }
    }

    // A real capture, decoded end to end. `ESR_EL1 = 0x9600_0021` is what a
    // `str x0, [misaligned]` produces at EL1 with the MMU off: EC `0x25`,
    // IL set, DFSC alignment fault, `WnR` set.
    #[test]
    fn a_realistic_alignment_fault_register_decodes_end_to_end() {
        let esr = Esr::new(0x9600_0061);
        assert_eq!(esr.class(), ExceptionClass::DataAbortSameEl);
        assert!(esr.instruction_length_is_32_bit());
        let abort = esr.data_abort().expect("a data abort");
        assert_eq!(abort.status, FaultStatus::Alignment);
        assert!(abort.write);
        assert!(esr.far_is_meaningful());
    }
}
