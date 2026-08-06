# 07C — The Stall Broke on a Manual Power Cycle, and the Stamp Costs 110 Cycles Everywhere

Session handover, written 2026-08-06, after
[`06C`](06C-the-tenth-stage-is-built-and-the-only-thing-left-to-buy-is-the-relay.md)
and in the same tree. **Written for a next session that does NOT have the smart
plug.** Everything below is executable on a laptop, or on a bench with a human
hand, and nothing in the ordered path waits on the relay.

**The stall broke.** `05C` measured it — release-gate evidence frozen at 23 since
`02A`, three consecutive sessions ending built-and-unmeasured. This session took
the boot by hand and **the numbers are in the tree**: fourteen metrics off the
wire, 89 spoor records, **0 lost, 0 refused**, one boot, continuous.

**The one sentence, if only one survives:** *one spoor stamp costs 110–117
cycles at p99 in three independent pairs with three different denominators, so
`G23` is a constraint on instrumentation **density** and never on stamp cost —
and the same absolute cost reads as 123% or 6.4% purely by what it is divided
by.*

---

## 1. What was measured, and it is the first evidence movement in three sessions

Image `b6dbabaea3431afa94cf9210374826bde9e5fb4efef7c5c861b92795c5006f02`
(298,089 bytes) — byte-identical to what `03B` staged and `05C` said had
outlived two sessions. Served over `tos64-netboot`, 583 blocks, digest confirmed
in the transfer log before the board ran a single instruction of it.

| pair (p50) | plain | +1 stamp | delta | overhead |
|---|---|---|---|---|
| **D04** `context_switch_yield_roundtrip_2switches` | 82 | 183 | +101 | **+123%** |
| **D05** `dispatch_run_once_cooperative_round` | 1650 | 1756 | +106 | **+6.4%** |
| **D07** `pool_u64x64_alloc_free_round_trip_per_op_of_8` | 475 | 592 | +117 | +24.6% |

At **p99**, which is what `G23` actually constrains:

| pair (p99) | plain | +1 stamp | delta | against a 2% allowance |
|---|---|---|---|---|
| **D04** | 82 | 192 | **+110** | **+134%** — fail by ~67× |
| **D05** | 1650 | 1760 | **+110** | **+6.7%** — fail by ~3.3× |
| **D07** | 475 | 592 | **+117** | +24.6% (already filed) |

Direct measurement of the same stamp, same boot:
`D11/spoor_stamp_park_rung_per_op_of_8` p50 = **136**, p99 = **141**.

### The finding, which is the cross-validation and not any single row

**110, 110, 117.** Three pairs, denominators spanning 82 to 1650 cycles, and the
stamp costs the same everywhere to within 6%. `05C` predicted *"a 137-cycle
stamp on an 80-cycle round trip"* and the round trip is 82 with the stamp adding
110 — **the prediction was right, and it is now measured rather than argued.**

`PERF-D07-G23`'s filed note already concluded that `G23` constrains density
rather than stamp cost. **That was one domain and could have been a property of
pool traffic. It is now three, independently, and it is the method's result.**
The derived budgets differ by two orders of magnitude for the same physical act:

- **D04** — 2% of 82 cycles is 1.6, a stamp costs 110: **one stamp per ~67
  context switches.**
- **D05** — 2% of 1650 is 33: **one stamp per ~3.3 dispatch rounds.**
- **D07** — filed: one stamp per ~14 pool operations.

### The question this raises, stated as a question because it is not settled

`PERF-D07-G23` was filed with the argument that the shipping park loop contains
**no pool traffic at all**, so the 26% has no denominator in shipping code.
**That argument does not obviously transfer to `D05`.** The capture shows the
park loop stamping `DispatchRound` once per beat and running a dispatch round
per beat — one stamp per round, against a budget of one per 3.3.

That would put the shipping path **over** the `D05` budget by ~3×. It may well
not: the park loop's round is nearly idle and the fixture's is 1650 cycles, so
the denominators may not be comparable at all. **Nobody has checked, and the
next session should before either filing `D05` as a shipping concern or waving
it away.** It costs a read of the park loop, not a board.

## 2. The two gates are NOT filed, and that is the next session's item 1

