//! The x87/MMX/SSE register state an interrupt must preserve
//! (`STORY-P1-04-01`, closing `LE-14`).
//!
//! **The gap this closes.** `kernel::context::switch` saves callee-saved
//! *integer* registers and flags, and nothing else. That is sound for a
//! *cooperative* switch: the SysV AMD64 ABI makes every XMM register
//! caller-saved, so a task that reaches `switch` by calling it has already
//! spilled anything it still needed. A timer interrupt makes no such
//! promise. It can suspend a task between two halves of an SSE computation,
//! and `crate::boot` deliberately enables SSE (ADR 0003), so preemption
//! without this is **silent data corruption in the preempted task, not a
//! fault** — the resumed task simply reads whatever the preempting task left
//! in `XMM0`.
//!
//! **Where the save actually happens, and why it is not where it was first
//! written.** The obvious placement is around the context switch on the
//! preemption path: save when a tick decides to preempt, restore when the
//! task is resumed. That was implemented first and it is **wrong**, and the
//! Tier 0 fixture caught it on its first run — the low-priority task read
//! back `0x124df8`, which is neither its own pattern nor the preempting
//! task's. The reason is that an interrupt handler is *itself* ordinary
//! compiled code running on the interrupted task's stack: it may use SSE
//! registers whether or not it goes on to preempt anything. Guarding only
//! the preempting ticks leaves every other tick free to corrupt the task it
//! interrupted.
//!
//! So the save/restore lives in `crate::interrupts`' ISR **stub**, around
//! the entire handler call, with the area carved out of the interrupted
//! stack. That placement is correct by construction rather than by an
//! argument about what the handler happens to compile to: nothing Rust can
//! emit runs before the `fxsave` or after the `fxrstor`. It also composes
//! with a context switch taken inside the handler for free — the saved area
//! lives on the *task's own* stack, so it travels with the task and is
//! restored when the task is resumed, however much later that is.
//!
//! This module is not gated to `not(target_os = "windows")`: `FXSAVE`/
//! `FXRSTOR` are ordinary user-mode x86-64 instructions with no
//! ELF-specific content, so its tests run on a Windows development machine
//! too — the same carve-out [`crate::tsc`] already has, and the reason the
//! *mechanism* half of `LE-14` is provable without QEMU at all.

/// Byte size of the `FXSAVE`/`FXRSTOR` area — architecturally fixed (Intel
/// SDM Vol 1 §10.5.1), not a tunable capacity.
///
/// `crate::interrupts`' ISR stub reserves exactly this much on the
/// interrupted stack, through a `const` operand rather than a second literal,
/// so the reservation and this type can never disagree.
pub const EXTENDED_STATE_BYTES: usize = 512;

/// Offset of the x87 control word (`FCW`) within an `FXSAVE` area.
const FCW_OFFSET: usize = 0;
/// Offset of `MXCSR` within an `FXSAVE` area.
const MXCSR_OFFSET: usize = 24;
/// Offset of `XMM0` within an `FXSAVE` area (`XMM0`-`XMM15` occupy
/// `160..416`, 16 bytes each).
const XMM0_OFFSET: usize = 160;

/// The x87 control word after `FINIT`: all exceptions masked, extended
/// precision, round-to-nearest.
const FCW_DEFAULT: u16 = 0x037F;
/// `MXCSR` at reset: all six SIMD exceptions masked, round-to-nearest,
/// flush-to-zero and denormals-are-zero off. **Not** zero — see
/// [`ExtendedState::new`].
const MXCSR_DEFAULT: u32 = 0x1F80;

/// A 512-byte `FXSAVE` area.
///
/// `#[repr(C, align(16))]` is a correctness requirement, not hygiene:
/// `FXSAVE` against a misaligned address raises `#GP`. Settling it in the
/// type is what stops a caller placing one at an arbitrary offset inside
/// some larger struct.
#[repr(C, align(16))]
#[derive(Clone, Copy)]
pub struct ExtendedState {
    bytes: [u8; EXTENDED_STATE_BYTES],
}

