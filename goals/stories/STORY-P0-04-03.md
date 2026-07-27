# STORY-P0-04-03 — Minimal Bus-Enumeration Pass

Status: **Verified**
Feature: [`FEAT-P0-04`](../features/FEAT-P0-04.md)
Introduced in: [`session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md`](../../session/hand-2026-07-26/08-cover-note-mvp-continuation-and-ns-file-access.md)
Implemented in: [`session/hand-2026-07-26/35-story-p0-04-03-pci-bus-enumeration.md`](../../session/hand-2026-07-26/35-story-p0-04-03-pci-bus-enumeration.md)

## Description

A minimal PCI(e) bus-enumeration pass — walking configuration space to discover attached devices and record them in the topology model `STORY-P0-04-01` established — sufficient groundwork for the class drivers `EPIC-P3` plans, without implementing any actual device driver here.

**Scope note (implementation):** devices are recorded into `hal::device::DeviceTable`, a new fixed-capacity sibling of `hal::topology::Topology` rather than a field grafted onto it — `Topology` is the CPU model with one reason to change, `DeviceTable` the device model with another, per Single Responsibility. Both are the same arch-neutral pattern (const-generic capacity, fail-closed `push`, no heap), and both are what "the topology model" means from `FEAT-P0-04`'s manifest perspective. Access is via legacy CAM (`0xCF8`/`0xCFC` port I/O), not MMIO ECAM — see *Named, not silently solved*.

## Depends on

`STORY-P0-04-01` (the topology model devices are recorded into).

## Acceptance criteria (final)

1. Enumeration under QEMU's `q35` model discovers at minimum the host bridge and any devices QEMU's default machine exposes, recorded with vendor/device ID and topology position (bus/device/function). **Met behaviorally** by the `fixture-pci-enumeration` Tier 0 fixture (`TEST-P0-04-03-A`): the host bridge is found at 0:0:0 with vendor `0x8086`, the discovered set is non-empty, and two complete walks produce identical tables; the recording shape itself (vendor/device id + bus/device/function, in discovery order) is proven by `hal_x86_64::pci`'s host tests against mock configuration spaces.
2. Enumeration is read-only against device configuration space at this stage — no driver bring-up, no device state mutation — keeping this Story's scope to discovery only, per Single Responsibility. **Met structurally**: the `ConfigSpace` trait the walk is generic over exposes exactly one operation, a 32-bit configuration *read* — there is no write method for any code built on it to misuse (Interface Segregation doing containment work). Host tests additionally observe that the walk touches only the vendor/device (`0x00`) and header-type (`0x0C`) dwords, and the Tier 0 fixture's identical-double-walk check is the closest behavioral witness that discovery mutated nothing. The one write the legacy-CAM backend issues — the `CONFIG_ADDRESS` register at `0xCF8` — is the selection mechanism itself, not device configuration state, and is documented as such at the `unsafe` boundary.

## Named, not silently solved

- **Legacy CAM only, no ECAM/MCFG.** MMIO ECAM access (q35's window at `0xB0000000`) would need MCFG table parsing plus a `boot.rs` identity-map extension; port-I/O CAM needs neither. ECAM lands with whatever Story first needs extended config space (offsets ≥ `0x100`), likely alongside `EPIC-P3`'s class drivers.
- **Bus 0 only, no bridge traversal.** Buses behind PCI-to-PCI bridges are not walked; q35's default machine exposes every device on bus 0, and the walk's per-spec multifunction handling is where recursion would slot in later.
- **Identity and position only.** No BAR, class-code, capability-list, or interrupt-line reads, and no I/O APIC device-IRQ routing (deferred from `STORY-P0-04-02`, still deferred here — it belongs to whatever Story first routes a real device interrupt, not to discovery).
- **Assurance state remains `baseline-debt`**: functional Green plus structural read-only-ness is not raw performance/security evidence (hostile-device campaigns, IOMMU confinement, latency tails). Per the assurance spine, a dated Report with that evidence closes those gates; this one doesn't claim to.

## Tests

`hal::device`'s `#[cfg(test)]` module (4 host tests: fail-closed capacity, discovery-order iteration, identity-and-position-only descriptor guard), `hal_x86_64::pci`'s `#[cfg(test)]` module (10 host tests: BDF validation, CAM packing, header decoding, and the walk against read-recording mocks), and the `fixture-pci-enumeration` Tier 0 QEMU fixture (`os/src/kernel/src/fixture_pci_enumeration.rs`), with bus-0 enumeration also wired into the real boot path's success gate in `os/src/kernel/src/main.rs`. See [`TEST-P0-04-03-A`](../tests/TEST-P0-04-03-A.md) and [`REPORT-2026-07-26-29`](../reports/REPORT-2026-07-26-29.md).

## Goals verified

G-HW-4; groundwork for `EPIC-P3`'s class drivers (not itself a G-HW-2/G-PA-4 Goal owner).