**`PERF-D04-G23` and `PERF-D05-G23` are still empty.** The numbers exist, the
capture is at `c:\tmp\env-2026-08-06.txt`, `xtask parse-meas` reads it cleanly,
and filing needs **no board and no owner decision** — which is exactly what
distinguishes this item from the board item that outlived three handovers.

`03B` §6 item 1 was "twenty minutes, everything is staged" and survived two
sessions. **This is that shape again and it must not survive one.** The failure
mode has simply moved: *built-and-unmeasured* became *measured-and-unfiled*, and
unfiled evidence counts for exactly as much as no evidence.

Everything needed is in §1. Two cautions when writing the rows:

- **Read the `target` column first and check the domain label is the subject's**
  (`LE-91`, standing). `D04` is context switch; `D05` is dispatch. The metric
  names carry their own domains here and they agree — verify rather than assume.
- **File `D04` as the fail it is**, with the residue caveat on the row. A 134%
  p99 overhead on an 82-cycle round trip is not a defect in the stamp; it is the
  smallest denominator in the tree. Say so on the row, the way `D07`'s row says
  it, or the number will be quoted as an instrumentation cost by someone who
  does not have this document.

## 3. `LE-97` — the boot that was nearly poisoned, caught by thirty seconds

`tos64-netboot` started with **`server address: 0.0.0.0`.**

`GuessLinkLocalAddress()` takes the first `169.254.x.x` on an interface that is
**up**, and the bench Ethernet is `Disconnected` until the board powers — link
needs a powered far end. So it found nothing and `IPAddress.Any` went into the
DHCP `OFFER`. The board would have been handed `siaddr=0.0.0.0`, failed to
fetch, and **looked like a board fault.** Seventh instrument failure in a device
failure's costume, and the only reason it was caught is that the tool prints the
address it chose — it prints `0.0.0.0` exactly as confidently as a real one.

Restarted with `--server 169.254.113.248` and the `OFFER`/`ACK` both carried it.

**It is worse once the link is up.** This laptop has **four** link-local
addresses — Ethernet, Bluetooth PAN and two virtual adapters — so first-one-wins
is a coin toss even in the working case, and the wrong pick fails the same way
with a plausible address in the log.

The fix is not better guessing. **The NIC has no link until the board is
powered, so the address cannot be discovered at start time on the very run that
needs it.** `tos64-netboot` must refuse to start rather than fall back to `Any`
— a server that cannot name its own address cannot serve — and `board_run`'s
`StartNetboot` must pass `--server` through, which it currently does not.
`06C` predicted this passthrough as a robustness gap; it is now an observed
defect.

## 4. `LE-98` — the canvas was dark on a run that was otherwise perfect

The board fetched the right image, ran the fixture, reached the park loop and
transmitted 89 records — **and the monitor showed nothing.**

`hal-arm64::boot` is explicit that the park loop *paints and transmits*, and
transmit is proven by the capture, so **TinyOS was writing pixels the whole
time and nothing was scanning them out.** `SIMPLEFB_BASE = 0x3F80_0000` is
hardcoded from a Raspberry Pi OS capture (`board.rs`, `STORY-P1-07-09`,
`BND-03` — there is still no device-tree parser), and nothing verifies the
firmware allocated a framebuffer there *this* boot. `config.txt` carries no
`hdmi_force_hotplug`, so a monitor absent or asleep at power-on, or on HDMI1
rather than HDMI0, leaves the firmware with no display.

**The part that is not cosmetic:** that is a 4 MB write to a constant physical
address on a machine with no IOMMU, on a project whose ordering puts safety
first. A surface that writes and reports nothing is `LE-87`'s shape — half a
success with no symptom.

And it **contradicts nothing on record**, which is the uncomfortable part:
`STORY-P1-07-09` records criterion 4 re-observed on 2026-08-05's *netbooted* run
(`BOARD VERDICT 14`). The channel worked over netboot before and did not this
time, **and the difference has not been identified.**

**Consequence for the plan:** `STORY-P1-09-16` criterion 4 reads the RX counters
off the **canvas**, because `04C` deliberately kept them off the wire for
beacon-cadence reasons. **That criterion is blocked on the display, not on the
board** — and the cadence argument should be re-read now that a criterion
depends on it.

## 5. Two smaller things the boot proved for free

- **A second DHCP client is on that wire.** `e0:70:ea:c9:1e:fa`, refused
  throughout the run. `tos64-netboot`'s `--mac` restriction — *"answering every
  DHCP client on a wire is how a bench tool takes down a household"* — is not
  theoretical on this bench. It has been earning its place all along and nobody
  knew.
