# FEAT-P1-13 — The Firmware Device Tree Is Read Under C4 Discipline, and the Framebuffer Stops Being a Guess

Status: **Specified — no Story started.** Owner-ordered 2026-08-07 as option 1 of
`LE-98`'s three recorded choices, with the 2026-07-30 hardware-evidence sprint rule
explicitly lifted for this Feature and nothing else; see
[`session/hand-2026-08-07/07F`](../../session/hand-2026-08-07/07F-the-relay-was-never-the-roadblock.md) §7b
Epic: [`EPIC-P1`](../epics/EPIC-P1.md)
Introduced in: [`session/hand-2026-08-07/07F-the-relay-was-never-the-roadblock.md`](../../session/hand-2026-08-07/07F-the-relay-was-never-the-roadblock.md)

## Description

TinyOS paints its canvas into `board::SIMPLEFB_BASE` — a physical address captured
once from a Raspberry Pi OS boot on 2026-08-03 and never confirmed since by anything
the firmware says on the boot it applies to. `LE-98` narrowed the hazard (the canvas
paints only when the firmware reports a display at all) and then recorded, from a
failed attempt, why it could not be closed where the fix naturally lands: the
firmware *does* publish the framebuffer's address, in the flattened device tree whose
pointer arrives in `x0` at entry — but a device-tree blob is a complex hostile
format, [`FEAT-P1-07`](FEAT-P1-07.md) §6 names a DT parser as an explicit non-goal
for exactly that reason, and the class-communication matrix requires C1 to fail
closed **without invoking a complex hostile parser** (`BND-03`, `PD-12`). The 4 MB
volatile write to an unverified constant on a machine with no IOMMU is a safety
hazard; the parse that would justify it is a security hazard if placed where the
constant lives. This Feature is the reconciliation: **the parse exists, and it is
contained** — and its first consumer is the framebuffer address.

The 2026-08-07 board evidence sharpened what is actually needed. The constant is
currently *correct* (a lit canvas proved it, photographed and wire-paired), so this
is not a firefight: it is the difference between a canvas that works until the
firmware moves something and a canvas whose write target is justified by what the
firmware said *this boot*. The same evidence showed the failure that fooled four
sessions — the firmware's native-size answer and its scanout bring-up are separate
facts — so the DT read is also the only way the board can ever *say* where its
framebuffer is, rather than merely whether a display exists.

## The containment question, stated for the Stories rather than presumed

The bring-up image has no Protection Domains: there is no C4 domain to host a
disposable parser at the moment the canvas base is chosen. The charter's C4
discipline (disposable, bounded, destroyed after inspection, output data-only) must
therefore be satisfied in one of two shapes, and **choosing between them is this
Feature's first Story-level decision, not this document's**:

1. **A bounded verifier in the boot path, held to C4's *properties* without the
   domain machinery.** A pure, allocation-free, depth- and iteration-bounded FDT
   walk (fixed caps asserted by test; every read bounds-checked against the header's
   own `totalsize`, itself capped) that extracts exactly one fact — the
   `simple-framebuffer` node's `reg` — and refuses everything malformed. Pure
   function over a byte slice, host-tested against adversarial blobs (truncated,
   cyclic, over-deep, lying sizes, hostile strings), with the mutation evidence
   `BND-03` demands. Its output is data (an address the MMU map and canvas then
   *verify they can honour*), never authority. If this shape is chosen, the argument
   that it is not the "complex hostile parser" the matrix forbids must be made in
   the Story against the matrix row's own wording, and the matrix row must not be
   weakened to fit.
2. **Deferred consumption.** Boot keeps the verified-constant-plus-fallback exactly
   as today; the DT is copied aside at entry as an immutable quarantined object, and
   the parse runs later, inside a real C4 domain, once the AArch64 side has one —
   feeding a *correction* (repaint target, or a refusal that darkens the canvas
   with its reason spoken) rather than the boot-time choice. This is the charter's
   letter with no argument owed, at the cost of a canvas that can still open on a
   wrong constant for the first seconds.

Either way: **the DTB is hostile input end to end** (`BND-02`, `BND-03`, `PD-12`,
`PD-14`). Firmware provenance does not make it trusted — the same firmware's
framebuffer descriptor is already treated as hostile by `hdmi.rs`, and that
discipline is the precedent, not an exception.

## Explicit non-goals

- **General device discovery from the DT.** One fact is extracted. The moment a
  second consumer wants a second node, that is a new Story in this Feature with its
  own adversarial evidence, not a generalisation of the first.
- **Weakening `FEAT-P1-07` §6.** That Feature's non-goal stands; this Feature is
  where the parser lives so that one keeps its boundary.
- **Trusting the answer.** The extracted address justifies a write target only
  after the MMU map covers it with the right attributes and the size bounds the
  canvas's own writes; an address the map cannot honour is a spoken refusal, not a
  remap-on-faith.
- **The scanout question.** The 2026-08-07 evidence shows the firmware can answer
  the size query while never bringing up scanout; no DT read fixes a monitor the
  firmware did not light. The bench procedure for that lives in `07F` §7b.

## Crate(s) involved

To be decided by the first Story against the containment question above — the
candidates are a new leaf crate under `os/src/` for the pure walker (so `hal-arm64`
never contains it) consumed behind a seam, versus `exec`-side C4 machinery for
shape 2. Nothing binds until the Story does.

## Acceptance shape

A boot on the board whose canvas base is either justified by the firmware's own
device tree on that boot or refused with the reason on the wire (`TOS64-DISPLAY/1`
gains the address and its source: `fb_addr=… src=dtb|constant|refused`), with the
walker's adversarial evidence filed and `LE-98` closed on the result.
