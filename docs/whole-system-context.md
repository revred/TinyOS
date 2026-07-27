# TinyOS Whole-System Destination Architecture

Status: **Governing context and flight plan. Current implementation, near-term commitments, later destinations, and research goals are deliberately distinguished.**

This document answers one steering question: if TinyOS succeeds, what complete system are today's kernel decisions preparing for?

The concise answer is:

> A minimal, capability-secured, real-time core that feels like bare metal; optional application and network profiles that disappear completely when unselected; and a workload platform that converts saved cycles, memory traffic, queue time, copies, and attack surface into lower control latency, faster interactive UX, and higher quality-adjusted AI token velocity.

The canonical machine-readable view is [`goals/context/landing-zones.tsv`](../goals/context/landing-zones.tsv). The concrete workload list is [`application-platforms.tsv`](../goals/context/application-platforms.tsv). The [Security Charter](../SECURITY_CHARTER.md) governs every runtime and data-to-code path.

## Current implementation truth

The latest Phase-0 code establishes useful primitives:

- an actual 256-entry IDT is installed on the production boot path;
- the legacy PIC is retired and a local-APIC timer is armed;
- unserviced interrupt vectors fail closed instead of silently resuming;
- four-level page-table construction and W^X section validation exist;
- local IPC uses fixed-capacity directional queues;
- shared-memory creation rejects zero pages, rolls back partial mappings, and uses generation-stamped grants;
- fixed pools, WCET bookkeeping, priority-inheritance work, and compact spoors exist;
- the TXE packer and hostile PE fixtures are deterministic and bounded.

Those are not active process containment yet. The IDT has no domain-aware `#PF`/`#GP` termination route, no TSS/IST for stack-failure exceptions, and no scheduler tick consumer. Process page tables are constructed but no task switch activates a per-domain CR3. Production capability spaces, full task teardown, executable signatures/sealing, quarantine/provenance, IOMMU programming, immutable updates, and campaign evidence remain open. The honest state remains `baseline-debt`.

This creates the correct short-term order:

```text
domain-aware exception handling
  → active per-task CR3 and complete security context
  → default-deny capability spaces
  → revoke-before-reuse task teardown
  → signed executable metadata and sealing
  → isolated C2/C3/C4 services
  → network, runtime, browser, and application profiles
```

## The four-plane steering model

Every material design decision is evaluated in the same row across four planes:

| Plane | Required question | Failure signal |
|---|---|---|
| Goal | Which falsifiable destination does this serve? | Work exists without a project outcome or roadmap owner |
| Performance | Which `Dnn` domains and all 25 guardrails must it satisfy? | Only average speed is measured, or safety/security cost is excluded |
| Application | Which real workload proves the abstraction useful? | A generic API exists with no demanding consumer |
| Security | Which classes, controls, PD contracts and RCG gates contain it? | The workload gains implicit authority or a new data-to-code route |

The join is enforced by `xtask check-assurance-spine`. It does not turn future ambition into a current claim; it stops current architecture from forgetting future load-bearing requirements.

## Nine landing zones

| Landing zone | Product merit | Horizon |
|---|---|---|
| Bare-metal reflex | Deterministic control, IPC, allocation and interrupts remain bounded under hostile competing load | Next |
| Quality-adjusted AI velocity | Highest useful token throughput and lowest TTFT per watt, byte and protected CPU cycle | Later |
| Native application and UX platform | Rust, Go/Wails, Rust/Tauri, .NET and JavaScript apps share one capability ABI | Later |
| Interactive graphics and multiplayer | Dangerous Dave, DOOM, Quake II and Quake III prove the complete interactive stack | Later |
| Browser and TinySpot remote UX | Rich web and remote-desktop UX remain separated from local authority | Research |
| Windows and Linux coexistence | WST and TLE bridge environments without ambient shared state | Later |
| Fleet and data-centre teamwork | Nodes coordinate inference and workloads through authenticated endpoints and spoors | Later |
| Browser-hosted laboratory | A compact WebAssembly/emulated build runs in a browser for demos and portable testing | Research |
| Minimal composable profiles | Every unselected driver, runtime, parser and app has zero bytes and zero live surface | Now |

The full row-level goal, performance, application, security, class and claim-gate selections are in [`landing-zones.tsv`](../goals/context/landing-zones.tsv).

## Performance: “speed on steroids” made falsifiable

TinyOS should feel too fast to be real because it avoids work, not because it hides safety checks:

