# 10C — The Work Is on `main`, and the Runner Is in an Outage

Session handover, written 2026-08-06, closing the day that ran from
[`03B`](03B-the-arms-are-built-the-board-booted-them-and-nobody-read-the-wire.md)
to [`09C`](09C-the-guards-are-in-and-the-canvas-stops-painting-on-faith.md).
**No plug, no board run, and no code written here** — this document records a
commit, a diagnosis, and one loose end that got sharper without moving.

**The one sentence, if only one survives:** *`b4a7010` is on `origin/main` with
seven sessions of work in it, and the reason no CI run exists for it is that
GitHub Actions is in a `major_outage` — which took six ruled-out candidates to
reach and would have been guessed wrong in one.*

---

## 1. What landed

**`b4a7010`**, fast-forward from `420e875`. 66 files, +8531 / −197.
`origin/main` and `main` level. It carries `03B` through `09C`: the two `G23`
rows, `LE-97`/`LE-98`/`LE-99` and their guards, `config.txt`'s
`hdmi_force_hotplug`, `tos64-power`, `board-run`, `gem_receive`, and seven
handovers.

Staged under `CONCURRENT_SESSIONS` rule 1: **`git add -A` never used**, all 66
paths listed explicitly, the two directories expanded and checked for build
output first. The pre-commit hook validated the **index** rather than the
working tree (rule 2), then `check-boot-images` and `check-guest-images` ran
before the push, because the hook prints its own reminder that it compiles
nothing for either target and this commit touches `kernel` and `hal-arm64`
(`LE-72`, `LE-92`).

**Rule 3 is handled in the commit message rather than assumed.** It names what
this session authored and reviewed against what it inherited from `03B`–`07C`
and did not read — `work/tools/power/` and its 99 tests, `gem_receive.rs`,
`guest_images.rs`, `Send.cs`, `TransferBeacon.cs`, `STORY-P1-09-16`,
`TEST-P1-09-16-A`, and five handovers. Authorship on a commit asserts review;
where that assertion would be false, the message says so.

## 2. `LE-99`'s residue, and the mutations that proved nothing

Review found two holes in the first cut of the guard, both of which **failed
quiet**, which is the direction that matters: a stamp inside `dispatch::run_once`
was invisible to a body scan, and an unqualified call after a `use` counted zero
against a fully-qualified needle. Both are closed — a transitive assertion that
the round's stamp surface is empty, and a bare-identifier needle — and both are
mutation-tested.

**Three attempts, and the first two proved nothing.** They died on lint config
(`mismatched types`, then `missing documentation for a function`) before the
assertion ever ran. A mutation that fails for the wrong reason is a mutation
that was not run, and reporting "falsified" after the first red would have been
false. The third put the stamp inside `run_once`'s real body — the realistic
change anyway — and the guard caught it.

`LE-99`'s owner path now states what the guard **cannot** see: the transitive
check is one module deep; both scans are text, so a macro-emitted or generated
stamp counts zero; a renaming re-export slips the needle; and the exits count is
whitespace-pinned, which fails *loud* and is the acceptable direction. **A text
scan is not a call graph.** A closed row that reads as fully guarded is worse
than an open one.

## 3. The CI diagnosis, and why it is worth writing down

No workflow run exists for `b4a7010`. Six candidates ruled out before the
answer:

| candidate | ruled out by |
|---|---|
| push did not land | `git ls-remote` → `b4a7010` is `refs/heads/main` |
| Actions disabled | `permissions.enabled: true`, `allowed_actions: all` |
| workflow disabled | both workflows `state: active` |
| workflow broken by this commit | `.github/` untouched — 0 of 66 files |
| path filters excluding it | `ci.yml` is `on: push: branches: [main]`, no `paths:` |
| run exists but hidden | `?head_sha=b4a7010` → `total_count: 0` |

Then the one that fails **silently and by omission**, which is this project's
recurring shape: **a push authenticated as `GITHUB_TOKEN` or a GitHub App does
not create workflow runs** — deliberate recursion prevention, producing exactly
this signature: ref updated, zero runs, no error anywhere.

It is not that either:

```
PushEvent 2026-08-06T20:52:07Z actor=revred head=b4a7010
PushEvent 2026-08-06T01:12:58Z actor=revred head=420e875   ← did create a run
gh auth  → revred, token gho_******** (OAuth user, not ghs_)
```

Same actor, same token class, as the pushes twenty hours earlier that did
trigger runs.

**The answer:**

```
githubstatus.com/api/v2/components.json
  "name":"Actions","status":"major_outage"
```

