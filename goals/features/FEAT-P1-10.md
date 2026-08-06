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
5. **This Feature holds no receive grant**, and its evidence says so. That sentence used to read "the receive path stays disabled" and it was a claim about the *board*; since [`STORY-P1-09-16`](../stories/STORY-P1-09-16.md) it is a claim about *this egress path only*. The image now has a receive path under `FEAT-P1-09`, its frames are counted and discarded, and no byte of one is ever handed to the spoor stream. `LE-67` is re-argued there rather than inherited here.

## Stories

| Story | What it does | Status |
|---|---|---|
| [`STORY-P1-10-01`](../stories/STORY-P1-10-01.md) | The on-wire format: raw packed `u64` records, MTU-filling frames, `u64` sequence for exact loss accounting, reusing the journal's own magic and record layout | Verified (functional) 2026-08-05 — 13 host tests, Red-verified; criterion 1 (byte-identity, no re-packing on the board-side path) checked on silicon by `BOARD VERDICT 13`'s record-for-record comparison of a live frame 0 against its certificate, with 0 refused across 400+ decoded records |
| [`STORY-P1-10-02`](../stories/STORY-P1-10-02.md) | Boot and park stamping, and the park-loop drain that transmits frames | Verified (functional) 2026-08-05 — criteria 1, 2, 3 and 5 Green on silicon 2026-08-04 (`BOARD VERDICT 10`), criterion 6 MEASURED off the wire 2026-08-05 (stamp 136, announce 3099, drain 122005 cycles), criterion 4 held by the compiler-enforced no-heap gate plus a refused transmit observed on the shared path |
| [`STORY-P1-10-03`](../stories/STORY-P1-10-03.md) | The same records wrapped in IPv4/UDP so an unprivileged host socket can read them with no capture driver installed — a shim for the host's benefit, never the protocol | In progress — host-Green 2026-08-05, 8 tests, filed to close `LE-73`. **`encode` has no callers**: nothing on the board has emitted one, and the 181-record ceiling is already paid for this unused framing |
| [`STORY-P1-10-05`](../stories/STORY-P1-10-05.md) | The machine says how hot it is: the AVS die temperature stamped raw onto the stream, converted only on the host | In progress — criteria 1, 2, 3, 4 and 5 met, and the raw AVS word is on the wire moving (`BOARD VERDICT 14`); the missing thing is the paired Pi OS reading, without which no temperature may be quoted |
| [`STORY-P1-10-04`](../stories/STORY-P1-10-04.md) | The retained boot certificate and the boot epoch: a listener joining late learns which boot it joined, and learns the boot rungs it never saw | Verified (functional) 2026-08-05 — criteria 1–5 and 7 Green on silicon 2026-08-05 (`BOARD VERDICT 11`–`13`: boot state read from a capture opened at record 74, epoch changing across power cycles, two boots in one window with 0 lost, certificate byte-identical to a live frame 0). Criterion 6 met: thousands of repeating park rungs across records 74..1738 displaced none of the three the boot established |

**Enumerated, deliberately not decomposed** (just-in-time rule): a C# `tos64-listen` host
decoder growing into Ti64Dink (`FEAT-P2-10`), and the extension that wires the existing
`dispatch`/`lock`/`wcet`/`actuation` spoor call sites into the AArch64 path. The owner's
order is boot/park first, then the kernel call sites stream through a channel already shown
to work.

## What this Feature deliberately does not do

- **It does not enable receive.** Bidirectional exchange is a different risk class and is
  specified in §7 of the architecture document rather than built here. This paragraph
  predicted that reversing `no_path_in_this_module_ever_enables_receive` would be a Security
  Charter change requiring adversarial tests, a bounded command vocabulary, authenticity, and
  a replacement for the containment argument `LE-67` rests on. `STORY-P1-09-16` took three of
  those four and **owed the fourth nothing**: it read the charter in full, replaced the
  containment argument, and enumerated the refusals — but it accepts no commands at all, so
  there is no vocabulary to bound and no authenticity to establish. A counted frame needs
  neither. Step 2 of [`hand-2026-08-06/03B`](../../session/hand-2026-08-06/03B-the-arms-are-built-the-board-booted-them-and-nobody-read-the-wire.md)
  §5 is where both debts fall due, and that Story is where this paragraph's prediction still
  holds in full.
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
- **Epoch entropy** (`LE-74`). `STORY-P1-10-04`'s boot epoch distinguishes boots because
  firmware timing varies between them, which is borrowed entropy. It is a change detector and
  never a boot count, and no Report may describe it as one until a hardware RNG or persisted
  state exists.
- **Announcement cadence.** `ANNOUNCE_EVERY = 5` is chosen, not measured — the same class of
  debt as ring sizing, and stated for the same reason.
- **`LE-67`** applies, and it moved: the image's grant is now *two* pinned regions, one per
  direction, and neither aliases the other (`STORY-P1-09-16`). Nothing on **this** Feature's
  path changed — `STORY-P1-10-04` adds no grant and no second buffer, the announcement rides
  the transmit path `STORY-P1-10-02` already proved — but a reader who took "one pinned
  buffer" from this bullet as a statement about the board would now be wrong, which is why
  the bullet says so instead of staying quietly correct about its own half.