- no scheduler, interrupt or IPC hot-path allocation;
- no general container tax around a Protection Domain;
- fixed-capacity queues and explicit backpressure;
- typed small-message copy and rights-sized page loans for bulk data;
- PCID/ASID-assisted address-space switching where hardware permits;
- immutable shared code and model pages with private writable state;
- zero-copy CPU/GPU buffers only where ownership and IOMMU evidence permit;
- content-addressed caches for verified binaries, model blocks, kernels, shaders, and generated-code units;
- opt-in drivers, protocols, codecs and runtimes physically absent from profiles that do not use them;
- caller-funded broker work so one client cannot externalise its cost into another task's latency.

The 625-test catalogue remains canonical. Application targets select its existing domains rather than creating a softer benchmark set. Selecting one domain imports all 25 guardrails: latency percentiles, observed maximum/WCET argument, jitter, cycles, PMU efficiency, image size, working memory, allocations, queue wait/service time, throughput, burst recovery, cold/warm start, competing-load isolation, denial cost, fault containment, soak stability, observability overhead, and same-hardware comparison gates.

### AI claims

Token performance is reported as a vector, never one flattering number:

- time to first token;
- prefill tokens/second;
- decode tokens/second;
- prompt-to-complete latency;
- quality score under a fixed evaluation set;
- quality-adjusted time and energy;
- peak and steady CPU/GPU memory;
- memory bandwidth and page-fault behavior;
- power, thermal state and throttling;
- RT degradation while inference is at 90% admitted load.

“Ten times faster” is permitted only for a named metric with fixed model, weights, quantisation, context, sampler, output-quality floor, hardware, power state, security profile, and raw evidence.

The repository's existing correction remains important: `blue.atom` and `blue-sharc.exe` are context/tooling concepts, not an Ollama-like inference engine. A Blue Atom pipeline can still improve end-to-end token delivery through faster context selection, content addressing, cache reuse, distribution and tool dispatch. Reports must separate those gains from model prefill/decode performance instead of attributing all improvement to a vague “atom” mechanism.

## Application execution architecture

```text
┌───────────────────── application package / remote content ─────────────────────┐
│ signed native TXE │ managed AOT │ runtime graph │ game data │ web content      │
└────────────────────────────────────────┬───────────────────────────────────────┘
                                  │ RCG admission when executable
┌────────────────────────────────────────▼───────────────────────────────────────┐
│ C3 application Protection Domains                                              │
│ Rust / Go / .NET AOT / JS runtime / browser controller / game / TLE / WST      │
└────────────────────┬───────────────────┬───────────────────┬───────────────────┘
                    │ typed IPC         │ page grants       │ endpoint caps
┌────────────────────▼───────────────────▼───────────────────▼───────────────────┐
│ C2 restartable brokers                                                         │
│ storage │ network │ GPU │ display │ audio │ input │ codecs │ secrets │ loader  │
└────────────────────────────────────────┬───────────────────────────────────────┘
                                  │ fixed kernel ABI
┌────────────────────────────────────────▼───────────────────────────────────────┐
│ C1 minimal kernel: MMU │ scheduler │ capability space │ IPC │ fault │ teardown │
└────────────────────────────────────────┬───────────────────────────────────────┘
                                  │ measured one-shot handoff
┌────────────────────────────────────────▼───────────────────────────────────────┐
│ C0 verified boot and recovery                                                  │
└────────────────────────────────────────────────────────────────────────────────┘
```

### What “native support” means

TinyOS uses six explicit support levels:

1. `core-native` — the tiny execution/protection core.
2. `native-txe` — compiled to the TinyOS ABI and admitted as TXE/TON.
3. `managed-aot` — self-contained AOT image with generated capability bindings.
4. `isolated-runtime` — a substantial runtime compartment system.
5. `compatibility-guest` — a foreign ABI behind a guest/personality boundary.
6. `browser-hosted` — a browser sandbox laboratory, not a hardware-equivalent port.

This lets the project commit to an application destination without pretending all frameworks have the same porting cost or trust profile.

## Framework and language decisions

### Tauri

Tauri's Rust core plus system-webview model aligns well with TinyOS if its application core is C3, the local frontend receives only typed commands, and remote web content is moved to C4 with no application IPC authority. Tauri's own capabilities remain useful app metadata, but TinyOS intersects them with the signed manifest and local policy instead of trusting them as the outer sandbox.

### Wails

