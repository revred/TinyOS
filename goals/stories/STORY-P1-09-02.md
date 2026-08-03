# STORY-P1-09-02 — The PHY Answers: Identification and Latched Link State

Status: **In progress — host half Green 2026-08-03 (clause-22 framing pinned, port discipline and bounded polls asserted by the recording double, identity-first scan, latched-twice link with downward speed resolution); criterion 4 awaits a board capture. Not Verified.**
Feature: [`FEAT-P1-09`](../features/FEAT-P1-09.md)
Introduced in: [`session/hand-2026-08-03/05A-ethernet-discovery-signal.md`](../../session/hand-2026-08-03/05A-ethernet-discovery-signal.md)

## Description

04A's sentence — the cable is a direct signal — needs the *device* to see the
cable. This Story opens the GEM management port (MDIO) and asks the board's
Ethernet PHY two things, in order of trust: **who are you** (identifier registers,
validated against the expected Broadcom identity before any further word is
believed) and **is the cable in** (basic-status register, read twice because the
link bit is latched-low, so the second read is the live answer).

Every management-port transaction is a bounded poll of the GEM idle bit with a
typed timeout; the PHY address is found by a bounded scan; a PHY that never
answers, answers with an unknown identity, or never settles is each a distinct,
named, fail-safe outcome. The result extends the `TOS64-LINK/1` line:
`phy=0x… link=up|down` — or `phy=absent reason=…`.

**No MAC configuration, no traffic.** Speed/duplex results are read and reported
for `STORY-P1-09-03` to consume; nothing is transmitted here.

## Depends on

- `STORY-P1-09-01` — no management port without a present RP1.

## Acceptance criteria

1. **MDIO is a pure state machine over the scripted seam.** Clause-22 read/write
   framing pinned by host tests; the idle-poll budget-bounded; scripted timeout,
   garbage, and abort answers each a distinct driven rejection.
2. **Identity before belief.** The PHY identifier words are validated against the
   expected identity; an unknown PHY reports `phy=unknown id=0x…` and stops
   there — reported, not trusted.
3. **Link state is read latched-twice** and reported with the negotiated
   speed/duplex when link is up; a down link is an honest `link=down`, not an
   error.
4. **Board: cable in, cable out.** Two captures — one with the peer-to-peer
   cable plugged into a live laptop, one without — show `link=up` and
   `link=down` respectively, protocol lines unchanged.

## Named debt this Story leaves open

- `LE-26` — a PHY conversation over a kept window is still not a driver stack.
- PHY power/reset/clock provisioning is believed to be firmware/hardware default;
  if the board run falsifies that, the finding lands in the Report and the fix
  becomes its own recorded step — not a silent widening of this Story.

## Progress, 2026-08-03

| Criterion | State |
|---|---|
| 1 — clause-22 framing exact bits | **Green.** `gem::mdio_read_word` pinned, field masking included. |
| 2 — identity before belief | **Green.** Scan classifies Known (revision steppings included), Unknown (reported, believed for nothing), Absent, and PortWedged; the seam double refuses any register outside the named set. |
| 3 — latched-twice link and speed resolution | **Green.** Second read believed; 1000/100/10 × duplex resolved downward from partner ability; down and unresolved are honest outcomes; line shapes pinned. |
| 4 — board, cable in and out | **Blocked, and the named risk is now evidenced (`LE-68`).** First physical attempt (2026-08-03 evening): the laptop NIC saw a dead-flat PHY across three power cycles with the power LED steady green — the PHY is most likely unpowered/held in reset until software releases it. The release step is `LE-68`'s recorded work, owed before this criterion can pass; serial remained unverified (fourth silent capture, loopback not yet executed). |

## Tests

[`TEST-P1-09-02-A`](../tests/TEST-P1-09-02-A.md) — written before implementation,
per the TDD mandate.
