# Handover 41A — MS-DOS 4.x Parity: Where We Stand, and the Dependency Nobody Wrote Down

Answers the question directly and registers **`LE-48`**. No code. The work order in
[38A](38A-outstanding-actions.md) is unchanged; this does not compete with it.

## The answer in one line

**Nothing is built. Parity is 0% implemented, spec-complete on paper for a 22-verb MVP, scheduled for
Phase 2 while the project is mid-Phase-1 — and gated behind a filesystem that exists in no crate, no
phase and no document.**

## What exists

| Artifact | State |
| --- | --- |
| **Founding intent** | `SeedMVP.md` §1 item 2 — *"Looks and behaves like MS-DOS 4+"*. Fixed, not negotiable |
| **Goal** | `G-RT-5` — DOS-familiar *and* POSIX-familiar operator experience, one canonical core |
| **Spec** | [`docs/cli-compatibility-mvp.md`](../../docs/cli-compatibility-mvp.md) — **22 canonical verbs**, each with a DOS binding and a POSIX binding, two thin syntax front-ends over one ACI-gated core |
| **Crate** | `shell` — *"TINYCMD canonical verb core + DOS/POSIX front-ends"*, `#![forbid(unsafe_code)]`, **Phase 2** |
| **Epic** | `EPIC-P2` exists **only as a row in [`goals/epics/backlog.md`](../../goals/epics/backlog.md)**. There is no `EPIC-P2.md`, no Feature, no Story, no contract |
| **Code** | **None.** No `os/src/shell/`, and zero files in `os/src/` mention `TINYCMD` |

The spec is good, and it is worth saying so: one command core with two front-ends is the right shape, it
is consistent with Design Pillar 2, and it avoids implementing every verb twice.

## What "parity" can and cannot mean here

**It can only ever mean ergonomic parity, never binary compatibility.** MS-DOS 4 was 16-bit real-mode and
segmented; TinyOS is **64-bit-only by charter**. No DOS `.COM` or `.EXE` will ever execute. `README.md` is
careful about this — *"the soul of MS-DOS"*, *"looks and feels like"* — and `SeedMVP.md` records the
`MsDOS/` submodule as *"a historical command-behavior reference only, not built upon."* That is the
honest framing and it should not drift into an implied compatibility claim.

**What is already specified as *better* than DOS 4**, and these are real:

- **Both syntaxes at once.** `DIR` and `ls` against one core, front-end selectable per session.
- **Every verb capability-gated with an audit trail.** DOS had no authority model at all.
- **`task-list` / `task-kill` over the real-time task table** — no DOS analog exists.
- **`ATTRIB` maps to capability scope**, not a permission triad — a deliberate non-goal, recorded.

## The finding — `LE-48`

**At least 15 of the 22 verbs presuppose a filesystem**: `DIR`, `CD`, `COPY`, `MOVE`/`REN`, `DEL`, `MD`,
`RD`, `TYPE`, `FIND`, `SORT`, `MORE`, `TREE`, `ATTRIB`, `VOL`, plus the `.TCB` batch runtime.

**No filesystem crate exists anywhere in [`docs/mvp-delivery-strategy.md`](../../docs/mvp-delivery-strategy.md)'s
18-crate map.** Storage appears once, as a Phase 3 **block-device class driver** inside `drivers` — a
driver is not a filesystem. Searching that document for `filesystem`, `vfs` or `fat` returns **zero
matches**.

The absence is already recorded three times, in passing, by code that ran into it:

- `os/src/exec/src/iat.rs` — *"the subsystem behind them does not exist yet (no heap, no filesystem, no …)"*
- `os/src/kernel/src/spoor_journal.rs` — *"TinyOS's Phase 0 equivalent has no filesystem yet"*
- `os/src/os/src/main.rs` — *"until one exists, an embedded …"*

**And the ordering inverts.** `shell` is **Phase 2**; the storage driver is **Phase 3**. The phase that
needs files is scheduled ahead of the phase that can read a disk.

This is exactly the shape [Handover 31](31-qemu-virt-fixture-scoping.md) hit when it found that nothing in
the workspace produced an AArch64 executable — **a dependency that is obvious in retrospect, absent from
every planning document, and cheapest to fix before the Epic that trips on it is written.** No
`EPIC-P2.md` exists yet, so that moment is now.

`LE-48` does not prescribe the answer. **Either resolution is fine**: give the filesystem its own crate and
phase and reorder `shell` behind it, or declare it out of MVP scope and narrow TINYCMD's file verbs to a
RAM or embedded-image backing, annotating the 22-verb table verb by verb. **The defect is that neither is
written down.**

## What would actually be needed, in dependency order

Recorded so the next session does not re-derive it. **Not a plan and not a commitment** — the ordering
is the point, not the sizing:

