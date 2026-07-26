# STORY-P0-02-02 — Context Switch

Status: **Verified**
Feature: [`FEAT-P0-02`](../features/FEAT-P0-02.md)
Introduced in: [`session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md`](../../session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md)
Implemented in: [`session/hand-2026-07-26/13-story-p0-02-02-context-switch-implementation.md`](../../session/hand-2026-07-26/13-story-p0-02-02-context-switch-implementation.md)

## Description

The x86_64 register-save/restore context switch: a small amount of hand-written assembly (permitted per `agent/CODING_STANDARDS.md`'s Language policy "boot/entry assembly" carve-out) wrapped by a thin, `unsafe`-documented Rust boundary, switching from one `TaskId`'s saved register state to another's on a timer-tick or explicit yield.

## Depends on

`STORY-P0-02-01` (a `TaskId`/TCB to switch to and from — this Story's `Context::new` takes `extern "C" fn() -> !`, the same type `sched::TaskEntry` already is).

## Acceptance criteria

1. A context switch preserves all callee-saved registers and the stack pointer across the switch, verified by a QEMU integration test (Tier 0) that switches between two tasks and confirms each resumes with its own state intact. **Met**: `TEST-P0-02-02-A`'s fixture switches into two tasks, interleaved, four times total, and only reports success if both tasks' independently-seeded, per-resume counters land exactly where two correctly-resumed executions would.
2. The switch path itself performs no heap allocation and no unbounded loop, per the Real-time discipline section. **Met**: `switch`/`context_switch_asm` is a fixed sequence of register pushes/pops and two memory moves — no loop, no allocation of any kind.
3. Every `unsafe` block in the switch path carries a `// SAFETY:` comment per the Unsafe code policy — this is exactly the kind of low-level register/stack manipulation that policy exists for. **Met**: every `unsafe` block/fn in `context.rs` (`Context::new`'s raw writes, `switch`'s call into `context_switch_asm`) carries a `// SAFETY:` comment stating the invariant that makes it sound.

## Tests

Implemented as `#[cfg(test)]` functions in `os/src/kernel/src/context.rs` (host-testable, `cargo test -p kernel --lib`) plus a Tier 0 QEMU fixture (`os/src/kernel/src/context_switch_fixture.rs`, only compiled under the `fixture-context-switch` feature). See [`TEST-P0-02-02-A`](../tests/TEST-P0-02-02-A.md) for the full specification, and [`REPORT-2026-07-26-08`](../reports/REPORT-2026-07-26-08.md) for the verification run.

## Goals verified

G-RT-1 (preemptive scheduling), G-RT-7 (indirectly — this is the x86_64-specific half of a portable scheduler core).
