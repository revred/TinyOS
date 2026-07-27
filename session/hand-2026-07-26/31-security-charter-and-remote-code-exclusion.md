# Session 31 — Security Charter and remote-code exclusion integrated

Date: 2026-07-26

## Outcome

TinyOS now has a root-level [`SECURITY_CHARTER.md`](../../SECURITY_CHARTER.md) governing process isolation, cross-class communication, remote-code admission, takeover containment, frugality, and release evidence.

The charter makes two outcomes load-bearing:

1. compromise of one C2, C3, or C4 process is insufficient to compromise another process or the system;
2. remote/external bytes have exactly one path to executable state, and every stage fails closed.

“Iron clad” is defined as an exhaustively enumerated, machine-checked, adversarially evidenced path—not a claim that defects or shared-hardware side channels are impossible. Runtime assurance remains `baseline-debt`.

## Canonical contracts

- `goals/security/protection-domain-contracts.tsv` defines `PD-01..PD-14`: private active memory, kernel-derived identity, empty authority, executable sealing, bounded IPC, generation-safe sharing, temporal/resource isolation, caller-funded broker work, device isolation, provenance, fault containment, revoke-before-reuse, and no ambient namespace.
- `goals/security/code-admission-gates.tsv` defines `RCG-01..RCG-14`, the only permitted path from data-only ingress through C4 quarantine/parsing/signature/policy to a fresh, sealed, empty-authority C3 domain.
- `goals/security/class-communication-matrix.tsv` defines every one of the 25 ordered C0–C4 pairs as denied, one-shot handoff, kernel-internal, or C1-mediated, with an explicit authority-transfer and failure rule.

Together, the `PD-*` and `RCG-*` catalogues select all 20 `SEC-*` controls and all 20 `BND-*` tests. The assurance checker rejects any disconnected control or boundary.

## Integral assurance-spine wiring

- Every one of the eight `feature-contracts.tsv` rows now selects exact `PD-*` and `RCG-*` obligations.
- Every one of the 22 Test documents repeats those exact selections.
- `xtask check-assurance-spine` compares Test metadata against the parent Feature, requires all 14+14 charter rows to be Feature-owned, validates the exact 25-pair matrix, and requires the root Charter to reference its canonical catalogues and honest assurance state.
- `agent.md`, `CODING_STANDARDS.md`, `README.md`, `SeedMVP.md`, `docs/security-spine.md`, the assurance/security READMEs, the current-state audit, traceability matrix, and goals dashboard now treat the Charter as governing rather than advisory.

## Remote surface hardening in the specifications

- Deploy transports may stage only immutable non-executable C4 data. `deployer` cannot activate bytes, write process memory, add a trust root, reset rollback state, or bypass `RCG-*`.
- Hot deploy uses a fresh admitted domain, typed untrusted state transfer, exclusive-authority revocation before regrant, and fresh last-known-good recreation rather than stale-domain resurrection.
- Core updates become eligible only through independent C0 signature/revocation/anti-rollback verification and A/B recovery.
- HBP and WCI explicitly have no eval, arbitrary shell, script/native payload, process-write, executable-map, driver-load, raw-syscall, or trust-root operation.
- Production profiles expose no W→X/X→W transition, writable executable alias, JIT exception, in-place patch/promotion, remote debugger write, or remote trust-root enrollment.

## Current runtime truth

The Charter is a construction and release contract, not a claim that the current Phase-0 runtime already enforces it. Active per-task `CR3`, IDT fault containment, production capability spaces, signed TXE/TON admission, quarantine/provenance, executable sealing, IOMMU isolation, task-exit teardown, immutable updates, and hostile campaign evidence remain incomplete. [`goals/security/current-state-review.md`](../../goals/security/current-state-review.md) states these blockers explicitly.

## Structural evidence

- `cargo test -p xtask`: 21/21 pass.
- `cargo run -p xtask -- check-performance-catalogue`: 625 cells.
- `cargo run -p xtask -- check-assurance-spine`: 8 Features, 25 Stories, 22 Tests, 14 Protection Domain contracts, 14 code-admission gates, 25 class paths, 20 controls, 20 boundaries, and 1,025 selected performance contracts.
- [`REPORT-2026-07-26-26`](../../goals/reports/REPORT-2026-07-26-26.md) records the structural result and keeps all runtime claims deferred.

The final workspace validation result is recorded in the completing assistant response; unrelated in-progress APIC/IDT formatting is not rewritten by this charter session.
