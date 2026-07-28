# STORY-P0-01-07 — Four Prose Rules Become Gates

Status: **Functionally Verified (Host), 2026-07-28** — assurance state `baseline-debt`; `LE-33`, `LE-35`, `LE-36` and `LE-44` closed. This Story builds gates and closes no performance guardrail of its own
Feature: [`FEAT-P0-01`](../features/FEAT-P0-01.md)
Introduced in: [`session/hand-2026-07-28/38A-outstanding-actions.md`](../../session/hand-2026-07-28/38A-outstanding-actions.md) §3

## Description

[Handover 38A](../../session/hand-2026-07-28/38A-outstanding-actions.md) §3 collects four loose ends
under one heading — *"the rows that stop recurrence rather than removing one instance"* — and the
heading is the Story. All four are the same defect in four places: **a decision this project made
correctly, stated in prose, with no machine behind it.** Handover 38A trap 3 names the cost directly,
and §3's closing paragraph counts five instances of the shape in one week.

This Story builds the four machines. It writes no kernel code and closes no performance guardrail.

| Row | The prose rule today | The machine this Story builds |
|---|---|---|
| `LE-33` | `ADR 0004` forbids promoting an x86_64 or Tier 0 measurement into a `G04`-class bound; `ADR 0005` adds ARM64 platforms holding no secure-world qualification record | `TINYOS-MEAS/2` carries platform identity and a qualification-record reference; a register of platforms and their qualification state; a bound-provenance gate inside `check-assurance-spine` |
| `LE-35` | Handover 25 refused to record `G11` evidence for domains whose subsystem does not exist. The rule that implies was never written down | The rule written into [`README.md`](README.md), plus `open-debt.tsv` and a gate: selecting a `design`/`stand-in-only`/`specified`/`unbuilt` domain requires initialising it as stated open debt |
| `LE-36` | `CONCURRENT_SESSIONS` rule 8 asks a session to validate a hand-edited spine TSV before its next tool call, and names the instrument | `xtask check-spine-files` — the named instrument, fast enough that skipping it has no excuse |
| `LE-44` | A Feature's Stories table and a Story's own `Status:` header can disagree indefinitely | `check-assurance-spine` cross-checks every Feature Stories-table row against the referenced Story's header |

## Depends on

`STORY-P0-01-05` (the guardrail-evidence register `LE-33`'s gate reads) and `STORY-P0-01-06` (the
`D09` disposition, which is the worked example of a domain's readiness deciding what may be claimed).

## Acceptance criteria

1. **A `G04`-class bound cannot be filed from a disqualified source (`LE-33`).** The `TINYOS-MEAS`
   envelope carries `platform=` and `qualification=` alongside `tier=` and `arch=`, bumped to
   `TINYOS-MEAS/2` because adding required keys is a breaking format change and this parser rejects
   versions it does not know rather than best-effort parsing them. `goals/assurance/qualified-platforms.tsv`
   records every measuring platform and whether it holds a qualification record. `check-assurance-spine`
   refuses a `guardrail-evidence.tsv` row for a bound-class guardrail unless its Report carries a
   well-formed `TINYOS-BOUND/1` claim line, and refuses that claim when its tier is `T0`, its
   architecture is `x86_64`, or its platform is not `qualified` in the register.
2. **The gate is shown to detect, not merely to return zero (`ADR 0005`'s trap, Handover 38A trap 7).**
   No `G04` row exists in the evidence register today, so the gate is vacuously satisfied against the
   committed tree and a passing run proves nothing about it. Host tests therefore drive each refusal
   positively — x86_64 source, Tier 0 source, unqualified platform, missing claim line, malformed
   claim line — and one acceptance case that passes only because a fabricated platform is marked
   `qualified`.
3. **A design-readiness domain selection is stated open debt, not a satisfiable obligation (`LE-35`).**
   The rule is written into [`README.md`](README.md), and `check-assurance-spine` enforces it:
   a Story contract selecting a domain whose catalogue `readiness` is `design`, `stand-in-only`,
   `specified` or `unbuilt` must carry a matching row in `goals/assurance/open-debt.tsv`; a debt row
   for an implemented domain is refused; and a `(story, domain)` pair may not appear in both
   `open-debt.tsv` and `guardrail-evidence.tsv`, because a gate cannot simultaneously be
   unclosable and closed.
4. **The instrument `CONCURRENT_SESSIONS` rule 8 names exists (`LE-36`).** `cargo run -p xtask -- check-spine-files`
   validates every hand-edited assurance/security/context TSV — header, field count, id uniqueness,
   id contiguity — and nothing else, so it returns in well under a second. It is a strict subset of
   `check-assurance-spine`: it can never pass where the full spine check would fail on the same file.
5. **A Feature and its Story cannot disagree about state (`LE-44`).** `check-assurance-spine` parses
   every `| [`STORY-*`](…) | … | status |` row in every Feature's Stories table and compares it against
   that Story's own `Status:` header: the state word exactly, and every `criterion N` / `criteria N and M`
   token as a set. The check is run against the committed tree and the disagreements it finds are fixed.

## Named debt this Story leaves open

- **`LE-33`'s gate is keyed on the evidence register, not on Report prose.** A Report can still write
  the sentence *"the worst case is 1.2 µs"* in English without filing a register row, and no lint reads
  English. What the gate makes impossible is a bound entering the machine-readable spine from a
  disqualified source. That is the boundary this project can actually enforce, and stating it here is
  the alternative to implying a stronger claim.
- **`qualified-platforms.tsv` starts with zero qualified rows and stays that way** until a `Q1`–`Q4`
  record exists per `ADR 0005`. The register's value today is that the count is machine-readable
  rather than a sentence in an ADR.
- **`check-spine-files` is a subset by construction and by test, not by proof.** A test asserts that
  every TSV the full check reads is also read by the fast one; nothing prevents a future validator
  landing in only one of them beyond that test.
- **No Story becomes assurance `verified`, and no performance guardrail closes.** This Story is
  governance machinery.

## Tests

[`TEST-P0-01-07-A`](../tests/TEST-P0-01-07-A.md) — written before implementation, per the TDD mandate.

## Reports

- [`REPORT-2026-07-28-10`](../reports/REPORT-2026-07-28-10.md)
