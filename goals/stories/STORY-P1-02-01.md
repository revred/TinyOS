# STORY-P1-02-01 — `#PF`/`#GP`/`#UD` Handlers: Capture, Terminate-vs-Halt, Spoor Audit

Status: **Functionally Verified (Tier 0 + Host), 2026-07-27** — assurance state `baseline-debt`; no `PERF-D02` guardrail closed, no double-fault survival (`STORY-P1-02-02`), no hardware-tier evidence (`LE-09`)
Feature: [`FEAT-P1-02`](../features/FEAT-P1-02.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)
Implemented in: [`session/hand-2026-07-27/06-story-p1-02-01-fault-handling.md`](../../session/hand-2026-07-27/06-story-p1-02-01-fault-handling.md)

## Description

Real exception handlers for page fault, general protection and invalid opcode: capture the full faulting context (the CPU's pushed frame plus the error code and, for `#PF`, `CR2`), route to an explicit fail-closed kernel policy, emit a spoor for every fault, and keep scheduling everything else. Every vector not explicitly handled keeps `STORY-P0-04-02`'s diverge-and-report default.

## Depends on

`STORY-P1-01-01` (the harness); `STORY-P0-04-02`'s IDT.

## Acceptance criteria (final)

1. **A deliberate `#PF`, `#GP` and `#UD` in a victim task each terminate *that task only* — a Tier 0 fixture proves another task keeps running and the fault appears in the spoor journal.** **Met**: `fixture-fault` raises all three from real instructions inside scheduled tasks; each victim ends `Finished`, the supervisor regains control, a fourth task then runs three times, and the journal holds six fault spoors (two per fault). Full capture in [`REPORT-2026-07-27-05`](../reports/REPORT-2026-07-27-05.md).
2. **Termination is the default policy; any resume case is explicitly enumerated, documented, and separately tested — no speculative "maybe recoverable" paths.** **Met by having none**: the disposition has exactly two arms — terminate the faulting task, or halt when the kernel itself faulted — and **no `Resume` arm exists at all**, because this kernel has no recoverable fault case (no demand paging, no copy-on-write, no guard-page growth). The entry stubs correspondingly save no registers and never `iretq`. An unreachable resume arm in a fault path is a liability, not future-proofing.
3. **Fault-frame parsing is defensive: error codes and fault addresses are hostile input and never trusted into authority decisions.** **Met**: decoders are pure host-tested functions; `FaultReport` does not carry the frame at all, so the policy cannot reach for it; and a host test pins that the same context yields the same disposition for vectors 0, 6, 8, 13, 14, 255 and `u64::MAX`. `CR2` is reported only for `#PF` — the type refuses to attach a stale address to another vector.

## The finding this Story produced

**"Architecturally must fault" and "faults under this emulator" are different claims, and Tier 0 silently sides with the emulator.** The `#GP` victim was first written as a `wrmsr` to a reserved MSR — an architecturally guaranteed `#GP` that QEMU/TCG simply accepted, sending the task through its own `unreachable!()` instead. Replaced with a load of a segment selector past the boot GDT's limit, which is guaranteed *and* observed, and which produces a non-zero error code so the decoder is exercised rather than the zero path.

This is the same class of gap `STORY-P1-01-01` hit from the other side (host-verified code that could not execute on target), and it is fresh, concrete evidence for `LE-09`'s hardware tier.

## Tests

[`TEST-P1-02-01-A`](../tests/TEST-P1-02-01-A.md) — eight clauses, written before any code. See that Report's process note: the Test document preceded implementation, but the two new modules' unit tests were written alongside their implementations rather than as a recorded Red run, unlike the three preceding Stories.

## Reports

- [`REPORT-2026-07-27-05`](../reports/REPORT-2026-07-27-05.md) — the Tier 0 capture for all three faults, the emulator finding, and what remains open.

## Goals verified

G-SEC-2 (fault-containment half), G-SEC-14. Neither closes: containment is demonstrated at Tier 0 within one identity-mapped address space, with no privilege boundary and no double-fault survival behind it yet.

## Named debt this Story leaves open

`STORY-P1-02-02` (TSS/IST — a fault inside the fault handler still triple-faults) is the direct successor and `FEAT-P1-02`'s other half. `LE-03` narrows: `#PF`/`#GP`/`#UD` are handled, but `#XF` and every other vector still reach the shared fail-closed default. `LE-04` (no TSS/IST) is unchanged and now the sharpest item in the register. `LE-11` (`Context::new` seeds `IF` with no IDT installed) is *mitigated* for this fixture — an IDT now exists — but the fail-open seam itself is untouched. New: **`LE-17`** — the fault path has no `FEAT-P1-01` latency baseline, which that Feature's own exit criteria ask for.
