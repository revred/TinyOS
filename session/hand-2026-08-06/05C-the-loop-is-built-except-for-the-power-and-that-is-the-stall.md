# 05C — The Loop Is Built Except For The Power, And That Is The Stall

Session handover, written 2026-08-06, immediately after
[`04C`](04C-the-board-can-listen-and-three-instruments-stopped-lying.md) and at
the owner's direct observation: **"we seem to have stalled for the last 48
hours."**

That observation is correct, it is measurable, and the cause is not effort or
sequencing. **Every stage of the board evidence loop is built and proven except
one, and the one is the power cable.** Because a session cannot power-cycle the
board, every session ends at the last thing a laptop can do — which is why the
last two ended *built, gated, and unmeasured*.

The single change that unstalls this project costs about £15 and one C# tool.
§3 is that. Everything else in this document is subordinate to it.

---

## 1. The stall, measured rather than asserted

| signal | 08-04 | 08-05 | 08-06 (`01A`) | 08-06 (`02A`) | 08-06 (`03B`) | 08-06 (`04C`) |
|---|---|---|---|---|---|---|
| release gates with evidence | 20 | 20 | 21 | **23** | 23 | 23 |
| Stories assurance-verified | 0 | 0 | 0 | 0 | 0 | 0 |
| board measurements taken | many | 2 | 1 | 0 | **0** | **0** |

**Evidence has not moved since `02A`.** Two consecutive sessions produced zero
measurements, and both ended the same way: a thing built, every local gate
green, and the number it exists to produce still empty. `03B` built the `D04`
and `D05` paired arms and measured neither. `04C` built the inbound path and
witnessed nothing arrive.

Two more numbers, because they say *why* rather than *that*:

- **`03B` §6 item 1 was costed at "~20 minutes, and everything is staged."**
  It is still staged — the digest in `os/target/pi5/` is still
  `b6dbabae…`, byte-identical, two sessions later. **A twenty-minute task has
  now survived two sessions.** Nothing about it got harder.
- **24 loose ends were raised in these 48 hours** (12 on 08-05, 12 on 08-06),
  against 46 open of which **38 are `unowned`**. The register is accumulating
  findings faster than anything is being closed against the product.

## 2. The diagnosis, and it is not "work harder"

**The owner is on the critical path twice, and sessions route around both.**

1. **Physically**, for every power cycle. A session cannot boot the board.
2. **By decision**, for the Tier 0 baseline (`LE-23`) and the platform
   qualification that locks `0/99` (`LE-94`).

An agent session optimises for what it can finish. So it builds the thing whose
last step is a laptop command, files it green, and writes a handover whose item
1 is a board run for somebody else. **The next session reads that item, cannot
do it either, and builds the next thing.** Three handovers in a row now open
with a board item nobody could take.

That is not a discipline failure and it will not be fixed by a stronger
instruction in a handover — `03B` already wrote *"one boot, and everything is
staged, ~20 minutes"* in bold at position 1, and it did not help, because the
instruction was addressed to someone who cannot execute it.

**The register agrees, from the other direction.** `assurance-status` reports
**zero** gates in implemented domains that need a board. So the board is not
what blocks the 460 — and yet the board is what blocks nearly every *Story
criterion 4* in the tree, the two `G23` arms, `LE-82`, and `STORY-P1-06-02`.
The evidence the product needs and the evidence the register counts have come
apart, and the board is the hinge.

## 3. The unlock: every stage of the loop exists except the power

This is the finding that makes the rest cheap, and it was not obvious until
`04C` closed `LE-93`. **The board evidence loop is now built end to end, with
exactly one gap:**

