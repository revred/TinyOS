# Handover 34 — Whole-System Context and Application Flight Plan

Follows: [`33-security-charter-review-and-next-steps.md`](33-security-charter-review-and-next-steps.md).

## Outcome

TinyOS now carries its long-range destination in the same checked spine as current Phase-0 work. Goals, performance, applications, and security can no longer evolve as four independent narratives:

1. [`goals/context/application-platforms.tsv`](../../goals/context/application-platforms.tsv) defines 19 application and platform targets.
2. [`goals/context/landing-zones.tsv`](../../goals/context/landing-zones.tsv) defines nine system outcomes that join those targets to exact Goals, performance domains, security controls, containment classes, and claim gates.
3. [`os/src/xtask/src/assurance.rs`](../../os/src/xtask/src/assurance.rs) validates both catalogues as part of `check-assurance-spine`; the committed catalogues expand to 6,575 selected application/performance obligations.
4. [`SECURITY_CHARTER.md`](../../SECURITY_CHARTER.md) now governs optional language runtimes, WebViews, browsers, games, compatibility guests, host bridges, browser-hosted builds, networking, and fleets.
5. [`docs/whole-system-context.md`](../../docs/whole-system-context.md) is the narrative destination architecture; [`goals/index.html`](../../goals/index.html) presents the same four-plane model and landing zones.
6. [`goals/reports/REPORT-2026-07-26-28.md`](../../goals/reports/REPORT-2026-07-26-28.md) records the structural evidence and preserves the boundary between a checked plan and an implemented capability.

## The architectural decision

“Native support” does not mean putting every runtime in the kernel or base image. TinyOS defines six support levels:

- `core-native`
- `native-txe`
- `managed-aot`
- `isolated-runtime`
- `compatibility-guest`
- `browser-hosted`

Only the selected signed profile exists on a deployment. Unselected drivers, runtimes, listeners, parsers, queues, tasks, IRQ routes, DMA windows, MMIO ranges, and capability types must have negative-surface evidence proving absence. This makes frugality and security the same compositional property.

## C#/.NET decision

C# is coherent with the Security Charter. The language is not the injection boundary; ambient authority and dynamic/native escape hatches are.

The production target is self-contained .NET Native AOT in a C3 Protection Domain. TinyOS forbids `Reflection.Emit`, unrestricted assembly loading, generic process-memory access or signalling, COM/remoting authority, unrestricted/name-only P/Invoke, and executable mutation. Native dependencies are independently signed, content-addressed TON objects. The GC, runtime, and bindings remain measured optional profile components with no authority beyond the application's signed manifest intersected with local policy.

Node's permission model is treated only as defense in depth; Bun remains research until its JavaScriptCore JIT, transpiler, FFI, native-addon, and lifecycle-script surfaces can satisfy the same Charter. Tauri and Wails use a split frontend/backend model with a local WebView receiving typed, manifest-limited calls rather than raw system authority.

## Application decisions

- Games are proving workloads for graphics, audio, input, clocks, storage, and multiplayer. Assets and server downloads remain data; mods/native modules require separate admission.
- A Chrome-class browser is an optional compartment system: controller in C3, remote renderers in C4, and network/GPU/storage/media/secrets behind separate C2 brokers. It is research, not a current compatibility claim.
- TinySpot is a capability-separated remote UX service; input, clipboard, telemetry, file transfer, capture, codec, and listener authority are independent.
- “WSL2 inside TinyOS” is renamed **TinyOS Linux Environment (TLE)** because WSL2 is specifically a real Linux kernel in a lightweight VM on Windows. **Windows TinyOS Tools (WST)** is the host-side bridge; it exposes typed revocable objects, never ambient drives, process handles, memory, shells, or automatically forwarded ports.
- The browser-hosted build is a lab/conformance profile. Its outer browser sandbox remains part of the trust boundary, so it cannot claim hard-RT, DMA/IRQ, verified-boot, bare-metal security, or hardware-equivalence evidence.
- Fleet coordination submits authenticated declarative work through scoped brokers and spoors. It does not create a remote command, code-loading, trust-enrolment, or durable-authority channel.

## Latest-code truth preserved

The latest boot path now has the real 256-entry IDT, local-APIC timer, legacy-PIC retirement, and fail-closed unrouted-vector behavior. Local IPC is bounded and shared-memory grants are transactional and generation-safe.

The destination catalogues do not hide the remaining blockers: active per-task `CR3`, domain-aware `#PF`/`#GP` teardown, TSS/IST, production capability spaces, executable sealing, IOMMU isolation, task-exit revocation, TCP/IP, GPU/display/audio brokers, runtime hosts, immutable update/recovery, and hostile-campaign evidence remain incomplete.

## Verification

- `cargo test -p xtask`: **23/23**
- `cargo run -p xtask -- check-assurance-spine`: **8 Features, 25 Stories, 23 Tests, 28 Reports, 19 application/platform targets, 9 landing zones, 1,025 Story/performance contracts, 6,575 application/performance contracts**
- `cargo run -p xtask -- check-performance-catalogue`: **625/625**
- `cargo test --workspace --lib`: **158/158**
- `cargo fmt --check`: clean
- `cargo clippy --workspace --lib -- -D warnings`: clean
- `cargo run -p xtask -- check-crate-sizes`: all crates below 20,000 lines
- `cargo run -p xtask -- check-image-size`: **17,288 bytes / 8 MiB**
- static HTML/link validation: all relative links resolve; four context cards, nine landing-zone rows, and four assurance lanes are present

The connected browser runtime exposed no available browser backend, so a rendered visual QA pass could not be observed. No unrelated browser-control mechanism was substituted.

## Recommended steering order

1. Complete `STORY-P0-04-03` without broadening driver authority.
2. Add domain-aware exception teardown and TSS/IST.
3. Activate per-task `CR3` only behind that fault boundary.
4. Build production capability spaces and task-exit revocation.
5. Seal admitted images into RX/RO/RW-NX mappings and prove the full RCG chain.
6. Establish opt-in IOMMU-isolated C2 driver brokers.
7. Build bounded TCP/IP endpoints before multiplayer, remote UX, browser, or fleet profiles.
8. Earn one application profile at a time; DOOM is a smaller systems proof than Chromium or a JavaScript runtime.

No new application/runtime capability is marked implemented or assurance-verified by this handover.
