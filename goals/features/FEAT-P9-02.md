# FEAT-P9-02 — Kernel Entropy Source

Status: **Specified — no Story started**
Epic: [`EPIC-P9`](../epics/EPIC-P9.md)
Introduced in: [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) §3, §7.5

## Description

There is no entropy source in this kernel. Verified: zero `RDRAND`/`RDSEED`/RNG/jitter-collector references outside `xtask`.

That single absence blocks three separate things, which is why it is its own Feature rather than a line item inside whichever of them is scheduled first: `FEAT-P9-03` needs unpredictable nonces, `FEAT-P9-05` needs salt diversity, and `FEAT-P9-08` needs randomized addresses. The proposal's first draft charged this cost to randomization alone and thereby made randomization look more expensive than it is ([`session/hand-2026-07-28/08-memory-confidentiality-review.md`](../../session/hand-2026-07-28/08-memory-confidentiality-review.md) §14.5).

The hard part is not obtaining bits — `RDSEED` exists on every target CPU this project runs on. It is (a) **not trusting a single source**, since a CPU RNG is exactly the component an adversary with `SEC-16`-class resources would attack, and it is opaque; (b) **health-testing continuously** rather than at init, because a source that silently degrades is worse than one that fails; and (c) **not destroying this project's reproducibility**, since fixtures assert exact addresses and both the `D04` measurement and the spoor journal rest on reproducible boots.

## Crate(s) involved

`os/src/hal-x86_64/` (the `RDSEED`/`RDRAND` backend and a timing-jitter fallback), `os/src/hal/` (the arch-neutral trait, mirroring `hal::time::CycleSource`'s own split), `os/src/kernel/` (the pool and its health state)

## Depends on

Nothing.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P9-02-01`](../stories/STORY-P9-02-01.md) | An entropy source with continuous health tests and a declared Tier 0 determinism carve-out | Specified |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0** · subject **C0** · boundary tests **BND-01, -17, -20**.

Entropy is a resource, and a bounded one: extraction is charged, rate-limited and never blocks an RT path (`PD-07` temporal isolation, `PD-08` finite charged resources). A caller that cannot be served gets a typed refusal, never a lower-quality substitute — silently degrading to a weaker source is the failure mode that makes every consumer's guarantee untrue at once.

## Exit criteria

Its Story **Verified** at Tier 0: entropy is available to kernel callers, health failures are detected and reported rather than absorbed, extraction holds a bounded cost, and the determinism carve-out is a declared, tested mode rather than a debug flag someone remembers to set.

**Named debt on exit.** Tier 0 cannot establish that the bits are *good* — QEMU's `RDSEED` is not a hardware entropy source, and statistical testing under emulation measures the emulator. What Tier 0 can establish is that the plumbing, the health-test logic and the failure paths are correct. Quality is `LE-09` debt and must be stated as such in the Report.
