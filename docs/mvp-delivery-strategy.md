# MVP Delivery Strategy and Cargo Workspace Structure

Status: **draft — the concrete "how" behind the Roadmap phases and Section 10 (Roadmap Alignment) of the seed specification**

## Purpose

Every other document in this repository specifies *what* TinyOS must do and *why*. This document specifies *how the code gets built*: the actual Cargo workspace layout, which crate gets created in which Roadmap phase, and the delivery sequencing that gets from an empty repository to the [5-axis CNC flagship MVP demonstration](physical-ai-reference-workloads.md) without ever violating [`agent/CODING_STANDARDS.md`](../agent/CODING_STANDARDS.md) along the way — the crate-size ceiling and TDD mandate apply from the very first commit, not retroactively once "real" development starts.

## A naming collision, resolved

The README's planned Repository Layout (written before the `agent/` guidelines folder existed) used `/agent/` as the code crate housing local LLM runtime integration. That now collides with `agent/CODING_STANDARDS.md` at the repository root. This document resolves it: **all Rust workspace crates move under a `crates/` directory**, and the LLM-integration crate is renamed **`crates/inference/`** (matching the "Agentic Inference" goal category name already used in [`TinyOS26thJulySeedMVP.md`](../TinyOS26thJulySeedMVP.md#32-agentic-inference-goals) §3.2) rather than sitting at `crates/agent/`, so there is no ambiguity anywhere in the tree between "the folder with development guidelines" and "a folder full of Rust source." The README's Repository Layout section is updated to match this document rather than the other way around.

## Top-level repository structure

```text
/agent/                 Development guidelines: CODING_STANDARDS.md (this is NOT a code crate)
/crates/                All Rust workspace members (kernel, HAL, drivers, ACI, shell, ...)
/targets/               Custom Rust target-spec JSON files for bare-metal x86_64/ARM64 builds
/xtask/                 Host-side build/test/QEMU-launch/deploy orchestration (a Rust binary, not shell scripts)
/tests/                 Cross-crate integration, HIL, and QEMU/Renode test harnesses
/docs/                  Architecture specs (HBP, WCI, deploy protocol, driver model, inference, workloads, this document)
/config/                Default system/boot configuration file templates
/session/               Dated handover snapshots (session/hand-YYYY-MM-DD/index.html)
/MsDOS/                 Git submodule — Microsoft's released MS-DOS source, reference-only
/Cargo.toml             Workspace manifest
/rust-toolchain.toml    Pinned toolchain version
/rustfmt.toml           Formatting rules
/deny.toml              cargo-deny policy (license/vulnerability scanning, per CODING_STANDARDS §Supply chain security)
/.cargo/config.toml     Build target defaults, custom target-spec paths
/README.md
/TinyOS26thJulySeedMVP.md
```

## Crate map

