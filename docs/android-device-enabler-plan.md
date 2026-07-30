# Android Device Enabler — Plan of Action

**Status: plan, spec-level. No support claim, no platform claim.** Under rule 10 of
[`agent.md`](../agent.md), the platform rows in
[`goals/context/application-platforms.tsv`](../goals/context/application-platforms.tsv) and
[`goals/context/landing-zones.tsv`](../goals/context/landing-zones.tsv) are Phase 0 of
*implementation* — this document is the decided shape that those rows will encode, written
2026-07-30 at the owner's order, following the 02A review cover note. Origin question:
*"If an Android-based device is formatted and this OS is installed, and the camera, gyros,
sensors and the cellular device are used by the OS — how will the OS behave, and what do we
have to do to utilise the device as a node for inference / local model token generation?"*

## 1. The honest starting point

- TinyOS has **one target**: `x86_64-tinyos.json`, Tier 0 under QEMU (`hal-x86_64`).
  There is no ARM64 HAL. An Android device does not boot this OS today.
- [ADR 0005](adr/0005-arm64-real-time-tier-is-conditional-on-secure-world-qualification.md):
  the ARM64 **real-time tier is conditional on secure-world qualification**. Until a
  platform qualifies, no WCET number is quotable from ARM64 (`LE-09` discipline).
- After a port, the answer to "how will the OS behave" is: **safe but inert, by design.**
  Deny-by-default means the camera, gyros, sensors and radio are unusable until each has a
  driver, a Protection Domain classification, and explicit policy grants. A port that boots
  to a TINYCMD console with every peripheral refusing is the *correct* intermediate state,
  not a failure.

## 2. Device-class doctrine (fixed before any driver is written)

| Device | Classification | Non-negotiable rule |
|---|---|---|
| Camera frames, gyro/IMU, ambient sensors | External byte producers: labelled `origin=external`, quarantined until C4-inspected; data only | A malformed frame is a hostile input to the ISP/driver parser — adversarial tests, never happy-path (`SECURITY_CHARTER` hostile-input contract) |
| Cellular baseband | **Hostile co-processor**, not a peripheral: it runs its own proprietary OS regardless of what we install | IPC boundary + IOMMU between baseband and RAM. A baseband with free DMA is a standing zero-day; without an IOMMU on the path, the radio stays off. Extends `PD-*`/code-admission with an explicit DMA policy (02A §4.2 names this gap) |
| NPU / GPU | Proprietary-driver territory | Later, behind a boundary, or not at all. CPU-only inference first |
| Vendor firmware blobs (ISP, secure world, power) | Remote bytes | **Never admitted as code** (`RCG-*`). Where the SoC cannot function without them, they run outside the trust boundary and the boundary is documented per landing zone |
| Model weights | **Data, not code** | mmap'd read-only under W^X; weight-swap never touches the 14 code-admission gates — that is what makes a model update safe by construction |

## 3. Phases, each with its gate

**Phase 0 — Governance.** Platform rows (`android-arm64-node`) in
`application-platforms.tsv` + `landing-zones.tsv` with goal/performance/security/class/
horizon/claim-gate selections; Feature contracts in the assurance spine; guardrail rows for
tokens/sec, first-token latency, and inference-vs-RT interference (the `D23`-style shape the
shell rows use). An ADR choosing the reference hardware. Gate: `check-assurance-spine`.

**Phase 1 — ARM64 HAL, Tier 0 first.** `hal-arm64` crate (GIC, generic timer, UART —
including the RX path `LE-55` already demands), `aarch64-tinyos.json`, QEMU `virt` machine
fixtures mirroring the x86_64 catalogue's two-signal discipline. **Reference hardware is a
devboard with an open BSP (Jetson/RB5/RPi5 class), not a phone** — phones bury the baseband,
TrustZone and power management in unreviewable blobs; the phone is Phase 6, not Phase 1.
Gate: the regression sweep green on `qemu-aarch64`; ADR 0005 qualification evidence started.

**Phase 2 — Storage + mmap.** Flash/UFS (devboard: eMMC/SD) driver; read-only file mapping
in the existing address-space machinery; the `.rac`-substrate direction (mmap/pointer-access
weights, the Sharc.Blue-modelled Phase 6 runtime) becomes implementable here.
Gate: a fixture maps a seeded read-only blob, W^X seal holds, labels survive.

**Phase 3 — Sensor/camera drivers as labelled producers.** Each device lands as: driver →
C4 inspection of the byte stream → labelled data available to granted domains. Gyro/IMU
first (simplest frames, RT-relevant), camera last (ISP complexity). Every driver ships with
hostile-input adversarial tests and spoor-journaled grant/deny.
Gate: per-device `BND-*` boundary evidence; a hostile frame corpus that fails closed.

**Phase 4 — The inference domain.** A C3 domain owning the token-generation service:
bounded memory pools, WCET budget with the existing degrade/restart machinery so inference
**cannot starve the RT floor**; CPU quantized kernels (deterministic, no blobs); weights
mmap'd from Phase 2. Sensor/camera tensors arrive only as Phase 3 labelled data.
Gate: guardrail rows measured (tokens/sec, first-token latency) under concurrent RT load —
tested, never asserted (owner directive); interference row green.

**Phase 5 — Serving tokens as a fleet node.** Network path under the charter (HBP):
requests in as data, tokens out, capability/rate/action budgets, full spoors, kill switch
(`SEC-16` row). The node registers in the fleet-role plane per `whole-system-context.md`.
Gate: campaign-level hostile-load tests; every request/refusal spoor-attributable.

**Phase 6 — Phone bring-up proper.** Only now: bootloader unlock, vendor BSP audit,
TrustZone/secure-world qualification (ADR 0005's actual test), battery/thermal/power
guardrails, and the baseband IOMMU policy from §2 enforced on real silicon. Device-family
selection criteria: unlockable bootloader, documented IOMMU/SMMU, mainline-supported SoC,
baseband isolatable. Devices failing the criteria are not targets — a repurposed handset
that cannot isolate its radio is a data-centre-adjacent compute brick with the radio off,
and that is an acceptable, *stated* landing zone.

## 4. What this plan refuses

No Android compatibility layer (this is not "run Android apps"); no proprietary driver
admitted as code; no RT claim from ARM64 before secure-world qualification; no radio
without IOMMU; no performance number without a guardrail row and a measurement. The
zero-zero-day asymptote (02A §4) governs: every phase adds attack surface only behind the
same deny-by-default, labelled, spoor-audited seams the x86_64 tier already proves.
