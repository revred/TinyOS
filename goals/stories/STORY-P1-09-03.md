# STORY-P1-09-03 — The Beacon: Board Presence in One Bounded Frame

Status: **Verified (functional) 2026-08-05 — all four acceptance criteria met; criterion 4 is `FEAT-P1-09`'s exit criterion and it was closed off the wire this session. Criteria 1, 2 and 3 host half Green 2026-08-03 (frame bytes pinned with the sequence field the only variance, two-descriptor ring with permanent stop, transmit order and bounds asserted, receive-absence tested). **Criterion 4 met 2026-08-05**: twelve whole beacon frames (seq 5964–5975) captured off the cable by `ti64dink --live 12 --raw` — Npcap, **unelevated**, a stock host — and compared to `gem::beacon_frame(seq)` **byte for byte including the 14-byte Ethernet header** by `gem::tests::the_captured_beacon_is_byte_identical_to_the_built_frame`. The beacon repeats and the sequence field is the only variance, exactly as the criterion asks. Three properties make it evidence rather than arithmetic: the header is included, so the destination MAC, source MAC and EtherType are compared rather than skipped by a payload-only capture; the sequence is an **input** read from the evidence file and never derived from the frame under comparison, so the test cannot compare a frame to itself; and it was **verified to fail** — one flipped byte in one captured frame fails it with the frame and offset named. Captured from a *late* attach at seq ~5964, so the beacon is unchanged deep into a run rather than merely correct after boot. Raw evidence: [`goals/reports/beacon-frames-2026-08-05.txt`](../reports/beacon-frames-2026-08-05.txt), `include_str!`d by the test so the bytes a Report cites and the bytes the test asserts are one copy and cannot drift. The transmit path's fail-safe arm (criterion 2) was additionally observed on silicon before this: `BOARD VERDICT 3`'s `STATE=STOPPED REASON=TIMEOUT` is a refused transmit permanently stopping the beacon and saying so, which is the refusal arm reached rather than argued. The criterion's "serial protocol lines unchanged" half is carried by the canvas, not serial — serial has never produced a byte on this bench (`LE-47`). **Assurance state remains `specified` and this Story is NOT release-assured**: assurance `verified` needs every applicable mapped release gate in a declared deployment profile, and `qualified-platforms.tsv` holds 0 qualified platforms, so that rung is closed to every Story in this project ([`hand-2026-08-05/06A`](../../session/hand-2026-08-05/06A-nothing-is-verified-and-the-reason-is-not-velocity.md) §2).**
Feature: [`FEAT-P1-09`](../features/FEAT-P1-09.md)
Introduced in: [`session/hand-2026-08-03/05A-ethernet-discovery-signal.md`](../../session/hand-2026-08-03/05A-ethernet-discovery-signal.md)

## Description

The smallest honest board-present signal a laptop can capture: a raw Ethernet II
**broadcast** frame, local-experimental EtherType `0x88B5`, payload a single
`TOS64-PRESENT/1` envelope line (`key=value` fields per the repository's wire
convention — board identity, image hash prefix, and a monotonically increasing
sequence number). The frame bytes are built by a pure function and pinned
word-for-word by host tests; the capture on the laptop is byte-comparable against
the same pinned builder, which makes the beacon itself the Report's raw evidence —
hardware evidence that arrives before the serial adapter works.

Transmission uses the GEM transmit path at its minimum: one two-descriptor ring
(second descriptor wrap-only), one static pinned buffer inside the image, MAC
speed configured from `STORY-P1-09-02`'s negotiated answer, transmit-status
readback validated, every wait bounded. After the verdict and splash, the park
loop beacons once per period; any transmit error stops beaconing permanently and
the board stays parked — fail-safe over keep-trying.

**Receive stays disabled.** The beacon announces; it does not listen. Remote
bytes are data, never code — and on this Story's image they are not even read.

## Depends on

- `STORY-P1-09-02` — no transmit without a validated PHY and a live link; a
  `link=down` board skips beaconing and reports exactly that.

## Acceptance criteria

1. **The frame is exact bytes.** Destination broadcast, locally-administered
   source MAC `02:54:4F:53:36:34` (ASCII `TOS64` behind the local bit — a
   board-serial derivation would drag the mailbox path into this Feature for
   no discovery value on a two-node wire), EtherType `0x88B5`,
   `TOS64-PRESENT/1` payload, minimum-size padding — pinned word-for-word by
   host tests, with the sequence field the only varying bytes.
2. **The transmit path is bounded and validated over the scripted seam.**
   Descriptor layout pinned; the used-bit poll budget-bounded; scripted
   no-completion, error-status, and abort answers each a distinct driven
   rejection that permanently stops beaconing and reports `beacon=stopped`.
3. **The DMA grant is one buffer.** The device is handed exactly one pinned
   buffer address inside the image; nothing on this path allocates, and the
   descriptor ring points nowhere else (`LE-67` names the missing IOMMU
   containment honestly).
4. **Board: the laptop sees the board.** A stock packet capture on a
   peer-to-peer-connected laptop shows the beacon repeating, byte-identical to
   the pinned frame apart from the sequence field; serial protocol lines and
   splash unchanged.

## Named debt this Story leaves open

- `LE-67` — transmit DMA without IOMMU containment; one pinned buffer is the
  whole grant, and `BND-07` evidence stays narrow until the C2 device-service
  work lands.
- `LE-26` — unchanged; a beacon is not a transport.
- RGMII transmit-clock provisioning is believed firmware/hardware-default at
  negotiated speed; a board run that falsifies this lands in the Report as a
  finding, and the clock work becomes its own recorded step.

## Progress, 2026-08-03

| Criterion | State |
|---|---|
| 1 — the frame is exact bytes | **Green.** Pinned word-for-word; sequence digits proven the only variance; `u32::MAX` fits the buffer. |
| 2 — bounded, validated transmit | **Green.** Register order asserted (speed, `ADDR64`, queue base, stale-status clear, enable, start); timeout and MAC-error each a distinct permanent stop. |
| 3 — one-buffer DMA grant | **Green (host).** `tx_ring` points only at the given frame address with a permanent-stop second descriptor; the glue's single 64-byte-aligned static is the whole grant (`LE-67` stays open for the real containment). |
| 4 — board, laptop witnesses the beacon | **Blocked on hardware.** Carries this Story's named risk: RGMII transmit-clock provisioning is believed hardware-default until a capture (or its absence at `link=up`) says otherwise. |

## Tests

[`TEST-P1-09-03-A`](../tests/TEST-P1-09-03-A.md) — written before implementation,
per the TDD mandate.
