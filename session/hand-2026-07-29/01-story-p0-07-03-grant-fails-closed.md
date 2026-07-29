# Handover 01 — `grant` Fails Closed, and the Reachable Panic `LE-40` Had Not Described

**Kernel code, not governance.** `LE-40` is closed by `STORY-P0-07-03`, on the recommendation
[`45A`](../hand-2026-07-28/45A-the-composed-scenario-under-preemption.md) §8 carried forward from
[`43A`](../hand-2026-07-28/43A-degrade-and-inheritance-compose.md) §7: the top row, blocked on
nothing.

`STORY-P0-07-03` · [`TEST-P0-07-03-A`](../../goals/tests/TEST-P0-07-03-A.md) ·
[`REPORT-2026-07-29-01`](../../goals/reports/REPORT-2026-07-29-01.md). **Host tests 613 → 621.**

## 1. What was outstanding

`LE-40`'s remedy line splits the row in two: *"State the invariant in the function contract now
(minutes); replacing the `.expect` with a fail-closed path is a Story, and it must happen before any
SMP work."* The first half was already in the source — a twenty-line comment block naming the
invariant and the two things that would silently invalidate it. This session is the second half.

The row is also careful about what it is **not**: not the TOCTOU it was originally reported as. That
analysis is correct, was re-checked, and is unchanged. `owner_space` is held under a shared borrow on
a single core; the re-read cannot diverge today.

## 2. The part worth reading: the audit found a worse defect than the row named

`LE-40` named one `.expect`. Auditing the whole function for the same *defect class* — anything on
this path that panics rather than failing closed — found a second instance:

```text
panicked at src\exec\src\shared_memory.rs:228:35: attempt to add with overflow
```

`grant` computed every page address with unchecked `+` and `*` on **caller-chosen** values. The
request that produces that line is well-formed by every check the function had: two legitimately
mapped owner pages, and a `sharee_virt` of `0xFFFF_FFFF_FFFF_F000` — page-aligned, so the alignment
check passes it; far above `KERNEL_RESERVED_REGION_END`, so the kernel-collision check passes it.
The second page's address then overflows.

Two things make it worse than the `.expect` it was found next to:

- **It is reachable today.** Single core, no shared page directory, no SMP. The `.expect` is not —
  and that is the argument for auditing the defect class rather than fixing the line the row pointed
  at.
- **The release build is worse than the debug build.** `os/Cargo.toml` sets no `overflow-checks` for
  `[profile.release]`, so it takes Cargo's default of off: no panic, the address wraps silently to
  `0x0`, and **the wrapped value is never re-checked against `KERNEL_RESERVED_REGION_END`** because
  the function is already past that check. A safety defect in debug becomes a containment defect in
  release. The debug disposition is not mild either — both profiles set `panic = "abort"`, so a panic
  here is a whole-system abort on a path whose entire contract is to be transactional.

## 3. The asymmetry inside the original defect

Worth recording, because it is the substantive half of the fix and the row did not name it.

The mapping loop re-read the owner's translation, `.expect`ed its **presence**, and re-checked its
**permissions not at all** — the first loop's permission verdict was carried across the re-read.
Under either condition `LE-40` names as invalidating that re-read, the permission half is exactly as
stale as the presence half. So the old code would have **panicked** on a page that vanished, and
**mapped the page anyway** on a page whose permissions had narrowed.

The privilege-escalating outcome was the unguarded one. Presence and authority are one verdict about
one page, and they are now read together by one function that both loops call.

## 4. What changed

| # | Change | Why |
|---|---|---|
| 1 | One `grantable_owner_page` — presence **and** authority — called by both loops | §3 |
| 2 | The re-read fails closed and rolls back, sharing frame exhaustion's rollback path | `LE-40`'s named half |
| 3 | A range check before any page is inspected, new `SharedMemoryError::RangeOverflow` | §2 |
| 4 | The generation counter refuses to wrap or saturate | It is the whole basis of `StaleGrant` |
| 5 | `revoke` inherits the range guarantee as a documented type invariant | A check that cannot fail is dead code |
| 6 | A test gating this module's non-test source against explicit panic constructs | `LE-33`/`-35`/`-36`/`-44`'s pattern |

