# TEST-P0-01-05-A — A Guardrail Evidence Register, and the First Release Gates That Close Without Hardware

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P0-01-05`](../stories/STORY-P0-01-05.md)
Tier: Host unit tests only — this Story is deliberately hardware-free, and that is its point
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`, `BND-18`
Protection Domain contracts: `PD-02`, `PD-12`, `PD-13`, `PD-14`
Code admission gates: `RCG-05`, `RCG-06`, `RCG-07`, `RCG-12`, `RCG-14`
Assurance state: `baseline-debt`

## What this test is for

The dashboard reports **0 Stories assurance-verified**, and that is correct: a Story reaches `verified` only when every applicable release gate closes, which for most domains includes a 72-hour soak and a million-operation tail measurement on hardware this project does not have.

But the zero is also **uninformative in a way that is a defect rather than a fact** (`LE-32`). The spine records one all-or-nothing state per Story, so a Story with 20 of its 23 release gates closed is indistinguishable from one with none. No amount of genuine evidence becomes visible until a threshold flips, and no reader can tell which gates are blocked on hardware from which are blocked on nobody having looked.

Two things follow, and this Story does both:

1. **Record evidence at the granularity the gates are defined at.** Which `PERF-Dnn-Gnn` ids have dated evidence, in a machine-checked register beside the contract rows.
2. **Close the ones that need no hardware.** The catalogue's own cadence column says `G09`, `G11` and `G21` run *every PR*, not on HIL. And `G11` is the strongest case in the whole catalogue, because it is not a measurement at all.

**`G11` is `heap allocations per steady-state work unit = 0; pool claims are separately counted`. There is no heap.** Every shipped crate is `#![no_std]`, none declares a `#[global_allocator]`, none names `extern crate alloc`, and in Rust that combination is enforced by the compiler rather than observed by a benchmark: a crate that tried to allocate would not build. That is a **stronger** result than the guardrail asks for — zero allocations in *every* state, not merely in steady state — and it is completely independent of which CPU runs it.

## Specification

### 1. The register exists and is machine-checked

**Given** `goals/assurance/guardrail-evidence.tsv`,
**then** `check-assurance-spine` validates it exactly as it validates the contract rows and the loose-ends register: a fixed header, no empty fields, a `PERF-Dnn-Gnn` id that resolves to a real catalogue row, a domain matching that row's domain, a Story that exists in `story-contracts.tsv`, and a Story that actually **selects** the domain it claims evidence in.

**And** an evidence row for a guardrail whose Story does not select that domain is rejected. That is the failure mode the whole register exists to prevent: evidence filed against a gate nobody's contract obliged them to close.

**And** duplicate `(guardrail, story)` pairs are rejected.

### 2. No aggregate score, no hidden failure (`SEC-19`, the charter's rule)

**Given** the register,
**then** it records only *which gates have dated evidence*, and carries **no pass/fail rollup, no percentage, and no score**.

**And this is the distinction that makes the register legal under the assurance README's rule** that no mapped release gate may be "failed, missing, waived silently, or hidden by an aggregate score". That rule forbids concealing a failed gate behind a summary. It does not forbid — and the spine is weaker for lacking — a record of which gates have evidence at all. A gate absent from the register is *unevidenced*, which is exactly what it is, and never *passed*.

**And** a Story's assurance state is **not** derived from the register. `baseline-debt` does not become `verified` because rows accumulated; that conversion still requires every applicable gate, by hand, with Reports.

### 3. The no-heap property is a gate, not an observation

**Given** the shipped crates — `hal`, `hal-arm64`, `hal-x86_64`, `exec`, `kernel`, `os` —
**then** a host-run check asserts that each declares `no_std`, and that **none** declares `#[global_allocator]`, names `extern crate alloc`, or imports `alloc::` outside `#[cfg(test)]`.

**And** it fails when any of those appears. The property is true today by accident of design; this clause makes it true on purpose, so that the day someone adds an allocator the evidence for `G11` is withdrawn by CI rather than silently invalidated.

**And** `#[cfg(test)]` code is deliberately exempt: host tests link `std` on purpose, and `kernel::measure`'s tests use `String` today. The claim is about the **shipped image**, and conflating the two would either make the check unpassable or make it meaningless.

### 4. `G11` is claimed for built subsystems only

**Given** the 17 domains selected by at least one Story,
**then** `G11` evidence is recorded for exactly the **10** whose catalogue `readiness` says the subsystem exists — `D01`, `D03`, `D04`, `D05`, `D06`, `D07`, `D08`, `D09`, `D11`, `D24` —

**and not** for `D02` (`unbuilt`), `D10` and `D14` (`stand-in-only`), `D12` and `D13` (`specified`), `D22` and `D25` (`design`). **A guardrail cannot be closed for a subsystem that does not exist**, and the absence of a heap in code nobody has written is not evidence about anything.

**And** the count is 10, not 17 and not 25, and the Report states why the other 7 were excluded by name. Inflating this number would be the cheapest possible lie in this repository and the hardest to spot.

### 5. What is deliberately *not* claimed

- **`G09` (image and feature footprint) is not claimed**, though its cadence is also every-PR. Its method is "compare stripped release map files and section sizes against the parent commit", and `check-image-size` today enforces one whole-image ceiling rather than a per-feature delta. That is a different measurement, and claiming the gate on the strength of a related one is exactly the substitution this project refuses elsewhere. Named as follow-on.
- **`G21` is not claimed** for the same shape of reason: its every-PR cadence covers fault-containment completion, and the *timing* half of it is not hardware-free.
- **No Story becomes `verified`.** 10 gates of 391 in play. The number that changes is the gate-level one, which was previously not tracked at all rather than zero.

### 6. The zero stays, and is joined

**Given** the dashboard,
**then** `0 / 56 Stories assurance-verified` remains, unaltered, beside a gate-level count that is not zero.

**And** the two are labelled so they cannot be confused. Replacing the Story-level zero with a friendlier gate-level number would be the failure this Story exists to avoid — the point is to add a true signal, never to retire an uncomfortable one.

## Test type

Host unit tests in `os/src/xtask/src/assurance.rs` (register validation, every rejection case) and a host-run source check for the no-heap property.

## Implementation location

- `goals/assurance/guardrail-evidence.tsv` — the register.
- `os/src/xtask/src/assurance.rs` — `validate_guardrail_evidence`, wired into `check_assurance_spine`, and the no-heap gate.

## Reports

- [`REPORT-2026-07-28-08`](../reports/REPORT-2026-07-28-08.md) — the 10 recorded gates, the 7 exclusions by name, and what is still unevidenced.
