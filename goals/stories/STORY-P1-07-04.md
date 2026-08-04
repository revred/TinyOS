# STORY-P1-07-04 — GIC and Generic-Timer Tick, and the Counter Decision That Closes `LE-15`

Status: **In progress — host-testable half Green 2026-08-04 (275 host tests in `hal-arm64`, from 259, Red first: GIC enable order and refusals against latching doubles, ratio arithmetic, the `LE-15` decision function with its fallback arm exercised, every report line pinned to exact bytes); acceptance criteria 1, 2, 3, 4 and 5 blocked on a board capture. The evidence channel is the canvas (`TOS64-TICK/1` live, `TOS64-CONF/1` and `TOS64-PMU/1` at boot) — serial has never produced a byte on this bench (`LE-47`). Not Verified.**
Feature: [`FEAT-P1-07`](../features/FEAT-P1-07.md)
Introduced in: [`session/hand-2026-07-28/17-raspberry-pi-5-bring-up-plan.md`](../../session/hand-2026-07-28/17-raspberry-pi-5-bring-up-plan.md) §4.2

## Description

Two things that share one set of registers: a periodic tick (GIC distributor/redistributor and CPU interface, `CNTP_*_EL0` or `CNTV_*_EL0`), and the decision about *which counter microbenchmarks read* — the question `STORY-P1-01-03` raised as `LE-15` and deliberately deferred until a board existed.

**A board exists, and the decision is recorded** (user confirmation, [Handover 19](../../session/hand-2026-07-28/19-feat-p1-07-acceptance-and-spine.md) §7.5): **`PMCCNTR_EL0`, the PMU cycle counter, becomes the ARM64 `CycleSource` for microbenchmarks; `CNTVCT_EL0` stays the `Timebase`/wall-clock source.** The two roles are genuinely different and `hal::time` already separates them.

The arithmetic that forces it: at 54 MHz one `CNTVCT_EL0` tick is ~18.5 ns, and a Cortex-A76 at ~2.4 GHz retires roughly **44 cycles per tick**. `D05/dispatch_select` measures a Tier 0 p50 of ~168 cycles. Read through `CNTVCT_EL0` that entire operation is **under four ticks** — quantisation noise, not a measurement. It is exactly the failure mode `LE-24` already documents for `D07`, arriving on a second axis.

This Story is also where `LE-27` closes: the `Cntvct`/`GenericTimerTimebase` code shipped by `STORY-P1-01-03` has never executed against a real register, so `hal::time::conformance` passing on the host is evidence about arithmetic, not about hardware.

## Depends on

`STORY-P1-07-03` (a counter read on uncached memory measures the memory system, not the counter) and `STORY-P1-07-02` (enabling the PMU at the wrong exception level faults, and a fault must be reportable).

## Acceptance criteria

1. **A periodic tick at a declared interval, verified by ratio between consecutive intervals** — the way `kernel::fixture_idt_apic_timer` already does it — **and not by absolute value.** An absolute-value check on a first hardware bring-up conflates a wrong divisor, a wrong clock source and a wrong frequency register into one indistinguishable failure.
2. **`hal::time::conformance` runs against the real registers** and passes unchanged. `LE-27` closes here, on that run, and not on the host run that already passes.
3. **`PMCCNTR_EL0` is enabled and readable, or the fallback is taken and recorded.** Enablement touches `PMCR_EL0`, `PMCNTENSET_EL0` and `PMUSERENR_EL0`, and if the firmware left us at `EL2` the trap configuration in `MDCR_EL2` matters. **If `PMCCNTR_EL0` traps or reads zero, the Story does not fail** — it falls back to `CNTVCT_EL0` with batched iteration and records a *narrowed* `LE-15` naming exactly which register was unavailable and why. A bring-up Story that can only succeed is a bring-up Story that will be made to look successful.
4. **`CNTFRQ_EL0`'s honest-absence behaviour survives silicon.** `STORY-P1-01-03` made zero and implausible frequencies return `None` rather than a guess; this criterion is the first time that path meets firmware that may genuinely not have programmed the register.
5. **`LE-15` closes with a recorded decision**, naming which counter serves which role and what the measured resolution of each actually was on this board — not what the architecture manual says it should be.

## Named debt this Story leaves open

- **No preemption and no WCET enforcement.** A tick that fires is not a scheduler; the `FEAT-P1-04` port is a follow-on Feature.
- **No `LE-08` equivalent resolved.** Device-IRQ routing beyond the timer is out of scope; only the timer interrupt is routed.
- `LE-09` stays open until `STORY-P1-07-06`.

## Tests

[`TEST-P1-07-04-A`](../tests/TEST-P1-07-04-A.md) — written before implementation, per the TDD mandate.
