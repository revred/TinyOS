# STORY-P0-04-02 — Interrupt Controller (APIC) Bring-Up

Status: **Verified**
Feature: [`FEAT-P0-04`](../features/FEAT-P0-04.md)
Introduced in: [`session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md`](../../session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md)
Implemented in: [`session/hand-2026-07-26/32-story-p0-04-02-idt-apic-bring-up.md`](../../session/hand-2026-07-26/32-story-p0-04-02-idt-apic-bring-up.md)

## Description

Brings up a real x86_64 IDT and the local APIC timer, retiring the legacy 8259 PIC — the interrupt-routing foundation the scheduler's future timer tick (`FEAT-P0-02`) and `STORY-P0-04-03`'s device work both need, and the concrete first step of closing "no IDT/fault containment," the largest gap the security-spine audit (`goals/security/current-state-review.md`) named after `FEAT-P0-07` landed.

**Scope correction from this Story's own draft description.** The original text said "bring up the local APIC *and* I/O APIC." This Story implements the local APIC only (timer + spurious-vector handling) plus a full 256-entry IDT with a fail-closed default handler for every vector this kernel doesn't explicitly service. I/O APIC bring-up is real device-IRQ *routing* — meaningless with no device driver yet requesting a routed line — and is deferred to whichever Story first has a real device to route for (`STORY-P0-04-03`'s own bus-enumeration work, or a later driver Story), matching this codebase's established "wire a primitive when its own acceptance criteria need it, not speculatively" discipline (`STORY-P0-03-02`'s precedent for capacity constants, `STORY-P0-07-02`'s handover 28 for task-exit revocation). Both of this Story's own acceptance criteria are about the local APIC timer and the fail-closed default — the I/O APIC page was never actually load-bearing for either.

**A real, empirically-found QEMU constraint shaped this Story's implementation.** The local APIC's MMIO window is architecturally relocatable via `IA32_APIC_BASE` (Intel SDM Vol 3A §11.4.1), and this Story's first working version used that to reach the APIC from inside `boot.rs`'s existing first-1GiB identity map without touching boot assembly. Under QEMU's own APIC device model, relocation is not honored: every local-APIC register read back all-zero after a relocated base was programmed — including the read-only APIC ID register, which real hardware always answers — proving the access was landing on ordinary backing memory, not being intercepted by the APIC at all. The fix was the opposite of relocation: `hal_x86_64::boot`'s identity map was extended with a second `PDPT[3]`-rooted `PD` (`boot_pd_gib3`) covering `0xC0000000-0xFFFFFFFF`, so the local APIC's real, non-relocated default address (`0xFEE00000`) is reachable. This is a `boot.rs` change (shared by every binary in this workspace), verified not to regress any of the nine pre-existing QEMU fixtures — see this Story's own Report.

## Depends on

`STORY-P0-04-01` (topology discovery landed first; this Story's own IDT/APIC bring-up does not actually consume MADT routing data, since it targets the local APIC's own fixed default address and vector space, not a firmware-described interrupt line).

## Acceptance criteria (final)

1. A timer interrupt configured through the local APIC fires at a bounded, measured interval under QEMU — verified by a Tier 0 test, not assumed from datasheet timing alone. **Met**: `fixture-idt-apic-timer` (`TEST-P0-04-02-A`) arms the timer, waits for 5 real ticks via `hlt`, and asserts every measured inter-tick `RDTSC` interval is nonzero and within 20x of the smallest — a self-consistency bound rather than a fixed microsecond figure, since QEMU's own APIC-timer-to-wall-clock relationship under software emulation is not a stable absolute number this Story should depend on.
2. Spurious/unrouted interrupts are handled explicitly (a documented default handler), never silently ignored in a way that could mask a real hardware fault. **Met structurally**: `idt::Idt::every_entry_present` (7 host tests) proves every one of the 256 vectors is wired before `interrupts::init` ever calls `lidt`. **Met behaviorally**: `fixture-idt-apic-unrouted` (`TEST-P0-04-02-A`) deliberately executes `int 0x21` (a vector this kernel never explicitly services) and asserts the fail-closed default handler is actually reached — this fixture's *correct* result is a QEMU isa-debug-exit **Failure** code, mirroring `fixture-broken-boot`'s own established precedent, since reaching the default handler (which itself calls `exit_qemu(Failure)` and never returns) *is* the pass condition.

## Named, not silently solved

- **No TSS/Interrupt Stack Table.** A genuine `#DF`/`#MC` whose own stack is already invalid can still fault a second time while the CPU pushes that vector's interrupt frame — see `idt::Idt`'s own doc comment for the full statement. IST-backed known-good stacks for those two vectors specifically is a real, general follow-up, not assumed solved by this Story's fail-closed default.
- **No I/O APIC / device-IRQ routing** — see this Story's own scope-correction note above.
- **No production consumer of ticks.** `hal_x86_64::interrupts::init` is called from the real (non-fixture) `kernel_main` boot path — an IDT with a fail-closed default for every vector is now this kernel's actual boot-time state, not a fixture-only demonstration — but nothing yet reads `tick_count()` (no scheduler dispatch loop exists to consume it), matching this codebase's "wire the primitive, don't invent a speculative consumer" discipline applied to a HAL primitive rather than a capacity constant.

## Tests

`os/src/hal-x86_64/src/idt.rs`'s `#[cfg(test)]` module (host-testable gate-descriptor bit-packing) and two new Tier 0 QEMU fixtures, `fixture-idt-apic-timer` and `fixture-idt-apic-unrouted` (`os/src/kernel/src/fixture_idt_apic_timer.rs`/`fixture_idt_apic_unrouted.rs`), proving the timer-interval and fail-closed-default acceptance criteria against real target-CPU interrupt delivery. `hal_x86_64::interrupts` (IDT loading, PIC retirement, local-APIC bring-up) has no host unit tests of its own — everything in it only means anything on a real CPU (`lidt`/`sti`/`rdmsr`/`wrmsr`/port I/O/MMIO), the same split `paging.rs`/`boot.rs` already established between host-testable structure and target-only execution. See [`TEST-P0-04-02-A`](../tests/TEST-P0-04-02-A.md) and [`REPORT-2026-07-26-27`](../reports/REPORT-2026-07-26-27.md).

## Goals verified

`G-HW-4`; indirectly supports `G-RT-1` (a future scheduler tick source) once a dispatcher consumes `tick_count()`.
