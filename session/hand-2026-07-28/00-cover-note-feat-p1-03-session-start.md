# Cover note — `FEAT-P1-03` session start: making memory private by construction

Follows: [`session/hand-2026-07-27/09-le-17-fault-latency-baseline.md`](../hand-2026-07-27/09-le-17-fault-latency-baseline.md), the last handover of the previous date. This is a session-start orientation note, the same shape as [`hand-2026-07-27/00-cover-note-epic-p1-session-start.md`](../hand-2026-07-27/00-cover-note-epic-p1-session-start.md) — read it before assuming you know the current state, then read the documents it points at rather than trusting its summary.

> **Two housekeeping notes on this file itself.** The folder is dated `2026-07-28` on the assumption that the next session is the next day; rename the folder if it lands later, since the convention is one folder per calendar date, not per session. And per [`session/README.md`](../README.md) rule 10, this folder's existence closes `hand-2026-07-27/` to further edits — so if any work from the 27th is still unfinished, its handover belongs there, written **before** this folder is used.

## Where the project stands

`EPIC-P0` is functionally complete. Within `EPIC-P1`, two of six Features are done:

- **`FEAT-P1-01` — complete.** All three Stories Verified: the measurement harness, the committed Tier 0 baselines plus the `check-timing-regression` CI gate, and the AArch64 cycle source written with no board in hand.
- **`FEAT-P1-02` — functionally complete.** Real `#UD`/`#GP`/`#PF` handlers with a two-arm fail-closed policy and no resume arm; TSS/IST double-fault survival closing `LE-04`; and a committed `D02` fault-latency baseline closing `LE-17`. All three exit-criteria clauses met.

Every Story in both carries assurance state **`baseline-debt`, not `verified`** — Tier 0 QEMU only, no hardware-tier evidence. That distinction is load-bearing and this session must not quietly erode it.

The remaining six Stories across `FEAT-P1-03`..`-06` are `specified`.

## What is in flight, uncommitted, and yours to finish

**Check `git status` before writing anything.** As of the close of the 27th, `STORY-P1-03-01` had been *started* but not finished, and none of it was committed. What exists:

| Piece | Location | State |
|---|---|---|
| `read_cr3` / `write_cr3` / `cr3_reload_needed` | `os/src/hal-x86_64/src/paging.rs` | primitives written, host test on the reload predicate |
| `AddressSpace::cr3()` | `os/src/exec/src/address_space.rs` | written, host tested |
| `Tcb::address_space: Option<u64>` + accessor | `os/src/kernel/src/sched.rs` | written; `None` for every task, so nothing changes behaviour yet |

What does **not** exist: any `context::switch` integration, `TEST-P1-03-01-A`, any Tier 0 fixture, any Report. The primitives are the easy half. If handover 10 landed on the 27th describing more than this, that handover wins over this table.

Also unpushed as of that point: the whole `LE-17` change set plus `REPORT-2026-07-27-07` and handover 09. `origin/main` is current only through `91c95c1`.

## The mandate

**`FEAT-P1-03` — active per-task address spaces, W^X, and generation-safe teardown.** This is the Feature that turns `G-SEC-2` ("process memory is private *by construction*") from dormant machinery into a runtime fact. `EPIC-P0` built `exec::AddressSpace` page tables that are constructed and verified and then never installed; every task still runs on the boot identity map, all-RWX. `LE-05` has tracked that since `STORY-P0-05-02`.

Two Stories, in order:

1. **[`STORY-P1-03-01`](../../goals/stories/STORY-P1-03-01.md)** — per-task `CR3` in the context switch. Its acceptance criteria are still marked draft and must be finalized as the Story starts, the same way `STORY-P1-02-02`'s were. Criterion 1 is the one that matters: two tasks in distinct address spaces, and a cross-space probe from one **faults and is contained** while the other keeps running — isolation proven adversarially, not inferred from reading the mapping tables. Criterion 2 wants same-space switches to skip the `CR3` reload, with the measured cross-space delta recorded against the D04 budget.
2. **[`STORY-P1-03-02`](../../goals/stories/STORY-P1-03-02.md)** — W^X/NX mappings and generation-safe teardown (`PD-04` executable sealing, `PD-13` revoke-wipe-advance before reuse).

The hard-ordering precondition both were waiting on is now satisfied, and the Story says so explicitly: *"this Story does not start until they are Verified."* They are. That ordering was not bureaucracy — Handovers 32, 33 and 35 each refused to switch `CR3` without a real fault handler behind it, on the grounds that a live switch with no containment is strictly more dangerous than the identity map. The containment now exists, and criterion 1 is the first thing that actually *uses* it adversarially rather than demonstrating it.

