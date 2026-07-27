# Verification Reports

Status: **29 reports filed** — all are functional or structural execution records; none yet closes a Story's complete performance, security, containment, and application-profile assurance contract.

## Purpose

A Report is the record that a Test actually ran, not just that it was specified. Every entry in [`goals/tests/`](../tests/) that has been implemented and executed at least once gets a corresponding Report here, linked back from the Test's "Reports" field.

Reports are also the only mechanism that can move a Story's [`assurance/story-contracts.tsv`](../assurance/story-contracts.tsv) state to `verified`. A functional pass alone does not do that.

## When a Report gets filed

- The first time a new Test passes in CI or on real hardware.
- Any time a previously-passing Test fails (regressions get a Report too — a Report is not only good news).
- At minimum once per Roadmap phase's Tier 1/2 hardware validation pass, per the [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix).

## Naming convention

`REPORT-YYYY-MM-DD-NN.md`, sequential within the date, mirroring the [`session/hand-YYYY-MM-DD/`](../../session/) dated-folder convention but for test execution records rather than design handovers.

## Report format

Each Report states:

- **Test(s) covered** — one or more `TEST-*` IDs.
- **Result** — pass / fail, with failure detail if applicable.
- **Environment/tier** — Tier 0 (QEMU/Renode), Tier 1 (Jetson Orin Nano Super), or Tier 2 (x86_64 mini-PC), per the Target Hardware & Test Matrix.
- **Toolchain/commit** — the exact Rust toolchain version and commit hash the test ran against, for reproducibility.
- **Linked session** — the `session/hand-*` handover (if any) this Report's finding was discussed in.
- **Assurance IDs** — selected `PERF-Dnn-Gnn`, `SEC-nn`, `BND-nn`, `PD-nn`, and `RCG-nn` IDs, each with pass/fail/deferred status. Deferral is visible release debt, never a pass.
- **Raw evidence** — samples/histograms/counters, image/link maps, peak-memory and allocation accounting, driver/capability manifest, hostile inputs, and observed safe failure state as applicable.
- **Security under load** — confirmation that signing, isolation, policy, provenance, fault containment, and RT reserves held at the measured performance point.

## Status

| Report | Test(s) | Result | Tier |
|---|---|---|---|
| [`REPORT-2026-07-26-01`](REPORT-2026-07-26-01.md) | `TEST-P0-01-01-A` | Pass | Tier 0 (QEMU) |
| [`REPORT-2026-07-26-02`](REPORT-2026-07-26-02.md) | `TEST-P0-01-03-A` | Pass | CI / Tier 0 |
| [`REPORT-2026-07-26-03`](REPORT-2026-07-26-03.md) | `TEST-P0-01-02-A` | Pass | CI |
| `REPORT-2026-07-26-04` through `REPORT-2026-07-26-20` | Phase-0 functional Stories | Pass (local) | Host and/or Tier 0 |
| [`REPORT-2026-07-26-21`](REPORT-2026-07-26-21.md) | `TEST-P0-07-01-A` local message IPC | Pass (functional; assurance debt remains) | Host |
| [`REPORT-2026-07-26-22`](REPORT-2026-07-26-22.md) | `TEST-P0-07-02-A` shared-memory grant | Pass (functional; assurance debt remains) | Host + Tier 0 |
| [`REPORT-2026-07-26-23`](REPORT-2026-07-26-23.md) | `TEST-P0-01-02-A` assurance-spine extension | Pass (structural; assurance debt remains) | Host/CI |
| [`REPORT-2026-07-26-24`](REPORT-2026-07-26-24.md) | `TEST-P0-07-02-A` transactional/generation-safe shared-memory grant | Pass (functional; assurance debt remains) | Host |
| [`REPORT-2026-07-26-25`](REPORT-2026-07-26-25.md) | `TEST-P0-01-02-A` five-class containment-contract extension | Pass (structural; assurance debt remains) | Host/CI |
| [`REPORT-2026-07-26-26`](REPORT-2026-07-26-26.md) | `TEST-P0-01-02-A` Security Charter/code-admission extension | Pass (structural; runtime charter debt remains) | Host/CI |
| [`REPORT-2026-07-26-27`](REPORT-2026-07-26-27.md) | `TEST-P0-04-02-A` IDT/local-APIC bring-up | Pass (functional; assurance debt remains) | Host + Tier 0 |
| [`REPORT-2026-07-26-28`](REPORT-2026-07-26-28.md) | `TEST-P0-01-02-A` whole-system/application context extension | Pass (structural; runtime and profile debt remains) | Host |
| [`REPORT-2026-07-26-29`](REPORT-2026-07-26-29.md) | `TEST-P0-04-03-A` read-only PCI bus-0 enumeration | Pass (functional; assurance debt remains) | Host + Tier 0 |

All 29 passed locally against the pinned `nightly-2026-07-26` toolchain; none has yet been observed passing in the GitHub Actions CI pipeline against this work. Reports 23, 25, 26, and 28 prove structural gates, not runtime controls, class isolation, remote-code prevention, or application compatibility, so all current functional Stories remain `baseline-debt` until reports contain the required raw assurance evidence.
