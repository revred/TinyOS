# Handover 37 — EPIC-P1 Session Mandate: Pi 5 Hardware Direction, TDD Enforcement, Loose-Ends Register

Follows: [`36-epic-p1-determinism-proof-decomposition.md`](36-epic-p1-determinism-proof-decomposition.md). This is a **mandate for the next session**, not a record of new feature work — it captures four user directives given after reviewing the EPIC-P1 decomposition, resolves what they change, and hands the next session an unambiguous starting point.

## The four directives (2026-07-27) and what each changes

### 1. "Raspberry Pi 5 is the only viable hardware in the short term"

Recorded as governing in [`EPIC-P1.md`](../../goals/epics/EPIC-P1.md)'s Hardware & test tier section, superseding `SeedMVP.md` §9's generic "both MVP boards" line for this Epic's planning. The honest consequence, stated there and repeated here because it is a real planning fork:

- The Pi 5 is **ARM64**; TinyOS has an x86_64 HAL only. The arch-neutral `hal` crate (`topology`, `device`) was built for a future device-tree/ARM64 backend, but no ARM64 boot, timer, serial, or HAL backend exists — the current backlog parks all of that in `EPIC-P7`, and `README.md`'s test matrix lists the Pi 5 as Tier 1 "Phase 3 onward".
- Getting real timing evidence onto a Pi 5 therefore needs a **minimal ARM64 bring-up slice** (boot + timer + the measurement harness's cycle/serial primitives — not a full HAL) pulled forward from `EPIC-P7`, plus a deploy path (`EPIC-P1_5` territory — where `blue.atom`/`blue-sharc` prior-art patterns and the Pi 5 are already cross-referenced in `goals/epics/backlog.md`).
- **Decision the next sessions must sequence, not dodge** (loose end LE-09): whether the ARM64 slice starts in parallel with `FEAT-P1-01`'s QEMU-side harness work (the harness's kernel-side API should be arch-neutral from day one either way), or after `FEAT-P1-02`. Nothing in EPIC-P1's QEMU-tier work blocks on it; every *hardware* timing claim does.
- Until Pi 5 measurement exists, every timing Report carries hardware-tier evidence as named, dated release-blocking debt — unchanged rule, now with a named board.

### 2. "Test & TDD discipline should be maintained and enforced"

Reaffirmed as binding, no change to the standing rules (`agent/CODING_STANDARDS.md` §Test-Driven Development): a failing test precedes the implementation that makes it pass; Test documents are written when a Story starts (never pre-written in bulk — Handover 36 deliberately created zero `TEST-P1-*` docs for exactly this reason); adversarial tests are mandatory for the security/safety-relevant Features (`FEAT-P1-02/-03/-05`); property-based tests enter with `FEAT-P1-05`. **Enforcement stays mechanical where possible**: the assurance spine already blocks unmapped work, and `FEAT-P1-01`'s gate extends the same blocking discipline to timing. Reviewer-level enforcement (Red-before-Green in session records) is part of every EPIC-P1 handover's checklist from here on.

### 3. "Good to act on Goal List Discrepancy"

Confirmed acted on: `EPIC-P1.md` follows `SeedMVP.md` §9's Phase 1 row (G-SEC-12 **through -15**) and notes the supersession; the epic-backlog table row was updated to "-12 – -15" in Handover 36. No further action open.

### 4. "Open caveats are to be recorded as loose ends and we have to fix them soon"

Done — the register below is now the canonical list. Rule going forward: **every EPIC-P1 handover must carry this register forward, updating status per item**; a loose end leaves the list only by being fixed (with the fixing Story/Report named) or by an explicit user decision to retire it. "Soon" is interpreted as: each item is either closed or has a named owning Story *in progress* before EPIC-P1's midpoint (the `FEAT-P1-03` exit).

## Loose-ends register (canonical as of Handover 37)

