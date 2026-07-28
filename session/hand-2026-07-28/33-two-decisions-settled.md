# Handover 33 — Both Owed Decisions Are Settled: `ADR 0005` and `ADR 0006`

Follows [Handover 32](32-next-session-mandate.md), whose fallback order this session took at step 1.
**No board time and no serial adapter were available**, so the board session — still the
highest-value work in the project by a wide margin — was not possible. **This session wrote no kernel
code and no test code**, because neither decision is a coding task. It closes the two rows that were
*decisions* rather than defects, and it is the first session in five whose deliverable was not
blocked on a physical object.

**On the folder date.** This repository's document dates run one day ahead of the clock, per Handover
13 §"A note on dates".

**Concurrency (rule 7).** No commits arrived on `main` mid-session. Slot 33 was claimed by creating
this file before its contents were written (rule 4), and `goals/assurance/loose-ends.tsv` was edited
field-wise and field-count-validated in the same tool call (rule 8). `goals/reports/_soak-p0-03-01.log`
was dirty on entry and was left alone, as Handover 32 asked.

## What landed

| Artifact | What it does |
| --- | --- |
| [`ADR 0005`](../../docs/adr/0005-arm64-real-time-tier-is-conditional-on-secure-world-qualification.md) | The ARM64 real-time tier is conditional on per-platform secure-world qualification. **Supersedes `ADR 0004`.** Settles `LE-39` |
| [`ADR 0006`](../../docs/adr/0006-mit-licence-confirmed-and-open-core-optionality-dropped.md) | MIT confirmed; open-core optionality dropped. **Owner decision.** Settles `LE-41` |
| `ADR 0004` | Status → **Superseded**, with a forward pointer. **Body unedited**, because `README.md`, `EPIC-P1` and the Handover series cite it |
| `loose-ends.tsv` | `LE-39` and `LE-41` closed against `hand-2026-07-28/33`; **`LE-33`'s row gains a second condition** |
| `README.md`, `EPIC-P1`, `goals/index.html` | Reconciled to `ADR 0005`; the dashboard's two ADR-0004-dependent claims corrected in place |

`check-assurance-spine` green at close: 23 Features, 58 Stories, 45 Tests, 46 Reports, **42 loose ends
(29 open)**, 84 status headers, 11 release gates with evidence.

## 1. `LE-39` — the premise, and why the repair is not "pick a different architecture"

`ADR 0004` elected ARM64 as the real-time tier in two moves: disqualify x86_64 because SMIs are
invisible, unmaskable and unattributable; then elect ARM64 on the sentence *"Interrupt masking at
`EL1` means what it says."*

**The first move survives untouched. The second does not.** A GIC signals secure-group interrupts as
FIQ; `SCR_EL3.FIQ` routes them to `EL3`; and when it does, they are taken **irrespective of
`PSTATE.I` and `PSTATE.F` at NS-EL1**. The secure handler consumes cycles NS-EL1 cannot attribute and
perturbs cache, TLB and predictor state it depends on. NS-EL1 cannot even read the configuration —
`SCR_EL3` is inaccessible below `EL3`. Invisible, unmaskable from the OS's exception level,
unattributable: the same three words, on the architecture chosen to escape them.

**The temptation is to conclude TinyOS has no real-time tier. That would be wrong, and the reason is
one row of the ADR's table:**

> On x86_64 the defect is a property of the *architecture* and its firmware model. On AArch64 it is a
> property of *how one platform's firmware configured one GIC*. A bare-metal AArch64 board with no
> secure interrupts routed to `EL3` has no such hole at all — and you cannot tell the two apart
> without looking.

So the tier stops being a property of the instruction set and becomes a property of a **qualified
platform**. A qualification record is a dated Report naming one platform at one firmware version,
carrying four things: **Q1** platform and firmware identity exactly (a record is void for any other
firmware version — firmware updates are not neutral to a real-time claim, and this is the row that
makes that visible); **Q2** the secure-world configuration and *how it was determined*, saying so in
those words where closed firmware makes it undeterminable; **Q3** an NS-EL1 residency campaign with a
stated duration and conditions; **Q4** the refusal to over-claim, which is `ADR 0004`'s own sentence
against x86_64 measurement turned on ourselves.

**Three things to carry forward, because they are the ones most likely to be misread:**

1. **No measurement is invalidated.** Both ADRs constrain what may be *promoted* into a bound, never
   the measurements. If applying `ADR 0005` leads you to retract a Tier 0 number, you have misread it.
2. **Zero platforms are qualified, the Pi 5 included** — it cannot be, since this repository has not
   yet read the board's `current_el=` line, so its entry exception level is still an *input*.
3. **A campaign that observes nothing is the most dangerous possible result**, because it reads as
   qualification and is the cheapest thing to obtain. `ADR 0005` closes with the rule that follows:
   **a Q3 campaign is inadmissible unless the same instrument has been shown to detect a known
   perturbation.** A zero from an instrument never shown to produce a non-zero is not a measurement of
   zero — it is an absence of measurement, and the Report cannot tell them apart unless the positive
   control is recorded alongside. That is Handover 32's trap 3, in the narrow form the `D09` session
   arrived at.

Bare-metal `EL3` was considered and rejected: **Pi 5 firmware owns `EL3` and cannot be displaced**, so
mandating it makes the RT claim unreachable on the only board in hand. It survives in the ADR as a
platform-specific route to a very strong `Q2` where `EL3` *is* ours.

**Deliberately not settled**: whether the qualification record is `STORY-P1-07-06`'s scope or a
seventh Story. `FEAT-P1-07` §6 says a seventh means re-decomposing, and that is a scope decision, not
this ADR's to take.

