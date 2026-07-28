# TEST-P0-01-07-A — Four Prose Rules Become Gates

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P0-01-07`](../stories/STORY-P0-01-07.md)
Tier: Host unit tests only — every clause here is a property of machine-readable artifacts, and none of it needs a CPU this project does not have
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`, `BND-18`
Protection Domain contracts: `PD-02`, `PD-12`, `PD-13`, `PD-14`
Code admission gates: `RCG-05`, `RCG-06`, `RCG-07`, `RCG-12`, `RCG-14`
Assurance state: `baseline-debt`

## What this test is for

Four loose ends — `LE-33`, `LE-35`, `LE-36`, `LE-44` — describe the same defect in four places: a
decision this project took correctly, recorded in prose, with nothing mechanical enforcing it.
[Handover 38A](../../session/hand-2026-07-28/38A-outstanding-actions.md) §3 groups them and states
the cost in one sentence: *"A Report from `FEAT-P1-07` quoting one of its numbers as a `G04` bound
would still be wrong under `ADR 0005` and still pass every gate in this repository."*

The strongest clause below is clause 2, and it is not about any of the four rules. It is the
requirement that each new gate be **shown to reject something**. `ADR 0005`'s own trap section and
Handover 38A trap 7 both say the same thing: an instrument that has never been demonstrated to detect
a known defect cannot be believed when it reports zero, and every gate here is vacuously satisfied
against today's tree. A green `check-assurance-spine` is therefore *not* evidence for clauses 1, 3 or
5, and this document says so before the implementation exists to be tempted by it.

## Specification

### 1. A bound cannot be filed from a disqualified source (`LE-33`)

**Given** the measurement envelope,
**then** its `BEGIN` line carries `platform=` and `qualification=` in addition to `tier=`, `arch=`,
`cycle_source=`, `overhead_cycles=` and `cycles_per_us=`, and the version sentinel reads
`TINYOS-MEAS/2`.

**And** the host-side parser requires both keys, rejects a stream carrying `TINYOS-MEAS/1` as an
unsupported version rather than parsing it best-effort, and rejects a `qualification=` value that is
neither the literal `none` nor a well-formed `REPORT-YYYY-MM-DD-NN` id.

**Given** `goals/assurance/qualified-platforms.tsv`,
**then** `check-assurance-spine` validates it: fixed header, no empty fields, unique `platform_id`,
an `arch` this project builds for, a `state` of exactly `qualified` or `unqualified`, and — the
clause that carries `ADR 0005` decision 3 — a `qualified` row must name a qualification Report that
exists, while an `unqualified` row must record `-`. **Silence is not evidence**: a platform absent
from the register is unqualified, never presumed clean.

**Given** a `guardrail-evidence.tsv` row whose guardrail is bound-class (`G04`),
**then** `check-assurance-spine` refuses it unless the Report named in `evidence_path` carries a
`TINYOS-BOUND/1` line for that guardrail id, and refuses the claim if:

- its `tier` is `T0` — a Tier 0 number is emulator behaviour (`ADR 0004`, unmodified by `ADR 0005`);
- its `arch` is `x86_64` — `ADR 0004`'s surviving half, restated in `ADR 0005` decision 1;
- its `platform` is absent from the register or not `qualified` — `ADR 0005` decision 2.

**And** a bound claim whose `platform`/`arch` disagree with the register's row for that platform is
refused, so a claim cannot launder an x86_64 platform by writing `arch=aarch64` beside it.

### 2. Every gate here is demonstrated to reject (`ADR 0005`'s trap; Handover 38A trap 7)

**Given** that no `G04` row exists in the evidence register and no Story yet selects a `design`
domain without debt,
**then** a passing `check-assurance-spine` on the committed tree is **not** evidence that clauses 1,
3 or 5 work, and no Report may cite it as such.

**Therefore** host tests drive each new gate with a fabricated input that must be refused — a bound
sourced from `x86_64`, one from `T0`, one from an unqualified ARM64 platform, one with no claim line
in its Report, one with a malformed claim line, one whose platform contradicts the register; a Story
contract selecting a `design` domain with no debt row; a debt row for an implemented domain; a
`(story, domain)` pair present in both registers; a Feature table row whose state word differs from
its Story's header; and one whose `criteria N` tokens differ.

