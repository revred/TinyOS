# STORY-P1-07-02 — AArch64 Exception Vectors: a Fault Prints a Decoded `ESR_EL1` Instead of Hanging

Status: **In progress — host-testable half Green 2026-07-28 (115 host tests in `hal-arm64`, from 64); acceptance criterion 2 blocked on a board, and there is no version of this Story that passes without it. Not Verified.**
Feature: [`FEAT-P1-07`](../features/FEAT-P1-07.md)
Introduced in: [`session/hand-2026-07-28/17-raspberry-pi-5-bring-up-plan.md`](../../session/hand-2026-07-28/17-raspberry-pi-5-bring-up-plan.md) §5

## Description

The AArch64 counterpart of `STORY-P1-02-01`, reduced to the half this slice needs: an exception vector table at `VBAR_EL1`, and a synchronous-exception handler that decodes and prints `ESR_EL1` (exception class, instruction-length bit, ISS) and `FAR_EL1` over the UART.

This Story is why the whole ordering exists. On x86_64 under QEMU a fault without a handler produces a triple fault that `-d int,cpu_reset` still narrates. On a Raspberry Pi 5 there is no `isa-debug-exit`, no monitor, and no log: a fault with no vector table is a silent hang with **no output whatsoever**, indistinguishable from a dead adapter, a bad image, or a board that never started. Everything after this Story — the MMU, the GIC, the measurement — is work whose failures are diagnosable only because this Story landed first.

## Depends on

`STORY-P1-07-01` (there is no way to see a fault report without a UART).

## Acceptance criteria

1. **A vector table is installed at `VBAR_EL1` with all sixteen entries present**, mirroring `Idt::every_entry_present`: an unfilled vector is a silent hang by another name. Entries this Story does not handle reach one shared fail-closed default that reports and halts, exactly as `STORY-P0-04-02`'s default does on x86_64 — this Story narrows the set of faults that are terminal, it does not widen the set that is silent, which stays empty.
2. **A deliberately-triggered synchronous exception prints exception class, fault address, and a decoded `ESR_EL1`.** **A deliberate fault fixture is mandatory.** The whole value of this Story is that failure becomes visible, and that is unprovable without inducing a failure — the same reasoning that made `fixture-broken-boot` a required artifact rather than an optional one.
3. **The decoders are pure functions, host-tested.** `ESR_EL1`'s exception class and ISS fields, and the instruction-length bit, decode on the dev host with no board and no `unsafe`, per `TEST-P1-02-01-A` clause 2's precedent: a fault frame is evidence, never authority, and nothing decoded from it may widen a decision.
4. **The report is bounded and non-reentrant.** The handler takes no lock, allocates nothing, contains no unbounded loop, and cannot be re-entered mid-decision. A fault inside the fault handler is **not** survivable by this Story and must not be claimed to be — there is no AArch64 equivalent of `STORY-P1-02-02`'s IST work in this Feature, and stating that here keeps a reader from inferring containment that does not exist.
5. **Every fault report is a spoor**, carrying category/actor/action/outcome and the faulting context identity — and carrying **no** register content or faulting address, per `PD-12`. The full frame goes to the serial report, which is bounded and explicitly not the audit log. This is the same split `STORY-P1-02-01` established; a second architecture is where a split like that either holds or turns out to have been x86-shaped.

## Named debt this Story leaves open

- No double-fault or nested-exception survival (no AArch64 `SP_EL0`/`SP_EL1` stack-switching safety net in this Feature).
- No `EL0`, so "which context faulted" means which execution context, not a privilege transition.
- `LE-09` stays open. A decoded fault is diagnostics, not a measurement.

## Tests

[`TEST-P1-07-02-A`](../tests/TEST-P1-07-02-A.md) — written before implementation, per the TDD mandate.
