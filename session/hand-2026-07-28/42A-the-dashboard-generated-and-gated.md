# Handover 42A — Grounding 41A, and the Dashboard Stops Being Hand-Maintained

Grounds [41A](41A-the-dashboard-as-a-work-order.md) against the tree, then executes its `L3`.
`LE-30` is closed by `STORY-P0-01-08`. **41A is not edited** — the convention is that you record
corrections in *your* document and point back, so §2 below is the errata and 41A stands as written.

## 1. What 41A got right, checked rather than assumed

**Its central arithmetic is exact, and is now derived rather than asserted.** Counted independently
from [`catalogue.tsv`](../../goals/performance/catalogue.tsv):

| | 41A said | Verified |
|---|---|---|
| In-play release gates (17 selected domains × 23 release guardrails) | 391 | **391** ✔ |
| Reachable at Host or Tier 0 — no board | 345 | **345** ✔ |
| Hardware-only | 46 | **46** ✔, and all 46 carry the identical tier string `T1+T2` |

`G24`/`G25` correctly excluded as `claim` gates. The finding — **eight of nine in-play release gates
are not waiting for a board** — holds exactly as stated, and `345 / 391` is now a self-recomputing
tile beside `11 / 391` rather than a number in a handover.

**§1's four tile figures are all correct**: `41/49` functionally verified (8 `Functionally Verified`
+ 33 `Verified` of 49 `EPIC-P0`/`EPIC-P1` Stories), `46` Tests, `59/59` mapped, `0/59`
assurance-verified, and `41 baseline-debt / 18 specified / 0 verified`.

**§4's soak state is right**: ten checkpoints, one anomalous, first at `2026-07-27T00:01:16Z` and
last at `2026-07-28T11:51:09Z` — ~35.8h elapsed, which is the "~36.5h" §4 states.

**§1's own thesis proved itself again during this session.** The page went stale *twice more* while
this Story was being built — once for `LE-47`, once for this Story's own artifacts. Both were caught
by the gate rather than by a reader, which is the entire point.

## 2. Errata — three corrections, one of them load-bearing

### 2.1 `L1`'s two named candidates are both already refused, in writing

This is the one that changes the agenda. §3 `L1` nominates `G09` and `G21` as strong candidates,
`G09` "unevidenced only because nobody wrote the row". **Both were considered and deliberately
declined by [`STORY-P0-01-05`](../../goals/stories/STORY-P0-01-05.md)'s named debt**, with reasons:

> - **`G09` is not claimed.** Its cadence is also every-PR, but its method is a per-feature
>   section-size delta against the parent commit, and `check-image-size` enforces one whole-image
>   ceiling. **Claiming the gate on the strength of a related measurement is the substitution this
>   project refuses elsewhere.**
> - **`G21` is not claimed** — its every-PR half is fault containment, and its **timing half is not
>   hardware-free**.

The catalogue bears this out: `PERF-D01-G09`'s target is *"feature code plus read-only data **delta**
≤ 96 KiB"*, which is not the quantity `check-image-size` computes; and every `G21` row carries a
latency threshold (`≤ 2000 µs`, `≤ 0.25 µs`) beside its containment clause.

**41A's own §5 caution is the correct verdict on 41A's own `L1`**: *"a gate that is merely likely
satisfied stays open."* No gate was recorded from `L1` in this session, and none should be without
first reading `STORY-P0-01-05`'s debt list — which is where the next `L1` attempt should start,
because it names what has already been ruled out and why.

**The underlying question survives and is still worth asking.** *Which guardrails are true by
construction, or already measured but never recorded?* is a good question; `G09` and `G21` are simply
the two answers already known to be wrong.

### 2.2 `check-image-size` does not run in CI

§3 `L1` says it "already measures it on every PR". It does not: `.github/workflows/ci.yml` invokes
`check-crate-sizes`, `check-performance-catalogue`, `check-assurance-spine`,
`governance-fixture-test`, `qemu-x86_64` and `measure` — **`check-image-size` appears nowhere.** It
exists as an `xtask` subcommand that someone has to run by hand. That is arguably its own small
finding, and it makes `G09` weaker than §3 presents it, not stronger.

### 2.3 The "zero hits" grep is literally false and substantively right

§4.1 says grepping `session/` and `goals/` for `framebuffer`, `HDMI`, `mailbox`, `VideoCore`, `JTAG`
or `blink` returns **zero hits**, and instructs the reader to verify. A reader who does will find
hits, and might conclude the analysis is loose. It is not — every hit is unrelated:

| Term | Where | What it actually is |
|---|---|---|
| `framebuffer` | `goals/context/application-platforms.tsv` | a **game** workload's capability list, not a bring-up channel |
| `JTAG` | `session/hand-2026-07-28/07-memory-confidentiality-proposal.md` | threat **A5**, *"cold boot, JTAG"* — an **attacker**, not a tool |
| `blink` | four `index.html` files | `BlinkMacSystemFont` in a CSS font stack |
| `HDMI`, `mailbox`, `VideoCore` | — | genuinely zero, outside `LE-47` itself |

**The `JTAG` hit sharpens §4.1 rather than weakening it.** In this repository's entire history, JTAG
appears once, as something an adversary does to us. It has never appeared as something we could do.
That is a cleaner illustration of §4.1's thesis than "zero hits" was.

**§4.1's substance stands**: the option space was never enumerated, and `LE-47` is the right place
for it. This session did **not** verify the option table's technical claims — whether the Pi 5
mailbox property interface is reachable without RP1, and whether the status LEDs are RP1-driven, are
both open and both need a datasheet rather than a grep.

### 2.4 §6's state block is stale

