# TEST-P1-02-01-A — `#PF`/`#GP`/`#UD`: Capture the Faulting Context, Contain It to One Task, Audit Every Decision

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P1-02-01`](../stories/STORY-P1-02-01.md)
Tier: Host unit tests (fault-frame decoding, policy, spoor encoding) **plus** a Tier 0 QEMU fault-injection fixture that raises all three faults for real, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D02`
Security controls: `SEC-03`, `SEC-14`, `SEC-19`
Containment classes: `C0`, `C1`
Boundary tests: `BND-04`, `BND-17`, `BND-20`
Protection Domain contracts: `PD-02`, `PD-12`, `PD-13`
Code admission gates: `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## What this test is for

Today every CPU exception reaches `STORY-P0-04-02`'s shared fail-closed default: correct, but terminal for the whole system, and silent about what happened. `STORY-P1-01-01` paid for that twice in one session — a `#UD` from missing SSE enablement produced a triple fault with no diagnostic at all, costing two debugging cycles. And `LE-09`'s remaining pieces are explicitly blocked on this Story: on a Raspberry Pi 5 with no `isa-debug-exit` port, a fault with no handler is a silent hang with **no output whatsoever**.

So this Story is not "add handlers". It is: a fault becomes a **reportable, attributable, contained event** — captured, decided on under an explicit fail-closed policy, audited, and survived by everything that did not fault.

## Specification

### 1. The faulting context is captured, in full and in one shape

**Given** a `#UD` (vector 6), `#GP` (13) or `#PF` (14),
**then** the handler receives a single `FaultFrame` carrying the vector, the error code, and the CPU's own pushed frame (`rip`, `cs`, `rflags`, `rsp`, `ss`) — and, for `#PF` only, `CR2`.

**And** the frame has one layout for all three vectors: `#UD` pushes **no** hardware error code, so its stub pushes a synthetic zero, and nothing downstream needs to know which vectors carry one. A per-vector frame shape would be three parsers, and the third would be the one with the bug.

**And** the `FaultFrame` is `#[repr(C)]` with a host test pinning its exact size and field offsets against the assembly stub's push order — that correspondence is invisible to the type system and silently corrupts every field if it drifts.

### 2. Error codes and faulting addresses are hostile input (`BND-04`)

**Given** a fault frame,
**then** the error code is **decoded into named, documented bits** (`#PF`: present/write/user/reserved-write/instruction-fetch; `#GP`: external/table/selector-index) as pure, host-testable functions —

**and** no decoded bit, and no faulting address, is ever used to *widen* a decision. They are recorded and reported; the disposition below depends only on **where the fault happened** (which context), never on what the faulting code claimed about it. A fault frame comes from arbitrary, possibly attacker-steered execution: it is evidence, never authority.

**And** an unknown vector reaching the shared entry point is itself a fault-handling failure and halts fail-closed, rather than being decoded as if it were one of the three.

### 3. The disposition is explicit, fail-closed, and has exactly two enumerated arms

**Given** a captured fault,
**then** the policy returns exactly one of:

- **`TerminateTask(id)`** — the fault happened inside a task. That task is marked `Finished`, and the scheduler keeps running everything else.
- **`HaltSystem`** — the fault happened in kernel context, with no task to contain it to. The kernel cannot know its own invariants still hold, so it reports and stops.

**And there is deliberately no `Resume` arm.** Criterion 2 of this Story forbids speculative "maybe recoverable" paths, and this kernel has **no** recoverable fault case today: no demand paging, no copy-on-write, no stack guard growth. An unreachable resume arm in a fault path is a liability, not future-proofing — the day a genuine recoverable case exists it arrives with its own Story, its own enumeration and its own test. This test asserts the absence: no code path resumes a faulting instruction.

**And** the policy is a pure function over the captured report, host-tested for every (vector × context) combination, with no `unsafe` and no hardware dependency.

### 4. A fault in one task terminates *that task only*

**Given** a Tier 0 fixture with two tasks, where task A deliberately faults and task B does not,
**when** A raises `#UD`, then (a fresh victim) `#GP`, then `#PF`,
**then** for each: the fault is captured, A is marked `Finished`, control returns to the supervisor context, **B still runs to completion afterwards**, and the fixture reports its own verdict over the UART.

This is the criterion the whole Story exists for, and it is what makes `FEAT-P1-03`'s per-task address spaces safe to attempt: a CR3 switch with no fault containment behind it is strictly more dangerous than the current identity map.

