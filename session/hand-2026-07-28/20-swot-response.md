# Handover 20 — Response to the Session SWOT: What Was Fixed, What Was Registered, What Stays Open

Written at the close of 2026-07-28, against the session SWOT covering `6b47b4d` → `d310e64`. The SWOT was written by the session that produced most of those commits; this handover is the disposition of it. **Every item is accounted for below — fixed, registered as a loose end, or explicitly declined with a reason.** An unactioned SWOT is a document that made everyone feel reviewed.

## Fixed in this handover

### 1. The spine counts stop churning (Weakness 4)

`assurance.rs` hard-coded eleven totals, and any document landing anywhere in the repository — including in a tree the committer could not see — broke them. They were re-synced **five times in one day**, each time as a symptom. The sixth break happened during this session, which is what forced the pattern to be named.

The test is now split along the line the old one blurred:

- **`committed_assurance_spine_catalogues_are_exact`** — 5 classes, 20 boundary tests, 20 controls, 14 PD contracts, 14 code-admission gates, the 25-pair matrix, 19 applications, 9 landing zones. These are fixed by charter documents, not by how much work has landed, and changing one **should** break a test. Still asserted exactly.
- **`committed_assurance_spine_population_never_shrinks`** — Features, Stories, Tests, Reports, loose ends. Asserted as **floors and relationships**, never totals.

The rule, stated so it generalises: **a count of how much work exists is a floor, not a total.** The floor still catches what matters — documents are added, never deleted, so a shrinking count means an artifact was lost or a contract row was dropped — while being immune to a Story landing concurrently. 129 xtask tests pass.

### 2. A broken spine can no longer be committed (Weakness 1)

The root cause was stated exactly in the SWOT: the gate was run **before** staging, `git add -A` then swept in a half-finished Feature, and it was never re-run. CI went red on `585a027`.

The discipline was already written down and already believed. What was missing was a machine that runs it at the moment that matters. [`.githooks/pre-commit`](../../.githooks/pre-commit) now runs `check-assurance-spine`, `check-performance-catalogue` and `check-crate-sizes` after staging, and it is installed in this clone (`git config core.hooksPath .githooks`).

**The timing gate is deliberately absent from the hook.** It measures the host, and a host-sensitive pre-commit hook trains people to pass `--no-verify` — which would leave the repository strictly worse off than having no hook at all.

### 3. Concurrent-agent contention has a protocol (Threat 1)

The SWOT is right that this is now an operational hazard rather than a curiosity: in one day it broke `main`, produced a transient non-compiling `main.rs`, forced two handover renumbers, and put an unreviewed Feature into a commit under the wrong authorship. **None of that was a reasoning error. All of it was the absence of a protocol.**

[`agent/CONCURRENT_SESSIONS.md`](../../agent/CONCURRENT_SESSIONS.md) is that protocol, binding and short — seven rules, of which two would have prevented every incident on the list: **stage narrowly (`git add -A` is banned when another session may be live)**, and **never commit a file you have not read** (Weakness 5 — disclosure is not review, and authorship is not a technicality). It is linked from `agent.md`'s orientation list.

### 4. Two loose ends stopped describing the world before the Story that changed it (Threats 2 and 3)

`LE-16` and `LE-18` both still named "the fourth Story under `FEAT-P1-01`" as their owner. That Story landed as `STORY-P1-01-04`. The register was describing the pre-Story world while the handovers described the post-Story one, which is precisely how a register stops being read.

- **`LE-16`** restated at **~2x**: a real 50% regression on a gated path now passes. The trade is right — the 1.6x it replaced applied to a quantity observed swinging +318% on unchanged code — but something was given up, and the row now names the `const _: () = assert!(…)` and its derivation comment as **the only defences** against someone trimming the tolerance back to make the gate look sharper. Owner: a sensitivity Story gated on `LE-09`, because no Tier 0 work recovers it.
- **`LE-18`** restated with the distinction the four-quadrant table cannot make: **addressed for host *load*** (demonstrated across four quadrants on one machine), **survived for host *identity*** and inside the ratio itself — `D05/dispatch_select` swung +80% then +4.4% between two CI runs of unchanged code, absorbed by the 100% tolerance rather than fixed. It remains the gate's least-headroom metric, exactly as it was before the Story. Owner: `LE-23`'s second candidate fix.
- **`LE-19`** retargeted: it is the prerequisite that makes `LE-23`'s re-baseline safe to apply, not a prerequisite of a Story that has already landed.

## Registered rather than fixed

Three items were real and are now in the register instead of in prose, because [that is where open defects live](../../goals/assurance/loose-ends.tsv):

- **`LE-28`** (Threat 4) — `--update-baseline` rewrites measured rows with whatever the current host produced, and nothing asks where the previous rows came from. Handover 16 warns about it; **a warning is weaker than a gate**. The fix shape is named: record the recording host in the baseline file and refuse a silent cross-host rewrite. Pairs with `LE-19(b)`.
- **`LE-29`** (Opportunity 4) — declared-but-never-exercised is guarded for Tier 0 fixtures only. That question found **9 of 23 fixtures with no CI step**; it has never been asked of the 20 security controls, the 20 boundary tests, or the 625 performance cells, every one of which is declared by a contract row and selected by Stories that may never have exercised it. This is the highest-value generalisation on the list.
- **`LE-30`** (Opportunity 6) — `goals/index.html` is hand-maintained against the machine-readable spine and drifted three times in one day, while `list-status` already emits the same data as TSV.

**`LE-09`, `LE-23` and `LE-24` (Opportunities 1–3) needed nothing.** All three are already in the register with owners, and `FEAT-P1-07` is decomposed, contracted and next — which is also the answer to Threat 5 (the `baseline-debt` pool growing with every Story). Opportunity 5 is already recorded: `TEST-P1-07-02-A` is built on exactly the observation that on a Pi 5 the UART is the only diagnostic that exists.

## Declined, with reasons

- **Weaknesses 2 and 3** (a scope decision requested from a wrong analysis; a conclusion published from one CI datum) have **no artifact to repair**. Both were corrected in the record at the time, both are retained as findings rather than edited away, and the SWOT's own diagnosis is the fix: *acting on the first reading of a measurement*. It is stated in `CONCURRENT_SESSIONS.md`'s sibling discipline — the gates — and it is not something a document prevents.
- **Threat 2's deeper form** — someone trimming the tolerance — is not further fixable today. The const assert already blocks the edit mechanically; `LE-16` now carries the reason in the register so a future reader meets the argument before the number.

## The thing worth carrying forward

The SWOT's net judgement is right, and its most useful sentence is the last one: **the same instinct that cost a red `main` and a retracted claim is what found the nine fixtures.** Acting fast on the first reading of a measurement is a liability pointed at your own output and an asset pointed at the code. The three findings this session produced — 9 unrun fixtures, a kernel that panicked in complete silence, and `D07` gated on quantisation — were all found by *running* things rather than reading them.

Nothing about that instinct should be dampened. What was missing was the gates, and the gates now exist.

## State at the close

`cargo run -p xtask -- check-assurance-spine`: 23 Features, 56 Stories, 43 Tests, 44 Reports, **30 loose ends (20 open)**, 82 status headers. 129 xtask tests pass; clippy clean.

`LE-09` is open. `FEAT-P1-07` is specified, contracted, and the next work — see [Handover 19](19-feat-p1-07-acceptance-and-spine.md).
