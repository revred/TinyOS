# ADR 0015 — The Real-Time Claim Is Enforcement, Not a Measured Worst Case; and RT Evidence Must Be Measured Live

Status: **Accepted**
Date: 2026-08-06
Complements: [`ADR 0005`](0005-arm64-real-time-tier-is-conditional-on-secure-world-qualification.md) — which governs *when a bound is quotable*. This governs *whether the product claims one at all*
Introduced in: the owner's decision, 2026-08-06, after the risk review recorded in [`session/hand-2026-08-06/09C-the-guards-are-in-and-the-canvas-stops-painting-on-faith.md`](../../session/hand-2026-08-06/09C-the-guards-are-in-and-the-canvas-stops-painting-on-faith.md)

## Context

`ADR 0005` established that a worst-case bound is quotable only from a platform holding a
secure-world qualification record, and that zero platforms hold one. That is a rule about
**provenance**. It does not say what TinyOS claims when no such platform exists, and in
practice the answer has been *"the bound, with the debt named"* — which reads to an
outside reader as a bound.

Two facts made that untenable, both established on 2026-08-06 and neither disputed.

**Every timing number this project holds was produced under conditions the shipping
system will never run in.** Interrupts masked — the measure fixtures run before the park
loop with IRQs off, and `FEAT-P1-11` says outright that *"the round's cost with
interrupts live is unmeasured and is a different number from the fixture's masked one."*
Single core — `_start` parks cores 1–3 forever. Fixture, not product — `LE-20`/`LE-85`:
the actuation path has never run on the shipping `os` image, and neither preemption nor
deadline enforcement does either.

**The variance exceeds the margins.** `PERF-D03-G20` was refused at **55% run-to-run p99
CV**. `BOARD VERDICT 9` measured **~3% build-to-build movement on untouched code paths**.
`PERF-D11-G02` was refused for passing by **0.7%** on the correct reasoning that a
verdict which flips on a recompile is not a verdict. And `REF/fixed_integer_loop` — the
reference metric, the one thing in the envelope that should be flat — carries a `max` of
3915 against a `p99.9` of 3007, a **30% excursion on a fixed integer loop** that nothing
in the tree explains.

**A worst case cannot be sampled.** This is the category error underneath both. An
observed maximum over n=1000 is not a worst case and no quantity of sampling makes it
one. `PERF-D03-G04`/`D05-G04` are titled *"observed maximum and WCET bound"*, and those
are two different quantities sharing a row.

## Decision 1 — the claim is enforcement and violation reporting

**TinyOS claims that the system enforces a declared budget and reports every violation.
It does not claim a measured worst case.**

`G-PA-1`'s own second clause already said the provable half — *"enforced by the
scheduler, not merely observed in testing"* — and the enforcement **is** provable, today,
at Tier 0: the budget and deadline are declared, the monitor enforces them, and a
deliberate overrun trips the declared policy. That is a statement about the mechanism,
and the mechanism is in the tree with a positive control behind it.

### The distinction that makes this workable, and without which it would be an over-correction

**A *declared* WCET budget is a design input and stays. A *measured* worst case is a
claim about the system and goes.**

This matters because a blanket ban on the letters *WCET* would break the thing that
actually works. Two of the three places this claim appears were already correct:

- `README` — *"every task **declares** its period, deadline, and worst-case execution
  budget. The kernel **enforces** them and screams (loudly, safely) when they're
  violated."* **Unchanged.** This is the model sentence and it was written before the
  question was asked.
- `G-RT-3` — *"Every RT task **declares** a worst-case execution time budget; the
  scheduler and the CI timing regression suite both hold code to that budget."*
  **Unchanged.**
- `G-PA-1` — *"has a **bounded, tested worst-case latency** from decision to
  actuation"*. **Amended.** This is the only one that claims the system *has* a worst
  case rather than *is held to* a budget.

The rule, stated so a future reader can apply it without re-deriving it: **a budget is
something a task declares and the kernel enforces. A bound is something the system
claims. TinyOS declares and enforces; it does not claim.**

## Decision 2 — RT evidence must be measured under deployment conditions

**A measurement offered as real-time evidence must be taken with interrupts live, and its
conditions must be recorded as data rather than as prose.**

The conditions that must travel with every row: interrupt state, core count, image kind
(fixture or shipping), and platform. `guardrail-evidence.tsv` records none of them today —
they live in free-text `note` prose, so nothing checks them and nothing can refuse a row
for them. That is the `LE-89`/`LE-91` family a fourth time: a fact recorded *beside* the
thing it determines rather than derived from it.

### The cost, accepted rather than discovered

**This invalidates re-quoting the existing masked numbers as RT evidence, so the count
goes down before it goes up.** That is the point rather than a side effect. A number that
looks like evidence and is not is worse than a gap, because a gap is visible — `0/100`
assurance-verified is loud, and nobody deploys on it, while a p99 measured with interrupts
masked on a parked-core fixture and filed as `measured` looks exactly like proof.

The affected rows keep their value as **mechanism evidence** — they show the mechanism
works and what it costs under stated conditions. They stop counting as evidence about a
running system.

## Consequences

- **`G-PA-1` is amended** in `SeedMVP.md`, with the original sentence preserved and dated
  rather than overwritten.
- **`guardrail-evidence.tsv` gains condition columns**, and `bound_provenance.rs`'s
  refusal — which already declines a `G04` row from Tier 0, from `x86_64`, or from an
  unqualified platform — extends to decline an RT-claim row whose conditions do not
  support it.
- **Existing rows are backfilled with their actual conditions**, and the ones that lose
  RT standing are named rather than quietly reclassified.
- **`ADR 0005` is unchanged and still governs bounds.** If a platform ever holds a
  secure-world qualification record, a bound becomes quotable *and this ADR still applies*:
  the claim would then be *"the system enforces a declared budget, reports every
  violation, and on this qualified platform the observed distribution supports a bound of
  X with margin Y."* Enforcement remains the claim; the bound becomes an additional,
  sourced statement rather than a replacement for it.
- **Nothing here weakens what TinyOS does.** The deadline monitor, the WCET budget, the
  overrun policy and the spoor attribution are unchanged. What changes is the sentence
  written above them.

## What this ADR does not decide

It does not decide whether the `ADR 0005` Q1–Q4 qualification campaign runs for the Pi 5
(`LE-94`), nor the Tier 0 baseline question (`LE-23`). Both remain open owner decisions,
and both are upstream of ever quoting a bound at all.