## Three constraints that bite specifically on this work

**Measurement is not trustworthy on a loaded host right now.** `LE-18` and `LE-19` are both open and both live. The committed baselines were recorded on a quiet machine; on a loaded one the gate reports two to six *different* false regressions per run, and a `measure` run during the 27th showed `overhead_cycles` at 256 against the baseline run's 70, with `D05/dispatch_select` at 32,374 cycles against its own 174-cycle baseline. `STORY-P1-03-01` criterion 2 asks for a measured same-space-versus-cross-space delta — **decide how that number will be recorded, and on what machine state, before measuring it**, not after. A CR3 reload plus TLB cost is a real effect of maybe a few hundred cycles; ambient noise on this host is currently larger than the effect.

**`LE-19` part (b) is still open.** Part (a) was done — the five wrongly-rewritten baseline rows were reverted and only the new `D02` row kept — but `--update-baseline` still rewrites *every* measured row when asked to add one. If this session records a new D04/D08 baseline, that hazard is live again. Fixing it is a small `gate.rs` Story and should carry the test that would have caught it.

**A cross-space fault is the first fault this project will raise that it did not hand-place.** Everything `FEAT-P1-02` proved used a deliberate `ud2`, a chosen selector, a chosen unmapped address. Criterion 1's probe faults because the *hardware* says it must, which is a stronger claim and a harder thing to get wrong quietly. Expect the fixture to fail for real reasons before it passes, and record what those were — `STORY-P1-02-01`'s `wrmsr` finding and `STORY-P1-02-02`'s no-IST triple fault both came out of exactly that phase of work and were worth more than the code.

## Before writing implementation code

The [`agent.md`](../../agent.md) rules that will actually be checked here:

- **Test document first.** `TEST-P1-03-01-A` does not exist. Write it before the fixture, as every Story on the 27th did — that part of the TDD mandate has held even where a host Red count has not.
- **Assurance contract before code.** `STORY-P1-03-01`'s row already exists in [`story-contracts.tsv`](../../goals/assurance/story-contracts.tsv) selecting `D04`/`D08`, `SEC-03`/`SEC-19`, classes `C0`/`C1`/`C3`; it moves off `specified` only when the evidence exists. Run `cargo run -p xtask -- check-assurance-spine` from `os/`, and expect the pinned Test and Report counts in `assurance.rs` to need incrementing.
- **Read [`SECURITY_CHARTER.md`](../../SECURITY_CHARTER.md) before touching mapping code.** This Feature's containment contract names `BND-04`, `BND-05` and `BND-20`, and `PD-04`/`PD-13` are exactly what `STORY-P1-03-02` implements. Executable mapping is one of the subsystems `agent.md` requires adversarial tests for, not happy-path coverage.

## What "done" means for this Feature

Both Stories Verified at Tier 0 **with adversarial evidence**: a task provably cannot touch another task's memory (the attempt faults and is contained), W^X violations fault, teardown-then-probe fixtures prove generation safety, and the D04/D08 baselines absorb the CR3-switch cost within budget. Note the pattern `FEAT-P1-02` set on the 27th — both its Stories were functionally Verified and the Feature still did not exit, because a third exit clause (`LE-17`) was outstanding. Feature exit is not "the Stories are done."

## Standing loose ends most relevant here

The canonical register is [Handover 09 §Loose-ends](../hand-2026-07-27/09-le-17-fault-latency-baseline.md#loose-ends-register-canonical-as-of-this-handover), `LE-01`–`LE-19`. The ones this session will touch:

- **`LE-05`** — `AddressSpace` built but never installed. This Feature is what closes it.
- **`LE-11`** — `Context::new` seeds task `rflags` with `IF` set. Mitigated three times now and still not fixed; a per-task address space is a good moment to stop mitigating it.
- **`LE-14`** — `context::switch` saves no SSE/x87 state. Not this Feature's, but this Feature edits that function.
- **`LE-18`/`LE-19`** — measurement trust, above.
- **`LE-09`** — no hardware has ever run any of this. Every claim below stays Tier 0 until a Pi 5 produces a number.

## What this note does not do

It does not decide `STORY-P1-03-01`'s final acceptance criteria — that is the first act of the session, not a thing to inherit. It does not sequence `EPIC-P1_5`, whose transport decision was recorded on the 27th ([Handover 08](../hand-2026-07-27/08-epic-p1_5-deploy-loop-transport-decision.md)) and which remains undecomposed and unsequenced against this Feature. And it asserts nothing about CI: three commits reached `origin/main` on the 27th carrying the first runs of `--fixture=double-fault` and of the timing gate on a GitHub runner, and no one has yet looked at what the runner did with them.
