# Handover 45A — The Composed Scenario Under Real Preemption, and the Deadlock Behind It

**Kernel code, not governance.** `LE-50` is closed by `STORY-P1-04-05`, on the recommendation
[`43A`](43A-degrade-and-inheritance-compose.md) §7 made: the smallest piece of real engineering then
available, fully unblocked.

`STORY-P1-04-05` · [`TEST-P1-04-05-A`](../../goals/tests/TEST-P1-04-05-A.md) ·
[`REPORT-2026-07-28-13`](../../goals/reports/REPORT-2026-07-28-13.md). **Host tests unchanged at 613**
— deliberately. This Story adds Tier 0 evidence and changes no kernel behaviour.

## 1. What was outstanding

`STORY-P1-04-04` closed `LE-22` — `effective = max(base, inherited)`, derived on demand — and proved
it at the **host** level through `wcet::account_tick`. It did not claim Tier 0, said so in its own
acceptance criteria, and registered the gap as `LE-50` rather than absorbing it.

That gap is now closed by `kernel::fixture_degrade_inheritance`: a **new** fixture with its own
feature flag, `xtask` table row and CI step. `fixture_priority_inversion` was left untouched, because
its run is the evidence `STORY-P1-04-01` is Verified on.

## 2. The claim, and why it needed a fixture rather than another host test

A host test asserts priority *values*. This asserts a scheduling *outcome*.

| Task | Priority | Budget | Policy |
|---|---|---|---|
| low | 5 | 4 ticks | `Degrade(2)` — takes the lock, never yields |
| high | 25 | generous | preempts low, contends (boosting it to 25), blocks |
| medium | 15 | generous | busy-counts whenever it is selected |

```text
enforcements=19 contended_at_degrade=true base_at_degrade=Some(2) effective_at_degrade=Some(25)
low effective_after_release=Some(2) state_after_release=Some(Ready) medium_outranks=true
medium at_block=0 at_degrade=0 at_release=0 final=1000 ready=[true,true,true]
dispatch order=[0, 2, 0, 2, 1] (0=low 1=medium 2=high), release_split=3
```

`base_at_degrade=2` **and** `effective_at_degrade=25`, read from inside the ISR on the tick the
enforcement was applied: the degrade landed, and the blocked waiter kept its boost.
`effective_after_release=2` is the other half — the holder left `unlock` on its floor, not the 5 it
came in with. Between them, medium's counter is **identical at all three window samples** while medium
is `Ready` at each and demonstrably runs afterwards, and the dispatch log shows its slot appearing
only after the release.

**Under the pre-`STORY-P1-04-04` kernel that frozen counter was unattainable**, which is what makes
this a demonstration rather than a re-assertion.

## 3. The part worth reading: the falsification changed the fixture

Reverting the composition in `sched` (`set_base_priority` also clearing the inherited priority — the
old single-field collision) did **not** fail the fixture on the first attempt. **It hung**, and the
harness killed it at the 15s boot budget with an **empty serial capture**.

That is not a fixture quirk — it is the defect's real consequence. Boost discarded → low sits at 2 →
medium at 15 never yields → low can never reach its own `unlock` → high stays `Blocked` forever → the
run cannot end. **A genuine deadlock, and precisely the `G-RT-1` unbounded-waiter-latency failure this
subsystem exists to deny.**

Armed to detect and not to explain is `LE-46`'s shape one level along. The fixture now counts **ticks
charged to medium while the holder still has the lock** — *exactly zero* in a passing run, growing
every tick in a broken one — and ends the run past 20 so the defect is reported rather than timed out:

```text
FAILED clause 3/7: medium was given CPU time while low still held the contended lock. This IS the
defect: ... the inversion G-RT-1 denies
FAILED clause 2: low's EFFECTIVE priority was still 25 — the blocked waiter kept its boost across
the degrade (this is LE-22's boost-then-degrade half)
enforcements=1 base_at_degrade=Some(2) effective_at_degrade=Some(2)
dispatch order=[0, 2, 0, 1]  stalled=true  medium_ticks_in_window=21
```

`effective_at_degrade` reads **2** where the fixed kernel reads **25**. Fails 3 of 3.

A wall-clock or total-tick bound would have had to exceed a legitimate window of some hundreds of
ticks — slow to fire and sensitive to host speed. Counting the inversion itself is neither.

## 4. A fixture defect a single green run would have shipped

Twelve consecutive runs were used rather than one. **Three failed**, reporting `released=false`
alongside `restore_cost=Some(2)` — the unlock had plainly happened; the fixture's own record of it had
not survived.

`LOW_RELEASED = released` was assigned *after* the `without_interrupts` block. The unlock drops the
holder to 2 and makes a task at 25 `Ready`, so the holder becomes preemptible between any two
instructions the moment interrupts return — and once medium ends the run it is never selected again,
so the assignment is simply lost. The same latent window existed on the acquire path.