Every entry below is a `crates/<name>/` directory with its own `Cargo.toml` and `src/`. The **Phase** column ties each crate to the Roadmap in [`README.md`](../README.md#roadmap) and Section 10 of the seed specification, so this table is the concrete elaboration of that alignment, not a competing schedule.

| Crate | Purpose | `no_std`? | `unsafe` policy | Created in |
|---|---|---|---|---|
| `kernel` | Scheduler, IPC, memory pools, deadline monitor | Yes | Permitted (HAL boundary calls only) | Phase 0 |
| `hal` | Bus enumeration, ACPI/Device-Tree manifest normalization, arch-neutral HAL trait definitions | Yes | Permitted | Phase 0 |
| `hal-x86_64` | x86_64-specific HAL backend | Yes | Permitted | Phase 0 |
| `hal-arm64` | ARM64-specific HAL backend | Yes | Permitted | Phase 7 (Jetson bring-up); stubbed earlier if Tier 0 QEMU ARM64 testing needs it sooner |
| `aci` | Capability registry, policy engine, audit log | Yes | `#![forbid(unsafe_code)]` | Phase 5 |
| `shell` | TINYCMD canonical verb core + DOS/POSIX front-ends | Yes | `#![forbid(unsafe_code)]` | Phase 2 |
| `deploy-device` | On-device deploy/hot-swap/A-B-boot logic | Yes | Permitted (bootloader interaction) | Phase 1.5 |
| `deploy-client` | Host-side deploy tool (P2P Ethernet/WiFi) | No (`std`) | `#![forbid(unsafe_code)]` | Phase 1.5 |
| `drivers` | Mandatory class drivers: storage, network, HID | Yes | Permitted | Phase 3 |
| `drivers-can` | CAN 2.0B/CAN-FD bus stack | Yes | Permitted | Phase 3 |
| `bridge-device` | HBP device-side protocol | Yes | Permitted | Phase 4 |
| `bridge-host` | HBP host-side service (Windows/Linux) | No (`std`) | `#![forbid(unsafe_code)]` | Phase 4 |
| `wci` | Wireless Command Interface: mutual TLS, authority lease | Yes | Permitted (crypto/radio boundary) | Phase 4/5 boundary (needs ACI to gate against) |
| `motion` | Motion & Interpolation, Process-Synchronized Output, Position Feedback Abstraction, Safety Interlock — the shared RT primitives from `docs/physical-ai-reference-workloads.md` | Yes | Permitted | Phase 2–3, grown through the CNC milestone |
| `cnc-kinematics` | The committed trunnion-table RTCP/TCPC kinematics module — a plugin to `motion`, per the Open/Closed pattern | Yes | Permitted | CNC flagship milestone (spans Phases 0–3, see Section 10 note below) |
| `inference` | Local/external LLM runtime integration, Ollama adapter, ACI tool-call mapping | Depends on Phase 6 design work (§`no_std` vs `std` in CODING_STANDARDS) | `#![forbid(unsafe_code)]` at the ACI-facing boundary | Phase 6 |
| `compute` | Unified Memory Manager, GPU admission control, `-sys` vendor bindings | Yes | Permitted (isolated to `-sys` sub-crates) | Phase 6b |
| `config` | Boot/system configuration schema and parser | Yes | `#![forbid(unsafe_code)]` | Phase 0 (needed by `kernel` bring-up) |

Two crates are **not** part of the Cargo workspace's `no_std` build at all: `deploy-client` and `bridge-host` are ordinary host binaries (Windows/Linux), consistent with [`agent/CODING_STANDARDS.md`](../agent/CODING_STANDARDS.md#no_std-vs-std)'s existing `no_std`/`std` split — they're still workspace members for shared tooling/CI purposes, just not part of the on-device image.

Nothing here is created ahead of its need. `drivers-net`, `drivers-hid`, and similar further splits from the Universal Driver Model's [class-driver decomposition](universal-driver-model.md) happen only when `drivers` actually approaches the crate-size ceiling's 80% trigger point — starting pre-split for a crate that will hold perhaps a few hundred lines at Phase 3 would itself be a violation of "don't add abstraction before it's needed."

## Custom target specs

TinyOS's kernel is bare-metal `no_std`, which means the built-in `x86_64-unknown-none`/`aarch64-unknown-none` Rust targets are a starting point but typically need a custom target-spec JSON once boot-stage requirements (linker script, code model, disabled red-zone, disabled SIMD in kernel context) get specific — this is standard practice for from-scratch OS kernels in Rust, not a TinyOS-specific invention. `/targets/x86_64-tinyos.json` and `/targets/aarch64-tinyos.json` are introduced in Phase 0 alongside the `kernel` and `hal` crates, and `xtask` is responsible for invoking `cargo build` with `--target targets/<name>.json -Z build-std` so a contributor never has to remember the incantation by hand.

## Why `xtask`, not shell scripts

Per [`agent/CODING_STANDARDS.md`](../agent/CODING_STANDARDS.md#language-policy), TinyOS is Rust-primary; build/test/QEMU-launch/deploy orchestration living in `.sh`/`.ps1` scripts would be exactly the kind of undocumented, un-reviewed, platform-specific footprint that policy exists to prevent. `xtask` is an ordinary `std` Rust binary (the same pattern used by several established Rust projects for this purpose) invoked as `cargo run -p xtask -- <command>`, giving commands like `build`, `qemu-x86_64`, `qemu-arm64`, and `deploy` a single, cross-platform, testable home — and it directly serves the "remote-first, secure development loop" goal (G-DX-3) by being the same tool a contributor uses locally and the same tool CI uses.

## Delivery strategy: walking skeleton first

The first milestone is deliberately not a feature — it's a **walking skeleton**: the smallest possible end-to-end slice through the whole pipeline (`cargo build` → boot in QEMU → print nothing → halt cleanly → CI reports green), proven before any real kernel logic exists. This is a standard, well-established strategy for exactly the risk TinyOS faces at Phase 0: proving the build/boot/test pipeline works is a prerequisite for every subsequent phase, and discovering a toolchain or linker problem after three phases of feature work would be far more expensive than discovering it on day one with nothing at stake yet.

Concretely, in order:

1. **Workspace bootstrap.** `Cargo.toml` workspace manifest, `rust-toolchain.toml`, `rustfmt.toml`, `deny.toml`, empty `kernel` and `hal`/`hal-x86_64` crates, `targets/x86_64-tinyos.json`, and `xtask` with a `build` and `qemu-x86_64` command. Success criterion: `cargo run -p xtask -- qemu-x86_64` boots to a halt with no panic, in CI.
2. **CI governance gates activated immediately**, not bolted on later: format/lint (`rustfmt`, `clippy -D warnings`), the crate-size ceiling check (trivially passing at this size, but proven working now rather than discovered broken once it matters), and `missing_docs`. Per [`agent/CODING_STANDARDS.md`](../agent/CODING_STANDARDS.md#solid-principles--rust-adapted-never-compromised), the SOLID review checklist applies starting with this very first PR, not once the codebase is "big enough to matter."
3. **TDD from the first line of kernel logic.** The first real feature (context switch, or the priority scheduler, per Roadmap Phase 0) is built red-green-refactor per the mandatory TDD policy — a failing test for scheduler behavior exists before the scheduler code that satisfies it.
4. **Phase-by-phase crate growth**, following the Crate Map above and the existing Roadmap/Section 10 alignment: each phase both grows existing crates and, where the table indicates, introduces a new one — never introducing a crate's *code* before its Roadmap phase, even if the crate's empty shell was convenient to stub earlier for workspace-graph reasons.
5. **The CNC flagship milestone is a cross-phase integration point, not a single phase.** Per Section 10's existing note in the seed specification, `motion` and `cnc-kinematics` grow across Phases 0 through 3 (scheduler timing, shell/G-code front-end, and connectivity all have to land first) — the delivery strategy tracks this as an explicit milestone checkpoint (simultaneous 5-axis interpolation demonstrable against simulated axes in Tier 0/1) rather than letting it implicitly fall out of phase completion.
6. **Hardware-dependent work waits for hardware, but its software doesn't.** Per the MVP hardware plan, `hal-arm64` and GPU-related `compute` work are scheped to their Roadmap phases regardless of purchase timing — Tier 0 (QEMU ARM64) development proceeds against `hal-arm64` before the Jetson Orin Nano Super physically arrives, so hardware procurement lead time is never the pipeline's bottleneck.

## Status

This document is the concrete companion to Roadmap Phase 0 and to Section 10 (Roadmap Alignment) of [`TinyOS26thJulySeedMVP.md`](../TinyOS26thJulySeedMVP.md). The crate map will be revised as real implementation surfaces seams the design didn't anticipate — per [`agent/CODING_STANDARDS.md`](../agent/CODING_STANDARDS.md#crate-size-ceiling-hard-limit-no-exceptions), a crate approaching its size ceiling triggers a split PR, and this document should be updated in the same PR that performs that split, not left to drift out of sync with the actual workspace.
