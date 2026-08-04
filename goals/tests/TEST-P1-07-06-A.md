# TEST-P1-07-06-A — The First Numbers That Are Not About QEMU

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P1-07-06`](../stories/STORY-P1-07-06.md)
Tier: Host unit tests (batched-iteration arithmetic, envelope parsing) **plus** a Tier 1 hardware measurement run on a Raspberry Pi 5, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D02`, `D04`, `D05`, `D07`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: `PERF-D02-G01`/`G04`, `PERF-D04-G01`/`G04`, `PERF-D05-G01`/`G04`, `PERF-D07-G01`/`G04` — median and worst-case for fault entry, context switch, dispatch and pool allocation. **This Story produces the first hardware evidence toward them and closes none of them**: a guardrail closes on a release-gate run in a declared deployment profile, not on a first bring-up measurement.

## What this test is for

`EPIC-P1` has four complete Features and a great deal of Tier 0 evidence, and every Report carries the same debt: `LE-09`. Handover 16 stated the consequence plainly — Tier 0 tail variance of 39–61% is a statement about QEMU and not about TinyOS, so **no evidence about this system's jitter exists yet.**

This Story is the one that changes that, and it is the only one in the Feature that closes `LE-09`.

It also carries the measurement change the coarse-counter problem forces, which turns out to be the same fix a second loose end has been waiting for.

## Specification

### 1. The envelope parses with **no change to the parser**

**Given** `fixture_measure` running on the board,
**then** it emits a `TOS64-MEAS/2` envelope that the existing `xtask` parser reads with **zero modifications**.

**And that clause is the point of the whole Feature's arch-neutrality claim.** `STORY-P1-01-03` demonstrated the seam on the host with an AArch64 `CycleSource` and no consumer change; this is the first time it meets a real target. If the parser must change, that is a finding worth more than a clean run — it means the seam was x86-shaped all along, and it is recorded as such rather than quietly patched.

### 2. Batched-iteration measurement, with N recorded and justified

**Given** any metric,
**then** it is measured as N iterations divided by N, rather than one operation per sample, and the Report states N per metric and why.

**And** the trade-off is stated rather than hidden: a batch large enough to beat quantisation and small enough not to hide the tail is a judgement, and presenting one N as self-evident conceals which way each metric was resolved.

**And** the batched shape is host-independent, which is the property `LE-24`'s row asks for by name.

### 3. `LE-24` closes on the batched shape

**Given** `D07/pool_u64x64_alloc_free_round_trip`,
**then** it produces a non-zero median on the board **and** on the Windows dev host, where it currently medians to 0 cycles because a single operation costs less than the harness's own calibrated subtraction.

**And** `LE-24` closes on that pair of results, not on the hardware one alone — the loose end is about the metric being host-dependent, so evidence from one host cannot close it.

### 4. The Report carries reproducibility metadata

**Given** the Report,
**then** it states board revision, firmware version, clock policy (governor, any throttling) and thermal state, per the measurement protocol.

**And this is the only defence that exists against the one risk in this Feature with no local detection**: numbers that arrive, parse, look plausible, and are wrong. Nothing on the board can catch that. A third party reproducing the run can.

### 5. The Report states what the numbers are **not**

**Given** the first non-Tier-0 numbers in this project's history,
**then** the Report names their scope explicitly: **single core** (cores 1–3 parked), **no preemption**, **no per-task address spaces**, **no `EL0`**, **no WCET enforcement**, **no verified boot**.

**And** it draws the distinction every Story in this Epic has drawn — *the mechanism was demonstrated* versus *the guardrail closed* — most sharply here, because these numbers will be quoted, and a hardware tier for the measured paths in this slice is not a hardware tier for `EPIC-P1`'s claims at large.

### 6. Tier 0 remains green and unchanged

**Given** this Story,
**then** no Tier 0 baseline is re-recorded, re-interpreted or retired, and the Tier 0 gate is green before and after.

**This Feature adds a tier; it does not replace one.**

### 7. The measured paths are the ones that were actually built

**Given** the fixture phases,
**then** each measured path is one this Feature actually brought up — fault entry (`D02`, `STORY-P1-07-02`), context switch (`D04`), dispatch (`D05`) and pool allocation (`D07`) — and **no phase is enabled whose underlying mechanism is not on this board.** A phase that reports a number for a mechanism that does not exist here is worse than a missing phase.

### 8. What this test explicitly does **not** establish

