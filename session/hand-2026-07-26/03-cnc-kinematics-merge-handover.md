# Handover 03 — CNC Kinematics Merged into `motion`

Session date: 26 July 2026
Follows: [02-mvp-delivery-strategy-handover.md](02-mvp-delivery-strategy-handover.md)

## What changed

The crate map from Handover 02 originally gave the committed trunnion-table RTCP/TCPC kinematics module its own crate, `cnc-kinematics`, as a "plugin" to `motion`. That's been reversed.

## Key decision

**The CNC kinematics module is not a separate crate.** It now lives as a swappable submodule inside `os/src/motion/` rather than its own `os/src/cnc-kinematics/` crate.

Rationale: `docs/mvp-delivery-strategy.md` already states, for the `drivers` crate, that further splits (`drivers-net`, `drivers-hid`, etc.) only happen once a crate actually approaches the crate-size ceiling's 80% trigger point — pre-splitting ahead of any real size or coupling pressure would itself violate that same principle. Giving a single, initially small kinematics module its own crate from day one was exactly that kind of premature split. Folding it into `motion` keeps the Open/Closed extension pattern (new kinematics modules are additive, swappable components) without a crate boundary that isn't earning its keep yet. If `motion` later approaches its own size ceiling because multiple kinematics modules (5-axis trunnion, Wire DED serial-arm, etc. — see `docs/physical-ai-reference-workloads.md`) accumulate inside it, splitting kinematics out into its own crate at that point is the correct, non-premature response.

## Documents touched

- Updated: `docs/mvp-delivery-strategy.md` (crate map, top-level structure diagram, delivery-strategy step 5)
- Updated: `README.md` (Repository Layout)
- Updated: this handover folder's index

## Status

No further handovers on this date past this point at time of writing. See [`index.html`](index.html) for the running index of this date's handovers.
