# 03A — FEAT-P1-07's Whole Remaining Ladder Is on the Card: One Boot for the Numbers, One for the Fault, One Capture for Two Features

Session handover, written 2026-08-04 after the delivery session that took `FEAT-P1-07` from
"three Stories Specified" to "nine Stories In progress, host halves Green, the measure image
staged on the card". Read this top to bottom, then power the board. The repo tree is
**clean**, three commits pushed (`c329f69`, `fdda9c6`, `c4b246a`) — watch their CI runs.
The SD card is **in the laptop, in the TOS64 role, carrying the measure image**
(`kernel8.img` sha256 `0c709197ed26…`). Linkwatch is **armed** (PID at staging time: 4140).
The board is off.

---

## 0. The one-paragraph state

Three Stories landed today, TDD Red-first, each with its spine ritual and its own commit:
**`STORY-P1-07-03`** (the flat identity MMU — caches on, the prerequisite of measurement;
framebuffer Normal-Non-Cacheable, beacon staging cache-cleaned, mailbox exchange
clean-invalidated, stack-guard + null-guard pages unmapped, the `mmu-fault` fixture
registered), **`STORY-P1-07-04`** (GIC-400 routes exactly the virtual-timer PPI, slot 5
becomes the vector table's one resumable entry, the tick judged by interval *ratio*,
`PMCCNTR_EL0` probed against the generic timer per the recorded `LE-15` decision,
`MDCR_EL2` zeroed at the drop), and **`STORY-P1-07-06`** (the measured phases extracted to
`kernel::measure_phases` and driven verbatim by both architectures; the AArch64 fixture in
`kernel::fixture_measure_arm64`; the batched `LE-24` twin `per_op_of_8` measured non-zero
and *gated* on this Windows host with the eighth baseline row added and none re-recorded;
`xtask parse-meas` feeds a capture to the byte-for-byte unchanged parser). Spine: 29
Features / 90 Stories / 74 Tests, hal-arm64 at 279 host tests, kernel 143, xtask 269, fmt +
host and aarch64 clippy clean, Tier 0 timing gate green across 14 statistics.

## 1. Why one boot answers three Stories

The measure image runs, in order, on one power-up: boot report → vectors → **MMU on with
the before/after cache probe** (`TOS64-MMU/1 sctlr=… off=… on=…`) → **tick + conformance +
PMU** (`TOS64-CONF/1`, `TOS64-PMU/1`, live `TOS64-TICK/1`) → **fixture_measure** (IRQs
masked for its duration, the envelope emitted and recorded) → verdict
(`TOS64-RESULT/1 fixture=measure ok=…`) → splash → park loop, which paints everything and
transmits one envelope line per beat as an EtherType `0x88B5` frame behind the beacon.

The canvas rows, top to bottom: title · `TOS64-LINK/1` · `TOS64-BEAT/1` (live) · refusal ·
`TOS64-MMU/1` · `TOS64-CONF/1` · `TOS64-PMU/1` · `TOS64-TICK/1` (live, ratio bounds
accumulating) · the whole `TOS64-MEAS/2` transcript at 1× scale.

## 2. Step 1 — the measure boot (the card is already staged)

1. Safely eject the card → into the Pi 5 → monitor on → power.
2. Expect within ~6 s (the cache-off probe adds under a second): blue fill → `TINYOS` →
   the familiar LINK/BEAT rows → the four new evidence rows → the transcript block.
   Expect linkwatch: one `Down -> UP` at 1000 Mbps, then silence.
3. **The elevated capture — one capture closes two Features' gaps.** Elevated PowerShell
   (Start → "PowerShell" → Run as administrator):

   ```powershell
   pktmon filter remove
   pktmon filter add -d 0x88B5
   pktmon start --capture --pkt-size 0 --file-name $env:TEMP\meas.etl
   # wait ~30 s: the beacon (FEAT-P1-09's exit evidence) and the cycling
   # envelope lines (FEAT-P1-07-06's) share the filter
   pktmon stop
   pktmon etl2txt $env:TEMP\meas.etl -o $env:TEMP\meas.txt -v 3
   Select-String "TOS64-" $env:TEMP\meas.txt | Select-Object -First 20
   ```

   (If `pktmon` still refuses elevated, Wireshark/tshark with `eth.type == 0x88b5` is an
   acceptable stock capture — the point is the bytes.)
