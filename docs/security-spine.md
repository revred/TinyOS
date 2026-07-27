# TinyOS Security Spine — Sandbox-First, Provenance-First, AI-Attack-Ready

Status: **Target architecture and release invariants.** [`SECURITY_CHARTER.md`](../SECURITY_CHARTER.md) is the governing Protection Domain, remote-code exclusion, and application-runtime charter. Phase 0 implements only fragments; [`goals/security/controls.tsv`](../goals/security/controls.tsv) is the canonical control set, [`goals/security/containment-classes.tsv`](../goals/security/containment-classes.tsv) defines the five containment classes, the [`goals/assurance/`](../goals/assurance/) contracts bind them to Features and Stories, and [`goals/context/`](../goals/context/) binds future applications and landing zones to the same controls.

## Security objective

TinyOS is designed to remove the ambient authority that makes many Linux and Windows exploit chains valuable. Loading bytes, knowing a path, sharing a machine, reaching a port, or being installed does not itself grant authority. The security boundary is the smallest revocable capability that permits one typed operation on one named resource.

Security remains ahead of performance. A faster implementation that weakens signing, W^X, process isolation, origin labels, sandboxing, capability checks, auditability, or fail-safe behavior fails its performance test.

## Five containment classes

TinyOS uses five **containment classes**, not five conventional privilege rings. A class states how a component is isolated, launched, failed, and evidenced. It does not grant authority. Capabilities determine what a component may do; scheduling policy determines its priority and budget; provenance describes where its code and data came from. Those dimensions remain independent.

| Class | Name | Contains | Default posture |
|---|---|---|---|
| **C0** | Root of Trust | Hardware trust anchor, boot verifier, measurement, recovery, smallest pre-kernel transfer mechanism | Immutable, minimal, no reusable runtime command surface |
| **C1** | Trusted Kernel Core | Scheduler, active MMU switching, fixed-format IPC, capability validation, faults, interrupts, resource budgets | No complex hostile-format parsing; smallest privileged TCB |
| **C2** | Isolated System Service | Drivers, network/storage stacks, loader and crypto/device brokers | Assumed compromisable; empty authority, user-mode, restartable, device/service-scoped grants |
| **C3** | Sandboxed Application | Signed installed programs, inference runtimes, shell and administrative tools | Fresh private address space; empty authority until manifest and policy agree |
| **C4** | Hostile Transient Domain | Downloads, unknown executables, disposable parsers/renderers, scripts, documents and model output | Quarantined, zero ambient authority, disposable, no in-place promotion |

The canonical machine-readable definitions and evidence requirements are [`goals/security/containment-classes.tsv`](../goals/security/containment-classes.tsv). The mandatory adversarial matrix is [`goals/security/containment-tests.tsv`](../goals/security/containment-tests.tsv).

The lightweight sandbox primitive is defined by the 14 [`PD-*` Protection Domain contracts](../goals/security/protection-domain-contracts.tsv). The only external-data-to-code path is defined by the 14 [`RCG-*` code-admission gates](../goals/security/code-admission-gates.tsv). Every ordered source/target rule is explicit in the 25-row [`C0–C4 communication matrix`](../goals/security/class-communication-matrix.tsv).

### Non-negotiable class rules

1. **Class is not authority.** No class number creates a file, process, network, device, memory, install, actuation, or administration right.
2. **Class is not priority.** A C4 task cannot starve admitted RT work; a C2 driver does not become high priority merely because it handles hardware.
3. **No hostile parser in C1.** C1 accepts only small fixed-format kernel ABI values copied into kernel-owned memory. Packet, file, executable, document, model, archive, filesystem, USB, firmware-extension, and device-protocol parsers live in C2 or C4.
4. **Assume C2 compromise.** A driver is not “secure” because it is signed. Signing establishes identity; isolation, IOMMU enforcement, narrow grants, restart, and revocation contain defects or malice.
5. **C3 starts empty.** Installation and signature validation do not confer authority. The signed manifest requests capabilities and policy may grant only a subset.
6. **C4 never promotes in place.** Successful validation destroys the C4 inspection instance and creates a fresh C3 process with new address-space, handle, capability, queue, and generation state.
7. **Trust is non-increasing through data flow.** Rename, copy, extraction, conversion, compilation, IPC, or model generation cannot upgrade provenance.
8. **Termination revokes before reuse.** Memory, IPC, DMA, IRQ, MMIO, endpoint, queue, secret, and capability state is invalidated before an identifier, page, device, or slot can be reused.
9. **Every boundary is attributable.** Spoors record source and target class, actor, action, object, decision, result, sequence, and relevant generation without becoming an authorization mechanism themselves.
10. **One defect is insufficient.** The release campaign seeds one realistic defect at a time in C1–C4 and requires that no single defect reach C0, cross an ungranted boundary, or establish durable ambient authority.