Every value the checker reads is now captured **inside the critical section that makes it true**.
Clause 4's "outranked, not retired" was re-based off a phase-2 counter — whose value depends on
whether the holder gets one more iteration before the next tick — onto two facts recorded at the
release: the task's state, and whether medium outranks it. **20 of 20 pass**; the falsification still
fails 3 of 3.

The kernel was never wrong here. But `LE-45` is an open row about an unexplained intermittent failure
in this exact subsystem, and a fixture that loses its own evidence under preemption is one way to
produce one.

## 5. What is deliberately not claimed

- **`LE-49` is untouched**, and this fixture *cannot* see it — it holds one lock.
- **No performance guardrail closes.** `D03`/`D05`/`D06` are selected because the composition governs
  dispatch and lock latency, not because anything was measured. No `guardrail-evidence.tsv` row.
- **`LE-45` is still not explained.** This is Tier 0 coverage next door to the fixture that logged the
  anomaly; it does not recover a diagnostic that was never captured. The new CI step *does* run under
  `--serial-capture` and retains it on failure — `LE-46`'s remedy applied to the one fixture this
  Story owns, not to the sweep. `LE-46` stays open.
- **Clippy was not run workspace-wide on this host.** `hal-x86_64`'s `boot`/`interrupts`/`qemu_exit`/
  `serial` are `#[cfg(not(target_os = "windows"))]`, so `cargo clippy --workspace --all-targets`
  cannot build the `no_std` binaries on Windows — pre-existing, affecting `exec`'s fixtures
  identically. Clippy was run against the real target instead and is clean; CI runs the workspace form
  on Linux.

## 6. Concurrency — and this session did **not** commit

Another session is live in this tree and has work **staged but uncommitted**: `EPIC-P2`, the
`.gitmodules`/`WindowsTerminal` submodule, `agent/CONCURRENT_SESSIONS.md`, an `LE-51` row, a
`goals/index.html` edit, and [`44A`](44A-dos-parity-standing.md).

Two consequences, both handled per [`agent/CONCURRENT_SESSIONS.md`](../../agent/CONCURRENT_SESSIONS.md):

- **The slot.** `44A` was taken mid-session (that document was `41A` when this session started, and
  was renumbered by its own author). This document is `45A`, claimed by creating the file before
  writing it — rule 4. `LE-50`'s `closed_in` was repointed from `44A` to `45A` before anything
  referenced it. Their document is left exactly as it is, per rule 5.
- **The shared registers.** `goals/assurance/loose-ends.tsv` and `goals/index.html` each now carry
  *both* sessions' edits. Staging either **by path** would sweep theirs in under this session's
  authorship, which is the failure rule 1's second half was written for. **Nothing was committed.**
  The work sits in the tree for the owner to stage — and if it is committed while their edits are
  still pending, those two files need the `git hash-object` / `update-index` content-staging
  procedure, not `git add <path>`.

`main` remains **15 commits ahead of `origin`, unpushed**, unchanged by this session.

## 7. State at the close

```text
assurance spine         23 Features, 62 Stories, 49 Tests, 50 Reports
                        51 loose ends (30 open), 89 status headers
                        62 Feature/Story status rows agree, 51 dashboard badges agree
                        11 release gates with evidence, of 391 — unchanged
host tests              613, unchanged (this Story is Tier 0 evidence only)
kernel behaviour        UNCHANGED — this Story proves, it does not implement
fixtures                one added; every one still run by CI (both xtask drift guards green)
loose ends closed       LE-50. None registered
Stories verified        0 / 62 assurance-verified; unchanged and correct
```

`goals/reports/_soak-p0-03-01.log` is still dirty and still left alone. Tenth session.

## 8. Best unblocked work next

[`43A`](43A-degrade-and-inheritance-compose.md) §7's order stands with its first row now struck:

| # | Action | Blocked on |
|---|---|---|
| **`LE-40`** | `exec::shared_memory::grant` panics rather than failing closed — a containment defect in a fail-closed system, and the same *shape* as `LE-22`: a rule the code states and nothing enforces. **The recommendation** | Nothing |
| **W3 / `LE-23`** | Re-record the baseline from a CI run. The data already exists | Nothing |
| **`LE-42`** | The `D09` accept path at 17.6–39.1× its budgets. Still the most serious *unanalysed* substantive finding | A decision; `W3` first |
| **`LE-46`** | Run the soak sweep under `--serial-capture`. The flag exists and CI already uses it; this session used it for its own fixture | Nothing |
| **`LE-49`** | Per-lock inheritance records. Needs blocking waiters, so a scheduler Story rather than a lock patch | A scope decision |
| **W1** | The board. A procurement decision, not an engineering one | An adapter |
