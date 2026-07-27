# Handover 22 — The Zero Is Real, and the Reason This Repository Gives for It Is Wrong

Written at the close of 2026-07-28, prompted by a single objection to the dashboard: **0 Stories assurance-verified.**

The number is correct and it is not going to be adjusted. What came out of checking it is that **the explanation attached to that zero — in every Report, both index pages, and this Epic's exit criteria — is wrong for nine Stories**, and one of those nine needs no hardware purchase at all.

## What the zero actually means

A Story reaches assurance `verified` only when dated raw evidence closes **every** applicable release gate: `G01`–`G23` in each selected domain, plus its `SEC-*` and `BND-*` obligations. Selecting one domain brings in all 25 of its guardrails, so a dispatch Story inherits sustained throughput, cold start, burst recovery, isolation under 90% competing load, and a **72-hour soak**. The measurement protocol adds ≥1,000,000 operations for p99.9 and observed maxima, and 30 independent runs for run-to-run variation.

**So the first `verified` Story is a measurement campaign, not a boot.** Getting the Pi 5 to run `fixture_measure` is necessary and nowhere near sufficient. `FEAT-P1-07` closes `LE-09`; it does not close a single `PERF-*` guardrail, and `TEST-P1-07-06-A` clause 8 already says so.

That is worth stating plainly because the two have been running together in this project's prose for weeks, as though the board were the last thing between here and `verified`.

## The finding

Every Report in this repository carries the same sentence: *assurance state `baseline-debt`, pending hardware-tier evidence (`LE-09`)*. Checked against the catalogue's own `tier` column, that attribution does not hold for nine Stories:

```text
STORY-P0-02-03  D06            priority inheritance lock
STORY-P0-03-01  D07            static pool allocation
STORY-P0-03-03  D07
STORY-P0-05-01  D09            PE64 loading and import validation
STORY-P0-06-01  D11            spoor stamp and journal
STORY-P0-06-02  D11
STORY-P0-06-03  D06,D11
STORY-P0-07-01  D12            local IPC message channel
STORY-P0-07-02  D13            shared-memory grant
```

Every domain these Stories select names **no `T1` and no `T2` tier at all**. `D06`, `D07`, `D11`, `D12` and `D13` are `Host+T0+HIL`; `D09` is `Host+T0`. A Raspberry Pi 5 is a Tier 1 board. **`LE-09` is not what blocks these nine.**

Eight of the nine name **HIL**, which this project also does not have — `README.md` puts hardware-in-the-loop rigs at Phase 3 onward, behind the bus stack. So for those eight the blocker is real, it is just a *different* piece of missing hardware than the one being blamed.

**`STORY-P0-05-01` is the exception, and it is the interesting one.** It selects `D09` alone, and all 23 of D09's release gates are scoped `Host+T0`: an x86_64 development machine and QEMU. **That is hardware this project already has.** It is the only Story in the repository whose entire release-gate set is reachable without buying anything.

## What I am not claiming

I have not established that those 23 gates *close*. Two obstacles are visible from the catalogue rows and both need work rather than assumption:

- **`G08`** wants retired instructions, branch misses and L1D misses. QEMU TCG does not produce real microarchitectural counters, so this is plausibly Host-only — which may be fine, since the tier is `Host+T0` and Host is a real machine, but it has not been checked.
- **`G22`** is a 72-hour soak. Wall-clock rather than effort, and some soak infrastructure already exists in the tree, but it is not nothing.

There may be more once someone reads all 23 rows against what `STORY-P0-05-01` actually measures. **This is a candidate, not a plan** — and saying so is the whole discipline the SWOT named: the failure mode is acting on the first reading of a measurement, and this is a first reading.

Registered as **`LE-31`**, with the audit named as the work: establish per-Story what actually blocks `verified`, rather than attributing all 56 to `LE-09` because that was true of the ones anyone looked at.

## Why the number cannot move gradually

Separately, and this is why the zero is such an uninformative statistic: **the spine records one all-or-nothing state per Story.** A Story with 20 of its 23 release gates closed is indistinguishable from one with none. There is no per-guardrail evidence register, so no amount of genuine measurement work becomes visible until a threshold flips.

The charter's rule is that no mapped release gate may be *failed, missing, waived silently, or hidden by an aggregate score*. Recording **which `PERF-Dnn-Gnn` ids have dated evidence** does not violate that — it is the opposite of hiding a failed gate behind a summary. It is tracking, at the granularity the gates are already defined at.

Registered as **`LE-32`**.

## What changed on the dashboard

The bare `0` became `0 / 56`, joined by a second tile — **1 Story scoped entirely to hardware in hand** — and two paragraphs carrying both findings above, including the sentence that the first `verified` Story is a campaign and not a boot. The number did not move, and it should not have. What moved is that it now says what it means.

## For the next session

[Handover 21](21-next-session-mandate.md) still stands: `STORY-P1-07-01`, board or no board, is the next work. Nothing here displaces it — `LE-09` blocks 47 of 56 Stories and remains the largest single blocker in the project.

But `LE-31` changes what the *fallback* is. Handover 21 names the `LE-23` re-baseline as the thing to do when no board time exists. **The `LE-31` audit is a better fallback**, because it is the only line of work that could move the assurance-verified count off zero, and because the answer it produces — a per-Story statement of what actually blocks `verified` — is something every subsequent planning decision in this project has been made without.

## State at the close

```text
assurance spine   23 Features, 56 Stories, 43 Tests, 44 Reports
                  32 loose ends (22 open), 82 status headers
Stories verified  0 / 56 -- correct, and now explained
Blocked by LE-09  47 of 56
Blocked by HIL    8 of 56
Blocked by neither, hardware already present   1  (STORY-P0-05-01, unaudited)
```
