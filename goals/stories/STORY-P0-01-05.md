# STORY-P0-01-05 — A Guardrail Evidence Register, and the First Release Gates That Need No Hardware

Status: **Functionally Verified (Host), 2026-07-28** — assurance state `baseline-debt`; this Story records evidence for other Stories' gates and closes none of its own
Feature: [`FEAT-P0-01`](../features/FEAT-P0-01.md)
Introduced in: [`session/hand-2026-07-28/25-the-first-gates-that-need-no-hardware.md`](../../session/hand-2026-07-28/25-the-first-gates-that-need-no-hardware.md)

## Description

`LE-32`, made small enough to do in one sitting.

The dashboard's **0 Stories assurance-verified** is correct and will stay correct for a long time. What was wrong is that it was the *only* signal: the spine records one all-or-nothing state per Story, so a Story with 20 of its 23 release gates closed is indistinguishable from one with none, and no reader can tell a gate blocked on absent hardware from a gate blocked on nobody having looked.

This Story adds the missing granularity — a machine-checked register of which `PERF-Dnn-Gnn` ids have dated evidence — and then uses it to record the gates that genuinely need no hardware.

**The one worth doing first is `G11`, and it is not a measurement.** The guardrail asks for *heap allocations per steady-state work unit = 0*. There is no heap: every shipped crate is `#![no_std]`, none declares a `#[global_allocator]`, and none names `alloc` outside `#[cfg(test)]`. In Rust that is compiler-enforced rather than benchmark-observed — a crate that tried to allocate would fail to build. The result is **stronger** than the guardrail asks for (zero allocations in every state, not merely steady state) and completely independent of which CPU runs it.

## Depends on

`STORY-P0-01-04` (the same principle applied one level down: a declared thing nobody exercises is not evidence).

## Acceptance criteria

1. **A machine-checked evidence register.** **Met**: `goals/assurance/guardrail-evidence.tsv` is validated by `check-assurance-spine` like every other spine file — fixed header, no empty fields, `PERF-Dnn-Gnn` resolving to a real catalogue row, domain matching that row, Story existing, and **the Story must actually select the domain it claims evidence in**. That last check is the one the register exists for: evidence filed against a gate nobody's contract obliged them to close is the failure this prevents. Duplicates rejected. Every rejection case has a host test.
2. **No aggregate score, and no state derived from it.** **Met**: the register records only which gates have dated evidence. A gate absent from it is *unevidenced*, never *passed*, and no Story's assurance state is computed from row counts. The charter forbids hiding a failed gate behind a summary; it does not forbid recording which gates have evidence, and the spine was weaker for lacking that.
3. **The no-heap property is a gate, not an observation.** **Met**: `check-assurance-spine` asserts every shipped crate declares `no_std` and that none declares `#[global_allocator]`, `extern crate alloc`, or `use alloc::` outside `#[cfg(test)]`. The property was true by design; it is now true on purpose, so the day an allocator arrives the `G11` evidence is withdrawn by CI rather than silently invalidated. `#[cfg(test)]` is exempt deliberately — host tests link `std`, and `kernel::measure`'s tests use `String` today.
4. **`G11` recorded for built subsystems only — 10, not 17.** **Met**: `D01`, `D03`, `D04`, `D05`, `D06`, `D07`, `D08`, `D09`, `D11`, `D24`. Excluded by name: `D02` (`unbuilt`), `D10`/`D14` (`stand-in-only`), `D12`/`D13` (`specified`), `D22`/`D25` (`design`). **A guardrail cannot be closed for a subsystem that does not exist**, and the absence of a heap in unwritten code is evidence about nothing. Inflating this to 17 or 25 would have been the cheapest lie available and the hardest to spot.
5. **The Story-level zero is joined, never replaced.** **Met**: the dashboard still reads `0 / 56 Stories assurance-verified`, unaltered, beside a gate-level count that is not zero, labelled so the two cannot be confused.

## Named debt this Story leaves open

- **`G09` is not claimed.** Its cadence is also every-PR, but its method is a per-feature section-size delta against the parent commit, and `check-image-size` enforces one whole-image ceiling. Claiming the gate on the strength of a related measurement is the substitution this project refuses elsewhere. Follow-on.
- **`G21` is not claimed** — its every-PR half is fault containment, and its timing half is not hardware-free.
- **No Story becomes `verified`.** 10 gates of 391 in play across 17 selected domains. `LE-32` closes; `LE-31`'s audit does not.
- The register is written by hand. Nothing generates rows from Reports, so a Report can still land without its gates being recorded.

## Tests

[`TEST-P0-01-05-A`](../tests/TEST-P0-01-05-A.md) — written before implementation, per the TDD mandate.

## Reports

- [`REPORT-2026-07-28-08`](../reports/REPORT-2026-07-28-08.md)
