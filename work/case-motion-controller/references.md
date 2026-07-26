# References

## Primary reference

**Fanuc Series 30i-B Plus** CNC controller — the case owner's own equipment, a real 5-axis machine control. Manufacturer documentation:

- Fanuc Corporation, *Series 30i-B Plus / 31i-B Plus / 32i-B Plus* CNC connection/operation manual, publicly available from Fanuc's official site: `https://www.fanuc.co.jp/en/product/catalog/pdf/cnc/FS30i-B_Plus(E)-03.pdf`

This document is **not** stored in this repository — see [`README.md`](README.md#a-note-on-the-reference-manual) for why. It's referenced here by title/URL so anyone with legitimate access to Fanuc's documentation (an owner, an authorized integrator) can consult the primary source directly. Nothing in `requirements.md` or `test-cases.md` reproduces its text; both describe *expected observable behavior* in TinyOS's own words, informed by (a) hands-on familiarity with the case owner's own machine and (b) general, publicly-known CNC-controller conventions that are industry-standard across vendors, not unique to Fanuc.

## Secondary reference — conversational programming

A general milling-and-turning conversational/shop-floor programming guide, used as background for the programming-assistance and real-time control user stories in [`user-stories.md`](user-stories.md):

- *Manual Guide (Milling and Turning)* — publicly hosted reference copy: `https://www.cnc.uk.com/wp-content/uploads/2015/04/Manual-Guide-Milling-and-Turning-Manual.pdf`

Also **not** stored in this repository, for the same reason as the primary reference above. `user-stories.md` describes the *category* of capability (shape/cycle-based program generation layered on top of raw G-code, per US-10) in TinyOS's own terms — it does not reproduce this guide's text, screenshots, or specific dialog/parameter layouts.

## Repository references (already in this repository)

- [`docs/physical-ai-reference-workloads.md`](../../docs/physical-ai-reference-workloads.md) — the canonical TinyOS specification for the 5-axis CNC flagship demonstration, including the Fanuc-class operator-experience bar, RTCP/TCPC kinematics scope, and the explicit "no compromises" boundary.
- [`docs/mvp-delivery-strategy.md`](../../docs/mvp-delivery-strategy.md) — where the `motion` crate (interpolation, process-sync, kinematics submodule) fits in the Cargo workspace and Roadmap sequencing.
- [`goals/index.html`](../../goals/index.html) — current V&V status for `EPIC-P0`, the Roadmap phase the CNC milestone's earliest work depends on.

## Industry-standard conventions used in this case study

The following are general CNC-industry conventions (RS-274/ISO G-code dialect family, work-coordinate-system numbering, tool compensation modes) that are common across Fanuc, Siemens, Heidenhain, and other major controller vendors — used here as background domain knowledge, not as content specific to or copied from any one vendor's manual:

- G-code motion commands: rapid positioning, linear interpolation, circular/helical interpolation.
- Work coordinate system selection (a numbered offset table, conventionally G54 and up).
- Tool length and radius/diameter compensation.
- Feed/rapid/spindle override controls, single-block execution, dry-run and machine-lock diagnostic modes.
- Operator mode selection: automatic program execution, manual data input, jog (continuous and incremental), handwheel, program edit, and machine reference/home.