1. A **block device** — the Phase 3 storage class driver, or a RAM disk to unblock earlier.
2. A **filesystem** — FAT16/FAT32 is the obvious choice for DOS-shaped semantics (8.3 names, attributes,
   volume labels) and it is the format the firmware already reads to boot a Pi 5. **This is the missing
   crate.**
3. A **console** capable of interactive editing — today the only output path is a serial `println`-class
   writer; `CLS`, `MORE` and `TREE` need cursor control.
4. **`TINYCMD`'s verb core**, then the two front-ends, then `.TCB` batch (`IF`/`FOR`/`GOTO`/`CALL`/`%1`).
5. **`TASKMGR`** — the blue full-screen view, which needs 3 and the ACI task table.

Items 1–3 are all currently absent. **The shell is not the hard part; the storage stack beneath it is.**

## Honest summary for anyone quoting this

> **DOS 4.x parity: specified, unstarted, and blocked on an unscheduled filesystem.** The 22-verb surface
> is designed and exceeds DOS 4 in authority model and dual-syntax support. Zero verbs are implemented.
> The earliest it becomes startable is after a block device and a filesystem exist, neither of which is in
> any phase before 3, while the shell is nominally Phase 2.

## The Epic

[`EPIC-P2`](../../goals/epics/EPIC-P2.md) is now written, on the owner's instruction, and it **elaborates
this finding into three flavours rather than two**: `DOS`, `POSIX`, and a native **`RT`** flavour whose
verbs — `admit`, `slack`, `jitter`, `prio` with the inheritance chain, `trace`, `budget`, `tier`,
`qualify` — exist in neither MS-DOS nor vanilla Linux. Its §3 is a security doctrine written to be
testable rather than aspirational, and its §1 proposes the `LE-48` answer (a RAM-backed labelled volume
first) without taking the decision. **The `RT` flavour is ordered third, ahead of both familiar
flavours, because it needs no filesystem** — it is the only user-visible part of the Epic that is
unblocked today.

## Concurrency, per rule 7 — this commit was held, and then overtaken

**Written while a concurrent session was mid-Story on the scheduler** (`lock.rs`, `preempt.rs`,
`sched.rs`, `wcet.rs`, plus an untracked `STORY-P1-04-04`), which left
`check-assurance-spine` red in the shared tree for *their* reason:
`Story files missing assurance contracts: STORY-P1-04-04`. Handled per rule 8 — their row was **not**
repaired, `--no-verify` was not reached for, and this commit was **held**:

- `check-spine-files` passed on the register (15 files, ids contiguous, no duplicate keys).
- **`LE-48` was verified alone in a throwaway worktree over clean `HEAD`** — green at
  *48 loose ends (29 open), 86 status headers*.
- That surfaced a second requirement of my own change, which is `LE-30`'s new gate working:
  **`goals/index.html` must state the live loose-end counts**, so adding a row means updating it or the
  spine refuses. Note the count *sentence* is still hand-maintained even though the stat *tiles* are now
  generated by `xtask emit-dashboard`. **`LE-30` closing in `b4f590e` is why this was caught within the
  hour rather than three sessions later — the gate paid for itself immediately.**

**Then the hold was overtaken, and two things went wrong that are worth more than the finding above.**

**1. `LE-48` was committed by the other session, not by me — in `8b8f703`.** They staged
`goals/assurance/loose-ends.tsv` **by path**, which is exactly what rule 1 asks for — and staging a
*shared* file by path takes **every** session's edits to it, including my appended row. So a row I wrote
landed under another session's authorship, in a commit about priority inheritance, and my
`goals/index.html` count edit did **not** travel with it. No blame attaches: **rule 1 is insufficient as
written**, and §"Staging narrowly is not enough for a shared file" in
[`CONCURRENT_SESSIONS`](../../agent/CONCURRENT_SESSIONS.md) now says so, pointing at the blob-staging
technique [40A](40A-soak-anomaly-decision.md) used for `LE-45`/`LE-46` as the actual fix.

**2. Slot `41A` collided, and the collision reached the register.** This document was `41A`; another
`41A-the-dashboard-as-a-work-order.md` was committed and is cited by `LE-47`. For a period **`LE-47` and
`LE-48` both cited `hand-2026-07-28/41A` while meaning two different documents**, and `LE-48`'s citation
pointed at a file that did not exist in git at all. This document moved to **`44A`** — theirs was
committed and cited, mine was not, so mine moves; the cheap resolution again rather than a contest — and
`LE-48`'s `raised_in` was repointed.

**That is the `NN<Letter>` convention's first real failure mode**, and it is registered as **`LE-51`**:
the register cites a *slot*, so two documents sharing one make every citation to it ambiguous, and
**nothing validates that `raised_in`/`closed_in` resolves to exactly one existing document.** The spine
checks only the open/closed ↔ `-` consistency. One directory listing per distinct slot would have caught
both the ambiguity and the dangling citation instantly. Same class as `LE-44`, one register along.