Wails is an ahead-of-time Go backend plus a system webview and Go↔JavaScript bridge. Supporting it therefore requires a Go/TinyOS target and runtime adaptation, a WebView profile, graphics/input/window services, and a generated bridge mapped to capability-safe IPC. The Go garbage collector and scheduler must be measured as C3 workload behavior; neither may leak into RT guarantees.

### .NET 10 or later and C#

C# is coherent with the Charter when the OS, not managed-code marketing, is the boundary:

- production lane: self-contained Native AOT;
- no runtime `Reflection.Emit` or arbitrary assembly loading;
- no unrestricted P/Invoke, `NativeLibrary.Load`, COM/DCOM, remoting, debugger attach, or process memory API;
- every native dependency is a signed, hash-pinned TON with an enumerated ABI;
- generated TinyOS bindings translate C# calls into typed capability operations;
- GC pause, allocation, startup, footprint and interop cost are part of the selected performance domains.

Microsoft documents that Native AOT does not use a runtime JIT and disallows dynamic loading/code generation, which fits the sealed-executable policy. Microsoft also documents that modern .NET Code Access Security is not a sandbox boundary. TinyOS therefore gains C# productivity without granting a CLR process ambient authority.

### Node.js

Node's current permission model is useful defence in depth, but its own documentation says it does not protect against malicious code and relies on OS process isolation. That is exactly the TinyOS model: the Node runtime is one C3 domain, and filesystem, network, workers, child processes, WASI, FFI, native addons, inspector and process signalling are removed or separately brokered by default.

Production begins in JITless or sealed-code mode. A future generated-code accelerator must use the Charter's destroy/admit/recreate flow; it cannot receive an in-place W→X exemption.

### Bun

Bun is attractive for startup and developer velocity, but its current model transpiles input at runtime, embeds JavaScriptCore, supports native addons and offers an FFI path that JIT-compiles bindings. Those are directly relevant Charter surfaces. Bun is therefore a research target until a port proves:

- no unmediated executable generation;
- lifecycle scripts disabled unless independently admitted;
- FFI/native addons absent by default;
- no name-based `dlopen`;
- all module and transpiler inputs preserve provenance;
- bounded memory, CPU, queue and teardown behavior.

“Maybe Bun” is coherent as a measured future target, not as a current native-support claim.

## Graphics, games and networking

The game sequence is deliberate:

1. **Dangerous Dave** — simplest useful 2D/input/audio/storage compatibility proof.
2. **DOOM** — framebuffer, audio, input, timing, WAD parsing and optional datagram networking.
3. **Quake II** — 3D renderer, richer assets, client/server networking and sustained load.
4. **Quake III Arena** — high-rate multiplayer, prediction, UDP, downloads/content VM behavior and adversarial server testing.

These games are not kernel features. They are C3 applications using opt-in C2 display, GPU, input, audio, storage and network brokers. Multiplayer is also a serious network conformance workload: endpoint capabilities, packet queues, retransmission policy where applicable, rate limits, jitter, loss, discovery, session identity and malicious-server behavior all become measurable.

TinyOS needs a real TCP/IP stack, but not in C1. Network design is split into:

- minimal NIC drivers with IOMMU/bounce-buffer protection;
- a restartable C2 network service;
- private per-domain namespaces;
- capabilities for bind/listen/connect/discover/route/raw/admin separately;
- bounded TCP/UDP queues with admitted RT reserves;
- TLS/identity services separated from application parsers;
- zero default ports and automated listener-absence evidence.

## Chrome-class browser

Running Chrome is a major destination, not an early portability task. Chromium itself is a multi-process OS-like workload containing renderers, GPU processes, network services, storage, codecs, extensions, profiles and a JIT engine. TinyOS support requires mapping that architecture onto Protection Domains rather than flattening it into one privileged application.

Remote web content belongs in C4 renderers. The browser controller remains C3. Network, GPU, storage, codecs, secrets and downloads use narrow C2 brokers. Site profiles and cookies are partitioned. Remote debugging, native messaging, extensions, codecs and JIT are absent until independently admitted.

The browser profile is optional and necessarily far larger than the 8 MiB core. That does not violate minimalism: a device without the browser links none of it.

## TinySpot remote desktop

TinySpot is a remote UX protocol and service family:

- telemetry/video and input are separate lanes;
- viewing does not imply input authority;
- clipboard, file transfer, audio, administrative control and session recording are separate capabilities;
- the capture/codec/network path cannot block RT work;
- reconnect never silently restores command authority;
- session drops resolve according to deployment safety policy;
- every remote input is attributable by spoor.

