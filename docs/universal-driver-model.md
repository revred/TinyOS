# Universal Driver Model (UDM) — Draft Spec

Status: **draft / spans Roadmap Phase 0 (HAL) through Phase 3 (Connectivity)**

## Purpose

TinyOS targets x86_64 (Intel/AMD PC-class chipsets) and ARM64 (Jetson-class SoCs, generic ARM64 boards, and — with an explicit caveat below — Apple Silicon) per the [64-bit-only hardware policy](../README.md#4-runs-where-the-work-happens--64-bit-only). Getting driver support across that spread with "little to no friction" requires treating drivers as a designed contract, not an emergent property of kernel internals — which is where Windows, Linux, and macOS each went wrong in a different way (see the design rationale in [`README.md`](../README.md)). This document specifies the Universal Driver Model that avoids repeating those mistakes.

## Lessons encoded into the design

| Mistake | Where it happened | UDM response |
|---|---|---|
| Kernel-mode drivers with unrestricted access crash the whole system | Windows (historically the majority of OS crashes traced to driver bugs) | Drivers run outside the RT kernel's trust boundary by default; a driver fault can never take down the scheduler |
| No stable internal driver ABI, forcing out-of-tree modules that rot | Linux | A versioned, stable **Driver Capability Interface (DCI)** is the only contract a driver is written against — kernel internals can change freely behind it |
| Userspace isolation done right, but paired with closed hardware/no public interfaces | macOS / Apple Silicon | UDM adopts the userspace-isolation *pattern* without the vendor lock-in — but see the Apple Silicon section for the real, non-negotiable constraint this doesn't solve |
| Vendor drivers required even for basic device function | All three, to varying degrees | Mandatory **class drivers** cover common device classes generically; vendor extensions are additive, never required for baseline function |

## Architecture

```text
┌───────────────────────────────────────────────────────────────┐
│                   Vendor Driver (userspace)                   │
│   Implements the DCI trait for its device class + optional    │
│                 vendor extension capabilities                 │
└───────────────────────────────┬───────────────────────────────┘
                                │  Driver Capability Interface (DCI)
                                │  (versioned, stable, admission-controlled)
┌───────────────────────────────▼───────────────────────────────┐
│               Universal Driver Interface (UDI)                │
│ Class contracts: storage, network, HID, display, GPU/compute, │
│          CAN, sensor, ... — one Rust trait per class          │
└───────────────────────────────┬───────────────────────────────┘
                                │  DMA/IRQ/MMIO grants, capability-scoped
┌───────────────────────────────▼───────────────────────────────┐
│               Hardware Abstraction Layer (HAL)                │
│   Bus enumeration (PCIe, USB, platform/MMIO, CAN) + unified   │
│ hardware manifest (ACPI/Device Tree normalized to one model)  │
└───────────────────────────────────────────────────────────────┘
```

### Userspace-first driver isolation

- Drivers do not run inside the RT kernel's trust boundary. A driver is a resource-budgeted task, admitted and scoped through the ACI capability registry exactly like any other caller — the same pattern already used for the shell, the LLM agent, and remote callers over HBP/WCI (one gate, many callers, no bypass).
- A driver requests specific capabilities in its manifest — a DMA region, an IRQ line, an MMIO range, a bus address — and is granted only those, never blanket physical memory access. A misbehaving or malicious driver is contained by construction, not by discipline.
- A crashing driver is restarted or failed cleanly through the same hot-deploy/health-check machinery specified in [`docs/deploy-protocol.md`](deploy-protocol.md) — it never faults the kernel, and it never blocks an RT task waiting on it (consistent with [Non-Negotiable #1](../README.md#non-negotiables)).
- Only the thinnest possible bus-enumeration and interrupt-routing code lives inside the trusted HAL layer; everything device-specific lives outside it.

### Driver Capability Interface (DCI) — the stable contract

- The DCI is versioned independently of kernel internals. A driver written against DCI v1 keeps working across kernel releases as long as v1 remains supported — this is the direct fix for Linux's out-of-tree rot and Windows' ABI-break churn.
- The DCI is a Rust trait-based contract per [`CODING_STANDARDS.md`](../agent/CODING_STANDARDS.md#language-policy); vendor code that must call a C SDK is confined to an isolated `-sys` crate behind it, same as any other vendor binding.
- Breaking changes to a class contract require a new major DCI version, published alongside the old one for a defined deprecation window — drivers are never broken by a routine kernel update.

### Class drivers vs. vendor extensions

- Every common device class — mass storage, network (Ethernet/WiFi), HID, display/framebuffer, GPU compute, CAN, generic sensor — has a **mandatory generic class driver** implementing the baseline UDI contract for that class. Plugging in a device that identifies as that class works out of the box, at baseline functionality, with zero vendor driver installed. This mirrors what already works well in USB HID/Mass Storage class drivers industry-wide, generalized to every class TinyOS supports.
- A **vendor extension** is an additive capability layer on top of the class driver — advanced GPU features, vendor-specific sensor modes, higher-performance DMA paths — never a replacement requirement. Removing a vendor extension degrades a device to its class-driver baseline, it never breaks it.
- This is the direct answer to "little to no friction": friction is defined as *needing a vendor driver just to get basic function*, and the class-driver requirement eliminates that by construction.

### Unified hardware manifest

- x86_64/Intel-chipset hardware describes itself via **ACPI**; ARM64 platforms typically describe themselves via **Device Tree** (or SBSA/EBBR-style standardized boot firmware on server-class ARM). The HAL normalizes both into one canonical TinyOS hardware topology model at boot, so class drivers and the UDI never branch on "which firmware description format did this board use."
- Where a target platform has neither a standard ACPI table nor a Device Tree (common on some embedded boards), a static hardware manifest can be supplied at build/deploy time instead — declarative, versioned, and reviewed the same way any other config artifact is.

## The Apple Silicon constraint (stated plainly)

Apple Silicon Macs do not publish public hardware interface documentation, stable firmware descriptor tables, or a sanctioned third-party driver path — this is a deliberate platform policy, not a technical gap TinyOS's architecture can design around. The realistic position:

- The UDM's class-driver/DCI/HAL-normalization architecture is *necessary* for eventual Apple Silicon support, but not *sufficient* — it still requires either Apple publishing stable interfaces, or the kind of multi-year, community-driven reverse-engineering effort seen in other third-party-OS-on-Apple-Silicon projects.
- TinyOS should not promise "little to no friction" on Apple hardware in the same breath as Intel/ARM-generic hardware. The honest scope: **first-class support for Windows-PC-class (Intel/AMD, ACPI) and ARM-generic (Device Tree, SBSA/EBBR — Jetson and comparable boards) hardware**, with Apple Silicon explicitly tracked as a **best-effort, community-dependent target** rather than a committed one, until the underlying documentation gap changes.
- This should be reflected in the [Target Hardware & Test Matrix](../README.md#target-hardware--test-matrix) rather than glossed over — overselling cross-platform parity here would be the same mistake described in the table above, just made by TinyOS instead of by an existing OS vendor.

## Conformance & trust

- Every class driver and every vendor extension ships with a conformance test suite for its class contract, built test-first per [`CODING_STANDARDS.md`](../agent/CODING_STANDARDS.md#test-driven-development-mandatory) — this is TinyOS's equivalent of driver certification (à la WHQL), but automated, open, and re-run on every relevant change rather than a one-time gate.
- A driver's granted capability scope (DMA regions, IRQ lines, MMIO ranges) is logged with the same provenance discipline as every other ACI-gated action — what was requested, what was granted, and why.
- Adversarial tests are required for the DCI/UDI boundary itself: a driver attempting to request capabilities outside its declared manifest, or a malformed capability grant, must fail closed.

## Open questions

- Exact DCI versioning/deprecation window policy.
- Hot-swap semantics for a driver mid-use by an active RT task (e.g. a storage driver being updated while a file is open) — likely needs a quiesce/drain step analogous to the deploy protocol's health-check gating.
- Whether GPU-class drivers reuse the admission-control model from [`docs/inference-architecture.md`](inference-architecture.md) directly, or need a distinct capability class of their own given how different GPU workloads are from storage/network I/O.
- Formal decision on Apple Silicon: tracked as best-effort indefinitely, or dropped from the target list entirely pending a specific trigger (e.g. Apple publishing documentation, or a mature community reverse-engineering baseline).

## Status

This document accompanies the `/hal` and `/drivers` components in the [Repository Layout](../README.md#repository-layout-planned) and extends Roadmap Phase 0 (HAL) and Phase 3 (Connectivity). It should be read alongside [`docs/deploy-protocol.md`](deploy-protocol.md) (driver hot-deploy reuses the same health-check/rollback discipline) and [`docs/inference-architecture.md`](inference-architecture.md) (GPU driver admission control).
