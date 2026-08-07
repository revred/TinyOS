# 13A — The boots are cheap now: 12A §0 executed, and Sitting B waits only on hands

The session that executed [`12A`](12A-sitting-b-the-script-one-afternoon-two-locks.md)
§0 — the laptop work that makes the bench boots cheap. Solo session; the tree
was pulled clean at `6599cb2` and no concurrent commits arrived mid-session.

**The one sentence, if only one survives:** *Both Q3 fixture images exist,
build and lint for the board — the SMC positive control whose verdict is the
ADR 0005 trap clause itself, and the 60-second campaign whose every failure
mode is a named refusal instead of a distribution of zeros — and
`TOS64-DISPLAY/1` now states the canvas's write target and its justification,
so every boot in the Sitting B manifest has its image, its expected wire line,
and its filing path ready.*

## 1. What exists now, by boot of the `12A` manifest

- **Boot 1 needs nothing from this session** — it burns the current image, and
  `11A` already delivered the timestamped ti64dink its cadence capture needs.
- **Boot 2 — `xtask pi5 --fixture=qual-control`.** Sixteen idle residency
  windows self-calibrate the PMU rate (the median per-window ratio — never a
  datasheet number, and robust exactly because an excursion is the outlier a
  median discards), then one window carries a benign `PSCI_VERSION` SMC at its
  midpoint — the one documented synchronous EL3 entry this platform has
  (`REPORT-2026-08-07-01` Q2). Expected on the wire:
  `TOS64-QUAL/1 smc_control psci_version=… idle_windows=16
  idle_unaccounted_max=… control_unaccounted=… event_fired=true seen=true`,
  with the verdict folded into `TOS64-RESULT/1 fixture=qual-control`. The
  verdict *is* the trap clause: `seen` requires the control window's
  unaccounted ticks strictly above **every** idle window's — no margin
  constant, because the margin is what the boot measures — *and* the event
  actually fired, both halves printed, so a control that saw nothing stops
  the campaign as `ok=false` rather than passing on politeness.
- **Boot 3 — `xtask pi5 --fixture=qual-campaign`.** 6,000 windows × 10 ms —
  60 s of accumulated window time, satisfying both of `08A` §5's proposals
  (≥ 1,000 windows, ≥ 60 s) at the proven 540,000-tick size. One line:
  `TOS64-QUAL/1 campaign windows=6000 … unaccounted_min/p50/p99/p99_9/max …
  offset_disagreement_max=…` — the moved-`CNTVOFF_EL2` channel carried beside
  the paused-PMU channel because they are different hiding places. Every way
  the instrument can be broken is a **named refusal** (`no_windows`,
  `window_never_closed`, `pmu_dead`) — never a distribution of zeros, which
  would be the cleanest possible pass while measuring nothing.
- **Boots 4/5 remain S4-gated** and this session did not touch them, per the
  script's own instruction: the sentence is the owner's.

**"Unaccounted", pinned in code rather than prose** (`kernel::qual_campaign`):
`PMCCNTR_EL0` counts at EL1 and not in the secure world, so EL3 residency
advances the physical counter while the cycle counter stands still —
`unaccounted_i = cntpct_ticks_i − pmccntr_delta_i × 1000 / rate`, rate
self-calibrated per run. Saturating, so flattering rate jitter reads zero,
never a wrapped enormity.

## 2. `STORY-P1-13-01` criterion 4a — the display line speaks its write target

`TOS64-DISPLAY/1` gains `fb_addr=… src=constant|refused`, decided by the
canvas's own permission (`Canvas::is_dark`), exact bytes pinned by test:
`src=constant` with the pinned address when the canvas will paint,
`fb_addr=none src=refused` when the gate never opened. `src=dtb` is
deliberately unproducible until the shape-2 consumption increment — the
vocabulary is exactly as wide as what the boot can honestly claim. The field
rides whichever fixture image boots next, so **boot 1 of the manifest already
collects it**, and the off-board DTB walk (`fdt-walk`, `11A` §3) now has the
boot's own claim on the wire to corroborate or indict.

## 3. Discipline notes for the reviewer

- Everything host-testable was red first: 13 `qual_campaign` tests and 4
  event-probe tests failed under `todo!` bodies before implementation; the
  fires-once-mid-window property was proven over scripted counter doubles
  **before the `smc` instruction existed anywhere near it**. 30 new host
  tests total; hal-arm64 at 340, kernel at 224, all green.
- The one un-doubled piece is `hal_arm64::smc::psci_version` itself — an SMC
  cannot be meaningfully mocked and a mock EL3 would test the mock; it is one
  function, reviewed, with its SMCCC clobber set declared in full, exercised
  only by `fixture-qual-control`.
- `check-boot-images` now builds **5** AArch64 variants (the two new ones
  registered in `PI5_FIXTURES`, so the coverage tests hold them);
  `check-guest-images` 22; the verdict line names which image a boot actually
  was (`fixture=qual-control|qual-campaign|measure`), because a campaign
  capture whose `RESULT` said `measure` would leave the qualification record
  citing an ambiguous line.
- Crate headroom after: kernel 7,438; hal-arm64 11,696. The campaign's static
  buffers (6,000 × 24 B samples + scratch) live in the fixture image's `.bss`
  only.

## 4. What Sitting B now costs, exactly

Five power cycles and two passive captures, every command already written in
`12A` §2 — plus the pre-flight in runbook §0b (one server and it is yours;
the `LE-117` EEPROM tripwire if the Pi OS card boots). The filing path (§3 of
the script) is unchanged: the campaign Report cites `REPORT-2026-08-07-01`
for Q1/Q2/Q4, `qualified-platforms.tsv` gains row one, and the `G04` unlock
begins from the campaign boot's own envelope under ADR 0015's `irq_state`
honesty. Nothing in this session pre-empted the two decisions that are not
its to take: the S4 sentence, and the bound the record will claim — the
fixture reports the distribution and refuses to editorialize it.

## 5. Standing instructions

All previous hold. The one this session leaned on rather than earned: *an
untestable cap is a comment* — the campaign's token-flood sibling from `11A`
applied again, this time as the trap clause: an instrument's zero is worth
exactly as much as its demonstrated ability to say otherwise, which is why
`seen=true` is a fixture verdict and not a Report sentence.