**And** all three faults are raised by *real instructions* on real target-compiled code — `ud2`, a load of an out-of-range segment selector, and a read from a canonical-but-unmapped address — not by simulating a frame.

> Corrected 2026-07-27 during implementation. This clause originally named a `wrmsr` to a reserved MSR as the `#GP` source. **QEMU/TCG accepted that write without faulting**, so it was replaced with the selector load, which is both architecturally guaranteed *and* observed to fault here — and yields a non-zero error code, exercising the decoder rather than the zero path. See `REPORT-2026-07-27-05`'s finding. `STORY-P1-02-02` later moved the selector itself from `0x18` to index 511, so that it no longer depends on how many descriptors the GDT happens to hold.

### 5. Every fault is a spoor

**Given** any captured fault,
**then** a spoor is emitted carrying category `Fault`, actor `Kernel`, action `Fault` (capture) and `Terminate` (disposition), an outcome (`Failed` for the fault itself, `Ok`/`Failed` for the disposition) and the faulting task as the target — the existing `Spoor` atom, extended additively with one new `Category` and two new `Action` variants, never a repurposed one.

**And** the spoor carries **no** faulting address, error code or register content: `PD-12` scopes a fault record to class/actor/action/outcome, and an audit atom is not a debugging channel. The full frame goes to the serial report, which is bounded and explicitly not the audit log.

**And** round-trip encoding/decoding of the new variants is host-tested like every existing one.

### 6. The unhandled-vector default is preserved, not replaced

**Given** the 253 vectors this Story does not wire,
**then** they keep `STORY-P0-04-02`'s shared fail-closed default handler, and `Idt::every_entry_present` still holds before the table is loaded. This Story narrows the set of faults that are terminal for the whole system; it does not widen the set that is silently ignored, which stays empty.

### 7. Adversarial: the fault path itself is bounded and non-reentrant

**Given** the handler,
**then** it takes no lock, allocates nothing, contains no unbounded loop, and runs with `IF` clear (the IDT gates are interrupt gates, not trap gates) so it cannot be re-entered by an interrupt mid-decision.

**And** a fault *inside* the fault handler is not survivable by this Story and must not be claimed to be: that is `STORY-P1-02-02`'s charge (TSS/IST), and until it lands, a `#DF` still triple-faults. Stated here so no reader infers more containment than exists. (`STORY-P1-02-02` landed later the same day; `TEST-P1-02-02-A` clause 6 records the triple fault this clause predicted, observed under `qemu -d int,cpu_reset`.)

### 8. What this test explicitly does **not** establish

- **No hardware tier.** Tier 0 QEMU only; `LE-09` stays open.
- **No double-fault survival** (`STORY-P1-02-02`).
- **No resume of any kind**, by design (clause 3).
- **No user/kernel privilege boundary.** Everything still runs at CPL 0 in one identity-mapped address space, so "the fault happened in a task" means *which context was running*, not a hardware privilege transition. Real privilege separation is `FEAT-P1-03`'s.
- **No fault-latency baseline.** `FEAT-P1-02`'s exit criteria ask for one; the handler is not yet on a measured path, and wiring it into `fixture_measure` is named as follow-on work rather than quietly skipped.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-x86_64/src/fault.rs`, `os/src/kernel/src/fault.rs`, `os/src/kernel/src/spoor.rs`) plus a Tier 0 QEMU fixture (`cargo run -p xtask -- qemu-x86_64 --fixture=fault`).

## Implementation location

- `os/src/hal-x86_64/src/fault.rs` — `FaultVector`, `FaultFrame`, error-code decoders, the three assembly stubs, IDT wiring.
- `os/src/hal-x86_64/src/interrupts.rs` — fault vectors installed alongside the existing default.
- `os/src/kernel/src/fault.rs` — `FaultReport`, `Disposition`, the policy, spoor emission.
- `os/src/kernel/src/spoor.rs` — `Category::Fault`, `Action::Fault`, `Action::Terminate`.
- `os/src/kernel/src/fixture_fault.rs` — the Tier 0 three-fault fixture.
- `os/src/kernel/src/main.rs` — the default `tinyos_fault_entry` for the normal boot path.

## Reports

- [`REPORT-2026-07-27-05`](../reports/REPORT-2026-07-27-05.md) — Red run recorded then Green, the Tier 0 capture for all three faults, and what remains open.