Nothing in the repository is wrong. **Recorded because the six ruled-out
candidates are all things a next session would otherwise re-check**, and because
the instinct on seeing a missing run is to change configuration — which here
would have broken something that works in pursuit of a fault that is not ours.

**What this means for the next session:** re-check
`gh run list` for `b4a7010` when Actions recovers. The run has never executed,
so its result is unknown rather than green — and §4 says what it will say.

## 4. `LE-23` — sharpened by this commit, and still the owner's

The gate has been red since `fb3f36c`, and **not for the reason the row
records**. The offset `LE-23` is about is a systematic Windows-vs-Linux skew;
what CI is failing on is metrics with *no baseline at all*, which
`check-timing-regression` refuses rather than silently ignores — correctly.

`b4a7010` committed the `D04` and `D05` spoored arms into the x86_64 Tier 0
fixture, so it goes **9 → 11 metrics** and CI will name **three** unbaselined
metrics where it named one. **The gate is getting louder while gating nothing**,
which is the cost `05C` §5 predicted in the decision table — now with a number.

`--update-baseline` was declined again, by a fourth independent session, and it
is not a judgement call: it rewrites the whole file, so running it on a laptop
replaces CI-runner rows with Windows-host rows and re-creates the exact offset
this row exists to record. That is `LE-23` by name.

**What did get cheaper.** All three unbaselined metrics are `_spoored` arms
whose **plain twins already have baselines**, measured in the same fixture on
the same run. That is a better-posed form of the second half of decision 1 —
*which of `min_cycles`/`p50_cycles` versus the ratios is a reader entitled to
trust* — because a paired arm makes the **ratio** self-evidently the durable
quantity and the absolute cycles self-evidently the host-dependent one. The
owner still decides; the evidence for deciding is now in the tree rather than
hypothetical.

## 5. The next session, in order, still with no plug

1. **`LE-91`** — unchanged, and now the headline. One session by its own
   estimate: a per-metric domain-and-owning-Story declaration parsed out of the
   fixture sources, asserted against the Story's contract. `PERF-D11-G01` is the
   worked example of what a bent label costs — a gate nobody read for two days.
2. **Check `b4a7010`'s CI run** (§3) once Actions recovers. Expect red, expect
   it to name three unbaselined metrics, and expect that to be `LE-23` rather
   than a regression. If it is red for any *other* reason, that is new.
3. **`LE-98`'s remaining half** — the device-tree parse that makes
   `SIMPLEFB_BASE` evidence rather than folklore, and removes the fault path's
   named exception with it.
4. **When the board next runs**, it is a checklist rather than a discovery:
   `hdmi_force_hotplug=1` and HDMI0; whether `TOS64-CANVAS/1 painting=no`
   appears — its presence says the 2026-08-06 dark canvas was a firmware display
   failure, its absence says TinyOS; and `LE-96`'s remaining half.

**Do not start:** `FEAT-P1-05`'s RT reserve, `G09`/`LE-86`, `06A` §4.3. The
hardware-evidence sprint rule from 2026-07-30 has not been lifted.

## 6. State at close

- **Committed and pushed.** `b4a7010` on `origin/main`. This handover and the
  `LE-23` amendment follow it.
- **Gates, locally:** spine green, `check-boot-images` 3 variants,
  `check-guest-images` 22 binaries, `check-lints` 8 packages,
  `check-citations`, `fmt`, workspace suite, `netboot.tests` 54/54,
  `power.tests` 99/99. `check-timing-regression` RED — §4.
- **CI:** no run, platform outage, **unknown rather than green** (§3).
- **Spine:** 31 Features / 99 Stories / 82 Tests / 62 Reports, **99 loose ends
  (49 open)**, **25 of 460** release gates carrying evidence — up from 23, the
  first movement in four sessions.
- **Bench:** board **powered and beaconing**, untouched all day by `08C`–`10C`.
  UDP 67/69 clear. **No plug on the desk.**

**The standing instructions, all holding.** Do not report `x/460` undecomposed.
`PERF-Dnn-Gnn` is only meaningful if `Dnn` is the domain of what you measured.
Verify the digest and size the window before you spend the boot. A gate written
for one architecture, one tool or one direction does not generalise itself.
Build the unblocker rather than the next blocked artifact, and say so. A tool
that prints the value it chose is not the same as a tool that refuses a value it
cannot justify. An escape argument is a property of one denominator. Check what
the code does on the machine before writing down what it does.

**And a ninth, from this session:** *a mutation that fails for the wrong reason
is a mutation that was not run* — two of the three that were supposed to falsify
`LE-99`'s transitive guard died on lint configuration before reaching the
assertion, and both would have been reported as red by anyone reading only the
exit code.
