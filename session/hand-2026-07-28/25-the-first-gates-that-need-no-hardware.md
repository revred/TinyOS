# Handover 25 — `STORY-P0-01-05`: Ten Release Gates That Needed No Hardware, and a Zero That Now Has Company

Written at the close of 2026-07-28, immediately after [Handover 22](22-the-zero-is-real-but-its-reason-is-wrong.md), and in answer to one question: **what can be done right now to make the dashboard reflect where this project actually is, given that some things do not depend on hardware?**

Answer: **10 release gates now carry dated evidence, up from a number that was not zero but untracked.** The Story-level `0 / 56` is unchanged and correct, and stands beside it.

## What was wrong with the zero

Not that it was false. That it was the *only* signal. The spine recorded one all-or-nothing state per Story, so a Story with 20 of its 23 release gates closed was indistinguishable from one with none — and no reader could tell a gate blocked on an absent board from a gate blocked on nobody having looked. That was `LE-32`, and it **closes here**.

## The finding, and it is not a measurement

The catalogue's own cadence column already says which guardrails run *every PR* rather than on HIL: `G09`, `G11`, `G21`. Of those, **`G11` is the strongest case in the entire 625-cell catalogue, because it is not a benchmark at all.**

`G11` asks for *heap allocations per steady-state work unit = 0; pool claims are separately counted.*

**There is no heap.** Every shipped crate — `hal`, `hal-arm64`, `hal-x86_64`, `exec`, `kernel`, `os` — is `#![no_std]`, none declares a `#[global_allocator]`, and none names `extern crate alloc` or `use alloc::` outside `#[cfg(test)]`. In Rust that is **compiler-enforced, not benchmark-observed**: a `no_std` crate without a global allocator cannot use `alloc` and fails to build if it tries.

The recorded result is therefore *stronger than the guardrail's own wording asks for* — zero heap allocations in **every** state, not merely steady state, on every architecture, under every load, with no measurement uncertainty and no hardware. It was true by design; it is now true on purpose.

## What was built

- **`goals/assurance/guardrail-evidence.tsv`** — which `PERF-Dnn-Gnn` gates carry dated evidence, validated by `check-assurance-spine` like every other spine file. The check that gives it value: **a Story may only file evidence in a domain its own contract selects.** Evidence filed against a gate nobody was obliged to close is a more convincing way to be wrong than having no register at all. Seven host tests, one per rejection case.
- **A no-heap gate** in the same check. Deliberately falsified before being trusted, per the discipline `fixture-broken-boot` established and `STORY-P0-01-04` found nine violations of. Appending `use alloc::boxed::Box;` to `kernel/src/lib.rs` produces:

  > `no-heap gate: os/src/kernel/src/lib.rs:27 contains ``use alloc::`` outside #[cfg(test)]; every PERF-Dnn-G11 row in guardrail-evidence.tsv rests on this system having no heap, so add an allocator only by withdrawing that evidence first`

  The message names the consequence rather than the rule, so whoever trips it learns what it costs. Reverted; green again.
- **`STORY-P0-01-05`**, `TEST-P0-01-05-A` written first, and [`REPORT-2026-07-28-08`](../../goals/reports/REPORT-2026-07-28-08.md).

137 xtask tests pass; clippy clean.

## Ten, not seventeen — and this is the part that mattered

Seventeen domains are selected by at least one Story. Evidence was recorded for **ten**: `D01`, `D03`, `D04`, `D05`, `D06`, `D07`, `D08`, `D09`, `D11`, `D24`.

Excluded by name on the catalogue's own `readiness` field: `D02` (`unbuilt`), `D10` and `D14` (`stand-in-only`), `D12` and `D13` (`specified`), `D22` and `D25` (`design`).

**A guardrail cannot be closed for a subsystem that does not exist**, and the absence of a heap in code nobody has written is evidence about nothing. Recording all seventeen would have validated cleanly, read as a better number, and been the cheapest lie available in this repository — the rows would pass every check and nothing would be wrong except the meaning. That is the whole reason the exclusions are listed by name in the Report rather than summarised.

One caveat flagged rather than resolved: **`D08`'s recorded readiness is `prototype-inactive`, which `FEAT-P1-03` made stale** — address spaces are active on the shipping image. It was included on the strength of the subsystem existing, but the readiness column has drifted and deserves a pass.

## What did not change, deliberately

- **`0 / 56 Stories assurance-verified` is untouched.** Ten gates of **391** in play (17 domains × 23 release gates). The Story-level zero is *joined*, never replaced — swapping an uncomfortable true signal for a friendlier one is precisely the failure this Story exists to avoid, and it would have been easy.
- **`G09` is not claimed**, though its cadence is also every-PR. Its method is a per-feature section-size delta against the parent commit; `check-image-size` enforces one whole-image ceiling. Different measurement. Claiming a gate on the strength of a related one is the substitution this project refuses elsewhere.
- **`G21` is not claimed** — its every-PR half is fault containment, its other half is timing.
- **Nothing about latency, cycles, tails, throughput, soak, or isolation.** Those are `LE-09`'s.
- **The register is hand-written.** Nothing derives rows from Reports, so a Report can still land without its gates being recorded. Named as this Story's own debt.

## What this suggests for next

The general shape is worth noticing: **`LE-09` is the largest blocker but it is not the only kind of blocker, and some gates were never waiting on hardware at all.** Nobody had checked which.

That is `LE-31`, still open, and now clearly the highest-value non-hardware work in the project: a per-Story statement of what actually blocks `verified`. This Story did the easiest slice of that audit by hand and found ten gates sitting there. The rest of the audit is unlikely to be empty.

[Handover 21](21-next-session-mandate.md) still stands for the board work.

## State at the close

```text
assurance spine   23 Features, 57 Stories, 44 Tests, 45 Reports
                  32 loose ends (21 open), 83 status headers
                  10 release gates with dated evidence   <-- new
Stories verified  0 / 56 -- unchanged, and correct
Release gates     10 of 391 in play
LE-32             CLOSED
LE-31             open, and now the clearest non-hardware work in the project
```