### Boundary communication

C2, C3, and C4 communicate only through typed, bounded, capability-checked channels mediated by C1. Ordinary IPC transfers data, not authority. Authority transfer requires a message schema that explicitly accepts a delegable, rights-reduced, generation-safe capability and records the delegation. Cross-process pointers are never an interface. Bulk zero-copy uses SEC-04 shared-memory grants with explicit participants, bounds, permissions, lifetime, teardown, and revocation.

No runtime caller enters C0. C0 verifies and transfers to C1 once, or enters recovery. C4 has no direct persistent storage, driver, device, install, process-management, raw-network, secret, or actuation path. A broker may expose one narrowly typed operation without delegating its own broader authority.

### Remote-code exclusion

Remote transport authentication is peer identity, not code trust. Network packets, HBP/WCI frames, ACI requests, shell input, model output, files, downloads, debugger input, compatibility calls, and deploy payloads can create only non-executable C4 data.

There is exactly one permitted transition to executable state:

```text
bounded ingress
→ immutable origin-labelled quarantine
→ disposable C4 parsing and canonicalisation
→ exact content/dependency identity
→ signature/trust/revocation/anti-rollback verification
→ manifest ∩ policy ∩ resource admission
→ destroy C4 inspection state
→ fresh empty-authority C3 Protection Domain
→ sealed RX code + RO constants + NX private writable data
→ explicit attributable scheduler activation
```

No production profile exposes a general W→X/X→W transition, writable executable alias, JIT exception, self-modifying-code path, process-memory write, raw loader, in-place promotion, or remote trust-root enrollment. A deploy channel may stage data only. Core updates activate through C0 verified A/B boot; non-core updates create a fresh admitted domain and never patch a live executable mapping.

An attacker who gains instruction-pointer control inside an admitted C2/C3/C4 domain still cannot manufacture memory access, capabilities, CPU budget, device authority, persistence, or a class transition. This is how TinyOS addresses code-reuse attacks as well as injected shellcode: process compromise remains bounded by the Protection Domain rather than becoming ambient system authority.

### Application runtimes are subjects, not boundaries

Supporting Rust/TXE, Go/Wails, Rust/Tauri, .NET 10+ C#, Node, Bun, games, Chromium, TinySpot, TLE, WST or fleet workloads does not create a second security architecture:

- Native and managed-AOT code is signed and admitted into C3.
- CLR/GC, Go runtime, V8, JavaScriptCore, webviews and compatibility personalities receive no authority beyond their OS capabilities.
- P/Invoke, FFI, native addons, dynamic libraries, inspectors, child processes, shell execution and generated code are absent unless a signed manifest and local policy select a narrower operation.
- Runtime permission models and framework ACLs are useful inner checks but never replace the Protection Domain.
- Local privileged UI assets and remote web origins never share one trust identity; remote origins are C4 and have no local application bridge.
- Large runtimes, browsers, codecs, games and compatibility bundles are opt-in and contribute zero linked/live surface when absent.

The complete workload and claim-gate mapping is [`goals/context/application-platforms.tsv`](../goals/context/application-platforms.tsv) joined by [`landing-zones.tsv`](../goals/context/landing-zones.tsv). [`whole-system-context.md`](whole-system-context.md) explains the performance and roadmap consequences.

### Design-to-release contract