impl ExtendedState {
    /// An area holding the architectural power-on defaults — **not** a zeroed
    /// one.
    ///
    /// The distinction is load-bearing. A zeroed area has `MXCSR == 0`, a
    /// perfectly legal encoding whose meaning is "unmask every SIMD
    /// floating-point exception": restoring it would arm `#XM` on the next
    /// inexact result anywhere in the system, at a point arbitrarily far from
    /// whatever caused it. This constructor writes `FCW == 0x037F` and
    /// `MXCSR == 0x1F80`, so an area that has never been saved into is still
    /// something the CPU can safely be handed.
    pub const fn new() -> Self {
        let mut bytes = [0u8; EXTENDED_STATE_BYTES];
        let fcw = FCW_DEFAULT.to_le_bytes();
        bytes[FCW_OFFSET] = fcw[0];
        bytes[FCW_OFFSET + 1] = fcw[1];
        let mxcsr = MXCSR_DEFAULT.to_le_bytes();
        bytes[MXCSR_OFFSET] = mxcsr[0];
        bytes[MXCSR_OFFSET + 1] = mxcsr[1];
        bytes[MXCSR_OFFSET + 2] = mxcsr[2];
        bytes[MXCSR_OFFSET + 3] = mxcsr[3];
        ExtendedState { bytes }
    }

    /// Saves the CPU's current x87/MMX/SSE state into this area (`FXSAVE`).
    ///
    /// Bounded and allocation-free — one instruction — as
    /// `agent/CODING_STANDARDS.md`'s real-time discipline requires of
    /// anything reachable from an interrupt path.
    ///
    /// # Safety
    /// `CR4.OSFXSR` must be set (`crate::boot` sets it before calling any
    /// Rust code) and `CR0.EM` clear, or the instruction raises `#UD`.
    pub unsafe fn save(&mut self) {
        // SAFETY: `self.bytes` is a live, exclusively-borrowed 512-byte
        // buffer whose type guarantees 16-byte alignment; `FXSAVE` writes
        // exactly that region and touches nothing else.
        unsafe {
            core::arch::asm!(
                "fxsave [{area}]",
                area = in(reg) self.bytes.as_mut_ptr(),
                options(nostack, preserves_flags),
            );
        }
    }

    /// Loads this area back into the CPU's x87/MMX/SSE state (`FXRSTOR`).
    ///
    /// # Safety
    /// [`save`](Self::save)'s contract, plus: this area must hold either
    /// state a previous `save` wrote or the defaults [`new`](Self::new)
    /// seeds. `FXRSTOR` from arbitrary bytes can install a reserved `MXCSR`
    /// encoding, which is itself `#GP`.
    pub unsafe fn restore(&self) {
        // SAFETY: as `save` — a live, 16-byte-aligned 512-byte region, read
        // only, holding state this type's own constructors produced.
        unsafe {
            core::arch::asm!(
                "fxrstor [{area}]",
                area = in(reg) self.bytes.as_ptr(),
                options(nostack, readonly, preserves_flags),
            );
        }
    }

    /// The low eight bytes this area holds for `XMM0` — the read the tests
    /// use to prove a save actually captured the register rather than merely
    /// running.
    pub fn xmm0_low(&self) -> u64 {
        let mut value = [0u8; 8];
        value.copy_from_slice(&self.bytes[XMM0_OFFSET..XMM0_OFFSET + 8]);
        u64::from_le_bytes(value)
    }

    /// This area's `MXCSR` field — see [`new`](Self::new) for why a caller
    /// ever needs to check it.
    pub fn mxcsr(&self) -> u32 {
        let mut value = [0u8; 4];
        value.copy_from_slice(&self.bytes[MXCSR_OFFSET..MXCSR_OFFSET + 4]);
        u32::from_le_bytes(value)
    }

    /// This area's x87 control word.
    pub fn fcw(&self) -> u16 {
        let mut value = [0u8; 2];
        value.copy_from_slice(&self.bytes[FCW_OFFSET..FCW_OFFSET + 2]);
        u16::from_le_bytes(value)
    }
}

