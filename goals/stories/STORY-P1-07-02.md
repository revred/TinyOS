# STORY-P1-07-02 — AArch64 Exception Vectors: a Fault Prints a Decoded `ESR_EL1` Instead of Hanging

Status: **In progress — host-testable half Green 2026-07-28 (115 host tests in `hal-arm64`, from 64). **Acceptance criterion 2 Green on silicon** — `BOARD VERDICT 8` (mmu-fault boot 2026-08-04, kernel `fde0f2ce3f91`) put a fully decoded fault frame on the canvas from a real exception: `SLOT=CUR_EL_SPX/SYNC INDEX=04`, `ESR=0x96000005` broken into `EC=0x25` / `IL=32` / `DFSC` translation-level-1 / `WnR=read` / `ISV=no`, `FAR` and `ELR` and `SPSR` raw beside their decode, and `HALTED REASON=NO-RESUME-PATH`. `SIZE=UNKNOWN` where `ISV=0` is the honest-absence behaviour surviving silicon rather than a guess. The evidence channel is the canvas, not serial (`LE-47`).

**Criteria 1, 3 and 4 are met** — sixteen vector entries present with one shared fail-closed default, the `ESR_EL1`/ISS/`IL` decoders pure and host-tested, and the handler bounded, allocation-free and non-reentrant (allocation-freedom compiler-enforced by the `no_std` gate rather than merely asserted).

**Criterion 5 is NOT met, and the closing pass of 2026-08-05 found it by reading the code rather than the header.** The criterion says *"Every fault report is a spoor, carrying category/actor/action/outcome and the faulting context identity"*, with the full register frame going to the bounded serial report and explicitly **not** to the audit log — the `PD-12` split `STORY-P1-02-01` established, and the criterion exists precisely to test whether that split was x86-shaped. **`Rung::FaultTaken` is declared in both vocabularies — `kernel::spoor_stream` and `hal_arm64::spoor`, discriminant 7, agreeing in the parity test — and nothing anywhere stamps it.** Its only appearances in the tree are the round-trip and parity tests that prove the *vocabulary* is closed and consistent. So a fault on this board paints a decoded frame on the canvas and leaves **no audit record at all**, which is the one thing this criterion was written to prevent, and `BOARD VERDICT 8`'s fault boot could not have shown it either way because it predates spoor egress entirely (the first spoor-emitting image is `b44040659702`, `BOARD VERDICT 9`).

This is the same defect class as `LE-73`'s and `STORY-P1-10-03`'s: **a name joined to the spine with no call site behind it**, which reads as delivered from every direction except the one that looks for a caller. The host tests pass, the parity test passes, the discriminant is on the wire's append-only list, and the behaviour does not exist. Closing it needs a stamp at the fault-report site and one `mmu-fault` boot on a spoor-emitting image — at which point criterion 5 and its `PD-12` no-register-content clause become checkable off the wire in the same capture. **Not Verified.**
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
