# Handover 33 — Cover Note: Security Charter Confirmed as Governing Authority; Two Small Fixes; Next-Session Priorities

Follows: [`32-story-p0-04-02-idt-apic-bring-up.md`](32-story-p0-04-02-idt-apic-bring-up.md).

This is a mandate for the next session, not a record of new feature work — the only code changes this session are two small corrections surfaced by a parallel security-review agent's pass over the repository (detailed below). No Story moved status.

## What the parallel security review confirmed

A security-review agent audited the concurrent Security Charter work (Handovers 30/31) independently and reported it complete and governing:

- **[`SECURITY_CHARTER.md`](../../SECURITY_CHARTER.md)** is now TinyOS's governing design and release authority — subordinate only to `SeedMVP.md`'s founding intent and `agent/CODING_STANDARDS.md`'s safety-first priority order, binding on every Feature/Story/Test/Report/deployment profile/release.
- **14 Protection Domain contracts** ([`goals/security/protection-domain-contracts.tsv`](../../goals/security/protection-domain-contracts.tsv)), **14 Remote Code Admission gates** ([`code-admission-gates.tsv`](../../goals/security/code-admission-gates.tsv)), and the **complete 25-pair C0–C4 containment matrix** ([`class-communication-matrix.tsv`](../../goals/security/class-communication-matrix.tsv)) are all present and machine-checked.
- **This resolves the open question Handover 32 left for the user**: whether the concurrent PD/RCG/class-communication-pair work already satisfied Handover 29's "explicit machine-readable source→destination transition/communication matrix" ask. Confirmed yes — `class-communication-matrix.tsv` *is* that matrix, enumerating every ordered C0–C4 pair with its authority-transfer rule. That thread is now closed.
- **The only permitted remote-data-to-code route** is now a charter-level invariant, not just documentation: C2 transport → immutable C4 quarantine → disposable parsing → hash/type/dependency verification → signature/revocation/rollback checks → manifest ∩ local policy → destroy C4 environment → fresh C3 domain → sealed RX/RO/RW-NX mappings → explicit activation. No route exists for remote trust-root enrolment, writable executable memory, W→X conversion, cross-process memory modification, generic shell/eval execution, in-place promotion, or C4 authority transfer.
- The contracts are wired into all 8 Features and (as of this session's fix, see below) all 23 Test documents; `check-assurance-spine` blocks release on any missing PD contract, admission gate, containment pair, Feature ownership, Test inheritance, security control, boundary test, or performance contract.
- The charter is explicit that it does **not** claim TinyOS is "unhackable" — the required-but-still-missing runtime evidence is named directly: active per-task address spaces, production capability spaces, executable sealing, IOMMU isolation, teardown, quarantine/provenance, immutable updates, hostile-campaign testing. Until that evidence exists, the charter itself requires the claim to stay "architecture established; runtime assurance incomplete" — the same distinction this project's assurance spine has enforced since Handover 27 (`baseline-debt` vs. `verified`).

## Two things this session actually fixed

The review flagged one open item and one thing it deliberately left alone, both now resolved:

1. **A real regression, not just a lint nit**: `cargo test -p xtask` had one failing test, `assurance::tests::committed_assurance_spine_is_complete`, hardcoding `test_count == 22` and `report_count == 26` — stale since Handover 32 added `TEST-P0-04-02-A` (Test #23) and `REPORT-2026-07-26-27` (Report #27) in between the charter work landing and this check being run. Updated both hardcoded expectations to `23`/`27`; `cargo test -p xtask` is 21/21 again.
2. **The formatter finding the review flagged and deliberately left untouched** ("one unrelated formatter finding remains at `os/src/hal-x86_64/src/idt.rs:62`; I left that concurrent IDT work untouched") was already resolved by the time this session ran — `cargo fmt --check` passes clean workspace-wide. No action was needed; recorded here so the next reader doesn't go looking for a problem that isn't there.
3. **`goals/index.html` dark-mode fix, twice.** The user asked for the dashboard to default to dark mode; the first pass made dark the `:root` default with a `@media (prefers-color-scheme: light)` override for an explicit light preference — technically "dark by default," but the user reported it still rendering light (their system/browser evidently signals a light preference, which matched that override exactly as designed, just not as intended). Corrected to unconditional dark: removed the light-mode media query entirely, so the dashboard renders the same dark theme for every viewer regardless of OS/browser color-scheme setting.

## Full verification (this session)

- `cargo test --workspace --lib`: **152/152** (`exec` 51, `hal` 4, `hal-x86_64` 37, `kernel` 60) — unchanged from Handover 32.
- `cargo test -p xtask`: **21/21** (was 20/21 before this session's fix).
- `cargo fmt --check` (workspace): clean.
- `cargo clippy --workspace --lib -- -D warnings`: clean.
- `cargo run -p xtask -- check-assurance-spine`: 8 Features, 25 Stories, **23 Tests**, **27 Reports**, 5 containment classes, 20 boundary tests, 20 security controls, 14 Protection Domain contracts, 14 code-admission gates, 25 class communication pairs, 1,025 selected Story/performance contracts.
- `cargo run -p xtask -- check-performance-catalogue`: 625/625.

## What this does not claim

- No Story's functional or assurance status changed this session. `FEAT-P0-04` remains 2/3 Stories Verified (`STORY-P0-04-03` still Planned).
- The Security Charter's own runtime-evidence gap list is unchanged and still fully open: active per-task address spaces, production capability spaces, executable sealing, IOMMU isolation, teardown, quarantine/provenance, immutable updates, hostile-campaign testing. Nothing in this session narrows it.
- "Iron clad" per the charter's own definition is an architecture claim right now, not a demonstrated one — this session did not add or remove any evidence toward it.

## Immediate next steps

Handover 32's own list stands, now with its one open question (the transition-matrix confirmation) resolved:

1. **`STORY-P0-04-03`** (read-only PCIe enumeration under QEMU `q35`) — the last `FEAT-P0-04` Story, independent of everything else queued.
2. **Real CPU exception handling.** `STORY-P0-04-02`'s IDT currently only has a fail-closed *diverge-and-report* default (no vector resumes except the timer and the spurious vector) — the next real step toward "no IDT/fault containment" is a genuine `#PF`/`#GP` handler that can resume or terminate the faulting context appropriately, which the Security Charter's "active per-task address spaces" and "teardown" runtime-evidence items both depend on.
3. **Active per-task `CR3` switching** — do this only after (2): a live page-table switch with no real fault handler behind it is strictly more dangerous than the current all-RWX identity map, per Handover 32's own reasoning.
4. **Pick off the charter's runtime-evidence list one item at a time**, in whatever order the user prioritizes — production capability spaces, executable sealing (RX/RO/RW-NX per the charter's own admission-chain description), IOMMU isolation, quarantine/provenance, immutable updates, and hostile-campaign testing are each independently substantial and none has runtime evidence yet.