| ID | Loose end | Origin | Owner / fix path | Status |
|---|---|---|---|---|
| LE-01 | Priority-inheritance behavioral half never verified (host-only bookkeeping proof) | `STORY-P0-02-03` | `STORY-P1-04-01` acceptance criterion 2 | Open — owned |
| LE-02 | WCET enforcement has no timer and no watchdog behind it | `STORY-P0-02-04`, twice re-surfaced | `STORY-P1-04-02` | Open — owned |
| LE-03 | No real `#PF`/`#GP`/`#UD` handling; every fault is terminal diverge-and-report | Handover 32 | `FEAT-P1-02` (`STORY-P1-02-01`) | Open — owned |
| LE-04 | No TSS/IST; a fault during fault handling triple-faults | Handover 32 | `STORY-P1-02-02` | Open — owned |
| LE-05 | `exec::AddressSpace` built but never installed; system runs all-RWX identity-mapped | `STORY-P0-05-02`, Handovers 32/33 | `FEAT-P1-03` (CR3 `-03-01`, W^X/teardown `-03-02`) | Open — owned |
| LE-06 | `pool-bench` fixture exits harness-error 2 (no isa-debug-exit handshake) | Handover 35 (concurrent session 34's fixture) | `STORY-P1-01-01` subsumes pool-bench onto the general harness | Open — owned |
| LE-07 | CI (GitHub Actions) has never been observed running any of this work; all "Verified" claims are local | Standing since Handover 07 | Phase-independent: trigger/observe a CI run | **Closed 2026-07-27** — the first push of this work (commit `cbaee41`) triggered CI: both QEMU jobs passed, lint failed on one `clippy::needless_lifetimes` in a test helper (local runs were `--lib`, CI runs `--all-targets`); fixed in `f1d7c90`, whose run `30226663769` is fully green — all three jobs including the assurance-spine and catalogue gates. "Verified (local; CI pending)" qualifiers are now simply "Verified" for everything in those runs |
| LE-08 | I/O APIC device-IRQ routing deferred (local APIC only); first real device interrupt has no route | `STORY-P0-04-02`/`-03` | Whichever Story first routes a device IRQ — likely `EPIC-P3` driver work; re-check when `FEAT-P1-06` picks its output primitive | Open — deferred with trigger |
| LE-09 | Pi 5 (ARM64) is the short-term hardware, but no ARM64 bring-up slice or deploy path exists; `EPIC-P7`/`EPIC-P1_5` ordering vs. this directive unreconciled | Directive 1 (this handover) | Sequencing decision + minimal ARM64 slice scoping — next session proposes, user decides | Open — decision needed |
| LE-10 | ECAM/MCFG config access and PCI bridge traversal deferred (legacy CAM, bus 0 only) | `STORY-P0-04-03` | First Story needing extended config space (`EPIC-P3` class drivers) | Open — deferred with trigger |

Dashboard visibility: the register is linked from [`goals/index.html`](../../goals/index.html)'s EPIC-P1 section so it cannot quietly disappear into session history.

## Next session — start here

1. **`STORY-P1-01-01`** (measurement harness), strict TDD: write the failing host tests for percentile/parsing logic and the Test doc **first**, then the harness; refactor `pool-bench` onto it (closes LE-06). Design the kernel-side measurement API arch-neutral (no `rdtsc` in the interface, cycle-source behind a trait) so the Pi 5 slice (LE-09) reuses it unchanged.
2. **LE-07 cheap probe**: push/trigger the existing CI workflow once and record the observed result (pass *or* fail, either is information) in the next Report before deep work — it has drifted for 30 handovers.
3. **LE-09 proposal**: a one-page scoping of the minimal ARM64/Pi 5 slice (boot + timer + serial + harness primitives + deploy path sketch) with two sequencing options (parallel with FEAT-P1-01 vs. after FEAT-P1-02) for the user to pick — decision, then work.
4. Then `STORY-P1-01-02` (baselines + `check-timing-regression` gate, demonstrated-to-fail), then `FEAT-P1-02` per the Epic's ordering.

## What this handover does not do

No implementation, no Test docs, no status changes to any Story (all ten EPIC-P1 Stories remain `specified`); no spine-relevant files touched by this handover itself (gates re-verified green after the doc edits: 14 Features / 35 Stories / 24 Tests / **30** Reports / 1,500 Story-performance contracts; xtask 23/23; catalogue 625/625). The Report count moved 29 → 30 mid-handover because the concurrent session filed [`REPORT-2026-07-27-01`](../../goals/reports/REPORT-2026-07-27-01.md) — its adversarially-verified pool-allocation (D07) guardrail-evidence report, which is also directly relevant to LE-06/`STORY-P1-01-01`: it scores real cycle data against `catalogue.tsv`'s numeric targets and is the honest template the general measurement harness's Reports should follow. The xtask count assertion was rebased accordingly.
