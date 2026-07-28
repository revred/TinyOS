# Handover 41A — The Dashboard as a Work Order: What Is Blocked, and What Is Merely Unstarted

Follows [40A](40A-soak-anomaly-decision.md). **No code.** This document re-syncs
[`goals/index.html`](../../goals/index.html) against the spine and then reads the synced numbers as a
work order, because one ratio on that page turns out to say something the work order in
[38A](38A-outstanding-actions.md) does not.

**38A remains the work order.** Nothing below reorders `W1`–`W5`. What this adds is a *sizing* of what
each unblocks, a category of unblocked work that the page's own figures make visible for the first time,
and — in §4.1 — a correction to how `W1`'s blocker has been stated since Handover 26.

## 1. The sync, and the eighth hand-edit

Four stat tiles were stale: `39/48 → 41/49` functionally verified, `45 → 46` Test docs, `58/58 → 59/59`
mapped by CI, `0/58 → 0/59` assurance-verified. The prose paragraph was already current. The page now
matches `check-assurance-spine` exactly:

```text
23 Features · 59 Stories · 46 Tests · 47 Reports
46 loose ends (28 open) · 11 release gates with dated evidence, of 391
5 platforms (0 qualified) · 0 bound claims checked
41 baseline-debt · 18 specified · 0 verified
```

**This is the eighth consecutive session to hand-edit this page, and the figures moved twice while the
sync was being written** — Reports 46 → 47 and loose ends 44 → 46, both mid-edit. That is `LE-30` and
`W4`. It is worth stating plainly that hand-syncing is now costing more than the fix would: eight
sessions have each paid the ten-minute price rather than the one-Story price, and the page was wrong
again before this paragraph was finished.

## 2. The finding: hardware blocks 46 release gates, not 391

The dashboard's worst ratio is not `0 / 59 Stories assurance-verified`. It is **`11 / 391` release gates
with dated evidence**, and until now nothing said how much of that 391 the board would actually move.

Counted from [`goals/performance/catalogue.tsv`](../../goals/performance/catalogue.tsv), restricted to
the same 17 in-play domains whose 23 release guardrails give the 391 denominator:

| | Release gates |
| --- | --- |
| In play (17 domains × 23) | **391** |
| Reachable at Tier 0 or Host — no board required | **345** |
| Hardware-only (`T1`/`T2` with no `Host` or `T0` in tier) | **46** |
| Already carrying dated evidence | **11** |

**So a successful board session moves at most 46 of 391, and 334 in-play release gates are reachable
today, unevidenced, and blocked on nothing but somebody doing them.**

This does not demote `W1`. The board is still the highest-value *session*, because `Q1`/`Q2` platform
qualification and every `T1`/`T2` bound depend on it and nothing else can produce them — `0 platforms
qualified` is a hard zero that only hardware moves. What the count refutes is the *reflex*, which
`LE-31` already names: attributing this project's evidence deficit to `LE-09`. **Eight of nine
in-play release gates are not waiting for a board.**

`STORY-P0-01-05` already proved the point on ten of them, and Handover 25's `G11` argument is the
template: *heap allocations per steady-state work unit = 0* is not a benchmark, it is
compiler-enforced — every shipped crate is `no_std` with no `#[global_allocator]`, so `alloc` cannot
compile. The recorded result is **stronger than the guardrail's wording asks for**: zero in every state,
every architecture, every load, no measurement uncertainty.

## 3. Low-lying wins, ranked by evidence-per-effort

None of these need hardware. All are visible from the synced page.

**L1 — Sweep the 345 for more `G11`-shaped gates.** The question is narrow and mechanical: *which
guardrails are already true by construction, or already measured but never recorded?* `G09` (image and
feature footprint) is a strong candidate — `check-image-size` already measures it on every PR and its
number is committed; the gate is unevidenced only because nobody wrote the row. `G21` (exhaustion and
fault containment) has Tier 0 fixtures that already assert it. **This is the highest evidence-per-hour
work available**, and it converts existing artifacts into recorded evidence rather than producing new
measurements. It is also the work most likely to be mistaken for bookkeeping and skipped.

**L2 — `W3` / `LE-23`**, unchanged from 38A: re-record the timing baseline from a CI run. `LE-24` may
come free, `LE-42` depends on it. Two CI runs already showed the Windows-recorded baseline reads 23–53%
low on the Linux runner, consistently signed. **The data to act on already exists.**

**L3 — `W4` / `LE-30`**, and §1 is this session's own argument for it. Eight hand-edits. Generating the
tiles from `list-status` removes a recurring, guaranteed-to-recur error class.

**L4 — `LE-29`, and it is mine.** `STORY-P0-01-04` found nine of twenty-three Tier 0 fixtures with no CI
step — declared, compiling, passing, unexercised. The guard added covers **fixtures only**. The same
question is unasked of the 20 security controls, 20 boundary tests and 625 catalogue cells: *what is
declared and never exercised?* That question found nine defects in evidence the first time it was asked
and it has not been asked anywhere else.

