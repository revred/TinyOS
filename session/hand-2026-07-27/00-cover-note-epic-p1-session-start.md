# Cover Note — 27 July 2026: EPIC-P1 Session Start

New session folder per the naming convention (all handovers from one calendar date in one folder). The 26 July folder ([`../hand-2026-07-26/`](../hand-2026-07-26/index.html)) is closed at Handover 37 and stays untouched — its documents are referenced from Story/Feature/matrix files and are not renamed or moved. Handovers 35–37 were *written* on the 27th but filed there mid-transition; from this note onward, everything lands here. Reports already moved to the new date sequence (`REPORT-2026-07-27-01` is filed).

## Where the project stands this morning

- **`EPIC-P0` — functionally complete, and the dashboard now says so** (the stale `IN PROGRESS` badge was corrected today). All 25 Stories Verified, 24/24 Test docs passing locally, 30 Reports filed. Three things stand between "functionally complete" and *done*, per the Epic's own exit criteria — none of them functional scope:
  1. **A CI run observed green** (`LE-07`) — every claim is local-only; 30 handovers old.
  2. **Assurance evidence** — every Story is `baseline-debt`; conversion is `EPIC-P1`'s charge.
  3. **Hardware-tier validation** — nothing has run on a physical board; the Pi 5 direction (`LE-09`) is the path.
- **`EPIC-P1` — decomposed and mandated**: 6 Features, 10 `specified` Stories ([`EPIC-P1.md`](../../goals/epics/EPIC-P1.md)), governed by the four directives and the loose-ends register `LE-01`–`LE-10` in [`Handover 37`](../hand-2026-07-26/37-epic-p1-mandate-hardware-direction-and-loose-ends.md) — which every EPIC-P1 handover in this folder must carry forward, updating per-item status.

## This session's mandate (from Handover 37, unchanged)

1. **`LE-07` cheap CI probe first** — **done, same day**: the first push of this work (commit `cbaee41`) triggered CI; both QEMU jobs passed, lint failed on one `clippy::needless_lifetimes` in a test helper (local runs were `--lib`-scoped, CI runs `--all-targets`), fixed in `f1d7c90` whose run `30226663769` is fully green. LE-07 is closed in the register.
2. **`STORY-P1-01-01`** — the measurement harness, strict TDD (failing tests + `TEST-P1-01-01-A` before implementation), arch-neutral API (cycle source behind a trait — the Pi 5 slice reuses it), subsuming `pool-bench` (`LE-06`). `REPORT-2026-07-27-01`'s scored-against-catalogue discipline is the template its Reports follow.
3. **`LE-09` proposal** — one-page minimal ARM64/Pi 5 slice scoping with two sequencing options; the user decides.
4. Then `STORY-P1-01-02` (baselines + regression gate, demonstrated to fail), then `FEAT-P1-02`.