- **No comparative claim.** `G24`/`G25` run only after absolute release gates pass, on the same hardware and safety-equivalent configuration. Nothing here licenses a "faster than Linux" or "10× an RTOS" statement.
- **No hardware CI**, per the recorded §7.4 decision (b).
- **`LE-23` and `LE-18` are unaffected.** Both are about which *host* recorded the Tier 0 baseline; a hardware tier neither fixes nor worsens them.
- **No release-gate closure.** `PERF-*` guardrails close in a declared deployment profile, and this is a bring-up.

### Amended 2026-08-04, at implementation — the decisions this session owed and the facts the 2026-07-28 text could not know

Recorded rather than silently absorbed:

1. **The `ADR 0005` sentence: criterion 4 absorbs it.** The Story's named-debt section
   deferred one decision to the session that started the Story — whether "these numbers
   establish the tier, not a bound" becomes part of criterion 4's Report or a seventh Story
   for the qualification record. Decided: **criterion 4 absorbs the sentence.** The Report's
   "what the numbers are *not*" list gains *no worst-case bound, WCET claim or jitter
   envelope — `ADR 0005`, no secure-world qualification record exists for this platform*;
   the qualification record itself stays with `LE-33`'s open second condition and is not a
   Story here (`FEAT-P1-07` §6: a seventh Story means re-decomposing).
2. **Where the batching landed.** The "Implementation location" above says `os/src/hal/`;
   the shared harness actually lives in `kernel::measure`, and the batched shape landed as
   phase-level arithmetic in `kernel::measure_phases` — the module both architectures'
   fixtures now drive verbatim (the x86_64 binary and `kernel::fixture_measure_arm64`),
   which is a *stronger* form of the arch-neutrality claim than relocating the harness
   would have been. Clause 2's batch policy, per metric: the denial metric keeps its
   recorded 64; the round-trip twin uses **8** (large enough that the calibrated
   subtraction stops being the whole answer, small enough to stay near the batch-of-1
   regime the recorded 607-vs-58 superlinearity finding indicts at 64); every other metric
   is unbatched with `PMCCNTR_EL0` as the source, and the envelope's metric *names* carry
   the batch (`_per_op_of_N`), so the parser needed no new key.
3. **Clause 3's host half is measured**: on this Windows dev host — the exact host whose
   median-0 is `LE-24`'s subject — the batched twin medians **35 cycles/op (release,
   3 runs, min 33)** and is *gated*, with the eighth baseline row added on 2026-08-04 and
   none of the seven existing rows re-recorded (clause 6 checked: the timing gate reports
   no regression across 14 gated statistics). The board half lands with the capture.
4. **The evidence channel.** Serial has never produced a byte on this bench (`LE-47`), so
   the envelope leaves the board three ways at once: the PL011 (if it ever decodes), the
   canvas (the whole transcript at 1× scale, transcribable), and — the owner's standing
   "diagnosis moves onto the cable" — as `TOS64-*` Ethernet frames, one envelope line per
   beat, cycling, behind the same EtherType as the proven beacon.
   `cargo run -p xtask -- parse-meas <capture>` strips the capture container's framing and
   feeds the lines to the **unchanged** `timing::parse_stream` (clause 1: the parser is
   byte-for-byte untouched; only capture plumbing was added around it).
5. **The D02 metric's board name.** `fault_brk_capture_escape_kernel_context`: the victim
   raises `BRK` (the `ud2` sibling), the span runs fault-to-escape-decided through
   `STORY-P1-07-02`'s real vector table, and the name differs from x86_64's
   `capture_terminate` because `kernel::fault`'s disposition/audit half is x86_64-gated —
   stating in the metric name that a different span was timed, rather than borrowing a
   name the mechanism does not earn (clause 7's discipline).

### The board captures (to be quoted verbatim when the Tier 1 run happens)

Pending. The parsed envelope (`parse-meas` output), the packet-capture lines it came from,
and the canvas transcript land here, via the ground-truth file first.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal/src/`, `os/src/xtask/src/`) plus a Tier 1 hardware measurement run.

## Implementation location

- `os/src/hal/` — batched-iteration measurement in the shared harness (the change that also affects Tier 0 metrics, and therefore the one to land with the most care).
- `os/src/kernel/` — the AArch64 `fixture_measure` entry and its phase selection.
- `os/src/xtask/` — the `pi5` run path consuming the envelope, unchanged parser.

## Reports

To be filed when the Story goes Green. **`LE-09` closes on that Report and on nothing earlier.**