- **Goal/Epic:** state which class boundary or invariant the work advances.
- **Feature:** declare implementation classes, subject classes, authority posture, hostile inputs, applicable `BND-*` tests, and required evidence in [`feature-contracts.tsv`](../goals/assurance/feature-contracts.tsv).
- **Story:** select containment classes alongside performance domains and security controls in [`story-contracts.tsv`](../goals/assurance/story-contracts.tsv).
- **Test:** attack class confusion, authority inheritance, memory/IPC/DMA crossing, lifecycle revocation, promotion, exhaustion, priority isolation, and negative component presence as applicable.
- **Report:** identify the actual deployment profile and class placement, retain raw allow/deny/fault/spoor/timing evidence, and distinguish implemented boundaries from simulations or stand-ins.
- **CI:** reject an unmapped Feature or Story, malformed class, incomplete class/test/control catalogue, or boundary-test reference that does not exist.

## Threats and mandatory response

| Attack pattern | TinyOS invariant | Controls |
|---|---|---|
| Unsigned executable hijack, binary substitution, malicious patch | Verified boot plus signed content-addressed TXE/TON objects; anti-rollback before mapping | SEC-01, SEC-02, SEC-17 |
| One process overwrites or reads another process | Separate active address spaces, guard pages, W^X/NX, no ambient process handles | SEC-03, SEC-19 |
| Illegal shared memory or stale shared mapping | Rights-sized typed grants, explicit participants, generation counters, deterministic revocation | SEC-04 |
| Sandbox escape or confused deputy | Sandbox-first launch with an empty capability set and one policy engine for every caller | SEC-05, SEC-14 |
| Files mixed without origin or entitlement | Immutable origin/signer/trust labels, capability-mounted storage, label propagation through transforms | SEC-06, SEC-07 |
| TCP/IP, port, listener, or lateral-movement attack | No default listeners; endpoint and route capabilities; per-task network namespaces; bounded parsers | SEC-08, SEC-20 |
| Browser, document, media, script, or deserializer exploit | Disposable parser sandbox, no ambient authority, no required JIT, bounded CPU/memory/time | SEC-09, SEC-20 |
| Drive-by or illegal download and install | Non-executable quarantine, origin labels, archive limits, verified promotion, explicit entitlement | SEC-10 |
| Cookies, tracking, fingerprinting, persistent identifiers | Origin-partitioned, opt-in, inspectable, least-lived state; no ambient device identity | SEC-11 |
| Ransomware, worms, autorun, persistence | Immutable system image, atomic signed updates, snapshots, mutation-rate tripwires, no autorun, no lateral authority | SEC-12, SEC-17 |
| Driver or malicious-device compromise | Drivers absent unless opted in, signed, isolated, narrowly granted DMA/IRQ/MMIO through IOMMU | SEC-13, SEC-18 |
| Credential theft and replay | Purpose-bound non-exportable keys, rotation/revocation, no shared long-lived secret | SEC-15 |
| Autonomous AI exploit chaining and high-rate retry | Treat all model output as hostile; capability/rate/action budgets; full spoors; kill switch; campaign-level tests | SEC-16 |
| Resource and parser bombs | Compile-time/runtime capacity bounds, admission control, priority-safe reserves, bounded recovery | SEC-20 |

## Execution and memory model

1. Boot verifies the next stage before control transfer. The boot policy rejects wrong-key, unsigned, revoked, corrupted, and downgraded images.
2. A TXE/TON object is data until its content hash, signature, origin, entitlement, anti-rollback counter, import manifest, memory map, and requested capabilities validate.
3. The loader creates a fresh address space with no inherited mappings other than the minimal runtime ABI. Code is executable and non-writable; data is writable and non-executable; guard pages surround stacks, heaps, IPC windows, and control structures.
4. The process never receives an ambient “open any file,” “inspect any process,” “bind any port,” or “load any driver” primitive. Its manifest requests specific capabilities and policy may grant a subset.
5. Cross-process memory is impossible through ordinary pointers. Shared pages use SEC-04 grants with explicit participants, access mode, byte/page extent, lifetime, generation, and revocation behavior.
6. Process teardown invalidates capabilities and generations before pages can be reused. A wipe test proves no residual data, mapping, queue item, DMA descriptor, or authority survives.
7. DMA is memory access by another processor and receives the same isolation treatment through the IOMMU. No-IOMMU hardware must use bounce buffers or be rejected for configurations requiring strong isolation.