Change 3 is checked **once up front** rather than per iteration, so the loops keep plain arithmetic —
against an invariant now stated in the same comment that establishes it. That placement is
deliberate: an invariant stated nowhere is what `LE-40` was.

## 5. The falsification

Per `ADR 0005` and `STORY-P0-01-07` clause 2. All four new failure-mode tests were run against the
pre-fix source first, and all four failed:

```text
---- this_modules_non_test_source_contains_no_panic_constructs ----
`.expect(` on this module's non-test path, line 262: `grant` is a kernel path and
`agent/CODING_STANDARDS.md` puts fail-safe above keep-trying (LE-40)

---- a_grant_whose_sharee_range_overflows_the_address_space_is_rejected ----
panicked at src\exec\src\shared_memory.rs:228:35: attempt to add with overflow

---- a_grant_whose_owner_range_overflows_... / ..._page_count_overflows_... ----
  left: Some(RegionNotOwned)   right: Some(RangeOverflow)

test result: FAILED. 14 passed; 4 failed
```

The last two read as a mismatched enum and are more than that: before this Story a nonsense `pages`
count was rejected only *incidentally*, by whichever unmapped page the loop reached first. Being
rejected for the wrong reason is how a request that overflows on a **fully mapped** region gets
through.

## 6. What is deliberately not claimed

- **SMP is not made safe.** One precondition is removed, for one function. `LE-40`'s "before any SMP
  work" ordering constraint is satisfied here and nowhere else.
- **No performance guardrail closes.** `D13` is selected because shared-memory grant is that domain's
  subject; nothing was measured. No `guardrail-evidence.tsv` row, no `TINYOS-MEAS/2` envelope, and an
  `open-debt.tsv` row records `D13` as still `specified`.
- **No Tier 0 evidence, deliberately.** No fixture drives `grant`, and the claim is the *absence* of
  a panic. A host test that provokes one and watches it become an error is sharper than a boot that
  never reaches the code.
- **`LE-52` is registered, not closed.** The §4.6 gate covers one module; every other non-test path
  in `kernel`/`exec` is ungated and no lint in this workspace enforces it. The row names why it is
  not a one-liner: the explicit half is a mechanical clippy sweep, the implicit half
  (`overflow-checks` in release) is a per-subsystem judgement about whether a panic is genuinely
  safer than a wrap.
- **Implicit panics are not covered even here.** The gate catches explicit constructs. The reachable
  defect in §2 was an unchecked `+`, not an `.expect` — the gate alone would not have found it.
- **Clippy was not run workspace-wide on this host**, for the pre-existing reason
  [`45A`](../hand-2026-07-28/45A-the-composed-scenario-under-preemption.md) §5 records:
  `hal-x86_64`'s `boot`/`interrupts`/`qemu_exit`/`serial` are `#[cfg(not(target_os = "windows"))]`.
  Clippy was run against the **real** custom target instead and is clean —
  `cargo clippy -p exec --target targets/x86_64-tinyos.json -Zjson-target-spec
  -Zbuild-std=core,compiler_builtins --lib --bins -- -D warnings` — as is host `--lib --tests`. CI
  runs the workspace form on Linux.

## 7. Concurrency — two commits landed mid-session, and the tree cleared underneath this work

This session started against `8b8f703` with two other sessions' work sitting uncommitted in the
tree, exactly as [`45A`](../hand-2026-07-28/45A-the-composed-scenario-under-preemption.md) §6
describes. **Both landed while this session was working**, in order:

| Commit | What it carried |
|---|---|
| `dd73d05` | `EPIC-P2`, the `.gitmodules`/`WindowsTerminal` submodule, `agent/CONCURRENT_SESSIONS.md`, `backlog.md`, the `LE-51` row, a `goals/index.html` edit, [`44A`](../hand-2026-07-28/44A-dos-parity-standing.md) |
| `e980b9a` | `45A`'s whole Story — `STORY-P1-04-05`, `TEST-P1-04-05-A`, `REPORT-2026-07-28-13`, `fixture_degrade_inheritance.rs`, `FEAT-P1-04`, the `kernel`/`xtask`/CI wiring, `45A` itself |

Per rule 3 of the §"second incident" correction — *re-derive your state when a concurrent commit
lands* — this was checked rather than assumed, both times:

- **Neither commit swept this session's rows.** At `e980b9a`, `LE-40` is still `open`, there is no
  `LE-52`, and `story-contracts.tsv` has no `STORY-P0-07-03`. The other sessions content-staged
  correctly; the protocol worked in the direction it was written for.
- **Committed `main` was green at each step.** `dd73d05` was verified in a throwaway detached
  worktree rather than in anyone's working tree (61 Stories, 51 loose ends (31 open), all rows
  agreeing); the worktree was then removed. The gates were re-run in full against `e980b9a`.

