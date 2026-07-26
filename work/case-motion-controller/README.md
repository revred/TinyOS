# Case Study: 5-Axis Motion Controller Workload on TinyOS

Status: **working case study — grounds the CNC flagship demonstration against a real, owned machine**

## Purpose

This is a working folder, not a formal spec — it exists to populate the [5-axis CNC flagship MVP demonstration](../../docs/physical-ai-reference-workloads.md#workload-1-5-axis-cnc-controller-flagship-mvp-demonstration) with concrete requirements and test cases grounded in a real 5-axis Fanuc-controlled machine the case owner has direct access to, rather than purely abstract industry conventions.

## What's in this folder

- [`references.md`](references.md) — citation of the reference manuals used (Fanuc controller documentation, a milling/turning conversational-programming guide), and why the manuals themselves aren't stored in this repository.
- [`requirements.md`](requirements.md) — functional requirements for TinyOS's 5-axis motion controller workload, organized the same way [`docs/physical-ai-reference-workloads.md`](../../docs/physical-ai-reference-workloads.md) already scopes "no compromises" vs. deferred.
- [`test-cases.md`](test-cases.md) — concrete test scenarios, cross-referenced to [`goals/`](../../goals/) Stories/Tests where a corresponding entry exists.
- [`user-stories.md`](user-stories.md) — user stories for real-time G-code motion control, the flagship demo case: an application on TinyOS that streams/executes G-code and controls the motion platform live, immediately legible to a non-technical observer in a way the other deployment modes aren't.

## A note on the reference manual

A Fanuc controller manual (the case owner's own equipment documentation, downloaded from Fanuc's official public site) is kept locally in this folder for reference **but is excluded from version control** via the repository's [`.gitignore`](../../.gitignore). It's copyrighted material published by Fanuc for its equipment owners — appropriate for the case owner to have locally, not appropriate for this project to redistribute by committing it to a public repository. `requirements.md` and `test-cases.md` cite it by title and section where relevant, and describe expected behavior in TinyOS's own words, not by reproducing the manual's text.

## Relationship to the rest of the project

- **Design authority**: [`docs/physical-ai-reference-workloads.md`](../../docs/physical-ai-reference-workloads.md) remains the canonical specification for the CNC workload's architecture (shared RT primitives, "no compromises" scope). This case study doesn't override it — it feeds concrete, real-machine-grounded detail back into it as that detail is confirmed.
- **V&V tracking**: work identified here that becomes a committed piece of scope should get a corresponding entry under [`goals/`](../../goals/) (a Story under a `motion`-related Feature) rather than being tracked only in this folder — this folder is where requirements get worked out, `goals/` is where they become traceable, testable commitments.
- **Hardware**: per [`SeedMVP.md`](../../SeedMVP.md#53-hardware-chosen-for-mvp-and-why), TinyOS's own MVP hardware validates the *software* (interpolation, kinematics, RTCP) against simulated axes first. This case study's real Fanuc-controlled machine is a reference for *what correct operator-facing behavior looks like*, not (yet) a TinyOS deployment target — TinyOS isn't replacing the Fanuc controller on this machine; it's being held to the same behavioral bar.