## Origin, files, downloads, and active content

Every external object carries security metadata independent of its filename:

- content hash and immutable origin;
- signer and verification state;
- acquisition channel and time;
- declared type and detected type;
- entitlement/policy labels;
- quarantine and executable-promotion state;
- derivation parents for copies, extraction, conversion, compilation, or model generation.

Renaming, copying, archiving, extracting, or sending an object through IPC cannot upgrade trust. A derived object receives at most the intersection of its inputs’ trust and the transformer’s authority. Storage access is by object capability or capability-mounted namespace, not by an ambient global filesystem.

Browser-like functionality is optional. If selected, network fetch, HTML, script, media, font, archive, document, and extension parsers are separate disposable sandboxes. Cookies and equivalent state are disabled unless a profile enables them; enabled state is partitioned by origin, bounded, inspectable, and expiring. Unselected browser and parser features must be absent from the image and attack surface.

## Network model

The default image has no network stack unless its deployment profile selects one. Selecting a NIC driver does not grant a process a network endpoint. Binding, listening, connecting, routing, multicast, raw frames, DNS, and administration are distinct capabilities.

Each network-facing parser has fixed input, queue, memory, action, and retry budgets. Authentication occurs before command dispatch. The ACI remains the single authorization point after transport authentication, so a valid TLS connection is not command authority. Network failure and flood paths cannot consume RT-reserved CPU, memory, or queue capacity.

## Opt-in drivers and measurable absence

Drivers are separately selected components, not a permanently linked compatibility ocean. For an unselected driver, a release report proves:

- 0 linked executable and read-only data bytes;
- 0 registered interrupts;
- 0 DMA or MMIO grants;
- 0 capabilities or handles;
- 0 queues or worker tasks;
- 0 device-parser entry points;
- 0 network listeners.

A selected driver is signed, manifest-bound to a device class and hardware identity, runs outside the kernel trust boundary, and can be killed and recreated without preserving stale DMA, IRQ, memory, or capability state.

## Fable-class adversary definition

“Fable-class” is a **TinyOS project-defined adversary class, not an external certification or formal industry standard**. It means a frontier AI agent capable of long-horizon autonomous work, vulnerability discovery and validation, reconnaissance, exploit-chain construction, lateral-movement planning, tool use, adaptive retries, rewinding, and parallel probing at machine speed.

This interpretation is grounded in Anthropic’s published Fable 5 material, which describes strong autonomous and agentic cybersecurity capability, including reconnaissance, discovery, exploitation, and lateral movement, and evaluates automated attacks across hundreds of turns. Anthropic also states that layered safeguards can fail and that complete jailbreak robustness is probably impossible. TinyOS therefore does not rely on the attacking model provider’s classifier or alignment:

- [Claude Fable 5 and Claude Mythos 5](https://www.anthropic.com/news/claude-fable-5-mythos-5)
- [Fable 5 cyber safeguards and jailbreak framework](https://www.anthropic.com/news/fable-safeguards-jailbreak-framework)
- [Redeploying Claude Fable 5](https://www.anthropic.com/news/redeploying-fable-5)

SEC-16 tests campaigns, not isolated prompts. A campaign may retain observations, vary encodings and timing, split an exploit chain into benign-looking steps, probe denials, race revocation, exhaust queues, and coordinate multiple agents. Passing means the architecture preserves its capability, memory, timing, provenance, and recovery invariants throughout the campaign—not merely that one request was blocked.

## Assurance and performance coupling

The performance catalogue’s G19–G23 rows are security-under-load gates: isolation under competition, bounded denial, fault containment, soak stability, and observability overhead. They are not secondary benchmarks. Every mapped Story must show that:

- high throughput does not enlarge authority;
- low latency does not bypass validation;
- denial remains bounded and leaves no partial state;
- exhaustion preserves RT reserves and recovery;
- spoors remain ordered and attributable under wrap pressure;
- absent components remain absent from the image.

Linux and RTOS comparisons are blocked until these safety-equivalent checks are enabled on both sides. TinyOS does not earn a benchmark advantage by omitting the control whose cost is being measured.
