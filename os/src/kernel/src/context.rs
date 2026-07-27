//! x86_64 context switch (`STORY-P0-02-02`): the standard callee-saved
//! register/stack-pointer swap every cooperatively- or preemptively-scheduled
//! kernel uses (the same technique Linux's `switch_to`/`__switch_to_asm` and
//! every other production x86_64 kernel implement) — not a novel scheme,
//! just this project's compact version of it.
//!
//! A [`Context`] is nothing but a saved stack pointer: the callee-saved
//! registers (`rbp`, `rbx`, `r12`-`r15`) and flags for a suspended task live
//! *on that task's own stack*, pushed there by [`switch`] the moment it
//! suspends and popped back off the moment it resumes. This is why
//! [`Context`] itself needs no register fields — the stack already holds
//! them, exactly as it would for any ordinary (if very long-lived) function
//! call.

/// A task's suspended execution state: the stack pointer at which its
/// callee-saved registers and flags were last pushed by [`switch`], or (for
/// a task that has never run) the pointer [`Context::new`] pre-populated so
/// the *first* [`switch`] into it lands at the task's entry point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Context {
    rsp: u64,
}

/// [`Context::new`] failure: the caller-provided stack is too small to hold
/// even the fixed-size initial register frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextError {
    /// `stack.len()` was smaller than the minimum this module requires.
    StackTooSmall,
}

/// Number of 8-byte slots [`switch`] pushes/pops per context: `rflags`,
/// `r15`, `r14`, `r13`, `r12`, `rbx`, `rbp`, and (for a freshly initialized
/// task) the entry-point return address — see [`context_switch_asm`]'s push
/// order, which [`Context::new`] must lay out identically.
const FRAME_SLOTS: usize = 8;
const FRAME_BYTES: usize = FRAME_SLOTS * 8;

impl Context {
    /// A context with no saved state — only valid as the "prev" side of the
    /// very first [`switch`] call for a given task slot (e.g. the boot/idle
    /// context that calls into the first task), since [`switch`] always
    /// overwrites it before it could otherwise be read.
    pub const fn zeroed() -> Self {
        Context { rsp: 0 }
    }

    /// Builds a fresh [`Context`] for a task that has never run: `stack`'s
    /// top is pre-populated with a register frame such that the first
    /// [`switch`] into this `Context` pops harmless zeroed callee-saved
    /// registers and lands at `entry` exactly as if `entry` had been called
    /// normally (System V AMD64 stack alignment included), never returning.
    ///
    /// `entry` is `extern "C" fn`, matching [`crate::sched::TaskEntry`] — the
    /// same type this Story's dependency, `STORY-P0-02-01`'s `Tcb::entry`,
    /// already stores. `entry` takes no arguments and never returns, so no
    /// argument-passing register convention is actually exercised by the
    /// jump into it; only the entry-time stack alignment matters, which is
    /// identical between "C"'s two possible underlying conventions (System V
    /// on this kernel's real target, Windows x64 on the host `cargo test`
    /// toolchain) — this is why this module's own tests can run entirely on
    /// the host despite `entry` not being pinned to `"sysv64"` the way
    /// [`context_switch_asm`]'s two-argument call is.
    ///
    /// Fails closed with [`ContextError::StackTooSmall`] rather than writing
    /// past `stack`'s bounds if it is too small to hold the initial frame.
    ///
    /// # Safety
    /// `stack` must remain valid, exclusively owned by this task, and
    /// unmoved for as long as any [`Context`] built from it is switched
    /// into — the returned `Context` stores a raw pointer derived from it.
    pub unsafe fn new(stack: &mut [u8], entry: extern "C" fn() -> !) -> Result<Self, ContextError> {
        // 16 bytes of slack covers the alignment adjustment below, so a
        // `stack` just barely big enough for the frame itself is still
        // rejected rather than silently under-allocating.
        if stack.len() < FRAME_BYTES + 16 {
            return Err(ContextError::StackTooSmall);
        }

        // SAFETY: `stack` is a live, exclusively-owned slice per this
        // function's own contract; `add(stack.len())` points one-past-its-end,
        // a valid (non-dereferenced) pointer value per Rust's pointer rules.
        let top = unsafe { stack.as_mut_ptr().add(stack.len()) } as usize;
        let aligned_top = top & !0xF;
        // System V AMD64: at a callee's entry (immediately after `ret` pops
        // the return address), `rsp % 16 == 8`. `frame_base` is the address
        // [`switch`] restores `rsp` to, i.e. the address of the `rflags`
        // slot; the return-address slot sits `FRAME_BYTES - 8` above it, so
        // `frame_base % 16 == 8` is exactly what makes that hold.
        let frame_base = aligned_top - FRAME_BYTES - 8;
        if frame_base < stack.as_ptr() as usize {
            return Err(ContextError::StackTooSmall);
        }

        // SAFETY: `frame_base..frame_base + FRAME_BYTES` was just computed
        // to lie within `stack` (checked above) and every write below is to
        // an 8-byte-aligned offset within it (multiples of 8 from an
        // aligned_top-derived, 16-aligned base), so each `write` is in
        // bounds and correctly aligned for `u64`.
        unsafe {
            let base = frame_base as *mut u64;
            base.write(0x202); // rflags: reserved bit 1 + IF set
            base.add(1).write(0); // r15
            base.add(2).write(0); // r14
            base.add(3).write(0); // r13
            base.add(4).write(0); // r12
            base.add(5).write(0); // rbx
            base.add(6).write(0); // rbp
            base.add(7).write(entry as usize as u64); // return address
        }

        Ok(Context { rsp: frame_base as u64 })
    }
}

