# Handover 01 — `STORY-P1-03-01` Was Already Finished: Reconciling With the Cover Note

Follows: [`00-cover-note-feat-p1-03-session-start.md`](00-cover-note-feat-p1-03-session-start.md). Also follows, chronologically and in fact: [`session/hand-2026-07-27/10-story-p1-03-01-cr3-switching.md`](../hand-2026-07-27/10-story-p1-03-01-cr3-switching.md), which this note exists to reconcile against.

## What happened

Two threads worked this repository concurrently against the same uncommitted working tree. One (this one) continued straight through from `LE-17` (Handover 09) into finishing `STORY-P1-03-01` the same calendar day and wrote it up as Handover 10 in `hand-2026-07-27/`. The other forked at an earlier point — after the primitives (`read_cr3`/`write_cr3`/`cr3_reload_needed`, `AddressSpace::cr3()`, `Tcb::address_space`) existed but before `context::switch_address_space`, the fixture, the Test document, or the Report did — and wrote `00-cover-note-feat-p1-03-session-start.md` on that stale snapshot, plus created this `hand-2026-07-28/` folder.

Per `session/README.md` rule 10, a new calendar-date folder's existence is supposed to close the previous date's folder to further edits. That rule assumes one linear session, not two concurrent ones racing on the same tree — Handover 10 was written and committed to `hand-2026-07-27/` in good faith, since it was in fact still the 27th when that work finished. Rather than retroactively rewrite either thread's history (which the rule itself forbids once a newer folder exists, and which would just relitigate a race that already happened), this note does what the rule prescribes for exactly this situation: says explicitly, in the newer folder, what the older one got wrong or missed.

**The cover note's central table is now stale.** Everything it lists as *not existing* — `context::switch` integration, `TEST-P1-03-01-A`, a Tier 0 fixture, a Report — exists. `STORY-P1-03-01` is **Verified (Tier 0 + Host)**, not in-progress. The authoritative record of what was built and why is [Handover 10](../hand-2026-07-27/10-story-p1-03-01-cr3-switching.md) and [`REPORT-2026-07-27-08`](../../goals/reports/REPORT-2026-07-27-08.md), not this note's summary of them.

## Checking the delivered work against the cover note's own mandate

Worth doing explicitly, since the cover note set real constraints and it would be easy to have quietly missed one while working from a different starting point.

- **Test document first** — **not fully honored, and said so at the time.** `TEST-P1-03-01-A` was written alongside the fixture's debugging, not strictly before it, and its own "Process note" section says exactly that rather than presenting a clean Red run that didn't happen. The cover note asked for the same discipline "as every Story on the 27th did" — Handovers 06/07 also record this same partial honesty about TDD ordering, so this isn't a new failure mode, but it is one the cover note specifically flagged and it's worth naming as unmet rather than silently letting the Test document's own admission stand alone.
- **Assurance contract before code** — **honored.** `STORY-P1-03-01`'s row moved `specified` → `baseline-debt` only after the evidence existed; `check-assurance-spine` passes; the pinned Test/Report counts in `assurance.rs` were incremented (29→30, 36→37) as part of this same change, exactly as anticipated.
- **`SECURITY_CHARTER.md` boundary tests** — **honored.** `TEST-P1-03-01-A` carries `BND-04`, `BND-05`, `BND-20` and `PD-01`/`PD-04`/`PD-13`/`RCG-10`/`RCG-11` — the Feature's full selected set, matching `feature-contracts.tsv`'s row exactly (the assurance-spine validator enforces this and rejected a first, narrower attempt that carried only `BND-04`).
- **Measurement caution (the cover note's first constraint)** — **honored by not attempting it.** No `D04` same-space-vs-cross-space delta was measured. `STORY-P1-03-01`'s finalized acceptance criterion 2 says so explicitly and why: nothing in production yet pays a per-task `CR3`-switch cost, so measuring a fixture's own overhead would misrepresent it as a scheduling cost. This sidesteps the cover note's own worry about recording a real effect against a noise floor larger than it — because no attempt was made to record one at all yet.
- **`LE-19` part (b)** — **correctly left open, not touched.** No baseline rows were added or refreshed this Story; the existing `--update-baseline` hazard the cover note names is real and unrelated to this work.
- **"A cross-space fault is the first fault this project will raise that it did not hand-place"** — **true, and it found something.** The adversarial probe did fault for a hardware reason, not a hand-placed one — but the finding that actually surfaced was upstream of the probe itself: `AddressSpace::drop` silently zeroing both trees the instant they left scope, before either `CR3` was ever loaded, discovered via a real `qemu -d int,cpu_reset` triple-fault capture. Recorded in full in `REPORT-2026-07-27-08`.

## Current state of `FEAT-P1-03`

**In progress — 1 of 2 Stories Verified.** `STORY-P1-03-01` (mechanism) is done; `STORY-P1-03-02` (W^X/NX mappings, generation-safe teardown) is not started. Per the cover note's own framing (echoing `FEAT-P1-02`'s pattern the day before): Feature exit is not "the Stories are done" — both Stories, not one, and the D04/D08 baseline the cover note wants is explicitly deferred to whichever piece of `STORY-P1-03-02` first gives production dispatch a per-task address space to measure.

Also unpushed, still: everything Handovers 09 and 10 describe, plus this note's own changes. `origin/main` was current only through `91c95c1` as of the cover note; nothing in this session pushed anything further.

## Next session — start here

**`STORY-P1-03-02`** — W^X/NX kernel + task mappings, generation-safe address-space teardown. Concretely:

1. Replace `AddressSpace::drop`'s current unconditional-zero teardown with something generation-safe (`PD-13`: revoke, wipe, advance generation, only then permit reuse) — the `address-space-switch-fixture`'s own `core::mem::forget` workaround (Handover 10) is the immediate, concrete thing this replaces.
2. W^X-correct kernel mappings, shared across every task's tree — what the `address-space-switch-fixture`'s all-RWX, duplicated-per-space kernel replica stood in for. This is also the prerequisite for ever wiring `switch_address_space` into `kernel::dispatch::run_once`, which is in turn the prerequisite for `STORY-P1-03-01`'s own still-deferred `D04` measurement.
3. Decide the measurement recording question the cover note raised — how a same-space-vs-cross-space `D04` delta gets recorded, and on what machine state — before attempting it, not after, given `LE-18`'s live host-noise problem.
4. `LE-19` part (b) remains a separate, small, unowned `gate.rs` Story whenever someone picks it up — not blocking, not `FEAT-P1-03`'s.

## Loose-ends register

Unchanged from [Handover 10](../hand-2026-07-27/10-story-p1-03-01-cr3-switching.md#loose-ends-register-canonical-as-of-this-handover) — no items closed or opened by this reconciliation note itself.
