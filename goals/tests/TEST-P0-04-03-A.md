# TEST-P0-04-03-A — Read-Only Bus-0 PCI Enumeration Discovers q35's Devices Deterministically

Status: **Verified — passing locally, 2026-07-27**
Story: [`STORY-P0-04-03`](../stories/STORY-P0-04-03.md)
Tier: Host (`cargo test -p hal --lib`, `cargo test -p hal-x86_64 --lib`) plus Tier 0 (QEMU x86_64), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix) — CAM address packing, header decoding, and the whole bus-0 walk (generic over the read-only `ConfigSpace` abstraction) are host-testable against mock configuration spaces; only the legacy `0xCF8`/`0xCFC` port-I/O backend needs a real (or QEMU-emulated) host bridge, mirroring `TEST-P0-04-02-A`'s own host/Tier 0 split.
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01` (boot and topology discovery), `D22` (opt-in driver lifecycle)
Security controls: `SEC-13`, `SEC-18`, `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`, `C2`
Boundary tests: `BND-02`, `BND-06`, `BND-07`, `BND-08`, `BND-18`, `BND-20`
Protection Domain contracts: `PD-02`, `PD-08`, `PD-10`, `PD-12`, `PD-13`
Code admission gates: `RCG-07`, `RCG-12`, `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## Specification

**Given** an x86_64 machine (QEMU `q35`, or a mock configuration space standing in for one on the host),
**when**:
- `hal_x86_64::pci::enumerate_bus_zero` walks bus 0 — **then** every present function is recorded into `hal::device::DeviceTable` with vendor id, device id, and bus/device/function topology position, in (device, function) discovery order; absent slots (vendor `0xFFFF`) are skipped; functions 1–7 of a device are probed only when function 0's header-type byte reports multifunction (a single-function device is never probed beyond function 0, because the PCI specification does not require it to decode nonzero function numbers),
- the walk runs against QEMU `q35`'s real configuration space (`fixture-pci-enumeration`) — **then** the host bridge is discovered at position 0:0:0 with Intel's vendor id (`0x8086`), at least one device is discovered overall, and a second complete walk produces a table identical to the first (repeated *reads* of configuration space are stable),
- the walk is observed through a read-recording mock — **then** the only configuration offsets ever read are the vendor/device dword (`0x00`) and the header-type dword (`0x0C`) — no BAR, command-register, IRQ-line, or capability-list offset is touched, and the `ConfigSpace` abstraction the walk is generic over exposes *no write operation at all*, making "read-only against device configuration space" (`STORY-P0-04-03` acceptance criterion 2) a structural property, not a tested promise alone,
- a bus presents more functions than the caller-chosen `DeviceTable` capacity — **then** enumeration fails closed with `DeviceTableError::Full` (partial results intact, error visible to the caller) rather than silently truncating discovery into a "complete" claim.

## Test type

Unit tests (`hal::device`'s `#[cfg(test)]` module — 4 tests over the fixed-capacity table's fail-closed semantics — and `hal_x86_64::pci`'s `#[cfg(test)]` module — 10 tests over CAM packing, header decoding, and the walk against recording mocks) plus one Tier 0 QEMU fixture (`kernel`'s `fixture-pci-enumeration` feature) exercising the real legacy-CAM port-I/O backend against q35's emulated configuration space, mirroring the host/QEMU-both pattern this project has used since `TEST-P0-05-02-A`.

## Implementation location

`os/src/hal/src/device.rs` (arch-neutral `DeviceDescriptor`/`DeviceTable`), `os/src/hal-x86_64/src/pci.rs` (`BdfAddress`, CAM address packing, header decoding, read-only `ConfigSpace` trait, `enumerate_bus_zero`, `PortCam` legacy-CAM backend), `os/src/kernel/src/capacities.rs` (`MAX_PCI_DEVICES`), `os/src/kernel/src/fixture_pci_enumeration.rs`, `os/src/kernel/src/main.rs` (the new fixture feature, plus bus-0 enumeration wired into the real boot path's success gate).

## Reports

[`REPORT-2026-07-26-29`](../reports/REPORT-2026-07-26-29.md) — Pass.
