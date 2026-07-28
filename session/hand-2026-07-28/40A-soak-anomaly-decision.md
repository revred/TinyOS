# Handover 40A — The Soak Anomaly: the Owner's Decision, and the Missing Diagnostic Behind It

Registers **`LE-45`** and **`LE-46`** against the anomaly a long-running analysis pass surfaced. No code.
Follows [39A](39A-allocation-profiler-scoping.md); the work order in
[38A](38A-outstanding-actions.md) is unchanged.

## The decision, and it is the owner's

The 72h soak behind `STORY-P0-03-01` / `PERF-D07-G22` logged an **`ANOMALY` at `elapsed_hours=35.85`**:
the QEMU regression sweep returned `priority-inversion` exit `1` where `0` is expected. **Not reproduced
in 26 consecutive re-runs**, all exit `0` with `TINYOS-RESULT/1 ok=true`, and no scheduler, lock, preempt
or dispatch source differs from `HEAD` or changed since the previous checkpoint. **Cause unknown.**

The step-6 precondition — *"every logged cycle passed with no anomaly"* — is therefore **not met**, and
three options existed. **The owner chose: run to 72h for the stability data, do not close
`PERF-D07-G22` on this run, and register it.**

**That is the right call, and the reason is this project's own rule.** The available shortcut was to
record the host-contention hypothesis as the cause and treat the checkpoint as environmental. But the
hypothesis is **explicitly unproven** — a second agent was committing at 11:42Z, 11:44Z and 11:45Z, into
the sweep window, which is a *contributing factor at most.* Discounting an unexplained failure in order
to satisfy a gate's own stated precondition is precisely the over-claiming pattern `ADR 0005` and
`LE-33` exist to prevent, and it is worse here than elsewhere:

> **The soak exists to catch exactly the intermittent class this anomaly belongs to.** Explaining it away
> is not a neutral act — it is the instrument being told to ignore the only thing it has ever detected.

The 35.85h of passing checkpoints remain real evidence and are not discarded. What is refused is the
*promotion* of that evidence into a closed gate whose precondition it does not satisfy. **The same
distinction `ADR 0005` draws between a measurement and a bound**, one level along.

## `LE-45` — the anomaly

`owned`, and it **does not close when the remaining 35h finish.** It closes when either the cause is
identified or a clean unbroken 72h run exists. Nine clean checkpoints then one `FAIL` is not a clean run,
and 26 clean re-runs afterwards do not retroactively make the tenth checkpoint pass.

## `LE-46` — why nobody can say what happened, which is the worse defect

**The soak sweep runs without `--serial-capture`.** So when checkpoint 10 recorded the `FAIL`, **no
diagnostic existed for the one run that mattered**, and 26 clean re-runs cannot recover what the failing
instance would have printed.

That is the finding worth more than the anomaly itself. An unreproducible failure with no capture is
**indistinguishable** from an environmental artifact *and* from a real intermittent scheduler defect —
and those two have opposite consequences. The sweep is **armed to detect but not to explain**, which is
`LE-29`'s *declared-but-never-exercised* shape one level along: the instrument fires and leaves no trace.

The fix is cheap and the machinery exists. `--serial-capture` is already a flag, and CI already uses it
for `broken-boot` and `idt-apic-unrouted`. **Until it is on, any future `PERF-D07-G22` closure is exposed
to this same objection**, because an anomaly with no capture cannot be adjudicated in either direction.

## Concurrency, per rule 7 — and the register was not touched the easy way

**A concurrent session's four row-closures were sitting uncommitted in `loose-ends.tsv`** when these two
rows were written: `LE-33`, `LE-35`, `LE-36` and `LE-44`, all closed in
[39B](39B-four-prose-rules-become-gates.md). Appending to that file and staging it would have swept all
four into this commit under this session's authorship — **rule 1's exact failure, and the incident that
caused `CONCURRENT_SESSIONS` to exist.**

So rule 8's prescribed technique was used instead, the one the `STORY-P1-07-02` session established:

1. A **throwaway worktree over clean `HEAD`**, with the two new rows appended there.
2. `check-assurance-spine` run **in that worktree** — 46 loose ends (32 open), 84 status headers, green.
   (`check-spine-files` does not exist at `HEAD`; it is part of the same session's uncommitted work,
   which is itself confirmation the worktree was clean.)
3. The verified blob staged **directly**, so the index contains `HEAD` + these two rows and **none of
   their four lines**. Confirmed before committing: `git diff --cached --numstat` reported exactly
   `2  0`.

**Their work stays theirs, unstaged and unrepaired.** `--no-verify` was not reached for.

**`LE-33`'s closure fires 39A's own caveat.** 39A §5 and §9 said to re-check the ranking of the
allocation profiler if `LE-33`'s second condition landed, *"rather than inheriting it"* — and it has
landed, in `39B`. The gate that would refuse to promote a profiler number into a bound now exists, so
**the argument against ranking W5 early is materially weaker than when 39A was written.** Recorded here
because that is what the caveat was for.

## The slot collision resolved itself

Two documents claimed `39A`. On the owner's instruction it was **not** settled by creation order, and
neither session contested it: the other renamed to `39B-four-prose-rules-become-gates.md`. **That is the
letter convention working exactly as [38A](38A-outstanding-actions.md) §7 argued it would** — a claimed
slot is cheaper to work around than to contest.

## State

```text
main                    dcbccad + this commit; ELEVEN ahead of origin, UNPUSHED, three sessions
register                46 rows, 32 open (LE-45, LE-46 added here; four closed in 39B, uncommitted)
soak                    36.5h of 72h; nine clean checkpoints, one ANOMALY at 35.85h
PERF-D07-G22            will NOT close on this run, by owner decision
host tests              593 across the workspace, 0 failed
next                    the board if an adapter exists; else -M virt (W2), then LE-23 (W3)
```

`goals/reports/_soak-p0-03-01.log` is **not edited by this session.** It belongs to the soak's owner and
these two rows are the register's record of what it says.
