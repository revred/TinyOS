# 02A — The Report That Closes the Tier Debt

Session record, written 2026-08-05, executing
[`01A`](01A-the-need-for-speed-and-what-must-not-be-traded-for-it.md) §5's second short item.
This document exists chiefly so the loose-ends register has a real citation for four
closures; the substance lives in the Report.

## What was delivered

**[`REPORT-2026-08-04-01`](../../goals/reports/REPORT-2026-08-04-01.md)** — the first hardware
Report in this project's history, covering `TEST-P1-07-03-A`, `TEST-P1-07-04-A` and
`TEST-P1-07-06-A` from the board verdicts of 2026-08-04/05 recorded in
[`pios-ground-truth-2026-08-03.txt`](../../goals/reports/pios-ground-truth-2026-08-03.txt).
Written as `hand-2026-08-04/05A` §6 ordered: machine-parsed bytes (Ti64Dink wire records)
quoted as such, canvas transcriptions quoted with their provenance stated, and the two
channels tied together by the one number that appears on both (`ON=183971` canvas vs
`MmuEnabled cost=183974` wire, three cycles apart).

## Closed on it

- **`LE-09`** — the tier exists; per the row's ADR-0005 amendment this is the tier and not a
  bound. Closed on `STORY-P1-07-06`'s Report and nothing earlier, as `FEAT-P1-07` required.
- **`LE-15`** — decided on the board: `PMCCNTR_EL0` at 2400 MHz over the 54 MHz counter.
- **`LE-24`** — batched shape non-zero on both hosts of record (35 cycles/op Windows, 480
  cycles/op board); the residue thesis observed on silicon.
- **`LE-27`** — the AArch64 `CycleSource`/`Timebase` executed on the registers themselves.

Register moves made alongside: `FEAT-P1-07` stopped reading `Specified` (it was `LE-73`'s
named class instance); `STORY-P1-07-06` records criteria 2–6 discharged by the Report with
criterion 1 held open — **no board-emitted `TOS64-MEAS/2` envelope has yet been machine-parsed
off the wire**, and the Story stays `In progress` on exactly that.

## What this session did not do

The rest of `01A` §5 stands untouched: boot `f06bfa8ac7ec` and read the thermal rung (needs
hands on the bench — card move and power), then the real milestone, one task dispatched once
on silicon. `LE-73`'s gate, `FEAT-P1-09`'s byte-compare exit, and `STORY-P1-10-02`
criterion 6's overhead measurement remain owed as `01A` §6 lists them.
