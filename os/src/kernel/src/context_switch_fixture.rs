//! `TEST-P0-02-02-A`'s QEMU fixture: two tasks, each incrementing its own
//! stack-local counter across repeated suspend/resume cycles interleaved
//! with the other, driven entirely through
//! [`kernel::context`](kernel::context) — the same mechanism and same
//! sequence `context.rs`'s own host unit test already exercises on the dev
//! toolchain, run here a second time under real QEMU/target-hardware
//! register semantics per `STORY-P0-02-02`'s Tier 0 requirement.
//!
//! Only reachable when the `fixture-context-switch` feature is enabled —
//! never part of a real boot image.

use kernel::context::Context;

const STACK_SIZE: usize = 4096;

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
        // SAFETY: single-CPU, single-threaded boot fixture; only `task_a`
        // ever writes `OBSERVED_A`/`STEP_A`, and `switch` never returns
        // concurrently on another execution context.
        unsafe {
            OBSERVED_A[STEP_A] = local;
            STEP_A += 1;
            kernel::context::switch(&raw mut CTX_A, &raw mut MAIN_CTX);
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
            kernel::context::switch(&raw mut CTX_B, &raw mut MAIN_CTX);
        }
    }
}

/// Runs the fixture: initializes two task contexts, switches into each
/// twice (interleaved), and reports whether both resumed with their own
/// state intact.
pub fn run() -> bool {
    // SAFETY: this fixture is the only code running (single-CPU boot path,
    // nothing else touches these statics), `STACK_A`/`STACK_B` are each
    // sized well above `Context::new`'s minimum and used by exactly one
    // `Context` for the fixture's whole duration.
    unsafe {
        let Ok(ctx_a) = Context::new(&mut *&raw mut STACK_A, task_a) else {
            return false;
        };
        let Ok(ctx_b) = Context::new(&mut *&raw mut STACK_B, task_b) else {
            return false;
        };
        CTX_A = ctx_a;
        CTX_B = ctx_b;

        kernel::context::switch(&raw mut MAIN_CTX, &raw mut CTX_A); // task_a: local 10 -> 11
        kernel::context::switch(&raw mut MAIN_CTX, &raw mut CTX_B); // task_b: local 1000 -> 1005
        kernel::context::switch(&raw mut MAIN_CTX, &raw mut CTX_A); // task_a resumes: local 11 -> 12
        kernel::context::switch(&raw mut MAIN_CTX, &raw mut CTX_B); // task_b resumes: local 1005 -> 1010

        OBSERVED_A == [11, 12] && OBSERVED_B == [1005, 1010]
    }
}
