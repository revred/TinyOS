# ADR 0013 — Zero-Copy Buffer Sharing Is Conditional on Per-Platform DMA-Containment Qualification

Status: **Accepted**
Date: 2026-08-01
Introduced in: [`session/hand-2026-08-01/01A-tinytile-planning-session-mandate.md`](../../session/hand-2026-08-01/01A-tinytile-planning-session-mandate.md) §6.1, which named the tension and predicted this resolution
Shape borrowed from: [`ADR 0005`](0005-arm64-real-time-tier-is-conditional-on-secure-world-qualification.md) — a capability quoted only from a qualified platform, defaulting to "not qualified", never "presumed clean"
Governs: the Unified Memory Manager (`G-HW-6`) and every `EPIC-P6B` Feature that shares a buffer with a bus-mastering device

## Context

Two commitments this repository has already made are in direct tension, and `EPIC-P6B` cannot be
decomposed until the tension is ruled on:

- **`PD-10`** requires device DMA to be constrained — *"by IOMMU or wiped bounce buffer"*. This is
  a safety-and-containment invariant: a bus-mastering device that can read or write arbitrary
  physical memory dissolves every Protection Domain on the board from below.
- **`G-HW-6`** promises zero-copy CPU/GPU buffer sharing through the UMM — whose explicit purpose
  ([`docs/inference-architecture.md`](../inference-architecture.md)) is to remove redundant copies
  on unified-memory hardware.

On hardware where no IOMMU/SMMU is available to TinyOS — not present, not documented, or owned by
firmware TinyOS cannot displace — `PD-10` forces exactly the copy (or wipe) that `G-HW-6` exists to
remove. Handing a device a raw pointer into shared physical memory *is* the unconstrained DMA
`PD-10` forbids. Both cannot be unconditional. One of them has to bend, and `PD-10` is a safety
property while `G-HW-6` is a performance property.

## Decision

**1. `PD-10` governs. It is never traded for throughput.** Safety before security before
correctness before performance; a zero-copy claim on hardware whose DMA is unconstrained is a
safety violation wearing a performance number.

**2. Zero-copy buffer sharing is a per-platform qualified capability, not an architecture or API
property.** The UMM may hand a device a mapping into CPU-visible memory only on a platform holding
a current **DMA-containment qualification record**: a dated Report under
[`goals/reports/`](../../goals/reports/) naming the platform and firmware version, stating which
IOMMU/SMMU constrains the device, how TinyOS programs or verifies that constraint, and carrying a
**positive control** — the instrument must be shown to *detect* a containment violation (a
deliberately out-of-bounds device access that faults) before its "no violation observed" is
believed. A record is void for any other firmware version. The default for an unqualified platform
is "not qualified", never "presumed clean", and **as of this ADR the count of qualified platforms
is zero** — including the Jetson Orin Nano Super, whose SMMU exists but has never been programmed,
read, or exercised by this project.

**3. The explicit-copy/wiped-bounce-buffer path is the honest default**, and it is not a stub: it
is the production configuration for every unqualified platform, sized and measured as such. The
UMM's handle API already abstracts the two paths (`inference-architecture.md` records this);
callers cannot observe which path they are on except through the telemetry that reports it.

**4. Every performance claim states its path.** A throughput or latency figure obtained on a
qualified zero-copy platform is not quotable for the copy path, and vice versa. Reports name the
path, the platform, and the qualification record alongside the number, per `ADR 0005`'s
discipline.

## Rationale

- **The alternative that fails silently is worse.** Without this ruling, the first unified-memory
  benchmark becomes the quoted number, the number becomes an implied property of TinyOS, and the
  property is false on every board without a qualified IOMMU — undetectably, until a hostile DMA
  transaction demonstrates it.
- **It preserves both commitments at their honest strength.** `G-HW-6` is not weakened — it is
  scoped to where it is true. `PD-10` is not softened — it is enforced by construction on the
  default path.
- **The qualification record is the same commercial asset `ADR 0005` identified.** Dated,
  firmware-versioned DMA-containment evidence per platform is what a buyer deploying into medical,
  UAV, or data-centre settings actually needs, and no consumer-OS vendor hands it over.

## Consequences

- The UMM API needs no change; the ruling constrains configuration and claims, not the handle
  contract.
- `EPIC-P6B`'s device-backend Features inherit a per-platform qualification obligation before any
  zero-copy configuration ships, and their performance Stories plan copy-path evidence first.
- The HBP-brokered accelerator stage (`EPIC-P6B` §driver) sidesteps this ADR rather than
  satisfying it — the accelerator's DMA is contained by the broker host's own kernel, not by
  TinyOS — and evidence from that stage says so.
- A platform can qualify later, and be dequalified by a firmware update, without amending this ADR.