// SysV64 is declared explicitly (not `extern "C"`, which follows the host
// OS's default convention — Windows x64 on this dev machine) so the calling
// convention `context_switch_asm` actually implements is the same one on
// every build target, including `cargo test`'s host toolchain.
unsafe extern "sysv64" {
    fn context_switch_asm(prev_rsp: *mut u64, next_rsp: *mut u64);
}

core::arch::global_asm!(
    r#"
    .section .text
    .global context_switch_asm
context_switch_asm:
        push rbp
        push rbx
        push r12
        push r13
        push r14
        push r15
        pushfq

        mov [rdi], rsp
        mov rsp, [rsi]

        popfq
        pop r15
        pop r14
        pop r13
        pop r12
        pop rbx
        pop rbp
        ret
    "#
);

/// Suspends the currently-running task, saving its callee-saved
/// registers/flags onto its own stack and recording the resulting stack
/// pointer into `*prev`, then resumes `next` by loading its stack pointer
/// and popping its saved registers/flags back off — landing either at the
/// instruction right after `next`'s own earlier `switch` call, or (for a
/// task [`Context::new`] just initialized) at that task's entry point.
///
/// Performs no heap allocation and cannot block: the entire operation is a
/// bounded, fixed number of register pushes/pops (`agent/CODING_STANDARDS.md`'s
/// real-time discipline).
///
/// # Safety
/// `prev` must be a valid, exclusively-writable pointer to the currently
/// running task's own `Context` slot, and `next` must point at a `Context`
/// previously produced by [`Context::new`] (not yet resumed) or by a prior
/// `switch` that suspended it (and which has not been switched into again
/// since) — switching into the same suspended `Context` from two places
/// concurrently would alias one task's stack from two callers at once.
pub unsafe fn switch(prev: *mut Context, next: *mut Context) {
    // SAFETY: `Context`'s only field is the `rsp` this asm routine reads and
    // writes; `prev`/`next` are valid per this function's own contract, and
    // `context_switch_asm` touches memory only through those two pointers
    // and the stack they each point into (both caller-guaranteed live).
    unsafe {
        context_switch_asm(prev as *mut u64, next as *mut u64);
    }
}

/// [`switch`], but first installs `next_cr3` as the CPU's live address space
/// if it differs from the one already loaded (`STORY-P1-03-01`).
///
/// The `CR3` compare-and-reload happens **before** `switch`'s register swap,
/// not after: `next_cr3` must already map everything the incoming task's
/// saved registers/stack need, so the address space has to be live before
/// its own suspended execution resumes into it, precisely mirroring how a
/// real page fault would be attributed to the *incoming* task's mappings,
/// never the outgoing one's.
///
/// Same-space switches (`next_cr3` equal to the value already in `CR3`) skip
/// the reload entirely — [`hal_x86_64::paging::cr3_reload_needed`]'s own
/// pure comparison, exercised host-side, is what this function's decision
/// reduces to; this wrapper is the one place that turns that decision into a
/// real, TLB-flushing hardware write.
///
/// # Safety
/// Same contract as [`switch`], plus [`hal_x86_64::paging::write_cr3`]'s: if
/// a reload is needed, `next_cr3` must be the physical, page-aligned address
/// of a fully populated PML4 mapping the currently executing code/stack and
/// (if interrupts are enabled) the IDT/GDT/TSS and their handlers, or the
/// reload itself is an immediate, unrecoverable fault with nothing mapped to
/// run a handler from.
#[cfg(not(target_os = "windows"))]
pub unsafe fn switch_address_space(prev: *mut Context, next: *mut Context, next_cr3: u64) {
    let current_cr3 = hal_x86_64::paging::read_cr3();
    if hal_x86_64::paging::cr3_reload_needed(current_cr3, next_cr3) {
        // SAFETY: per this function's own contract.
        unsafe { hal_x86_64::paging::write_cr3(next_cr3) };
    }
    // SAFETY: per this function's own contract (mirrors `switch`'s).
    unsafe { switch(prev, next) };
}