**And** each gate has at least one *acceptance* case that passes only because the fabricated input is
legitimate — a bound from a platform the fixture marks `qualified`, a debt row that matches a
`design` selection, a Feature row that agrees with its Story. A gate that only ever rejects is as
uninformative as one that only ever accepts.

### 3. A design-readiness selection is stated open debt (`LE-35`)

**Given** [`README.md`](README.md),
**then** it states the rule in its own section: selecting a performance domain pulls all 25 of its
guardrails into the selecting Story's contract, and where the domain's catalogue `readiness` is
`design`, `stand-in-only`, `specified` or `unbuilt`, **not one of those 25 can be closed, because the
subsystem does not exist.** Such a selection is initialised as stated open debt at selection time.
Handover 25 set the precedent by refusing to record `G11` for exactly these readinesses; this is that
precedent written down.

**Given** `goals/assurance/open-debt.tsv`,
**then** `check-assurance-spine` validates it and enforces the rule in both directions:

- a Story contract selecting a non-implemented-readiness domain **without** a matching debt row is
  refused, naming the Story, the domain and its readiness;
- a debt row for a domain whose readiness *is* implemented (`prototype`, `prototype-cooperative`,
  `prototype-inactive`, `partial`) is refused — debt may not be used to excuse a real obligation;
- a debt row whose recorded `readiness` disagrees with the catalogue is refused, so the register
  cannot drift away from the file it describes;
- a `(story, domain)` pair appearing in **both** `open-debt.tsv` and `guardrail-evidence.tsv` is
  refused. A gate cannot be simultaneously unclosable and closed, and this is the check that stops
  the cheapest available lie — recording all 25.

**And** the rule is applied to the committed tree rather than only to fixtures: every existing
Story/domain pair meeting the condition gets a row, each carrying its own reason.

### 4. The instrument rule 8 names (`LE-36`)

**Given** `cargo run -p xtask -- check-spine-files`,
**then** it validates every hand-edited TSV under `goals/assurance/`, `goals/security/`,
`goals/context/` and `goals/performance/` for header, field count, id uniqueness and id contiguity,
and does **nothing else** — no cross-file resolution, no markdown walk, no crate scan.

**And** it is a strict subset of `check-assurance-spine`: a host test asserts that every file the
fast check reads is also read by the full one, so the fast check can never pass where the full one
would fail on the same file.

**And** `agent/CONCURRENT_SESSIONS.md` rule 8 names this command, replacing *"a field-count pass or
the relevant `check-*` subcommand"* with the instrument that actually exists. Rule 8's own correction
stands: a field count is necessary and demonstrably not sufficient, because both duplicate `LE-43`
rows were well-formed at eight fields — so this command checks ids, which is what caught that.

### 5. A Feature and its Story cannot disagree (`LE-44`)

**Given** every Feature document's Stories table,
**then** `check-assurance-spine` extracts each row's Story id and status cell, and compares against
that Story's own `Status:` header:

- **the state word exactly.** `Functionally Verified` and `Verified` are different states in this
  project's own vocabulary — one carries assurance debt the other's reader will not look for — so
  they do not match each other;
- **the `criterion N` / `criteria N and M` tokens as a set**, in either direction. A Feature saying
  *"criteria 2 and 4"* where the Story says *"criteria 3 and 4"* is refused; that is `LE-44`'s
  originating instance, and criterion 3 is the one producing `Q1` evidence.

**And** a Feature table row naming a Story with no document is refused.

**And** the check runs against the committed tree, so the disagreements it finds are fixed rather
than grandfathered. Grandfathering the existing rows would make the gate green on day one and blind
to exactly the class of drift it exists to catch.

## What this test explicitly does not establish

- **That no Report anywhere states a bound in English.** Clause 1 gates the machine-readable spine.
  Prose is not parsed and this document does not pretend otherwise.
- **That any platform is qualified.** The register is created empty of qualified rows on purpose.
  Zero is `ADR 0005` decision 3's own number and this Story does not move it.
- **That the four loose ends' underlying risks are retired.** A gate stops the next instance; it does
  not audit the previous ones. `LE-29`'s bidirectional-coverage question and `LE-31`'s attribution
  audit are untouched.
- **Any performance guardrail.** No `PERF-Dnn-Gnn` closes here and no Story's assurance state moves.

## Reports

- [`REPORT-2026-07-28-10`](../reports/REPORT-2026-07-28-10.md)
