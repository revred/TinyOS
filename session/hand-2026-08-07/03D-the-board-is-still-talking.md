# 03D — Verifying the board-session investigation: what it got right, and the three things it did not

Independent verification of the read-only Pi 5 board-session investigation.
**No power was touched.** Every command below was executed; where a claim could
not be executed it says so.

`LE-110` (filed by the concurrent session in `da77cef`) already records the
headline — *the board has been transmitting a complete Tier 1 envelope for
fourteen hours and nothing was listening* — and the scoping correction behind
it. **This document does not restate that.** It records what verification
added on top.

---

## 1. Confirmed by execution, including the part flagged unverified

The investigation flagged exactly one thing as inferred: *"that a live
`ti64dink` capture actually receives frames right now"*. It does, and the
sequence it printed works verbatim:

```text
--list                      -> Realtek Gaming GbE Family Controller  (the direct link)
--any --live 30             -> TOS64-PRESENT/1 seq=52944..52955, SPOORJ01…, TOS64-MEAS/2 …
--live 90 --text <file>     -> 16 lines
xtask parse-meas <file>     -> 14 metrics, exit 0
```

**No elevation**, in a non-admin shell — `Live.cs`'s comment is now confirmed
by execution rather than by reading. Npcap `Running`; link `Up` at `1 Gbps`.

Also verified against source and matching the investigation exactly:
`board-run`'s `--plug=` check runs *before* `--dry-run` is consulted (so
`--dry-run` genuinely still refuses without it); `plan`'s ordering
`VerifyDigest → StartNetboot → PowerCycle → Watch → ParseMeas? →
EnsurePowerOn`, with `execute` refusing every step after a failure *except*
`EnsurePowerOn`; `parse-meas` taking a **positional** path; every `ti64dink`,
`tos64-netboot` and `tos64-power` flag as printed; `LE-95` owned/open and
`LE-96` unowned/open.

**UDP 67/69 clear on both instruments.** `Get-NetUDPEndpoint` *and* `netstat
-ano`, deliberately both, because `LE-87` is the row where they disagreed and
the stale server won.

## 2. Additive: the envelope is a replay, and that makes a harvest a reboot oracle

`LE-110` records that this is *"the SAME boot 04C and 07C already recorded"*.
That is right, and it is provable mechanically rather than by reasoning:

```text
diff  goals/reports/wire-meas-envelope-2026-08-06-spoor-pairs.txt
      goals/reports/assets/2026-08-07-board/tos64-meas-2026-08-07.txt
  -> IDENTICAL, all 16 lines
```

The mechanism is by design: `fixture_measure_arm64` records every emitted byte
into `hal_arm64::transcript`, and the park loop **paints and transmits that
transcript line by line, forever**. What is on the wire is a recording.

**Why this is worth its own paragraph.** `LE-110` makes harvesting a routine,
zero-cost action. Anyone doing it needs one sentence to know what they hold:

- **Identical to the committed capture** → the same boot is still running,
  nothing has been measured that was not measured on 2026-08-06. This is a
  *liveness and no-reboot* proof and nothing more.
- **Different** → the board rebooted without anyone ordering it. That is worth
  stopping for, and it is otherwise invisible on this bench.

A one-line `diff` turns the harvest from "evidence-shaped output" into a
decisive answer to the only question it can actually settle.

## 3. Additive: three evidence rows quote a boot that is no longer the one on the wire

`PERF-D07-G23`, `PERF-D11-G01` and `PERF-D11-G03` all quote
`spoor_stamp_park_rung_per_op_of_8` at **p50 = 137** (and `G03` the whole
distribution: `min 131, p50 137, p99 143, p99_9 144`). Their source is
`wire-meas-envelope-2026-08-06.txt`, the **`metrics=12`** capture — the boot on
which this metric still carried the `D07` label.

The board today, and the `metrics=14` `spoor-pairs` capture, say **p50 = 136**
(`min 130, p99 141, p99_9 142, max 143`).

**Nothing here is wrong.** Both captures are committed and dated, and each row
names its source — `PERF-D11-G01` even states `metrics=12` in its own note.
**No verdict moves**: 136/2400 = 0.0567 µs against a 0.03 µs target is 1.89×
where the row says 1.90×.

What changed is the likelihood of confusion. Before `LE-110`, nobody
re-harvested; now it is the cheapest action on the bench, and the number it
returns is the one the register does not quote. Filed as **`LE-111`**, scoped
as provenance rather than as a defect.

## 4. Additive: the investigation's link evidence was invalid, though its conclusion held

> *"The laptop's Ethernet adapter currently shows 169.254.113.248 … confirming
> the direct cable to the board is physically linked right now."*

**An APIPA address is not evidence of link, and this tree has already paid for
that lesson.** `09C` §1: *"a disconnected adapter keeps its APIPA address and
.NET reports it."* The counter-example is on this bench at this moment —
`Bluetooth Network Connection` holds `169.254.92.35` with no link at all, which
is the same adapter `09C` used to make the point.

The instrument that answers is `Get-NetAdapter` → `status=Up  speed=1 Gbps`.

Right answer, invalid instrument: `LE-80`'s family again, and the standing rule
holds unchanged — an instrument that returns the same answer whether or not the
thing is true has told you nothing. Recorded here rather than filed, because
the investigation's *conclusion* was correct and no artifact carries the bad
inference.

## 5. Additive, and small: `tos64-netboot`'s usage comment is stale

The investigation lists `--offer <ip>`. The parser accepts it
(`work/tools/netboot/Program.cs:74`); the file's own `Usage:` comment omits it.
The investigation is right and the tool's documentation is wrong. One line, for
whoever next touches that file.

## 6. Unchanged, and still the blocker

`LE-95` is a ~£15 purchase, not a task, and it is the owner's. `LE-110`'s
correction narrows what it blocks — **booting a new image**, not reading the
current one — but nothing in §1 above moves a guardrail, because the stream
carries no `TOS64-RESULT/1` line and its metrics were already captured on
2026-08-06.

`LE-96` remains the caveat on the other side: `board_run::execute` has never
run against a relay, and its first discharge should be `--dry-run`, then a
cycle with no image staged, then a full run.

## 7. For the next session

1. **`diff` every harvest against
   `goals/reports/wire-meas-envelope-2026-08-06-spoor-pairs.txt`** before
   reading anything into it (§2). Identical means no reboot and no new
   evidence; different means the board rebooted unasked.
2. **`LE-111`** — three rows quote the superseded `metrics=12` boot (§3). No
   verdict changes; the fix is a citation, and it is cheap while someone is
   already in that register.
3. **`tos64-netboot`'s usage comment**, one line (§5).
4. `LE-95` is a purchase and is the owner's.

**Nothing in the repository was modified by this verification** except this
document and `LE-111`. The working capture was written to `C:/tmp/board/`,
outside the tree; the committed capture is the concurrent session's at
`goals/reports/assets/2026-08-07-board/`.
