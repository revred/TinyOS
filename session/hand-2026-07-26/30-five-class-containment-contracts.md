# Session 30 — Five-class containment contracts integrated

Date: 2026-07-26

## Outcome

The five containment classes are now part of TinyOS's existing Goal → Epic → Feature → Story → Test → Report assurance spine rather than a parallel security note:

- `C0` Root of Trust;
- `C1` Trusted Kernel Core;
- `C2` Isolated System Service;
- `C3` Sandboxed Application;
- `C4` Hostile Transient Domain.

Containment class is explicitly independent of capability authority, scheduling criticality, and provenance. Drivers remain assumed-compromisable C2 services; a human is a principal rather than a class; C4 promotion is destroy/verify/recreate, never an in-place trust change.

## Contract apparatus

- Added `goals/security/containment-classes.tsv`: five canonical launch/input/failure/evidence contracts.
- Added `goals/security/containment-tests.tsv`: 20 canonical `BND-*` adversarial contracts covering boot handoff, runtime re-entry, privileged-parser absence, memory and W^X isolation, driver/DMA containment, launch authority, quarantine/promotion, provenance, IPC, exhaustion, scheduling orthogonality, spoors, negative surface, AI campaigns, and single-defect containment.
- Added `goals/assurance/feature-contracts.tsv`: exact coverage for all eight Phase-0 Features, including implementation/subject classes, authority posture, hostile inputs, selected `BND-*` tests, and evidence.
- Extended every row of `story-contracts.tsv` with containment classes.
- Extended every `SEC-*` control with containment classes and class-specific evidence.
- Added a `Containment contract` section to every current Feature.
- Added exact mapped performance/security/class/boundary/state metadata to all 22 current Test documents.

## Governance enforcement

`xtask check-assurance-spine` now rejects:

- anything other than the exact `C0..C4` catalogue;
- anything other than the exact `BND-01..BND-20` catalogue;
- unknown class, security, boundary, or performance references;
- missing or stale Feature containment contracts;
- a boundary test selected by no Feature;
- a Story selecting a class outside its parent Feature's implementation/subject classes;
- a Story security control with no applicable Story class.
- a Test document whose performance, security, class, boundary, or state metadata differs from its Story/Feature contract.

The governance Story/Test was revised and `REPORT-2026-07-26-25` records the structural result. This is not runtime containment evidence.

## Current runtime truth

The design is ready for decomposition and red-first implementation, but the class boundaries are not active:

- C0 lacks a hardware trust anchor and verified measured boot.
- C1 retains a broad RWX identity map; per-task CR3 switching and IDT fault containment are inactive.
- C2 has no isolated service/driver process or IOMMU grant lifecycle.
- C3 has no active sandbox lifecycle, signed manifest, real ACI policy, or teardown.
- C4 has no quarantine, disposable parser process, brokered output, or recreate-only promotion.

The next implementation work should remain `STORY-P0-04-02`'s IDT/APIC/fault foundation plus active address-space switching and task teardown, because those mechanisms make the C1 boundary real and unblock C2–C4 containment testing.

## Validation

- `cargo fmt --all -- --check`
- `cargo test -p xtask`: 18/18
- `cargo run -p xtask -- check-performance-catalogue`: 625 cells
- `cargo run -p xtask -- check-assurance-spine`: 8 Features, 25 Stories, 22 Tests, 25 Reports, 5 classes, 20 boundary tests, 20 controls, 1,025 selected performance contracts
- `cargo test --workspace --lib`
- `cargo clippy --workspace --lib -- -D warnings`
- `git diff --check`

Runtime security assurance remains `baseline-debt` until dated raw Reports execute the mapped `PERF-*`, `SEC-*`, and `BND-*` gates on the required tiers.
