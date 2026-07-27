# Handover 05 — `STORY-P1-01-02`: A Timing Gate, and the Two Tolerances It Took to Find an Honest One

Follows: [`04-story-p1-01-03-arm64-timebase.md`](04-story-p1-01-03-arm64-timebase.md). Evidence: [`REPORT-2026-07-27-04`](../../goals/reports/REPORT-2026-07-27-04.md). This closes `FEAT-P1-01` — all three of its Stories are now functionally Verified.

## What this session did

Delivered `STORY-P1-01-02` under strict TDD: committed timing baselines, the `check-timing-regression` gate, CI wiring, and — the criterion the Story exists for — a demonstration that the gate actually fails.

- **Test document first** ([`TEST-P1-01-02-A`](../../goals/tests/TEST-P1-01-02-A.md)), nine clauses. **Red recorded**: 28 failing tests (1 kernel, 27 `xtask`). Green with no test edited.
- **The gate**: `cargo run -p xtask -- check-timing-regression [--runs=N] [--baseline=PATH] [--update-baseline --date=YYYY-MM-DD] [--inject-regression]`. Exit **0** pass, **1** regression, **2** harness error.
- **Committed baselines**: [`goals/performance/baselines/tier0-x86_64.tsv`](../../goals/performance/baselines/tier0-x86_64.tsv) — five metrics, median of five release-profile runs, each row carrying tier/arch/profile/cycle_source/runs/date. Provenance is enforced, not decorative: a T1 run against a T0 baseline, or a release run against a dev baseline, is refused outright rather than absorbed into a tolerance.
- **Fails closed on everything**: bad header, wrong field count, non-numeric column, `min > p50`, `runs=0`, empty field, duplicate key, header-only file — one host test each. A **missing** baseline is a gate failure, not a skip.
- **Wired into CI** the same day, in the QEMU job, after the two existing `measure` smoke steps.
- **Two loose ends closed**: `LE-13` (measurement now runs **release** profile — the dev-profile numbers were gating a binary nobody ships) and `LE-09` **piece 4** (the UART-borne pass/fail bit).

## The result worth reading: the gate's sensitivity is poor, and that is the finding

Three tolerance constants, two of them falsified by evidence:

1. **20%** — chosen from `REPORT-2026-07-27-02`'s conclusion that `min`/`p50` were the *stable* statistics. Five release-profile runs then showed those "stable" statistics moving **+23% to +28%** run-to-run. This constant would have failed green code on its first CI run.
2. **40%** — clears all of that. Falsified by the gate's **own first run**: D07 alloc/free `min` came back at 92 against a 66 baseline (**+39%**, landing exactly on the limit) on unchanged code, minutes later.
3. **60% relative + a 24-cycle floor** — committed.

Which means: **at Tier 0 this gate catches regressions of ~1.6x or worse, and nothing finer.** It is a tripwire for an accidental O(n) in a selection loop or a lock added to an RT path. It is not a defense against a 10% creep, and no choice of constant makes TCG tighter. Recorded as new loose end **`LE-16`**, and it is the most concrete argument this Epic has produced for `LE-09`'s hardware tier: the numbers a gate can actually defend need a board.

## Seen to fail, not asserted to fail

