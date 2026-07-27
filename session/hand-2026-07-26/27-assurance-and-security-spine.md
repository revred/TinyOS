# Handover 27 — 625 Performance Tests Integrated as the Assurance Spine; Sandbox-First Security and Fable-Class Threat Model Added

Follows: [`26-feat-p0-07-local-ipc.md`](26-feat-p0-07-local-ipc.md).

## Direction received

The user found that the 625 performance tests were being treated as a bolt-on rather than the OS's implementation spine, and expanded the security objective to cover unsigned executable hijacking, cross-process memory tampering, illegal shared memory, uncontained TCP/IP/ports, missing file provenance/entitlement, lack of sandbox-first policy, browser/download attacks, ransomware, worms, cookies/tracking, and frontier AI-assisted attacks.

## What changed

1. **Made assurance mandatory before implementation.** Added `goals/assurance/story-contracts.tsv`, mapping every one of the 25 current Story files to performance domains and security controls. Selecting one domain selects all 25 guardrails for that Story. Current mappings expand to 1,025 Story/performance contracts.
2. **Added a lifecycle with honest debt.** Define → Red → Green → Measure → Claim → Release. Functional `Verified` and assurance `verified` are distinct. All 23 current functional-Verified Stories are `baseline-debt`; the two planned Stories are `specified`; zero Stories are presently assurance-verified.
3. **Added a machine-enforced spine gate.** New `xtask check-assurance-spine` validates the complete 625-cell catalogue, exactly 20 security controls, exact Story/Feature coverage, Test-to-Story resolution, Report-to-Test/Story linkage, IDs, states, and stale references. CI now runs it. `xtask` has four new assurance tests; 15/15 total pass. The governance change itself is traced as an extension to foundational `STORY-P0-01-02`/`TEST-P0-01-02-A`, with `REPORT-2026-07-26-23`; it does not sit outside the spine it enforces.
4. **Added 20 canonical security release controls.** `goals/security/controls.tsv` covers verified boot; signed TXE/TON provenance; per-process address spaces; revocable shared memory; sandbox-first least authority; origin/entitlement; storage and network namespaces; browser/parser and download quarantine; privacy partitioning; ransomware/worm containment; opt-in drivers; ACI/spoor provenance; secrets; Fable-class campaigns; signed atomic updates; IOMMU; memory-safe boundaries; and resource exhaustion.
5. **Defined the security architecture.** `docs/security-spine.md` maps the user's attack patterns to falsifiable invariants. Optional stacks must be measurably absent: zero linked bytes, interrupts, DMA/MMIO grants, capabilities, queues, worker tasks, listeners, and parser entry points.
6. **Defined “Fable-class” precisely.** It is a TinyOS project-defined frontier AI adversary—not an industry certification—capable of long-horizon autonomous reconnaissance, vulnerability discovery/validation, exploit chaining, lateral-movement planning, adaptive retries/rewinds, tool use, and parallel probing. Provider-side classifiers are explicitly not an OS trust boundary. The definition is grounded in Anthropic's public Fable 5 capability and safeguard material.
7. **Audited the code against all 20 controls.** `goals/security/current-state-review.md` records foundations and blockers without inflating claims. Notable blockers: the broad boot RWX identity map; page tables built but no active per-task CR3; incomplete context security state; no IDT/sandbox fault containment; unsigned TXE; no verified boot/ACI/IOMMU/origin/quarantine/update/secret/campaign implementation.
8. **Audited the new local IPC code after its functional V&V landed concurrently.** `STORY-P0-07-01`/`-02`, their Tests, Reports 21/22, and the shared-memory QEMU fixture are functionally Verified. The assurance review still names missing transactional grant rollback, task-generation safety, task-exit revocation, active address spaces, real ACI, and the `AllowAllPolicy` stand-in. Both Stories correctly remain `baseline-debt`.
9. **Corrected dashboard counting errors and merged the concurrent IPC state.** The repository has 25 Story files, not the earlier claimed 26. With IPC verified, functional status is 23/25 Stories and 22/22 Test documents, separate from 0 assurance-verified Stories.
10. **Integrated the spine into governing documents.** Updated `agent.md`, `agent/CODING_STANDARDS.md`, `SeedMVP.md`, root `README.md`, `goals/index.html`, traceability, report schema, performance catalogue documentation, EPIC-P0, and the backlog. Performance/security contracts now govern Story creation, TDD shape, Reports, claims, and release.

## Verification

- `cargo test --workspace --lib`: **140/140 passed** (`exec` 47, `hal` 4, `hal-x86_64` 30, `kernel` 59).
- `cargo test -p xtask`: **15/15 passed**.
- `cargo clippy --workspace --lib -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo run -p xtask -- check-performance-catalogue`: 625/625 cells valid.
- `cargo run -p xtask -- check-assurance-spine`: 25 Stories, 22 Tests, 23 Reports, 20 security controls, 1,025 selected Story/performance contracts.
- `cargo run -p xtask -- check-crate-sizes --ceiling=20000`: all crates pass (`exec` 1,847; `hal` 91; `hal-x86_64` 788; `kernel` 1,796; `xtask` 1,227 production lines).
- `cargo run -p xtask -- check-image-size --ceiling=8388608`: kernel release ELF 16,032 bytes.

## What this does not claim

- No performance catalogue row has runtime assurance evidence yet; all 625 remain specified.
- No current Story is assurance-verified.
- The OS is not yet sandboxed, verified-booted, ransomware-proof, Fable-class resistant, or proven better than Linux/RTOS baselines.
- Structural catalogue integrity is not a substitute for controlled Tier-1/Tier-2 measurements or adversarial HIL reports.
- The local IPC implementations are functionally Verified, but that does not close D12/D13 or their mapped security controls.

## Immediate next steps

1. Add follow-up Stories for shared-grant transactional rollback, generation-safe task-exit revocation, and production default-deny policy; the current IPC Stories are functionally complete to their written scope but cannot close assurance without these protections.
2. Prioritize the real isolation spine: IDT/fault containment, active per-task CR3, complete security context switching, guard pages, removal of boot RWX mappings, and task-exit teardown.
3. Define and test signed TXE metadata before any real loaded entry point executes.
4. Build the monotonic clock/PMU raw-record ABI so Phase-0 baseline debt can begin closing without relying on noisy host timing.
5. Add negative link-map/surface checks before optional driver, network, storage, browser/parser, and inference stacks arrive.
