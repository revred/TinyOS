# TEST-P1-09-15-A — A Window Is Claimed at Both Ends, and Twelve Dwords Are Believed One Readback at a Time

Status: **In progress — every clause Green 2026-08-04; clause 5 answered on silicon in its success arm (`STATE=BEACONING` on the beat line where every prior boot stopped or parked; the wire already trained at 1000 Mbps on linkwatch's baseline)**
Story: [`STORY-P1-09-15`](../stories/STORY-P1-09-15.md)
Tier: Host unit tests (size-encoding transcription, dword derivation, write order, readback belief, idempotence, confession wiring)
**plus** a Tier 1 boxed-boot readback
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-06`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-02`, `PD-07`, `PD-10`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: none closeable by a window assignment. No timing
claim anywhere; the pass is a register protocol, not a wait.

## What this test is for

The first spoken park verdict (`STOPPED REASON=TIMEOUT`) convicted the
transmit's DMA inbound path, and the same-night capture (2026-08-04
~02:05, `pios-ground-truth-2026-08-03.txt` tail) read the working
system's answer: three inbound windows, each an `RC_BARn_CONFIG` pair
plus a programmed `UBUS_BARn` remap pair — twelve dwords TinyOS never
writes. This test pins the repair: derive every dword from the captured
`dma-ranges` triples through the driver's own size encoding, write each
exactly once inside `establish` after enumeration, believe only
readbacks, never rewrite a settled window, and speak every refusal as
its own number.

## Specification

### 1. The dwords are derived from the capture, and the derivation is a transcription (`SEC-19`, `BND-06`)

**Given** the three captured windows — PCI `0x0`, 4 MiB, CPU
`0x1F_0000_0000`; PCI `0x10_0000_0000`, 64 GiB, CPU `0x0`; PCI
`0xFF_FFFF_F000`, 4 KiB, CPU `0x10_0013_0000` —
**then** the size encoding matches `brcm_pcie_encode_ibar_size`
(`4KB..32KB → 0x1C + (log2−12)`; `64KB..64GB → log2−15`; `0 =
disabled`), the derived dwords equal the captured raw values
bit-for-bit (`0x7/0x0`, `0x15/0x10`, `0xFFFF_F01C/0xFF` at the `RC_BAR`
pairs; `0x1/0x1F`, `0x1/0x0`, `0x0013_0001/0x10` at the `UBUS` pairs),
every offset lies word-aligned inside the mapped controller window, and
window 2's PCI offset equals `board::RP1_DMA_RAM_BASE`.

### 2. Every dword is written once and believed only from its readback (`SEC-19`, `PD-12`)

**Given** an unprogrammed window,
**then** its four dwords are written in pinned order (`RC_BAR_LO`,
`RC_BAR_HI`, `UBUS`, `UBUS_HI`), each write followed by a readback that
must equal the written value; a disagreeing `RC_BAR` readback refuses as
its own arm and a disagreeing `UBUS` readback as its own arm, each
carrying the readback, with no later dword written past the refusal.

### 3. The seat is pinned and a settled pass writes nothing (`PD-07`, `BND-17`)

**Given** one establishment pass,
**then** the inbound pass runs strictly after the enumeration's
endpoint memory-enable, covering windows 1, 2, 3 in order — twelve
writes on cold silicon. **Given** a window already holding its four
pinned dwords, **then** it sees zero writes; a fully settled pass
writes nothing and succeeds.

### 4. The confession speaks the new rungs and nothing else changes (`SEC-20`, `PD-10`)

**Given** each refusal,
**then** the `RC_BAR` arm counts 21 and the `UBUS` arm 22 —
exhaustively matched, no code shared — each spelling the readback's
decisive low half; `TOS64-LINK/1` names them `ibw-held` and `ibw-remap`
with the full readback; and every previously pinned protocol line is
byte-identical.

### 5. Board: the transmit completes

**Given** the next boxed boot with the rebuilt kernel in the TOS64 role,
**then** the beat line walks `STOPPED REASON=TIMEOUT` →
`STATE=BEACONING` and the beacon appears on the wire — or the confession
names codes 21/22 (or the beat names a different transmit refusal) with
the actual readback, recorded in the session log, and the next fix is
chosen on that number.

### 6. What this test explicitly does **not** establish

- No inbound resource allocation — three fixed windows from one capture.
- No DMA containment claim — `LE-67` stands exactly as raised.
- No change to the outbound window, the enumeration, the BAR rung, or
  any pinned line.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`), written Red
first; then the Tier 1 readback on silicon.

## Implementation location

- `os/src/hal-arm64/src/pcie.rs` — the `inbound` constants and
  derivations, the window pass inside `establish`, the two new
  `LinkAbsent` arms.
- `os/src/hal-arm64/src/etherrors.rs` — codes 21/22 and their decisive
  halves.
- `os/src/hal-arm64/src/ethernet.rs` — the `ibw-held`/`ibw-remap`
  report reasons.

## Reports

To be filed with the boxed boot.