4. Parse the capture through the unchanged parser, from the repo root:

   ```powershell
   cd os; cargo run -p xtask -- parse-meas $env:TEMP\meas.txt; cd ..
   ```

   Exit 0 with `verdict fixture=measure ok=true` is `TEST-P1-07-06-A` clause 1 answered.
   (The capture needs the `TOS64-RESULT/1` line to arrive at least once — it cycles with
   the transcript, so a ~30 s window holds several copies of everything.)
5. Transcribe the four boot-evidence rows (`MMU`, `CONF`, `PMU`, and one late `TICK`
   reading with its `rmin`/`rmax`) into the ground-truth file, append-only, under a fresh
   `===== BOARD VERDICT 5 =====` header — the dated `-04` file is fine per 01A.
6. Power off.

## 3. Step 2 — the mmu-fault boot (criterion 5 of `-03`)

```powershell
cd os; cargo run -p xtask -- pi5 --fixture=mmu-fault; cd ..
.\work\tools\cardswap\bin\Release\net10.0\tos64-cardswap.exe tos64
# eject → Pi → power. Expect the canvas to fill with the TOS64-FAULT/1 frame
# in alert orange: class=data abort from EL1, level 1 translation fault,
# far=0000002000000000. Transcribe under ===== BOARD VERDICT 6 =====.
```

No `TOS64-RESULT` line is expected on this boot — the fault preempts the verdict by
design. Power off; restage the measure image (`pi5 --fixture=measure` + `cardswap tos64`)
if the bench should be left carrying the numbers build.

## 4. What each capture advances

| Evidence | Advances |
|---|---|
| `TOS64-MMU/1` off/on pair (order-of-magnitude apart) | `-03` criteria 2, 3, 4 |
| The fault frame with `far=0x20_0000_0000` | `-03` criterion 5, and `-02`'s decoded-`ESR` clause on the proven channel |
| `TOS64-TICK/1` count + `rmin`/`rmax` near 1000 | `-04` criterion 1 |
| `TOS64-CONF/1 cntvct=pass … cntfrq=… cpus=…` | `-04` criteria 2 (`LE-27` closes) and 4, 5 |
| `TOS64-PMU/1 delta=… rate=…mhz source=…` | `-04` criteria 3, 4 (`LE-15` closes, decision or narrowed) |
| `parse-meas` exit 0 on the packet capture | `-06` criterion 1; the envelope's board half of `LE-24` (criterion 3) |
| The same capture's `TOS64-PRESENT/1` frames byte-checked | **`FEAT-P1-09`'s exit criterion** |

## 5. Then the Report — the one `LE-09` closes on

`REPORT-2026-08-04-01`: quote the parsed envelope and raw capture lines verbatim; state
board revision, firmware version, clock policy and thermal state (§4 of the measurement
protocol — `Q1` per `ADR 0005`); batch size per metric with its reason (names carry
`_per_op_of_N`); the host-side batched median (35 cycles/op release, this Windows host)
beside the board's for `LE-24`; and the full "what the numbers are *not*" list — single
core, no preemption, no address spaces, no `EL0`, no WCET enforcement, no verified boot,
**and no worst-case bound (`ADR 0005`; criterion 4 absorbs the sentence — decided and
recorded in `TEST-P1-07-06-A`'s amendment)**. Then the register: `LE-09`, `LE-15`, `LE-24`,
`LE-27` close with `closed_in` populated; statuses advance under the LE-44 join; the spine
ritual end-to-end.

## 6. Bench facts at close

- **Card: laptop → staged TOS64 measure image** `0c709197ed26…`; `pios-backup\` retained.
- Board off; cable connected; monitor available. Linkwatch armed and logging.
- Serial unchanged: five zero-byte captures, demoted, nothing today depends on it.
- CI: three pushes to watch (`c329f69`, `fdda9c6`, `c4b246a`) — the cross-target clippy
  blindness (`LE-64`) is why watching matters; local aarch64 clippy was run and clean.
- The tick is a 100 Hz virtual-timer interrupt now: the park loop's `wfe` waits wake 100×
  per second. Nothing in the loop depends on wait granularity; noted so a changed idle
  behaviour on the screen is not read as a defect.
- Deep context: [02A](02A-beaconing-one-capture-from-exit.md) (card loop, pktmon),
  [hand-2026-08-03/09A](../hand-2026-08-03/09A-window-poisoned-inbound-path-indicted.md)
  (ops runbook, transcription drills, spine ritual gotchas §8).

The method holds: ask the board a question it can answer with a number. Today the code
learned to answer with a thousand of them at once, on three channels, through a parser
that did not have to change. One boot decides whether they describe TinyOS.