**The consequence is that the reason not to commit has expired.** `45A`'s edits are no longer
pending in `loose-ends.tsv`, `story-contracts.tsv` or `goals/index.html` — they are in `e980b9a`.
This session's changes are now the *only* uncommitted content in every file it touched, so
`git add <path>` is safe here in a way it was not an hour ago, and the content-staging procedure is
not needed. **It is still left uncommitted**, because committing was not asked for; the tree is
clean, green, and ready to stage as one unit. The only other dirty path is the long-standing
`goals/reports/_soak-p0-03-01.log`, which is not this session's and is left alone.

Everything else was handled per [`agent/CONCURRENT_SESSIONS.md`](../../agent/CONCURRENT_SESSIONS.md):

- **The slot.** New dated folder, so no collision was possible; the file was still created empty
  before it was written, per rule 4.
- **The shared registers.** Every TSV edit was made with a **guard before the write** (target row
  absent, neighbour row present, exact line count) and `check-spine-files` run **immediately after**,
  per rule 8 as corrected. `goals/index.html` took four surgical replacements — two generated
  stat-tile numbers and two count sentences — and no other line in that file was touched. That
  discipline is why the two commits above could land underneath this work without a conflict.

`main` is **17 commits ahead of `origin`, unpushed**.

## 8. State at the close

```text
assurance spine         23 Features, 63 Stories, 50 Tests, 51 Reports
                        52 loose ends (30 open), 90 status headers
                        63 Feature/Story status rows agree, 51 dashboard badges agree
                        11 release gates with evidence, of 391 — unchanged
host tests              613 → 621 (+8, all in exec::shared_memory; module now 19)
kernel behaviour        grant/revoke reject strictly more than before; no accepted
                        request changes its outcome
fixtures                none added, none changed
loose ends closed       LE-40. Registered LE-52
Stories verified        0 / 63 assurance-verified; unchanged and correct
```

Those counts are the working tree against `e980b9a`, and the only uncommitted content in it is this
session's (plus the untouched soak log). Both gates were re-run in full after `e980b9a` landed, not
just before it: `check-spine-files`, `check-assurance-spine`, `check-crate-sizes`, and the whole
workspace test suite.

`goals/reports/_soak-p0-03-01.log` is still dirty and still left alone. Eleventh session.

## 9. Best unblocked work next

[`43A`](../hand-2026-07-28/43A-degrade-and-inheritance-compose.md) §7's order stands with its first
two rows now struck:

| # | Action | Blocked on |
|---|---|---|
| **W3 / `LE-23`** | Re-record the baseline from a CI run. The data already exists. **The recommendation** — cheapest row left, and `LE-42` waits behind it | Nothing |
| **`LE-46`** | Run the soak sweep under `--serial-capture`. The flag exists and CI already uses it | Nothing |
| **`LE-42`** | The `D09` accept path at 17.6–39.1× its budgets. Still the most serious *unanalysed* substantive finding | A decision; `W3` first |
| **`LE-52`** | Generalise the panic gate. Do the explicit half first — it is mechanical and it bounds the implicit half | A scope decision |
| **`LE-49`** | Per-lock inheritance records. Needs blocking waiters, so a scheduler Story rather than a lock patch | A scope decision |
| **W1** | The board. A procurement decision, not an engineering one | An adapter |

Two notes for whoever picks this up:

**Stage this session first; it is one clean unit.** Seven paths, no other session's content in any of
them, every gate green. `git add` by path is safe here — §7 explains why that changed mid-session.
Then push: `main` is 17 commits ahead of `origin` and has been unpushed for eleven sessions, which
means `LE-23`'s central question — *do the committed timing ratios survive a Linux CI runner?* — has
still never actually been asked. The recommended next work is blocked on nothing except that push.

**On `LE-52`:** this session is evidence that the *implicit* half is where the reachable defect was.
Do not let the mechanical half's satisfying green tick stand in for it.
