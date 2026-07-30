# Handover 11A — Owner-Ordered: The 10 µs De-Risking Contract, Software Half Delivered

The owner ordered
[`work/Derisk10usLatencyRequirement.md`](../../work/Derisk10usLatencyRequirement.md)
delivered. Of its §13 immediate artifacts, four are software-deliverable and are
delivered; three require hardware in hand and one requires the Hexapod solver — those
four are **registered debt (`LE-63`)**, not pretended progress. The requirement
itself (`LAT-PHYS-10`: strict `< 10,000 ns`, `t_apply_last − t_sample_first`) is
preserved and unweakened; nothing here claims any timing capability.

## Delivered

1. **[`ADR 0011`](../../docs/adr/0011-lat-phys-10-governs-and-the-two-event-100mbit-path-is-rejected.md)**
   (§13 items 1–2, WP0's software half): the timing vocabulary
   (`LAT-PHYS-10`/`LAT-OS-10`/`LAT-CALC-10`/`SYNC-1`/`PERIOD-N`) adopted as
   non-substitutable; the §3 event/endpoint contract frozen with the one-page
   diagram (`t_sample_first` … `t_apply_last`); **Option A (two-event 100 Mbit/s
   EtherCAT) rejected for strict `LAT-PHYS-10` on serialization physics alone**
   (84 bytes = 672 bits per minimum line interval → 6.72 µs; two events → 13.44 µs,
   already over the deadline before any software cost); **Option B (FPGA/edge closed
   loop) adopted as leading test architecture** with the §5 gate held open —
   closable only by a decision record naming board/NIC-ESC/drives/topology plus
   vendor latch evidence plus a first lower-bound capture. EtherCAT keeps the
   machine-fabric role `ADR 0010` gave it; `MFS-08`–`MFS-10` claims are bounded
   accordingly.
2. **[`STORY-P1-08-02`](../../goals/stories/STORY-P1-08-02.md) — Verified (Host)**
   (§13 item 6, WP1's core, risk `R4`): feedback identity in the `motion` crate is
   now a closed ownership sum — `Axis { axis, role }` /
   `EndEffector { effector, role }` / `Group { role }` — with `EffectorId` a bounded
   first-class identity, the `Auxiliary` dumping ground removed, profile bindings
   owner-typed, and whole-epoch validation enforcing total owner equality. The
   forbidden cast (probe presented as axis feedback) is a **driven positive
   control** rejecting the whole epoch, as are wrong-effector, wrong-group-role and
   group-cast arms. The Hexapod worked-case shape — 3 axes × motor+load, probe
   deflection, group metrology — validates as one atomic epoch, and a missing or
   stale probe bit rejects it whole. 61 tests from a 43-error compile-stage Red
   (the 51-test `-01` suite migrated without weakening);
   [`TEST-P1-08-02-A`](../../goals/tests/TEST-P1-08-02-A.md) written first;
   [`REPORT-2026-07-30-05`](../../goals/reports/REPORT-2026-07-30-05.md) filed.
3. **The §10 measurement-protocol template** (§13 item 8):
   [`goals/reports/_lat-phys-10-report-template.md`](../../goals/reports/_lat-phys-10-report-template.md)
   — underscore-prefixed (a template, not a Report), carrying the full provenance,
   endpoint-semantics and nine-item workload lists, with the two mandatory positive
   controls (injected known delay > 10 µs; clock-skew corruption) without which a
   run is inadmissible, and the §12 claim ladder pinned at the top.
4. **`LE-63`** registers §13 items 3–5 and 7 (BOM/topology, vendor
   latch/interpolation evidence, initial hardware lower-bound capture, bounded
   Hexapod solver + WCET harness) plus WP2–WP5, with the §8 kill rules quoted in
   the row.

## Gates, all green this session

`cargo test -p motion` (61) · `cargo fmt -p motion --check` ·
`cargo clippy -p motion --all-targets -- -D warnings` · `check-spine-files` ·
`check-assurance-spine` (28 Features / 70 Stories / 54 Tests / 59 Reports, 63 loose
ends 37 open, 52 dashboard badges agree). No other crate's implementation changed.

## What is deliberately not here

No timing figure of any kind, measured or estimated, anywhere. No architecture-gate
closure — Option B *leads*, it is not *selected*; selection requires the `LE-63`
artifacts. No calibrated quantities or frame transforms (WP1's second half — needs
`R8`'s probe-calibration model, named debt on the Story). No kinematics, no solver,
no transport, no board work: the 08A Pi 5 sprint remains the next headline, and this
delivery — like 10A — was an explicit owner reorder that created no new board
surface.

## For the next session

Unchanged from 10A: the Pi 5 silicon demo (start at
[`09A`](09A-story-p1-07-05-host-run-path.md)'s board checklist). When motion resumes:
`MFS-02` as `STORY-P1-08-03` (periodic phase-aligned release), and — once hardware
for the metrology rig is in hand — the `LE-63` artifacts in order (BOM → vendor
evidence → lower-bound capture → gate-closing ADR).

## Concurrency note

Hooks remain installed; staged narrowly; the soak logger owns
`goals/reports/_soak-p0-03-01.log` and it was left unstaged. No other session's
edits were observed mid-turn.
