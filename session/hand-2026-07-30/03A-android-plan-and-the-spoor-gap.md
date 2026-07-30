# Handover 03A — The Android Device Enabler Plan Is Documented; The Spoor Question Answered Honestly

Same session as [`01A`](01A-multitab-ux-visually-confirmed.md)/[`02A`](02A-cover-note-deep-review.md).
Two owner orders executed.

## 1. The Android Device Enabler Plan of Action

Documented as [`docs/android-device-enabler-plan.md`](../../docs/android-device-enabler-plan.md)
— spec-level, no platform claim, rule-10-shaped (the `application-platforms.tsv` /
`landing-zones.tsv` rows are Phase 0 of implementation, not of planning). Six phases, each
with a machine gate: governance rows → ARM64 HAL at Tier 0 on a devboard (not a phone) →
storage + mmap'd read-only weights → sensors/camera as C4-inspected labelled producers →
the inference domain as a WCET-budgeted C3 domain that cannot starve the RT floor →
fleet serving under the charter → phone bring-up last, gated on ADR 0005 secure-world
qualification and a baseband-behind-IOMMU rule. The plan's refusals are stated (no blobs
as code, no radio without IOMMU, no RT claim before qualification, no asserted numbers).

## 2. Were spoors captured by the console/tab runs? **No — and the register now says so.**

The owner asked for evidence that the console tab app captured spoors. Checked, not
assumed: **no spoor was captured in any console or tab run to date.**

- `Spoor` (`kernel::spoor`, `STORY-P0-06-01`) is the kernel's 64-bit audit atom, journaled
  by `kernel::spoor_journal`. It is exercised by exec-lane fixtures — `first-task` journals
  admission refusals and fault audits as spoors and reports `spoor_journal_len` over serial.
- The multitab smoke's target runs drove the **`shell-batch` fixture**, which asserts
  TINYCMD's own audit discipline (denial counters + `[audited]` session-carrying refusal
  text in the transcript) but **never touches the spoor journal**.
- The DOS tabs run the `shell` crate **on the host**, where no kernel journal exists at
  all; the console's own denial log (identity tuples in `smoke.json`, screenshot 07) is
  console-side evidence, not kernel spoors.
- The 14G-era console smoke ran the `measure` fixture — also spoorless.

What was captured is real audit evidence (console denial log, TINYCMD audited denials,
serial transcripts, two-signal parity verdicts) — but none of it is the spoor atom, and
prose must not blur that line. Registered as **`LE-56`** with the repair path: teach the
shell/target lane to journal verb denials as spoors (the batch fixture asserting
`spoor_journal_len` alongside its denial counter, streaming the count over serial), and/or
surface a journal-dump verb so a tab can *show* spoors — at which point the parity tab's
wall gains a spoor row and the claim becomes screenshotable.

## 3. Also in this handover

`main` fast-forwarded again to include 02A and this work — the owner's merge order is
treated as standing for this session's documents.
