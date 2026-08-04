# FEAT-P1-10 — Spoors on the Wire: the Observability Substrate

Status: **In progress — added 2026-08-04 on the owner's order. `STORY-P1-10-01` (the on-wire format) host-Green with 13 tests; `STORY-P1-10-02` (boot/park stamping and egress) specified. Two further Stories enumerated and deliberately not decomposed. No board evidence yet.**
Epic: [`EPIC-P1`](../epics/EPIC-P1.md) — Determinism Proof
Architecture: [`docs/spoor-transport-architecture.md`](../../docs/spoor-transport-architecture.md)
Introduced in: `session/hand-2026-08-04/04A` session, on the owner's framing that **a spoor is to a physical system what a token is to a language model**

## Why this is a Feature and not a diagnostic

`FEAT-P1-09` put a beacon and a measurement transcript on the wire as *bring-up evidence*.
This Feature is a different claim: the spoor stream is the system's **observable behaviour**,
the substrate on which measurement, replay, audit and eventually learning sit. That is a
product surface with its own containment posture and its own completeness obligation, not a
bring-up detail inheriting the beacon's.

It sits under `EPIC-P1` because determinism that cannot be observed cannot be proven. The
Epic's whole purpose is evidence, and this is the channel that evidence travels on.

**`LE-56` is this Feature's inheritance.** That row records that no console or tab run has
ever captured a kernel spoor — the audit atom exists, is well-tested, and has never been
seen leaving a running system. On the AArch64 path today the board mints essentially none:
`kernel::fault::audit` produces one per fault and nothing stamps the boot, tick, beacon or
park rungs at all.

## Exit criteria

1. **A spoor stream leaves the board and is decoded on the laptop**, with records byte-identical to what the kernel stamped — no board-side formatting anywhere on the path.
2. **Loss is reported as an exact count**, not inferred. A host that saw a gap says how many records it missed.
3. **The boot and park rungs stamp spoors**, so a run produces a stream rather than a trickle, and `LE-56` can close on a captured kernel spoor rather than an asserted one.
4. **Every Report on this Feature states what the stream does *not* yet cover.** Until the `dispatch`/`lock`/`wcet`/`actuation` call sites stamp on this path, the stream is boot-and-park behaviour only and must be described as such.
5. **The receive path stays disabled**, and the Feature's evidence says so. `LE-67` records that with no IOMMU, "receive disabled" *is* the containment story.

## Stories

| Story | What it does | Status |
|---|---|---|
| [`STORY-P1-10-01`](../stories/STORY-P1-10-01.md) | The on-wire format: raw packed `u64` records, MTU-filling frames, `u64` sequence for exact loss accounting, reusing the journal's own magic and record layout | In progress — host half Green 2026-08-04, 13 tests |
| [`STORY-P1-10-02`](../stories/STORY-P1-10-02.md) | Boot and park stamping, and the park-loop drain that transmits frames | In progress — criteria 1, 2, 3 and 5 Green on silicon 2026-08-04 (`BOARD VERDICT 10`), criterion 4 evidenced negatively by `BOARD VERDICT 9`, criterion 6 (stated cost) not yet met |

**Enumerated, deliberately not decomposed** (just-in-time rule): a C# `tos64-listen` host
decoder growing into Ti64Dink (`FEAT-P2-10`), and the extension that wires the existing
`dispatch`/`lock`/`wcet`/`actuation` spoor call sites into the AArch64 path. The owner's
order is boot/park first, then the kernel call sites stream through a channel already shown
to work.

## What this Feature deliberately does not do

- **It does not enable receive.** Bidirectional exchange is a different risk class and is
  specified in §7 of the architecture document rather than built here. Reversing
  `no_path_in_this_module_ever_enables_receive` is a Security Charter change requiring
  adversarial tests, a bounded command vocabulary, authenticity, and a replacement for the
  containment argument `LE-67` currently rests on.
- **It does not deploy code over the wire.** Rule 9 of `agent.md` — remote bytes are data,
  never code. The charter-neutral route to ending the card-swap loop is Pi 5 *firmware*
  netboot, which loads an image before TinyOS exists; that is an investigation, not part of
  this Feature.
- **It claims no confidentiality and no authenticity.** The link is readable and forgeable by
  anyone on the cable. It is a point-to-point bench and deployment link, and every Report
  must state that rather than let "minimal attack surface" be read as "secure".

## Named debt

- **Completeness (exit criterion 4).** The stream is not yet the system's whole observable
  behaviour, and the Feature's central claim is only fully true once it is.
- **Ring sizing.** `SPOOR_JOURNAL_CAPACITY` was chosen for a crash-dump ring, not a streaming
  jitter buffer. The right size is a function of burst rate against drain period and has not
  been measured — a ring that wraps before a drain is a *measured* loss, so this degrades
  honestly rather than silently, but it degrades.
- **Unprivileged host capture.** Raw `0x88B5` needs a privileged reader on Windows. A UDP
  shim would remove that, buys the board nothing, and is recorded as an option rather than
  adopted.
- **`LE-67`** applies unchanged: one pinned buffer, receive disabled, no device isolation.