#[cfg(test)]
mod tests {
    use super::*;

    const STACK_SIZE: usize = 4096;

    // Two independent tasks, each incrementing its own local variable across
    // repeated suspend/resume cycles interleaved with the other — proves
    // switch() preserves each task's own register/stack state independently
    // (STORY-P0-02-02 acceptance criterion 1's substance, exercised here on
    // the host toolchain since the asm's calling convention is pinned to
    // sysv64 regardless of host OS; TEST-P0-02-02-A additionally exercises
    // the same mechanism under QEMU per the Story's Tier 0 requirement).

    static mut MAIN_CTX: Context = Context::zeroed();
    static mut CTX_A: Context = Context::zeroed();
    static mut CTX_B: Context = Context::zeroed();
    static mut STACK_A: [u8; STACK_SIZE] = [0; STACK_SIZE];
    static mut STACK_B: [u8; STACK_SIZE] = [0; STACK_SIZE];
    static mut OBSERVED_A: [u32; 2] = [0, 0];
    static mut OBSERVED_B: [u32; 2] = [0, 0];
    static mut STEP_A: usize = 0;
    static mut STEP_B: usize = 0;

    extern "C" fn task_a() -> ! {
        let mut local: u32 = 10;
        loop {
            local += 1;
            // SAFETY: this test runs single-threaded and serially (Rust
            // test harness default), so no other task concurrently accesses
            // these statics; only `task_a` ever writes `OBSERVED_A`/`STEP_A`.
            unsafe {
                OBSERVED_A[STEP_A] = local;
                STEP_A += 1;
                switch(&raw mut CTX_A, &raw mut MAIN_CTX);
            }
        }
    }

    extern "C" fn task_b() -> ! {
        let mut local: u32 = 1000;
        loop {
            local += 5;
            // SAFETY: see `task_a` — only `task_b` ever touches these two.
            unsafe {
                OBSERVED_B[STEP_B] = local;
                STEP_B += 1;
                switch(&raw mut CTX_B, &raw mut MAIN_CTX);
            }
        }
    }

    #[test]
    fn switch_preserves_each_of_two_tasks_own_state_across_interleaving() {
        // SAFETY: single-threaded test; `STACK_A`/`STACK_B` are each used by
        // exactly one `Context` for this test's whole duration.
        unsafe {
            // `&raw mut` + deref avoids the `static_mut_refs` lint's "shared
            // reference to a mutable static" concern (there is exactly one
            // live reference to each array, scoped to this call); clippy's
            // `deref_addrof` doesn't know that distinction and flags the
            // idiom as a no-op, so it's silenced narrowly here too.
            #[allow(static_mut_refs, clippy::deref_addrof)]
            {
                CTX_A = Context::new(&mut *&raw mut STACK_A, task_a).expect("stack big enough");
                CTX_B = Context::new(&mut *&raw mut STACK_B, task_b).expect("stack big enough");
            }

            switch(&raw mut MAIN_CTX, &raw mut CTX_A); // task_a: local 10 -> 11
            switch(&raw mut MAIN_CTX, &raw mut CTX_B); // task_b: local 1000 -> 1005
            switch(&raw mut MAIN_CTX, &raw mut CTX_A); // task_a resumes: local 11 -> 12
            switch(&raw mut MAIN_CTX, &raw mut CTX_B); // task_b resumes: local 1005 -> 1010

            // Both tasks have suspended (returned control here via
            // `switch`), so nothing else is concurrently writing these
            // (still under the enclosing `unsafe` block above); see the
            // `deref_addrof` note above for why the `&raw const` idiom
            // (rather than a direct reference to the mutable static) needs
            // a narrow clippy allow here too.
            #[allow(clippy::deref_addrof)]
            let observed_a = *&raw const OBSERVED_A;
            #[allow(clippy::deref_addrof)]
            let observed_b = *&raw const OBSERVED_B;
            assert_eq!(observed_a, [11, 12]);
            assert_eq!(observed_b, [1005, 1010]);
        }
    }

    #[test]
    fn new_rejects_stack_too_small_for_the_initial_frame() {
        let mut tiny = [0u8; 8];
        // Never actually reached (Context::new fails before storing this
        // pointer anywhere callable) — mirrors sched.rs's dummy_entry.
        #[allow(clippy::empty_loop)]
        extern "C" fn dummy() -> ! {
            loop {}
        }
        // SAFETY: `tiny` is a live local; this call is expected to fail
        // closed before writing anything, per `Context::new`'s own contract.
        let result = unsafe { Context::new(&mut tiny, dummy) };
        assert_eq!(result, Err(ContextError::StackTooSmall));
    }
}
