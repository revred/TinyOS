# STORY-P1-13-01 — The Containment Question Is Decided, and the Walker Exists Only After It

Status: **In progress — criteria 1, 2 and 3 met 2026-08-07 (the containment decision is
recorded below: shape 2, with the argument against the quoted texts; the adversarial
suite was red first and the walker is green on the host with every cap refusing by
test); criterion 4a host-Green the same day (`TOS64-DISPLAY/1` gains
`fb_addr=… src=constant|refused`, pinned by test, riding whichever fixture image boots
next — `12A` §0's sanctioned increment); criterion 4b (`src=dtb`) awaits the
consumption increment and criterion 5 the board.** This
Story exists because
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

## The containment decision (criterion 1) — recorded 2026-08-07

**Shape 2 — deferred consumption — is chosen. Shape 1 is declined because the argument
it requires cannot be written without weakening three machine-checked texts, and this
Story's own terms forbid weakening them to fit.**

The texts, quoted exactly:

- The class-communication matrix's `C1 → C1` row
  ([`class-communication-matrix.tsv`](../security/class-communication-matrix.tsv)) —
  path: *"Fixed-format kernel object and architecture operations only"*; failure rule:
  *"Fail closed without invoking a complex hostile parser"*.
- The `C1` class row ([`containment-classes.tsv`](../security/containment-classes.tsv)),
  input rule: *"Parse no packet file executable document model or variable-length
  device format"*.
- `PD-12` ([`protection-domain-contracts.tsv`](../security/protection-domain-contracts.tsv)),
  enforcement: *"keep packet file executable document model archive filesystem and
  device-format parsers outside C1"*.
- `BND-03` ([`containment-tests.tsv`](../security/containment-tests.tsv)), success
  criterion: *"Privileged hostile-parser entry points and their linked executable bytes
  equal zero"*.

Shape 1 requires arguing that a pure, allocation-free, bounds- and depth-capped FDT
walk **in the boot image** "is not the 'complex hostile parser' the matrix forbids",
against those words, without re-reading them. Four reasons that argument does not
exist:

1. **A flattened device tree is a variable-length device format**, verbatim in the C1
   input rule: nested nodes of arbitrary depth, a token stream whose offsets and
   lengths are input values, a strings block reached through input-controlled offsets.
   Bounding the walk caps its cost; it does not change what the walk *is*. Values from
   the blob select offsets and sizes — exactly the property
   [`STORY-P1-09-16`](STORY-P1-09-16.md)'s admission filter satisfies `BND-03` *by the
   absence of*, and exactly where this project's own register already draws the line:
   fixed-offset total functions (GEM admission, the `hdmi.rs` mailbox descriptor) on
   the admissible side, input-steered reads on the other.
2. **`BND-03`'s success criterion is a scan, not a judgement.** Privileged
   hostile-parser linked bytes equal **zero**. A walker linked into the boot image
   makes that count nonzero however disciplined its code is; passing would require
   re-reading "zero", which is the weakening.
3. **The Feature's own description concedes the premise** — "a device-tree blob is a
   complex hostile format." A Story cannot inherit that sentence and then argue that
   its walker over that format is not a complex-hostile-format parser.
4. The strongest available framing for shape 1 — the walk as the `C0 → C1` *handoff
   verifier* (one-shot at entry, reject into known-good recovery, which its timing
   genuinely resembles) — changes the discipline and the timing of the code, not the
   location of its linked bytes, and `BND-03` scans the image. Attempted and declined
   here so a later session does not re-derive it.

**What shape 2 costs, stated rather than softened, and what is done about each:**

- The 4 MB volatile write keeps today's justification (the `LE-98` display-present
  narrowing, plus a constant a lit canvas proved on this firmware) rather than a
  per-boot one. The realistic invalidator is an EEPROM change, and `LE-117`'s runbook
  tripwire watches for exactly that; the hazard stays procedural-watched, not silent.
- `src=dtb` requires a C4 host for the parse. Two exist in principle: the real
  on-board C4 domain (the shape-2 letter, `EPIC-P3`-era), and — nearer, and adopted as
  this Story's consumption path — **off-board consumption**: boot copies the DTB aside
  at entry as an immutable quarantined object (a bounded copy under a capped
  `totalsize` read from a fixed-offset header field — a fixed-format read of the same
  class `hdmi.rs` already performs; a copy interprets nothing) and ships it as **data**
  over the proven outbound channel; the host — a disposable inspector with zero
  authority over the image — runs *the same walker crate*, and the capture then holds
  the boot's own device tree justifying (or indicting) the canvas base. The charter is
  satisfied trivially: the hostile parse never enters the image at all, and "zero
  linked walker bytes" becomes testable absence rather than argued discipline.
- The DTB transmit needs a bounded chunking envelope over the counted-frame transport;
  that is board-side work for a later increment under this Feature's sprint lift, not
  this session's.

**Criterion sharpenings under the decision (sharpen, not weaken):** criterion 4 splits
into 4a — the `TOS64-DISPLAY/1` source field, board-side, honestly saying
`src=constant|refused` today — and 4b — `src=dtb`, reachable only through a C4 host
for the parse, where the off-board path substitutes *"justified on the wire by that
boot's own DTB"* for *"chosen at boot"*. Criterion 5 closes on the off-board path: one
boot whose captured DTB, walked by this crate on the host, yields the address the
canvas base used — or a spoken divergence, which is a finding, not a failure of the
Story.

## The walker (criteria 2 and 3) — built 2026-08-07, shape-independent

[`os/src/fdt-walk`](../../os/src/fdt-walk/) — a pure `no_std`,
`#![forbid(unsafe_code)]` leaf crate, **linked into nothing** (a test reads the image
crates' manifests and asserts the absence), extracting exactly one fact:
`simple_framebuffer_reg(blob) -> Result<FbReg, Refusal>`. Every read is bounds-checked
against a capped `totalsize`; depth and token caps are hard constants asserted by
test; every refusal is distinct and named; the adversarial suite (truncated, over-deep,
token-flooded, size-lying, region-overlapping, hostile strings, malformed structure,
lying property lengths) was red first. Address translation through a non-empty
`ranges` is a named refusal (`RangesUnsupported`), not an attempt — if the target
firmware's node needs it, that is a second increment argued on the board's own blob,
never a silent guess.

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