It can use HBP for same-machine UX and WCI/TLS for network sessions while retaining one application-level protocol.

## Windows and Linux coexistence

### Windows TinyOS Tools

WST is the Windows host-side companion:

- discover and attest a TinyOS partition/VM;
- show task, timing, spoor and health state;
- exchange typed HBP messages;
- request explicit file/object transfers;
- connect IDE/debug/deploy workflows through scoped capabilities;
- coordinate Windows UX frameworks with TinyOS RT services.

For a Hyper-V topology, VMBus/Hyper-V sockets or virtio/vsock are preferable to a routable loopback listener. Shared memory is explicit and generation-safe. Windows compromise does not confer TinyOS process handles, page-table writes, raw devices or trust enrollment.

### TinyOS Linux Environment

TLE provides a progressive Linux developer environment:

1. TINYCMD's existing POSIX-familiar verbs;
2. a source-level POSIX compatibility library;
3. selected ELF/Linux ABI translation in C3;
4. only if justified, a lightweight Linux guest.

This is deliberately not called “WSL2 inside TinyOS.” WSL2 uses a real Linux kernel in a managed virtual machine. TinyOS should copy the useful UX and isolation outcome while choosing the smallest implementation that satisfies actual workloads.

## Browser-hosted TinyOS

Running TinyOS in a browser sandbox is feasible as a laboratory through WebAssembly or an emulator compiled to WebAssembly. WebAssembly provides sandboxed linear memory and is governed by the browser's same-origin and permission policies.

Two modes are useful:

- **semantic build** — scheduler, pools, IPC, shell, spoors, loader validation and application APIs compiled to WebAssembly;
- **machine emulator** — the real TinyOS x86_64/ARM64 image booted through a browser-hosted emulator.

The semantic build is smaller and faster; the emulator is more faithful. Neither proves hard-RT timing, interrupt latency, DMA/IOMMU behavior, verified boot, power-loss recovery or bare-metal security. The dashboard and reports must retain that distinction even if the demo looks spectacular.

## Roadmap horizons

### Now: make the spine real

- domain-aware CPU exception handling;
- active per-task CR3;
- production capability spaces;
- task death and revoke-before-reuse;
- signed TXE metadata and executable sealing;
- keep all optional surfaces absent.

### Next: determinism, deploy, UX and connectivity

- consume the APIC tick in the scheduler;
- gather raw D02–D13 and D24–D25 evidence;
- implement signed A/B deployment;
- TINYCMD and display/input foundations;
- isolated NIC and TCP/UDP services;
- WST/HBP integration;
- first game/source-port proving workload.

### Later: application and AI platform

- graphics/audio/window services;
- Tauri then Wails conformance;
- .NET Native AOT;
- Node JITless/sealed-code profile;
- local LLM and heterogeneous compute;
- TinySpot;
- Quake multiplayer and fleet coordination.

### Research

- Bun under the no-in-place-code-generation rule;
- Chrome-class browser compartment system;
- TLE guest depth;
- browser-hosted TinyOS;
- data-centre scale and stronger hardware compartmentalisation.

## Upstream architecture facts used

- [.NET Native AOT](https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/) produces self-contained native applications without runtime JIT and documents dynamic-code/loading limitations.
- [Node permissions](https://nodejs.org/api/permissions.html) explicitly describe a seat-belt model rather than malicious-code containment.
- [Tauri security](https://v2.tauri.app/security/) separates Rust core and webview trust groups and uses capability-scoped IPC.
- [Wails architecture](https://v3.wails.io/concepts/architecture/) uses an ahead-of-time Go backend and the operating system webview.
- [Bun runtime](https://bun.sh/docs/runtime) uses JavaScriptCore and runtime transpilation; [Bun FFI](https://bun.sh/docs/runtime/ffi) exposes native loading and generated bindings.
- [Chromium's multi-process architecture](https://www.chromium.org/developers/design-documents/multi-process-architecture/) isolates renderers and brokers access to resources.
- [WebAssembly security](https://webassembly.org/docs/security/) defines sandboxed execution subject to the embedding environment.
- [WSL architecture](https://learn.microsoft.com/en-us/windows/wsl/about) uses a Linux kernel in a lightweight virtual machine, which is why TinyOS uses distinct TLE terminology.
- [Hyper-V architecture](https://learn.microsoft.com/en-us/windows-server/virtualization/hyper-v/architecture) documents VMBus and enlightened I/O as efficient partition communication mechanisms.
