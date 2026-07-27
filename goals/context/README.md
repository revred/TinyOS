# TinyOS Whole-System Context

Status: **Machine-checked destination map; most application targets are future work, not current support claims.**

TinyOS is steered through four inseparable views:

| View | Question | Canonical evidence |
|---|---|---|
| Goals | Why should this exist and where are we trying to land? | `SeedMVP.md` goal IDs and roadmap Epics |
| Performance | Is it fast, frugal, compact and predictable under the real workload? | The 625 `PERF-Dnn-Gnn` guardrails |
| Applications | Which concrete workload makes the platform useful and what does “support” mean? | [`application-platforms.tsv`](application-platforms.tsv) |
| Security | Does the workload preserve containment, provenance, admission and recovery under attack? | `SEC-*`, `C0..C4`, `PD-*`, `RCG-*` and `BND-*` |

[`landing-zones.tsv`](landing-zones.tsv) is the side-by-side join. Each row names its goal IDs, selected performance domains, concrete application targets, security controls, containment classes, roadmap horizon and the evidence required before its headline claim can be made.

## Support levels

“Native support” is not used as an all-or-nothing slogan:

| Level | Meaning |
|---|---|
| `core-native` | Part of the minimal TinyOS execution and protection model. This level is kept extremely small. |
| `native-txe` | Compiled for the TinyOS ABI and packaged as signed TXE/TON objects. It runs as a C3 application, not in C1. |
| `managed-aot` | A self-contained ahead-of-time runtime image, such as .NET Native AOT, admitted like any other executable. |
| `isolated-runtime` | A substantial language/browser runtime is an optional C3 compartment system with C2 brokers and C4 hostile-input workers. |
| `compatibility-guest` | A compatibility personality or lightweight guest provides a foreign ABI without importing it into the kernel. |
| `browser-hosted` | A WebAssembly/emulator laboratory runs inside the host browser. It is useful, but it is not bare-metal or HIL evidence. |

These levels describe integration depth, not trust. Every executable begins with empty authority; a managed runtime, webview, browser, compatibility layer or signed application is never accepted as an OS security boundary.

## Application performance expansion

Each application selects one or more of the existing 25 performance domains. Each selected domain expands to all 25 guardrails, so application ambitions inherit latency tails, CPU-cycle, allocation, memory, queue, throughput, fault, observability, footprint and comparison tests without changing the canonical 625-cell catalogue.

This creates two useful views of the same tests:

- Story selection proves that current implementation work cannot bypass the performance spine.
- Application selection proves that future product destinations already have an explicit performance and security landing contract.

The application selections do not claim those future systems exist. They prevent their requirements from being discovered only after the kernel architecture is difficult to change.

## Runtime policy in one paragraph

Rust/TXE is the narrowest path. Go and .NET production applications should prefer ahead-of-time compilation. JavaScript and browser runtimes begin JITless; FFI, native add-ons, inspectors, process signalling, shell spawning and dynamic library lookup are absent by default. If generated native code is ever supported, it is produced outside the running application, content-addressed, passed through `RCG-01..RCG-14`, and mapped sealed into a fresh domain—never created by an in-place W→X transition.

## Integrity gate

From `os/`:

```text
cargo run -p xtask -- check-assurance-spine
```

The check rejects missing application or landing-zone IDs, malformed references, unsupported support levels or horizons, unknown domains/controls/classes, application targets owned by no landing zone, or a landing-zone promise disconnected from its selected applications.
