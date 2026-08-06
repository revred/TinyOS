# STORY-P1-09-08 — The Second Look: `rp1=absent` Is a State to Watch, Not a Verdict to Keep

Status: **Verified (functional) 2026-08-05 — all four acceptance criteria met. Criteria 1, 2 and 3 host half Green 2026-08-03 (re-probe eligibility total, refused probes touch nothing downstream, late settle runs the pipeline once with the release still exactly once). **Criterion 4 Green on silicon**, and it is written as a disjunction — "the chain clears or the confession sharpens" — with both arms informative. Both arms were in fact observed. `BOARD VERDICT 1` (2026-08-03 late) sharpened it: the confession stopped counting 3 and counted 16/57005, so `DL_ACTIVE` was reached and the refusal moved to the clocks rung — the re-probe did its job and the ladder advanced past the rung this Story was written for. `BOARD VERDICT 2` (2026-08-04 ~01:27) then took the other arm: **the plain pulse, no refusal anywhere in the chain**, with `RP1=PRESENT ID=0x0109`. The chain cleared. The criterion's warrant clause — "evidence that `DL_ACTIVE` needs our intervention, which is the next story's warrant" — was discharged in the direction the evidence pointed rather than the direction the Story guessed. **Assurance state remains `specified` and this Story is NOT release-assured**: 0 qualified platforms ([`hand-2026-08-05/06A`](../../session/hand-2026-08-05/06A-nothing-is-verified-and-the-reason-is-not-velocity.md) §2).**
Feature: [`FEAT-P1-09`](../features/FEAT-P1-09.md)
Introduced in: the 2026-08-03 confession boot — the lamp counted **3**: `DL_ACTIVE` clear at probe time with RC mode and the PCIe PHY both up, the chain's first on-silicon self-diagnosis

## Description

The confession said something precise: two of the three PCIe status gates
pass — the firmware kept the controller in root-complex mode and the PCIe
PHY trained — and only the data-link-active bit was clear when `discover`
looked, once, seconds after power. Everything downstream (window, identity,
PHY release, scan, beacon) was never reached; the wire stayed flat for want
of one bit that may simply have been late.

The fix is the owner's watch principle applied one rung up, the same shape
as `STORY-P1-09-06` and for the same reason: **no one bench's settle time
becomes the design.** While discovery reports anything short of a present
GEM, the park loop re-runs the probe once per second. The gate discipline is
unchanged — a refused probe still touches neither the window nor the GPIO —
so the PHY release still runs at most once, on the single pass where the
identity first validates. When a late probe finds the chain settled, the
rest of the pipeline runs exactly as it would have at boot: release, scan,
link read; the lamp adopts the new outcome (a fresh refusal re-counts, a
known PHY returns the plain pulse), the link watch arms, and the beacon
starts whenever the wire trains.

If the data link *never* settles, the lamp keeps confessing 3 forever and
the next story brings the link up ourselves against the working register
values in `pios-ground-truth-2026-08-03.txt` — with evidence that waiting
alone was not enough, rather than conjecture that it might not be.

## Depends on

- `STORY-P1-09-01` — the probe and its gate discipline, re-run unchanged.
- `STORY-P1-09-04` — the release whose exactly-once contract the gates
  preserve across retries.
- `STORY-P1-09-06`/`-07` — the park-loop watch shape and the lamp that
  reports each retry's outcome.

## Acceptance criteria

1. **Re-probe eligibility is total.** Every outcome short of a present GEM
   is due a second look (`LinkAbsent` in every rung, `IdentityRefused` in
   every shape); every present GEM — whatever its PHY or link state — is
   final and never re-probed.
2. **A refused probe touches nothing downstream.** Across any number of
   refused re-probes, the GEM window and the GPIO registers are never
   accessed — pinned with panicking doubles, the same discipline the boot
   pass already proves.
3. **A late settle runs the pipeline once, release included exactly once.**
   With a controller scripted to report `DL_ACTIVE` only from the Nth probe,
   the Nth pass validates identity, runs the release exactly once in total,
   scans, and reads the link; the lamp code, link watch and beacon
   eligibility all adopt the new outcome.
4. **Board: the chain clears or the confession sharpens.** The next boxed
   boot either reaches the plain pulse (and the NIC watch takes over the
   story) or keeps counting 3 through a minute of retries — evidence that
   `DL_ACTIVE` needs our intervention, which is the next story's warrant.

## Named debt this Story leaves open

- The one `TOS64-LINK/1` line still reports the *first* look only — the
  report's exactly-once contract is untouched, so a late-settled chain is
  visible on the lamp, the heartbeat and the beacon, not in the serial line.
- Bringing the data link up ourselves stays out of scope until criterion 4
  shows waiting alone fails.

## Progress, 2026-08-03

| Criterion | State |
|---|---|
| 1 — eligibility total | **Green.** Pinned per arm. |
| 2 — refused probes touch nothing | **Green.** Panicking window/GPIO doubles across repeated refusals. |
| 3 — late settle, release exactly once | **Green.** Scripted late `DL_ACTIVE`; release counter reads 1 across all passes; channels adopt the outcome. |
| 4 — board | **Blocked on the next power-on.** |

## Tests

[`TEST-P1-09-08-A`](../tests/TEST-P1-09-08-A.md) — written before
implementation, per the TDD mandate.
