# STORY-P1-13-01 — The Containment Question Is Decided, and the Walker Exists Only After It

Status: **Specified, not yet started.** This Story exists because
[`FEAT-P1-13`](../features/FEAT-P1-13.md) cannot be filed without one, and because the
Feature's own §"The containment question" ends in a decision that must be taken *first*
and taken deliberately: which of the two charter-compatible shapes the device-tree read
takes. It is owner-ordered work (2026-08-07, `LE-98` option 1, sprint rule lifted for
this Feature alone) but it is not an opportunistic pickup: the decision half is a
judgement against the class-communication matrix's own wording, and rushing it is how a
machine-checked boundary gets argued around instead of honoured
Feature: [`FEAT-P1-13`](../features/FEAT-P1-13.md)
Introduced in: [`session/hand-2026-08-07/07F-the-relay-was-never-the-roadblock.md`](../../session/hand-2026-08-07/07F-the-relay-was-never-the-roadblock.md)

## Description

Decide, in writing, between the Feature's two containment shapes — the bounded
boot-path verifier held to C4's properties, or the quarantine-and-defer parse inside a
real C4 domain — by arguing the choice against `BND-03`, `PD-12` and the C1 matrix
row's exact text, with `FEAT-P1-07` §6 left intact either way. Then, and only then,
build the chosen shape's first increment: the pure, allocation-free, bounds- and
depth-capped extraction of exactly one fact (the `simple-framebuffer` node's `reg`)
from a flattened device tree treated as hostile bytes end to end, test-first, with the
adversarial blob suite (truncated, over-deep, cyclic, size-lying, hostile strings) red
before the walker exists and mutation evidence that each cap actually refuses.

## Acceptance criteria (first cut — the deciding session may sharpen, not weaken)

1. The containment decision is recorded in this Story with the matrix row quoted and
   the argument stated; a reviewer can tell which shape was chosen and why the other
   was declined.
2. The walker is pure over a byte slice, allocation-free, with every read
   bounds-checked against a capped `totalsize` and hard depth/iteration limits
   asserted by test.
3. The adversarial suite exists and was red first; every malformed input resolves to
   a refusal, never a partial answer.
4. The extracted address is data, not authority: the canvas base changes only after
   the MMU map and size bounds verify the answer, and a refusal or absence resolves
   to today's constant-plus-`TOS64-DISPLAY/1` behaviour with the source field saying
   which (`src=dtb|constant|refused`).
5. `LE-98` is closed on board evidence — a boot whose canvas base is justified by
   that boot's own device tree, on the wire.
