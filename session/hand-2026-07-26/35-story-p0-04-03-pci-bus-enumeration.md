# Handover 35 — `STORY-P0-04-03` Implemented and Verified: Read-Only PCI Bus-0 Enumeration; `FEAT-P0-04` Complete; Every EPIC-P0 Story Functionally Verified

Follows: [`33-security-charter-review-and-next-steps.md`](33-security-charter-review-and-next-steps.md) (whose mandate this executes) and, concurrently, [`34-whole-system-context-and-application-flight-plan.md`](34-whole-system-context-and-application-flight-plan.md) (a parallel session that landed while this Story was in flight — see *Concurrency notes* below).

## What this session did

Executed Handover 33's priority 1: **`STORY-P0-04-03`** (minimal bus-enumeration pass), the last open `FEAT-P0-04` Story — implemented, Tier 0-verified under QEMU `q35`, and documented through the full assurance-spine artifact chain.

### Code

- **`os/src/hal/src/device.rs`** (new): arch-neutral `DeviceDescriptor` (vendor id, device id, bus/device/function — *identity and topology position only*, with a size-guard test making authority-shaped field creep a conscious act) and fixed-capacity, fail-closed `DeviceTable<N>`, mirroring `topology::Topology`'s exact pattern. 4 host tests.
- **`os/src/hal-x86_64/src/pci.rs`** (new): validated `BdfAddress` newtype, legacy-CAM `CONFIG_ADDRESS` packing, header-field decoding, and `enumerate_bus_zero` — the whole walk generic over a **read-only `ConfigSpace` trait with exactly one method, a config-dword read**, so nothing built on it can mutate device state (Interface Segregation as containment). Per-spec multifunction handling: functions 1–7 probed only when function 0's header-type byte says multifunction. The only target-gated piece is `PortCam`, the `0xCF8`/`0xCFC` port-I/O backend. 10 host tests against read-recording mocks (including "only offsets `0x00`/`0x0C` are ever read" and cross-walk determinism).
- **`os/src/kernel/`**: `capacities::MAX_PCI_DEVICES` (64, budget-checked), `fixture_pci_enumeration.rs` (Tier 0 fixture: host bridge at 0:0:0 with vendor `0x8086`, non-empty discovery, identical double walk), the `fixture-pci-enumeration` feature wired through all four of `main.rs`'s cfg lists, and **bus-0 enumeration added to the real boot path's success gate** after `interrupts::init`, exactly as ACPI topology discovery was for `STORY-P0-04-01`.
- **`os/src/xtask/`**: `--fixture=pci-enumeration` arm; `committed_assurance_spine_is_complete` expectations bumped to **24 Tests / 29 Reports**.

### Documents

- [`TEST-P0-04-03-A`](../../goals/tests/TEST-P0-04-03-A.md) (new), [`REPORT-2026-07-26-29`](../../goals/reports/REPORT-2026-07-26-29.md) (new, + reports README row/count), Story → **Verified**, Feature → **Complete — 3/3**, traceability matrix, dashboard (25/25 Stories, 24/24 Tests, `FEAT-P0-04` VERIFIED (local)).
- **Contract correction**: `story-contracts.tsv`'s pre-written `STORY-P0-04-03` row selected `D08`/`SEC-03` (paging/address-space isolation — irrelevant to a read-only discovery walk) with an ARM64 copy-paste rationale. Corrected to `D01,D22` / `SEC-13,SEC-18,SEC-19,SEC-20` / `C0,C1,C2` at `baseline-debt`, matching `STORY-P0-04-01`'s hardware-discovery selections — `SEC-13` because "discovery must not select or grant a driver" is precisely its invariant.

### Verification (full sweep, this session)

- `cargo test --workspace --lib`: **172/172** (`exec` 51, `hal` 8, `hal-x86_64` 47, `kernel` 66).
- `cargo test -p xtask`: **23/23**.
- `xtask qemu-x86_64 --fixture=pci-enumeration`: **pass** (first run, no bring-up bugs); default real boot with the new PCI gate: **pass**.
- Regression sweep over every pre-existing QEMU fixture: all unchanged, including `broken-boot` and `idt-apic-unrouted` still correctly exiting Failure. **Exception:** `--fixture=pool-bench` (added by the concurrent session 34 while this Story was in flight) exits harness-error 2 ("QEMU exited with unexpected code 0") — pre-existing relative to this Story's changes, which touch nothing on its path; left for its owning session rather than silently absorbed.
- `cargo fmt --check` / `cargo clippy --workspace --lib -- -D warnings`: clean.
- `check-crate-sizes` / `check-image-size`: pass (kernel image 18,024 bytes of 8 MiB).
- `check-assurance-spine`: 8 Features, 25 Stories, **24 Tests**, **29 Reports**, 5 classes, 20 boundary tests, 20 controls, 14 PD, 14 RCG, 25 pairs, 19 application targets, 9 landing zones, 1,025 Story/performance contracts, 6,575 application/performance contracts.
- `check-performance-catalogue`: 625/625.

## Named, not silently solved (this Story's own list, restated)

Legacy CAM only (no ECAM/MCFG — q35's `0xB0000000` window is outside `boot.rs`'s identity maps and nothing yet needs extended config space); bus 0 only (no bridge traversal); identity/position only (no BARs, class codes, capability lists, interrupt lines); I/O APIC device-IRQ routing still deferred to whatever Story first routes a real device interrupt. Assurance state remains **`baseline-debt`** — functional Green plus structural read-only-ness is not hostile-device/IOMMU/latency evidence.

## Concurrency notes

This session ran alongside Handover 34's whole-system-context session in the same working tree. Report/handover numbering was rebased mid-flight when that session claimed `REPORT-2026-07-26-28` and Handover 34 (this Story's artifacts are therefore `-29` and 35). The `pool-bench` fixture observation above is that session's work, not a regression from this one. The workspace test totals quoted here include both sessions' code as of this session's final sweep.

## Immediate next steps (Handover 33's list, updated)

1. ~~`STORY-P0-04-03`~~ — **done** (this session). `EPIC-P0` now has every Story functionally Verified; what remains phase-wide is assurance evidence, not functional scope.
2. **Real CPU exception handling** — unchanged priority: a genuine `#PF`/`#GP` handler that can resume or terminate the faulting context, which the Security Charter's "active per-task address spaces" and "teardown" runtime-evidence items both depend on.
3. **Active per-task `CR3` switching** — only after (2), per Handover 32's reasoning.
4. **Pick off the charter's runtime-evidence list one item at a time** — unchanged.
5. **`pool-bench` fixture** — the concurrent session's QEMU harness path for it currently exits harness-error 2; whoever continues that thread should make it report through the isa-debug-exit protocol like every other fixture, or document why it can't.
