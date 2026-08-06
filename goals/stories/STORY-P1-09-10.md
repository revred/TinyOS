# STORY-P1-09-10 — The Introduction: an Endpoint Is Enumerated Before Its Window Is Believed

Status: **Verified (functional) 2026-08-05 — all four acceptance criteria met. Criteria 1, 2 and 3 host half Green 2026-08-03 (bridge values pinned against lspci, ordering and masked command writes pinned, both vendor gates refuse honestly with zero or minimal writes). **Criterion 4 Green on silicon** in both arms: `BOARD VERDICT 1` counted 16, a rung deeper than this Story's 9, and `BOARD VERDICT 2` reached the plain pulse with the full chain — identity, release, scan, watch — and the wire trained to gigabit. Either way the routing rung closed on silicon, which is what the criterion asks. Criterion 1's pinned values were independently corroborated by the 2026-08-03 ~23:59 Pi OS `setpci` capture taken under working Ethernet: bridge `PRIMARY/SECONDARY/SUBORDINATE = 00/01/01` and mem base/limit dword `0x00400000` at offset `0x20`, read live from a machine whose Ethernet worked — so the encoding was checked against a running system and not only against the `lspci -vv` text it was derived from. **Assurance state remains `specified` and this Story is NOT release-assured**: 0 qualified platforms.**
Feature: [`FEAT-P1-09`](../features/FEAT-P1-09.md)
Introduced in: the 2026-08-03 window boot — the lamp counted **9**: the programmed window validated and the GEM identity read answered with the wrong module, the signature of memory reads that never reach RP1

## Description

The count of 9 said the window is fine and the *bus* is not: reads at PCI
address `0x100000` are completed by something that is not the GEM. The
ground truth names what is missing — under Pi OS the root port carries
`primary=00, secondary=01, subordinate=01`, forwards
`Memory behind bridge: 00000000-004fffff`, and both the bridge and RP1 read
`Mem+ BusMaster+`; and the driver source shows the PCI *framework*, not the
firmware, does all of that. On our bare-metal handoff, nobody has.

This Story is enumeration at bring-up size, over the config-space access
the controller provides (`EXT_CFG_INDEX`/`EXT_CFG_DATA` at
`0x9000`/`0x8000`, standard ECAM packing `bus<<20 | devfn<<12`; the root
port's own header memory-mapped at the controller base — `rpi-6.12.y`
`pcie-brcmstb.c`, retrieved 2026-08-03). The sequence: verify the root
port's vendor before writing a byte; program bus numbers `0/1/1` and the
forwarding window `0x0..0x4fffff`; enable the bridge's memory decode with
the status half masked out (status is write-one-to-clear and belongs to
nobody here); then read bus 1 device 0's vendor and refuse honestly if it
is not Raspberry Pi (`0x1de4`) before enabling the endpoint's decode. RP1's
4 MiB peripheral BAR is a fixed aperture at bus address zero (`lspci` marks
it `[virtual]`; Linux never programs it either), so no BAR is written.

Two new refusals join the honest vocabulary end-to-end: `root-vendor` and
`endpoint-vendor`, each carrying its readback in the `TOS64-LINK/1` line
and counting 14 and 15 on the lamp.

## Depends on

- `STORY-P1-09-09` — the window pass this Story runs after; window
  registers are controller-local and need no bus routing.
- `STORY-P1-09-08` — the re-probe cadence, which now retries the
  introduction each park-second too.

## Acceptance criteria

1. **Every value is pinned against the capture.** Bus numbers dword
   `0x0001_0100`, forwarding window dword `0x0040_0000`
   (base `0x0000`, limit `0x0040` ⇒ `0x0..0x4fffff`), command bits
   `MEM|MASTER`, the ECAM index `1 << 20` for bus 1 device 0, and both
   expected vendors — each asserted citing the `lspci -vv` lines and the
   driver-source encoding.
2. **The sequence is exact, ordered, and masked.** Root vendor read first —
   a wrong vendor refuses with zero writes; then bus numbers, forwarding
   window, bridge command (status half written as zeros, never echoed);
   then the endpoint vendor gate — a wrong vendor refuses with no endpoint
   write; then the endpoint command, same mask discipline. Pinned by a
   recording double with hostile status readbacks.
3. **Refusals are honest end-to-end.** `root-vendor` and `endpoint-vendor`
   carry their readbacks in the report line's pinned shapes and count 14
   and 15 on the lamp, distinct from every existing code.
4. **Board: the confession moves past 9.** The next boot reaches the plain
   pulse — identity, release, scan, watch, beacon — or counts a deeper
   rung; either way the routing rung closes on silicon.

## Named debt this Story leaves open

- `LE-10` stays open and untouched: this is index/data config access on one
  known controller for one known endpoint, not ECAM/MCFG discovery or
  bridge traversal.
- BusMaster is enabled for the beacon's transmit DMA under `LE-67`'s
  existing no-IOMMU debt; nothing new is granted.

## Progress, 2026-08-03

| Criterion | State |
|---|---|
| 1 — values pinned | **Green.** Each constant cites its lspci or driver-source line. |
| 2 — exact ordered masked sequence | **Green.** Recording double with hostile W1C status bits; wrong-vendor passes write nothing/nothing-further. |
| 3 — honest refusals end-to-end | **Green.** Report shapes and lamp codes 14/15 pinned; distinctness test extended. |
| 4 — board | **Blocked on the next power-on.** |

## Tests

[`TEST-P1-09-10-A`](../tests/TEST-P1-09-10-A.md) — written before
implementation, per the TDD mandate.