| stage | tool | state |
|---|---|---|
| build the image | `cargo run -p xtask -- pi5` / `check-boot-images` | **built** |
| verify the staged digest | `tos64-netboot` logs sha256 per transfer | **built** (`LE-87`) |
| serve it | `tos64-netboot`, refuses to share a port | **built** (`LE-87`) |
| read the log live | `AutoFlush` on every tool | **built** (`LE-90`, closed 04C) |
| **power-cycle the board** | — | **A HUMAN HAND** |
| wait for a board event | `ti64dink --until <cond> --timeout` | **built** |
| capture the envelope | `ti64dink --live --text` | **built** |
| **transmit to the board** | `ti64dink --send <arm>` | **built** (`LE-93`, closed 04C) |
| parse it | `xtask parse-meas` | **built** |
| gate the numbers | `xtask check-timing-regression` | **built** |

Nine of ten stages are working, tested, and were each hard-won — three of them
in the last two days. **The tenth is a mains socket.**

### What to build: `tos64-power`, and then `xtask board-run`

**A network-controlled relay or smart plug on the Pi 5's supply, driven by a C#
console tool under `work/tools/power/`** — the `sdprep` pattern, per the owner's
standing rule that bench tools are C# and never scripts.

Choose the device on one property and ignore every other feature: **it must be
controllable over the LAN without a vendor cloud account.** A Tasmota or
ESPHome-flashed plug exposes a plain local HTTP endpoint; several Shelly models
do the same natively. A cloud-only plug is not acceptable here and the reason is
not convenience — a bench whose board cannot be rebooted when someone else's
service is down is a bench with a new and worse instrument failure, and this
project has had five of those in a row.

Then `xtask board-run` composes what already exists:

```
stage → verify digest → start netboot → POWER OFF, POWER ON
      → ti64dink --until <condition> --timeout
      → capture → parse-meas → file
```

**Fail-safe, because this drives mains power and the priority order does not
bend for convenience:** the tool never leaves the board off on any error path
(off is the one state a subsequent run cannot recover from without a hand); it
refuses to cycle while a TFTP transfer is in flight; it bounds the off-interval
and the on-wait; and a plug that does not confirm its new state from a readback
is reported as *unknown*, never as *done*. That last one is `LE-87`'s lesson
applied before the defect rather than after: **half a success reported as a
success is this bench's signature failure**, and it has happened five times.

### What it converts

- **Every Story criterion 4 in the tree** stops being "blocked on hardware" and
  becomes an ordinary test. `STORY-P1-09-16`, `STORY-P1-06-02`, `STORY-P1-09-01`
  (the absence arm, which needs a boot with `pciex4_reset=0` *removed* — a
  config change plus a reboot, which is exactly what this automates),
  `STORY-P1-09-02` (the cable-out arm), `LE-82`.
- **The `G23` arms and every future measurement.** `BOARD VERDICT 9` measured
  ~3% build-to-build movement, which is why paired arms must share a boot. A
  bench that can boot on demand can also boot *thirty times*, which is the only
  way this project will ever have a run-to-run CV — and two gates are currently
  **refused** (`PERF-D03-G20`, `PERF-D11-G02`) for the sole reason that nobody
  has one.
- **`ADR 0005` Q3, and therefore the `0/99` ceiling.** Q3 is a *residency
  campaign with a stated duration*. A campaign is precisely the thing a manual
  bench cannot run and an automated one runs overnight. **The single locked gate
  in this project is downstream of a power switch.**

That last point is the argument. `LE-94` established that `0/99` is one row in
one TSV; this establishes that the row is reachable.

## 4. Do this first, today, regardless: one boot now closes four things

Even before any automation, **the batched board session has grown to four
deliverables and is still unspent.** It was two when `03B` wrote it. Take it in
one power cycle:

1. **`03B` §6 item 1.** Start `tos64-netboot` **first**, verify the staged
   digest is `b6dbabaea3431afa94cf9210374826bde9e5fb4efef7c5c861b92795c5006f02`
   (298,089 bytes), *then* power on — never rebuild while a server is serving.
   Capture **60 s** (18 lines at 1 Hz), `parse-meas`, expect `metrics=14`. File
   `PERF-D04-G23` and `PERF-D05-G23`, reading the `target` column first and
   checking the domain label is the subject's. **Expect a large `D04` fail** —
   a 137-cycle stamp on an 80-cycle round trip — and record it as one, with
   `D04`'s existing residue caveat on the row.