## 2. `LE-41` — the owner confirmed MIT, and dropped open-core optionality

Put to the owner as four options — AGPL open-core with a CLA, GPL/MPL dual-licence, confirm MIT, or
defer with the deadline mechanics written down. **The choice was to confirm MIT and drop open-core
optionality**, recorded in `ADR 0006` with the reasoning that made it defensible rather than merely
convenient:

- **Adoption is the binding constraint at this stage.** With `0 / 58` Stories assurance-verified and
  no hardware tier, the failure mode that ends this project is nobody building on it — not a
  competitor forking it. Copyleft on a kernel excludes the integrators and silicon partners who would
  be first users, at the moment there is least to protect.
- **The asset worth defending is not the source, and `ADR 0005` sharpened that the same day.**
  Qualification evidence — dated, firmware-versioned, campaign-backed — is what commercial RTOS and
  certification vendors charge for. **A fork gets the code and none of it.** Copyleft would have been
  protecting the reproducible half.
- **Open-core is a recurring tax on a single-author project with no revenue**: a core/commercial line
  to draw in every design decision, a CLA on every contribution, licence provenance in the build.
- **AGPL would not have prevented the feared outcome cleanly anyway** — the realistic scenario is a
  vendor shipping a derived system inside a product, where enforcement is slow, jurisdictional, and
  unaffordable for one author. That is the appearance of protection, not protection.

**The window is now deliberately allowed to close.** Fork-and-close is recorded in writing as an
accepted risk, so if it happens it is a known outcome of a made decision. Outside contributions may
be accepted with no licensing gate and nothing needs to be watched or timed. **No code, manifest or
build change follows** — `LICENSE` (landed `b663376`), the workspace `license` key and all seven
crates' `license.workspace = true` already agree; a future crate declaring its own `license` key
differently is a defect.

`LE-41`'s real cost was never MIT. It was that nobody had decided, so every downstream question — can
this be quoted in a proposal, can a partner evaluate it, does a contribution need a CLA — had no
answer, and that cost was being paid continuously.

## 3. `LE-33` grew, and it did not become a new row

`LE-33` is the observation that `ADR 0004` has no machine behind it: a Report may quote an x86_64 or
Tier 0 number as a `G04`-class bound and every gate stays green. `ADR 0005` gives its future lint a
**second condition** — refuse a `G04` bound sourced from an ARM64 platform with **no qualification
record** — which means the `TINYOS-MEAS/1` envelope must carry a **platform identity and a
qualification-record reference**, not only an architecture and a tier.

That is recorded on `LE-33`'s existing row rather than as `LE-43`, deliberately. It is the same defect
with a wider mouth, and splitting it would let the ARM64 half be closed while the x86_64 half stayed
open — which is exactly the shape that makes a register stop being readable.

## Where that leaves the next session

**The order in [Handover 32](32-next-session-mandate.md) §"What to do" stands, minus its step 1.**

1. ~~Settle the two decisions.~~ **Done.** Nothing in the remaining work is now blocked on a decision.
2. **The board, if an adapter is in your hand** — still the highest-value session available by a wide
   margin, and Handover 26 §"If an adapter *is* in your hand" is still exactly right and still not
   superseded. One board session closes both `STORY-P1-07-01`'s capture and `STORY-P1-07-02`'s clause
   2. **What changed here**: that session now also produces the first `Q1` and the beginning of `Q2`
   for a qualification record, because the `current_el=` line is the first fact anyone has about this
   board's exception-level configuration. It does **not** produce a bound.
3. **The `-M virt` fixture** — [Handover 31](31-qemu-virt-fixture-scoping.md) is the scoping, §7 lists
   four decisions to settle before writing anything. `ADR 0005` does not touch it: `virt` produces no
   timing evidence by design, so it cannot produce a bound, and a QEMU guest is not a qualifiable
   platform because its secure-world configuration is the emulator's rather than a product's.
4. **`LE-23`** — re-record the timing baseline from a CI run. `LE-24` may come free. One fix, two rows.
5. **`LE-30`** — generate the dashboard's status tables from `list-status`. This session hand-edited
   `goals/index.html` in **six** places, which is the fifth consecutive session to do so and is the
   argument for the row rather than a complaint about it.

**Traps from Handover 32 that are unchanged and still apply**: a green ARM64 fixture is not ARM64
coverage; do not patch `LE-37`/`LE-38` directly; an external reviewer's confidence is not evidence and
neither is your own; `LE-35` is load-bearing, not theoretical; do not reach for `--update-baseline`
locally; validate a hand-edited machine-checked file before your next tool call.

**One trap this session adds**, and it is `ADR 0005`'s own: **the qualification campaign is easy to
fake by accident.** Not by dishonesty — by running an instrument that has never been shown to detect
anything, getting a zero, and filing it. Every Q3 needs its positive control in the same Report.

## State at the close

```text
main                    ff980d0 + this session's commit
assurance spine         23 Features, 58 Stories, 45 Tests, 46 Reports
                        42 loose ends (29 open), 84 status headers
                        11 release gates with dated evidence, of 391
host tests              unchanged — no code was written
Stories verified        0 / 58
open decisions          none
ADRs                    0005 accepted (supersedes 0004), 0006 accepted
best available work     a board session, if an adapter exists
next best               the -M virt fixture, then LE-23, then LE-30
```

`goals/reports/_soak-p0-03-01.log` is still dirty in the working tree, still belongs to whoever is
running that soak, and was left untouched.
