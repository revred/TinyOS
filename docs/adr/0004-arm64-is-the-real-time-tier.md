# ADR 0004 — ARM64 Is the Real-Time Tier; x86_64 Keeps Throughput and Rich-Workload Claims

Status: **Superseded by [`ADR 0005`](0005-arm64-real-time-tier-is-conditional-on-secure-world-qualification.md)** (2026-07-28)
Date: 2026-07-28
Introduced in: [`session/hand-2026-07-28/17-raspberry-pi-5-bring-up-plan.md`](../../session/hand-2026-07-28/17-raspberry-pi-5-bring-up-plan.md) §7.1, confirmed by the user and recorded in [`19-feat-p1-07-acceptance-and-spine.md`](../../session/hand-2026-07-28/19-feat-p1-07-acceptance-and-spine.md)

> **Superseded, and this body is preserved unedited below.** `LE-39` found that the sentence
> *"Interrupt masking at `EL1` means what it says"* (in Context, last paragraph) is not true against a
> GIC with secure interrupt groups routed to `EL3` by `SCR_EL3.FIQ` — structurally the same hole this
> ADR disqualifies x86_64 for. [`ADR 0005`](0005-arm64-real-time-tier-is-conditional-on-secure-world-qualification.md)
> supersedes this document and makes the ARM64 real-time tier **conditional on per-platform
> secure-world qualification** rather than automatic. This ADR's case against x86_64 is unchanged and
> is restated there in full. **No measurement is invalidated by either document.** Nothing here is
> edited beyond this note, because `README.md`, `EPIC-P1` and the Handover series cite this text.

## Context

TinyOS's founding intent is a system whose real-time bounds are *proven*, not asserted. `EPIC-P1` exists to convert asserted determinism into measured determinism, and `agent/CODING_STANDARDS.md`'s priority ordering puts safety and correctness above performance — a worst-case bound that cannot be proven is not a bound.

Development has been x86_64-first for practical reasons: the dev host is x86_64, QEMU `q35` is the Tier 0 gate, and `hal-x86_64` is the only complete HAL. The README's hardware matrix reflects that history — it lists ARM64 boards as portability checks, and Raspberry Pi specifically at "Phase 3 onward, once the bus stack exists."

A Raspberry Pi 5 is now in hand, `FEAT-P1-07` brings it up, and the question of which architecture carries which claim can no longer be deferred by circumstance.

There is a technical argument that x86_64 cannot carry the real-time claim at all, and it is not about performance:

**System Management Interrupts are invisible to the operating system and unbounded in duration.** SMM is entered by firmware, at a privilege level above the kernel, with no notification to the OS before or after. The OS cannot mask an SMI, cannot observe that one occurred, cannot bound how long it ran, and cannot attribute the resulting latency to anything. Firmware-dependent SMI latencies in the hundreds of microseconds to low milliseconds are routinely reported on ordinary consumer hardware, and they vary by vendor, by firmware revision and by what the platform happens to be doing (thermal management, ECC handling, USB legacy emulation, TPM work).

The consequence is precise: on x86_64, **any worst-case latency bound TinyOS states is a claim about the firmware, not a claim about TinyOS.** It cannot be made stronger by better OS engineering, because the perturbation is by design outside the OS's authority. Measurement does not rescue it either — an SMI that did not fire during a measurement campaign is not an SMI that cannot fire.

AArch64 has no SMM equivalent. It has higher exception levels (`EL2`, `EL3`) and firmware that runs in them, but their entry is not an invisible, unmaskable, unattributable trap of the kind SMM defines. Interrupt masking at `EL1` means what it says.

## Decision

**ARM64 is TinyOS's real-time tier of record.** Worst-case latency bounds, WCET claims, jitter envelopes and every `G-RT-*` and `G-PA-*` guarantee are stated and gated on ARM64 hardware.

**x86_64 is retained as a full first-class target for throughput, rich-workload, host-bridge and developer-experience claims** — and remains the Tier 0 CI gate. It is not deprecated, not de-scoped, and not reduced in test coverage.

What changes is which architecture a *bound* may be quoted from.

## Rationale

- **Safety before security before correctness before performance.** A real-time guarantee whose worst case is set by a third party's firmware is not a guarantee, and stating it as one would be the first rule's exact inversion.
- **The distinction is honest about what each architecture is good at.** x86_64 machines are where the rich workloads, the host bridge and the large-memory inference cases live, and none of those claims are worst-case claims. Nothing about this decision weakens them.
- **It costs nothing today and would cost a great deal later.** `FEAT-P1-07` is the first hardware bring-up. Deciding now means the first hardware numbers are filed against the tier they belong to; deciding after a body of x86_64 "worst-case" numbers exists would mean retracting published claims.
- **It is falsifiable.** If a bounded, attributable SMI mechanism is demonstrated on a specific x86_64 platform — some server-class firmware exposes SMI counters and quiescence modes — that platform can be re-argued on its evidence. The decision is about what may be claimed without such evidence, not about x86_64 being unfixable in principle.

## Consequences

- **`README.md`'s hardware matrix is reconciled** to say which tier carries which claim, and its "Raspberry Pi from Phase 3 onward" line is corrected: the Pi 5 is `EPIC-P1`'s first physical timing target, being brought up now by `FEAT-P1-07`.
- **Every existing timing Report stays valid and stays Tier 0.** No Tier 0 number is retracted or reinterpreted. What this ADR forbids is *promoting* an x86_64 measurement into a worst-case bound; it does not touch the measurements themselves, and `LE-23`/`LE-18`/`LE-16` are unaffected.
- **`EPIC-P1`'s remaining Features inherit a caveat.** `FEAT-P1-06` (deterministic actuation, the Epic's flagship `G-PA-1` path) states a *bound*. Under this ADR that bound is quotable from ARM64 hardware; a Tier 0 or x86_64 run of it is mechanism evidence, not the bound.
- **The Jetson Orin Nano's role is unchanged and now better justified** — it is ARM64, it is the committed primary edge target, and it is the natural second board once `FEAT-P1-07`'s slice generalizes.
- **`SeedMVP.md`'s founding intent is untouched.** This is a claim-attribution decision within the existing hardware scope, not a change to what TinyOS is for. Neither this ADR nor `README.md` may weaken the Security Charter, and nothing here does.
- **This ADR asserts nothing about SMI magnitudes on any specific machine.** It rests on the structural property — invisible, unmaskable, unattributable — which holds regardless of how large any particular firmware's SMI latency turns out to be. No measurement in this repository is offered in support of it, and none is needed for the argument as stated.