`--inject-regression` builds `fixture-measure-regression`, a never-shipped Cargo feature (`fixture-broken-boot`'s precedent) that performs seven extra `highest_priority_ready` selections **inside the timed region**:

```
D05/dispatch_select_highest_priority_ready  min baseline=74  observed=1048 limit=118 REGRESSED
D05/dispatch_select_highest_priority_ready  p50 baseline=76  observed=1086 limit=121 REGRESSED
D04/context_switch_yield_roundtrip_2switches min baseline=216 observed=212 limit=345 ok
xtask: 2 gated statistic(s) regressed beyond tolerance   → exit 1
```

Real measured code, ~14x over baseline, caught and localized — the other four metrics still passed. A doctored baseline file would have proven only that the comparison arithmetic works; this proves the whole path (build → boot → measure → compare → fail) does, and anyone can re-run it.

## `LE-09` piece 4: cross-checked rather than trusted

The verdict now travels as `TINYOS-RESULT/1 fixture=measure ok=true`, emitted by both measurement fixtures. On Tier 0 the host reads **both** it and the QEMU `isa-debug-exit` code and requires them to **agree** — disagreement is a harness error naming both. New machinery that reports "pass" is exactly the machinery not to trust, so when the Pi 5 arrives and the UART bit is the only bit there is, it will already have been validated against an independent signal on another architecture.

## Verification

`cargo test --workspace --lib` 210 · `cargo test -p xtask` 79 · `cargo fmt --all -- --check` clean · scoped `clippy -D warnings` clean · `check-assurance-spine`: 14 Features, 36 Stories, 27 Tests, 33 Reports · `check-crate-sizes`: `xtask` 4071, `kernel` 3480 lines · gate exit 0 clean, exit 1 injected.

Clippy stays scoped rather than `--workspace --all-targets` for the pre-existing Windows/ELF reason `REPORT-2026-07-26-09` recorded; Linux CI is the authoritative `--all-targets` gate.

## Loose-ends register (canonical as of this handover)

Carried forward from [Handover 04](04-story-p1-01-03-arm64-timebase.md#loose-ends-register-canonical-as-of-this-handover); one new item, two closed.

| ID | Loose end | Origin | Owner / fix path | Status |
|---|---|---|---|---|
| LE-01 | Priority-inheritance behavioral half never verified | `STORY-P0-02-03` | `STORY-P1-04-01` criterion 2 | Open — owned |
| LE-02 | WCET enforcement has no timer and no watchdog behind it | `STORY-P0-02-04` | `STORY-P1-04-02` | Open — owned |
| LE-03 | No real `#PF`/`#GP`/`#UD` handling; every fault is terminal diverge-and-report | Handover 32 | `FEAT-P1-02` (`STORY-P1-02-01`); also route `#XF` | Open — owned |
| LE-04 | No TSS/IST; a fault during fault handling triple-faults | Handover 32 | `STORY-P1-02-02` | Open — owned |
| LE-05 | `exec::AddressSpace` built but never installed; system runs all-RWX identity-mapped | `STORY-P0-05-02` | `FEAT-P1-03` | Open — owned |
| LE-06 | `pool-bench` was a divergent sibling harness | Handover 35 | `STORY-P1-01-01` | **Closed 2026-07-27** |
| LE-07 | CI has never been observed running any of this work | Standing since Handover 07 | Phase-independent | **Closed 2026-07-27** |
| LE-08 | I/O APIC device-IRQ routing deferred (local APIC only) | `STORY-P0-04-02`/`-03` | First Story routing a device IRQ | Open — deferred with trigger |
| LE-09 | Pi 5 (ARM64) is the short-term hardware, but no ARM64 bring-up slice or deploy path exists | Handover 37 directive 1 | Option B with the carve-out ([Handover 03](03-le-09-arm64-pi5-slice-proposal.md)) | Open — **pieces 3 and 4 now delivered** (cycle source/timebase in `STORY-P1-01-03`; UART-borne pass/fail here, cross-checked against the exit code). Pieces 1, 2 and 5 wait for `FEAT-P1-02`. The item leaves this register only when a Pi 5 has produced a parsed measurement, and no board is recorded as purchased |
| LE-10 | ECAM/MCFG config access and PCI bridge traversal deferred | `STORY-P0-04-03` | First Story needing extended config space | Open — deferred with trigger |
| LE-11 | `Context::new` seeds task `rflags` with `IF` set, enabling interrupts with no IDT installed | `STORY-P1-01-01` | `FEAT-P1-02` | Open — owned |
| LE-12 | CI's clippy never lints target-only fixture code | `STORY-P1-01-01` | Per-fixture target clippy in the CI lint job. **Now larger**: `fixture-measure-regression` is a third unlinted fixture feature | Open — unowned, needs a Story |
| LE-13 | Measurement ran dev-profile (unoptimized) binaries | `STORY-P1-01-01` | `STORY-P1-01-02` | **Closed 2026-07-27** — `--profile=release`; the gate always measures release, and baselines record the profile they were taken at |
| LE-14 | `context::switch` saves no SSE/x87 state | `STORY-P1-01-01` / ADR 0003 | `FEAT-P1-04` | Open — owned |
| LE-15 | The AArch64 generic timer is a 54 MHz system counter (~18.5 ns/tick), so hardware metrics will be quantization-limited; `PMCCNTR_EL0` is finer but not architecturally guaranteed accessible | `STORY-P1-01-03` | Decide when a board exists; also an input to any hardware-tier tolerance | Open — owned |
| **LE-16** | The Tier 0 timing gate can only detect regressions of ~**1.6x or worse** (60% + 24-cycle tolerance), because release-profile run-to-run spread on the *gated* statistics reaches +39% on unchanged code. A smaller regression passes silently, and no Tier 0 work can improve this | `STORY-P1-01-02` (this handover) | Only a hardware tier fixes it (`LE-09`). Until then, do not describe this gate as protecting against small regressions — it does not | Open — owned, bounded by `LE-09` |

## Next session — start here

1. **`FEAT-P1-02` — real exception handling** (`STORY-P1-02-01` fault capture and terminate-vs-resume policy, `STORY-P1-02-02` double-fault IST). This is the Epic's ordering, it carries `LE-03`/`LE-04`/`LE-11`, and its exit is what unblocks `LE-09`'s remaining pieces — on a Pi 5 with no `isa-debug-exit` and no fault handling, a fault is a silent hang with no output at all.
2. **When `FEAT-P1-02` exits, the board question becomes live**: a Raspberry Pi 5, an SD card and a USB-TTL serial cable. Nothing in this repository records one as purchased, and it is now the only thing between the harness and a Tier 1 number — the gate, the baselines, the arch-neutral cycle source and the UART pass/fail bit are all in place and waiting for it.
3. **`LE-12` is cheap and now slightly larger** (a third unlinted fixture feature). One CI lint-job change.

## What this handover does not do

No hardware ran anything. No `PERF-D04`/`D05`/`D07` guardrail is closed and no Story is `verified` — every number in the committed baselines is Tier 0 QEMU/TCG emulation of a mechanism, not evidence about silicon. `FEAT-P1-02` through `-06` remain untouched, and their eight Stories remain `specified`. What changed is that a timing regression now fails a PR — within the honest, measured, 1.6x limit of what Tier 0 can see.
