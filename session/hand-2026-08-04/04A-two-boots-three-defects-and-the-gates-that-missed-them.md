# 04A — Two Boots, Three Defects, and the Gates That Could Not See Them

Session handover, written 2026-08-04 after executing [`03A`](03A-fixture-measure-staged-one-boot-from-le-09.md).
Two measure boots ran. Every number `03A` went looking for arrived. So did three defects
none of the project's gates could have caught, two of them in the gates themselves.

---

## 0. The one-paragraph state

`BOARD VERDICT 5` and `6` are transcribed into
[`pios-ground-truth-2026-08-03.txt`](../../goals/reports/pios-ground-truth-2026-08-03.txt).
`STORY-P1-07-03` criteria 2/3/4 and `STORY-P1-07-04` criteria 2/3/4/5 are **Green on silicon**;
`STORY-P1-07-06`'s board half ran to completion, eight metrics, `dropped=0` on every one.
Three new loose ends were raised and three closed. `LE-69` (GIC priority width assumed) and
`LE-70` (dashboard gate blind to placement) close here; `LE-51` (citations never resolved)
closes here by being implemented. `LE-71` is open and one boot from closing — the card is
staged with `619f40b8c076…`, which carries its fix. Four commits pushed, CI green on each
watched to completion. Spine: 29 Features / 91 Stories / 75 Tests, 71 loose ends (40 open).

## 1. What the board said

Both boots, in one table. Full transcripts are in the ground truth under their verdict headers.

| Row | Verdict 5 (`0c709197ed26`) | Verdict 6 (`a0d1773c8f10`) |
|---|---|---|
| `TOS64-MMU/1` | `sctlr=30D01805 off=75213055 on=183180` | `off=75252841 on=183419` |
| `TOS64-CONF/1` | `cntvct=pass span=168 cntfrq=54000000 cpus=54` | `span=153`, rest identical |
| `TOS64-PMU/1` | `delta=24000227 rate=2400mhz source=pmccntr` | `delta=24000200`, rest identical |
| `TOS64-TICK/1` | **`refused=gicc-pmr readback=000000F0`** | **`count=1 tval=540000 rmin=none rmax=none`** |
| fixture | ok, 8 metrics, `span=5696` | ok, 8 metrics, `span=5810` |

The MMU pair is the session's cleanest result: **410× faster with caches on**, same loop, same
memory. That ratio is what licenses every other number to be about TinyOS rather than about the
bus, and it settles `-03` criteria 2, 3 and 4 in one line. `cpus=54` is `cycles_per_us`, not a
core count — the field name invites the misreading and this handover records it so the next
reader does not repeat it.

**The board repeats itself.** Across two boots: context switch 80/80, per-op-of-8 479 vs 480,
round trip 379 vs 383, dispatch round 1647 vs 1645. Six of eight metrics land within a few
cycles. `fault_brk_capture` moved 109 → 120 (~10%), the largest honest drift; watch it.

## 2. `LE-69` — the code refused a conforming device

`gic.rs` declared `PRIORITY_MASK_ALL = 0xF8` on the belief that GIC-400 implements 32 priority
levels, and refused the timer PPI unless `GICC_PMR` read back with all three low bits clear.
BCM2712 implements **16** levels and read back `0xF0`, which is architecturally correct — GICv2
leaves unimplemented low-order priority bits RAZ/WI and mandates a minimum of four. The tick was
refused for conformance.

The fix is **not** `0xF8 → 0xF0`. That substitutes one board's measured value for another guess
and is barred by the standing no-bench-tuned-constants rule. `GICC_PMR` is now probed with all
ones and its readback taken as the widest legal open mask, gated on *shape* — a contiguous run
of implemented bits ending at bit 7, at least four of them. The width is discovered per board,
every boot. Verdict 6 proved it on the first try.

Fourth instance of the family the Ethernet ladder cured three times (outbound window, endpoint
BARs, inbound windows): **device state that must be read, not compiled in.** First one inside
the GIC.

## 3. `LE-71` — one tick is zero intervals

Verdict 6 replaced the refusal with a better question: `count=1`, and never again.

`fixture_measure` masked interrupts and **nothing ever unmasked them**.
`hal_arm64::boot::unmask_interrupts()` existed with no callers anywhere in the tree — written
for this and never wired in. The comment above the mask stated the premise outright: *"masking
is one-way here … the tick line having accumulated its pre-fixture intervals is itself
evidence."* The premise had never been measured. The tick's entire lifetime was the window
between the `daifclr` after GIC bring-up and the fixture's mask — the conformance run plus the
PMU probe, order 10 ms, which at 100 Hz is one tick.

One tick is one timestamp. One timestamp is **zero** intervals. `ratio_bounds_per_mille` returns
`None` below two by deliberate design. So `STORY-P1-07-04` criterion 1 — *a tick verified by
ratio* — was **unreachable on any board, however many times it was booted**, and every host test
stayed green. Every layer was individually correct; the ordering was the defect, and no test
owned ordering.