2. **`STORY-P1-09-16` criterion 4.** Five commands, both arms:
   `ti64dink --send ping | unicast | ethertype | prefix | notforus`. Read the
   canvas `TOS64-RX/1` row against each arm's stated expectation. **`notforus`
   must move neither counter** — that is the hardware address filter being the
   containment the Story claims it is, and a moved `refused` is a finding.
3. **`STORY-P1-06-02` criterion 4** — `Rp1CommandLines` compiles for AArch64 and
   nothing calls it; a probe on GPIO 20..27.
4. **`LE-82`** — `Rung::FaultTaken` is declared in both vocabularies and stamped
   by nothing, so no `mmu-fault` boot proves anything until it is stamped at the
   `hal-arm64` fault-report site.

Items 3 and 4 need a build before the boot; items 1 and 2 need nothing.

## 5. The owner decision queue, batched once instead of re-raised every session

Three decisions have now been carried forward by three consecutive handovers,
each re-argued from scratch in prose. **They are collected here so they can be
answered in one sitting, and they should not appear as prose in a fourth.**

| # | Decision | What it unblocks | Cost of not deciding |
|---|---|---|---|
| 1 | Is a Tier 0 baseline recorded **off** the CI runner one this project wants committed — and of `min_cycles`/`p50_cycles` versus the same-run ratios, **which is a reader entitled to trust?** (`LE-23`, `LE-19`) | `check-timing-regression`, **red on `main` since 08-06** | The gate stays red, so it gates nothing and its next real regression is invisible |
| 2 | Does an `ADR 0005` **Q1–Q4 qualification campaign** run for the Pi 5? (`LE-94`) | Every `G04`; the `0/99` ceiling | `0/99` is permanent and is reported as a backlog when it is a lock |
| 3 | Buy a LAN-controllable relay for the board's supply (§3) | The whole loop; Q3; every criterion 4 | Sessions keep ending built-and-unmeasured, which is exactly the last 48 hours |

**Decision 3 is the one that makes 1 and 2 cheap**, and it is the only one that
costs money rather than judgement.

## 6. What would make the next 48 hours different, stated as rules

Three, and each is a response to something that actually happened rather than a
general principle:

1. **A session may not build a second artifact whose only remaining step is a
   board run while a first one is still waiting.** `03B` and `04C` both did. The
   backlog of unmeasured built things is the stall, in one sentence.
2. **A handover's item 1 may not be an instruction to someone who cannot execute
   it.** If the next action needs the owner, it belongs in §5's table, not in a
   numbered list an agent will read and route around.
3. **Prefer the measurement to the mechanism when both fit the session.**
   `LE-91` is better *mechanism* work than filing gates, and it has been
   correctly deferred three times for that reason — but a project whose evidence
   count has not moved in two sessions should take the number.

## 7. What NOT to start, unchanged and still right

`FEAT-P1-05`'s RT reserve (Feature-sized, files nothing in a session).
`G09`/`LE-86` (needs per-feature section attribution that does not exist — a
decision, not a session). `06A` §4.3 (the owner's, and correctly sized). The 230
release gates in domains whose subsystem does not exist (there is nothing to
measure). And **do not add design surface** — the hardware-evidence sprint rule
from 2026-07-30 has not been lifted.

## 8. State at close, unchanged from `04C`

All gates green except `check-timing-regression`, red for §5 decision 1's
reason. 31 Features / 99 Stories / 82 Tests / 62 Reports, **94 loose ends (46
open, 38 of them unowned)**, **23 of 460** release gates carrying evidence,
**0/99** Stories assurance-verified, 5 platforms **0 qualified**. Nothing
committed; `03B`'s five files remain uncommitted alongside `04C`'s and this
document's, so **stage by path, never `-A`** (`CONCURRENT_SESSIONS` rule 1).

**The one sentence, if only one survives:** *nine of the ten stages of this
project's board evidence loop are built and proven, the tenth is a human hand on
a power cable, and until that is automated every session will end with something
built and nothing measured.*