impl Default for ExtendedState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pattern with a bit set in every byte lane, so a save/restore that
    /// truncated or byte-swapped is caught rather than coincidentally
    /// matching.
    const XMM_PATTERN: u64 = 0x0123_4567_89ab_cdef;
    /// What the "other task" writes over it.
    const XMM_CLOBBER: u64 = 0xfedc_ba98_7654_3210;

    // `TEST-P1-04-01-A` clause 2: `FXSAVE` against a misaligned address is
    // `#GP` and the area's size is architecturally fixed — both are
    // correctness requirements this type is supposed to settle, so both are
    // asserted rather than assumed from the field declaration.
    #[test]
    fn the_area_is_exactly_512_bytes_and_16_byte_aligned() {
        assert_eq!(core::mem::size_of::<ExtendedState>(), EXTENDED_STATE_BYTES);
        assert_eq!(core::mem::align_of::<ExtendedState>(), 16);
        let state = ExtendedState::new();
        assert_eq!((&state as *const ExtendedState as usize) % 16, 0);
    }

    #[test]
    fn a_fresh_area_holds_the_architectural_defaults_not_zeros() {
        let state = ExtendedState::new();
        assert_eq!(state.mxcsr(), MXCSR_DEFAULT, "a zeroed MXCSR unmasks every SIMD exception");
        assert_eq!(state.fcw(), FCW_DEFAULT);
        assert_ne!(state.mxcsr(), 0);
    }

    // The mechanism itself, in a single asm block so no compiler decision
    // about register liveness can stand between the write and the read back:
    // load XMM0, save, clobber XMM0 with a different value, restore, read
    // XMM0. Without the `fxrstor` this reads back `XMM_CLOBBER`.
    #[test]
    fn fxsave_fxrstor_round_trips_xmm0_across_a_deliberate_clobber() {
        let mut state = ExtendedState::new();
        let read_back: u64;
        // SAFETY: `state.bytes` is a live, 16-byte-aligned 512-byte buffer
        // exclusively borrowed for this block; every instruction here either
        // touches that buffer or `XMM0`, which is declared clobbered.
        unsafe {
            core::arch::asm!(
                "movq xmm0, {pattern}",
                "fxsave [{area}]",
                "movq xmm0, {clobber}",
                "fxrstor [{area}]",
                "movq {out}, xmm0",
                pattern = in(reg) XMM_PATTERN,
                clobber = in(reg) XMM_CLOBBER,
                area = in(reg) state.bytes.as_mut_ptr(),
                out = out(reg) read_back,
                out("xmm0") _,
                options(nostack),
            );
        }
        assert_eq!(read_back, XMM_PATTERN, "fxrstor did not restore XMM0");
        // And the saved area really holds the pattern, so the round trip
        // cannot have been satisfied by XMM0 simply never being written.
        assert_eq!(state.xmm0_low(), XMM_PATTERN);
    }

    #[test]
    fn save_captures_the_live_xmm0_into_this_areas_own_storage() {
        let mut state = ExtendedState::new();
        assert_eq!(state.xmm0_low(), 0, "a fresh area has no XMM contents to be confused with");
        // SAFETY: writes only `XMM0`, which is declared clobbered; then a
        // `FXSAVE` whose `CR4.OSFXSR` precondition holds on any host running
        // this suite (every mainstream OS sets it at boot) and on the real
        // target (`crate::boot`).
        unsafe {
            core::arch::asm!(
                "movq xmm0, {pattern}",
                pattern = in(reg) XMM_PATTERN,
                out("xmm0") _,
                options(nostack, nomem, preserves_flags),
            );
            state.save();
        }
        assert_eq!(state.xmm0_low(), XMM_PATTERN);
    }

    #[test]
    fn restore_loads_this_areas_stored_xmm0_back_into_the_register() {
        let mut source = ExtendedState::new();
        // SAFETY: as above.
        unsafe {
            core::arch::asm!(
                "movq xmm0, {pattern}",
                pattern = in(reg) XMM_PATTERN,
                out("xmm0") _,
                options(nostack, nomem, preserves_flags),
            );
            source.save();
        }

        let read_back: u64;
        // SAFETY: `source` holds state a real `fxsave` wrote, so `FXRSTOR`'s
        // contract is met; `XMM0` is declared clobbered.
        unsafe {
            core::arch::asm!(
                "movq xmm0, {clobber}",
                clobber = in(reg) XMM_CLOBBER,
                out("xmm0") _,
                options(nostack, nomem, preserves_flags),
            );
            source.restore();
            core::arch::asm!(
                "movq {out}, xmm0",
                out = out(reg) read_back,
                options(nostack, nomem, preserves_flags),
            );
        }
        assert_eq!(read_back, XMM_PATTERN, "restore did not put the saved XMM0 back");
    }
}