`STORY-P1-07-10` moves ordering to where a host can hold it: `hal::interrupts` supplies
`with_interrupts_masked(gate, body)`, `hal_arm64::boot::PstateInterrupts` supplies two register
pokes and no policy. State is **saved and restored**, never unconditionally unmasked — a boot
whose tick was refused must be left masked, or the door opens with no timer behind it. Red first
against the defect's own behaviour: the recording gate logged `[Masked, BodyRan]` with no
`Restored`.

**Watch the outlier.** Verdict 6's `pool_u64x64_alloc_free_round_trip` read `p99_9=379 max=3519`
— a lone ~9× excursion absent from Verdict 5, where the tick never armed. One tick and one
outlier on the first boot where an interrupt could be delivered is most probably the tick landing
in the pre-mask window. `TEST-P1-07-10-A` clause 7 is written to **refute** this: if the outlier
survives once the tick runs continuously outside the fixture, the masking is not doing what it
claims.

## 4. Two gates that could not see their own subject

**`LE-70`.** The owner read the rendered dashboard and found the four headline tiles missing.
They were never deleted — commit `3849ece` (08-03) *relocated* the `overall-progress` block into
*Assurance release status*, leaving *Overall progress* holding an empty `stat-row` div. The gate
byte-compares block **content** and never asserts **placement**, so it passed on all sixteen
commits across four days. Proved directly: the spine was run immediately before and immediately
after the corrective move and passed both times. `check_block_placement` and
`check_no_empty_stat_row` now anchor each block to its heading; the regression test builds the
actual broken page and asserts the *content* checks still pass on it, which is the whole point.

**`LE-51`.** The spine never resolved `raised_in`/`closed_in` against `session/`. Implemented,
and it found two real defects on its first run against the committed register:

- `LE-09` cited `hand-2026-07-27/37` — a slot that does not exist in a folder that stops at `10`.
  Corrected to `/03`, which is literally `03-le-09-arm64-pi5-slice-proposal.md`. **The
  project's most-cited loose end had a dangling citation.**
- `LE-56` cited `hand-2026-07-30/03A`, which is genuinely **two** documents. The citation format
  now accepts a full document stem so the row can name one without renaming a committed handover.

Third and fourth instances of the prose-versus-register class, after `LE-30` and `LE-65`. Both
found by a human reading output, never by a gate. The pattern is worth stating plainly: **this
project's gates check the things they were written to check, and the defects live in what nobody
thought to check.**

## 5. What is owed

1. **Boot the staged card.** `619f40b8c076…` carries the `LE-71` fix. Expect `TOS64-TICK/1` with
   `count` climbing and `rmin`/`rmax` near 1000 — that is `STORY-P1-07-04` criterion 1 and
   `STORY-P1-07-10` criteria 4 and 5 in one frame.
2. **The elevated packet capture, still owed from `02A` and `03A`.** It is now the single largest
   blocker: it closes `-06` criterion 1, `FEAT-P1-09`'s exit, and lets `REPORT-2026-08-04-01`
   quote machine-parsed bytes instead of a transcription of a photograph. `pktmon` needs
   elevation; this host has no Npcap. **Worth considering instead:** emit the envelopes as UDP
   broadcast alongside the raw `0x88B5` frames — ~28 bytes of constant header, no IP stack
   needed — so an unprivileged listener reads them forever, on any machine on the cable. That
   removes a permission wall from every future diagnostic session.
3. **`REPORT-2026-08-04-01`**, which closes `LE-09`, `LE-15`, `LE-24`, `LE-27`. Do not write it
   from the photographs. `LE-09` is release-blocking debt; closing it on a human reading of a
   screen is the same class of mistake as the three above.
4. `LE-47` and `LE-68` are closable on reasoning already recorded — `-68`'s premise (PHY held in
   reset) was **refuted**; the cause was the PCIe windows.

## 6. Bench facts at close

- **Card: in the laptop, TOS64 role, staged `619f40b8c076…`**; `pios-backup\` retained.
- Board off. Host NIC UP at 1 Gbps throughout; linkwatch armed (pid 4140).
- `LINK=DOWN BEACON=SKIPPED` on the report row is the **expected** boot snapshot, as in verdicts
  2–6. The live `TOS64-BEAT/1 STATE=BEACONING` row is the truth. Do not re-diagnose it.
- **`cargo clippy --workspace --all-targets` cannot run on this Windows bench** — 13 bins are
  `cfg(not(windows))`-gated. Verified pre-existing against a stashed tree. CI is the only place
  that gate exists, which makes watching every push non-optional.
- `LE-24` resolved in principle: the batched twin's 480 is **per-op** and exceeds the unbatched
  383 for the same operation. Batching divides calibration residue by eight; the 97-cycle gap is
  residue the unbatched number was crediting to the pool. **480 is quotable, 383 is not** — and
  the same unquantified residue attaches to the six metrics with no batched twin.