It says `main` is at `7e4e79b`, ten commits ahead, with "roughly twenty modified files from
concurrent work in flight". Those twenty files were `STORY-P0-01-07`, which landed as `443579d`
before 41A was read. At the start of this session `main` was at `443579d`, **13 commits ahead of
`origin` and unpushed**. Everything else in §6 was accurate: 593 host tests, `fmt` clean, spine valid.

## 3. What landed: `LE-30`

`STORY-P0-01-08` · [`TEST-P0-01-08-A`](../../goals/tests/TEST-P0-01-08-A.md) ·
[`REPORT-2026-07-28-11`](../../goals/reports/REPORT-2026-07-28-11.md). **Host tests 593 → 607.**

**The page has two kinds of content and they got different treatment**, which is the whole design:

- **The stat tiles are generated.** `cargo run -p xtask -- emit-dashboard` prints the block;
  `check-assurance-spine` byte-compares the committed region against it and, on a mismatch, **prints
  the expected block**, so the fix is in the error rather than in someone's memory of which tile
  moved. `emit-dashboard` deliberately does *not* run the dashboard check — a command whose job is to
  print the repair for a stale page must not require the page to be fresh — and deliberately does not
  write the file, so applying it is a reviewable diff.
- **The prose is gated, not generated.** That page is an *argument*, with paragraphs explaining why
  `0 / 59` is the right number and why the reason usually given for it is wrong. A generator that
  owned the whole page would destroy the thing that makes it worth reading. So only its *claims* are
  extracted: the spine-count sentence, the loose-end count, and every Story status badge.

### 3.1 The badge check found the same defect one document along — including mine

`LE-44`'s rule, applied to the dashboard, found **seven badges reading `VERIFIED` for Stories whose
own header says `Functionally Verified`**. Six were pre-existing. **The seventh was
`STORY-P0-01-07`'s, written by [39B](39B-four-prose-rules-become-gates.md) — the session that built
the `LE-44` gate**, in the same week, after correcting seven Feature-table cells for exactly this.

That is worth stating plainly rather than quietly fixing. A rule that the person who just implemented
it cannot apply reliably one document later is a rule that needs a machine, not a more careful
reader. `LE-30` and `LE-44` turn out to be the same row wearing different clothes. All seven were
corrected toward the Story; **nothing grandfathered**, for 39B's reason.

## 4. The gate demonstrated against something real

Mid-session the tree carried `LE-47` uncommitted. `check-assurance-spine` said:

```text
goals/index.html does not state the current loose-end counts.
Expected `<strong>47 loose ends (29 open)</strong>` (LE-30)
```

A register row and the page summarising it had gone out of step, and previously nothing would have
said so — the drift would have been found by the *tenth* consecutive hand-sync. This session's own
subset was verified over clean `HEAD` in a throwaway worktree, which is the response
[`CONCURRENT_SESSIONS`](../../agent/CONCURRENT_SESSIONS.md) rule 8 prescribes, and passed there.

`LE-47` and `41A` itself are carried in this session's commit as **reviewed, not authored** — they
are the owner's in-flight work, read in full, and no credit for them is claimed here.

## 5. The work order

Unchanged from [38A](38A-outstanding-actions.md), with 41A's sizing folded in and `L1` re-ranked on
§2.1.

| # | Action | Blocked on |
|---|---|---|
| **W1** | **The board.** `STORY-P1-07-01` criteria 3 and 4, `STORY-P1-07-02` clause 2. **It moves at most 46 of 391 release gates** — still the highest-value session because `Q1`/`Q2` and every `T1`/`T2` bound depend on it and nothing else produces them | An adapter. **A procurement decision, not an engineering one** (41A §4.1) |
| **W2** | The `-M virt` fixture. Three decisions, not four — `LE-35` landed in 39B. Handover 31's recommended slot is now `STORY-P0-01-09` | Nothing. Still the best unblocked engineering work |
| **W3** | `LE-23` — re-record the baseline from a CI run. `LE-24` may come free; `LE-42` depends on it | Nothing. **The data to act on already exists** |
| **L1′** | The `G11`-shaped sweep, **restarted from `STORY-P0-01-05`'s debt list** rather than from `G09`/`G21` | Nothing, but its yield is unknown and lower than 41A estimated |
| **`LE-42`** | The `D09` accept path at 17.6–39.1× its own budgets. 41A is right that this is the most serious substantive finding open and should not sit behind bookkeeping | A decision, and `W3` first |
| **`LE-47`** | Verify 41A's option table against the BCM2712 datasheet — mailbox reachability, whether the status LEDs are RP1-driven | Nothing. Cheap, and it could unblock criterion 3 |

**`W4` is done.** `L4` (`LE-29`) and `L5` (`LE-40`) are unchanged and unstarted.

## State at the close

```text
main                    443579d + this session's commit
                        THIRTEEN commits ahead of origin before this one, UNPUSHED
assurance spine         23 Features, 60 Stories, 47 Tests, 48 Reports
                        47 loose ends (28 open), 86 status headers
                        11 release gates with evidence, of 391
                        345 of 391 reachable with no board -- derived, not asserted
                        24 open-debt selections, 5 platforms (0 qualified)
                        0 bound claims checked -- still not good news, see 39B section 2
                        60 Feature/Story status rows agree, 49 dashboard badges agree
host tests              607 across the workspace, from 593
Stories verified        0 / 60 assurance-verified; unchanged and correct
loose ends closed       LE-30. LE-47 registered by the owner, unstarted
best available work     the board, if an adapter is ordered
best UNBLOCKED work     the -M virt fixture; then LE-42, which nobody has decided about
```

`goals/reports/_soak-p0-03-01.log` is still dirty and still left alone. Eighth session.