- **The transfer marker cleared correctly.** `.tos64-transfer` was gone seconds
  after the 583-block transfer completed, so `TransferBeacon.Clear` ran in its
  `finally` against a live server. That is half of `LE-96`'s marker claim
  discharged. The other half — a *separate process* seeing it *during* a
  transfer — was missed by seconds and is catchable on any future boot.
- **The firmware's first `kernel8.img` request is abandoned at block 1/583**,
  then it fetches the DTB, `cmdline.txt` and `armstub8-2712.bin` (all absent),
  then re-requests and completes 583 blocks. **That is probing, not a fault** —
  recorded here because the log reads alarmingly and the next reader should not
  spend a power cycle on it.

## 6. `LE-76`, reconfirmed rather than rediscovered

`parse-meas` read all fourteen metrics and then said:

```
no usable verdict line: the fixture never reported whether it passed
```

That is `LE-76` exactly — the fixture emits `TOS64-RESULT/1` once at completion
and a listener attaching later can never see it. **Unchanged, expected, and it
does not touch the fourteen metrics.** Noted so the next session does not
re-diagnose it.

## 7. The next session, in order, with no plug

1. **File `PERF-D04-G23` and `PERF-D05-G23`** (§2). No board, no decision, and
   it is the only item that moves the evidence count. **Do this before anything
   else in this list.**
2. **Check the `D05` density question** (§1). A read of the park loop against
   the fixture's round. It decides whether `D05`'s 6.7% is a shipping concern or
   a fixture artifact, and `D07`'s filed note cannot answer it.
3. **`LE-97`** — make `tos64-netboot` refuse rather than fall back, and pass
   `--server` through `board_run::execute`. Laptop work, and it protects every
   future boot including the automated ones.
4. **`LE-98`**, cheapest first: confirm HDMI0 and hotplug timing, add
   `hdmi_force_hotplug=1` to `config.txt` (no rebuild — `kernel8.img` and its
   digest are untouched), then one power cycle by hand gets the canvas back and
   with it `STORY-P1-09-16` criterion 4's five `ti64dink --send` arms.
5. **`LE-91`**, unchanged and still the right mechanism before the 127.

**Everything above runs without the relay.** When it arrives, `06C` §7 item 2 is
the checklist — and `LE-97` should be fixed first, because `board-run` inherits
the guess that nearly cost this session its boot.

**Do not start:** `FEAT-P1-05`'s RT reserve, `G09`/`LE-86`, `06A` §4.3. And do
not add design surface — the hardware-evidence sprint rule from 2026-07-30 has
not been lifted.

## 8. State at close

- **Gates:** all green except `check-timing-regression`, RED and unchanged for
  the `LE-23` owner decision. Workspace suite green, `power.tests` 99/99,
  `netboot.tests` 23/23, all eight C# tools build.
- **Spine:** 31 Features / 99 Stories / 82 Tests / 62 Reports, **98 loose ends
  (50 open)**, **23 of 460** release gates carrying evidence — **and that 23 is
  now stale by two.** The measurements exist; §2 is what makes them count.
- **Bench:** board **powered and beaconing** at close, epoch `0x047823BB`,
  thermal 52→57 °C. `tos64-netboot` was running on UDP 67/69 — **stop it before
  the next run** or `LE-87` collects its second instance. Capture at
  `c:\tmp\env-2026-08-06.txt`. **No plug on the desk.**
- **Uncommitted.** Nothing committed, `git add -A` never used
  (`CONCURRENT_SESSIONS` rule 1). `03B` through `07C` are all uncommitted in this
  tree, so **stage by path**.

**The standing instructions, all holding.** Do not report `x/460` undecomposed.
`PERF-Dnn-Gnn` is only meaningful if `Dnn` is the domain of what you measured.
Verify the digest and size the window before you spend the boot. A gate written
for one architecture, one tool or one direction does not generalise itself.
Build the unblocker rather than the next blocked artifact, and say so.

**And a sixth, from this session:** *a tool that prints the value it chose is
not the same as a tool that refuses a value it cannot justify* — `tos64-netboot`
printed `0.0.0.0` in the same confident column it prints a real address, and
only a human reading the line stood between that and a boot diagnosed as a
board fault.
