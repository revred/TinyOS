# TEST-P0-04-02-A — Local-APIC Timer Fires at a Bounded Interval; Every Unrouted Vector Fails Closed

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-04-02`](../stories/STORY-P0-04-02.md)
Tier: Host (`cargo test -p hal-x86_64 --lib`) plus Tier 0 (QEMU x86_64), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix) — real interrupt delivery, timer counting, and `lidt`/`sti` semantics need a real (or QEMU-emulated) CPU; only the IDT gate-descriptor bit-packing is host-testable, mirroring `TEST-P0-05-02-A`'s own host/Tier 0 split for `paging.rs`/`AddressSpace`.
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D02` (interrupt entry/exit), `D03` (timer tick and deadline accounting)
Security controls: `SEC-03`, `SEC-18`, `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-02`, `BND-06`, `BND-07`, `BND-08`, `BND-18`, `BND-20`
Protection Domain contracts: `PD-02`, `PD-08`, `PD-10`, `PD-12`, `PD-13`
Code admission gates: `RCG-07`, `RCG-12`, `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## Specification

**Given** `hal_x86_64::interrupts::init` has not yet run (no IDT loaded, legacy PIC still at its power-on-default vectors),
**when**:
- `init` is called with a small timer reload count — **then** `idt::Idt::every_entry_present` was already true before `lidt` ran (all 256 vectors wired, `TIMER_VECTOR`/`SPURIOUS_VECTOR` to their own save-everything/`iretq` stubs, every other vector to the shared fail-closed default), the legacy PIC is remapped off the CPU-exception vector range and then masked, and the local APIC is enabled and its timer armed for periodic delivery,
- a fixture then waits (via `hlt`) for 5 real local-APIC timer interrupts, recording `RDTSC` at each — **then** every measured inter-tick interval is nonzero and no interval exceeds 20x the smallest observed (a self-consistency bound, not a fixed microsecond figure — see `STORY-P0-04-02`'s own acceptance criterion 1 for why),
- a *different* fixture (`fixture-idt-apic-unrouted`) instead executes `int 0x21` (a vector nothing explicitly services) immediately after `init` — **then** the shared fail-closed default handler is reached and calls `exit_qemu(Failure)`, never returning — this fixture's own **correct** result is therefore a QEMU isa-debug-exit *Failure* code, mirroring `fixture-broken-boot`'s established precedent (`TEST-P0-01-03-A`).

## Test type

Unit tests (`hal_x86_64::idt`'s own `#[cfg(test)]` module — pure gate-descriptor bit-packing/table-population logic, fully host-testable, 7 tests) plus two Tier 0 QEMU fixtures (`kernel`'s `fixture-idt-apic-timer`/`fixture-idt-apic-unrouted` features) exercising real IDT load, real PIC retirement, and real local-APIC timer/interrupt delivery against target-CPU semantics, mirroring the host/QEMU-both pattern this project has used since `TEST-P0-05-02-A`.

## Implementation location

`os/src/hal-x86_64/src/idt.rs` (gate-descriptor structures), `os/src/hal-x86_64/src/interrupts.rs` (IDT load, PIC retirement, local-APIC enable/timer, ISR stubs), `os/src/hal-x86_64/src/boot.rs` (`boot_pd_gib3` — the identity-map extension this Story's own local-APIC bring-up needed; see `STORY-P0-04-02`'s own doc comment for why), `os/src/kernel/src/fixture_idt_apic_timer.rs`, `os/src/kernel/src/fixture_idt_apic_unrouted.rs`, `os/src/kernel/src/main.rs` (both new fixture features, plus `interrupts::init` wired into the real boot path).

## Reports

[`REPORT-2026-07-26-27`](../reports/REPORT-2026-07-26-27.md) — Pass.