**L5 — `LE-40`**: `exec::shared_memory::grant` panics rather than failing closed, on an invariant nobody
wrote down. A panic in a grant path is a containment defect in a system whose whole thesis is
fail-closed, and it is now diagnosable — `STORY-P0-01-04` gave panics a UART voice, so this one will
announce itself instead of stopping the machine silently.

## 4. What is genuinely blocked

- **`W1`** — see §4.1. The blocker is narrower than six sessions of documents have implied, and it is a
  **procurement item, not an engineering constraint.**
- **`PERF-D07-G22`** — settled in [40A](40A-soak-anomaly-decision.md): run to 72h for the stability
  data, do **not** close on this run, registered as `LE-45`/`LE-46`. Nothing here reopens that. As of
  this writing the soak is at ~36.5h of 72h, ten checkpoints, nine clean and one anomalous.
- **`LE-42`** — the D09 accept path measures **17.6–39.1x over every latency and cycle budget its own
  catalogue sets.** This is not blocked on hardware either; it is blocked on a decision about what to do
  with a measurement nobody expected. It is the most serious *substantive* finding currently open, and it
  should not be allowed to sit behind bookkeeping items simply because it is uncomfortable.

### 4.1 `W1` is not blocked the way six sessions of documents have said it is

**The framing needs correcting, and this document had it wrong too** — an earlier draft of §4 said "no
substitute exists," which is the reflex this section exists to break.

What the plan got *right*: [Handover 17](17-raspberry-pi-5-bring-up-plan.md) §4.3 and
[23](23-bcm2712-divergence-record.md) ruled out **Ethernet, USB and GPIO** because on Pi 5 all three sit
behind the **RP1 southbridge over PCIe**. That is correct and it is why `LE-25`/`LE-26` re-open the
`EPIC-P1_5` transport decision. Nobody should re-litigate it.

What was never done: **the option space was never enumerated.** Grep the whole of `session/` and
`goals/` for `framebuffer`, `HDMI`, `mailbox`, `VideoCore`, `JTAG` or `blink` and there are **zero
hits**. One channel was chosen, correctly, and then its absence silently became the project's definition
of "blocked."

Channels that do **not** go through RP1 and were never considered:

| Channel | Reaches host? | Verdict |
| --- | --- | --- |
| **HDMI framebuffer** via the VideoCore mailbox | Human-readable on a monitor; photographable | **Needs verification, and is the strongest candidate.** The mailbox property interface is SoC-side, not RP1-side. This is the classic bare-metal Pi "hello world" path |
| **Status/ACT LED** blink coding | Human-readable only | **Verify first** — on Pi 5 the status LEDs may themselves be RP1-driven, which would kill it |
| **SD-card writeback** | Yes, offline | Needs an SD/eMMC driver, an explicit `FEAT-P1-07` §6 non-goal |
| **JTAG** | Yes, bidirectional | Needs a probe — same procurement problem, more expensive |

**Why this matters for `STORY-P1-07-01` criterion 3.** That criterion is *`CurrentEL` printed first*.
Its purpose is to establish the exception level the firmware actually hands over at — **a framebuffer
print satisfies that purpose**, and a photograph of it is evidence. It would not satisfy criterion 4 (a
known byte sequence reaching the *host*), which genuinely needs a captured link. So the honest split is:
**criterion 3 may not be blocked at all; criterion 4 is.**

**And the blunt point.** A USB-serial adapter is a commodity part costing roughly the price of lunch,
available same-day almost anywhere. Six sessions of the highest-value work in the project have been
deferred on it. **That is a procurement decision that was never framed as one** — it has been carried in
every mandate as though it were a physical law. Whatever else is decided, someone should order the part
today, and the option table above should be verified rather than left as this document's assertion.

## 5. Two cautions for whoever picks this up

**Do not let `L1` become a way to make the dashboard look better.** The rule is `ADR 0005`'s and
`LE-33`'s: a gate is closed by dated evidence that meets its stated threshold, not by an argument that it
probably holds. `G11` qualified because compiler enforcement is *stronger* than the guardrail asked for.
A gate that is merely *likely* satisfied stays open, and the 11/391 figure is worth more accurate than
flattering.

**Do not read `345 reachable` as `345 easy`.** Reachable means no board is required. Many will need real
fixtures, and some will fail their thresholds when finally measured — which is the point. `LE-42` is
what a Tier-0-reachable gate looks like when someone finally measures it.

## 6. State

`main` is at `7e4e79b`, **ten commits ahead of `origin` and unpushed**, carrying three sessions' work.
The working tree also has roughly twenty modified files from concurrent work in flight. **Push before
starting anything**, and stage deliberately rather than with `git add -A` — a broken assurance spine
reached `main` once already this date by exactly that route (`428b7fd` added the protocol).

Verification at the time of writing: **593 host tests pass**, `cargo fmt --all -- --check` clean,
`check-assurance-spine` valid, CI green on its last four observed runs.
