# TinyOS Security Charter — Protection Domains, Remote-Code Exclusion, and Application Runtimes

Status: **Governing architecture and release charter. Runtime evidence is incomplete.**

This charter is subordinate only to the fixed founding intent in [`SeedMVP.md`](SeedMVP.md) and the safety-first priority order in [`agent/CODING_STANDARDS.md`](agent/CODING_STANDARDS.md). Every TinyOS design, Feature, Story, Test, Report, deployment profile, and release must preserve it. A functional result, latency target, compatibility request, administrative convenience, or emergency deadline cannot waive it.

## Charter outcome

TinyOS shall make compromise of one process insufficient for compromise of another process or of the system. It shall prevent remote-origin bytes from becoming executable except through one complete, fail-closed code-admission chain. Code that passes that chain still receives no ambient authority.

“Iron clad” is not used as an untestable claim that defects or covert channels are impossible. In TinyOS it means:

1. there is exactly one authorised transition from external bytes to executable pages;
2. every stage has a mandatory deny result and leaves the object non-executable on failure;
3. no remote caller can add a trust root, bypass policy, write another process, request writable executable memory, load a driver, or dispatch bytes as code;
4. compromise of one non-C0 component leaves every ungranted memory, capability, device, network, storage, scheduling, and persistence boundary intact;
5. release is blocked until adversarial evidence demonstrates those properties on the actual deployment profile.

The canonical machine-readable contracts are:

- [`goals/security/protection-domain-contracts.tsv`](goals/security/protection-domain-contracts.tsv) — the process-isolation and resource-isolation invariants;
- [`goals/security/code-admission-gates.tsv`](goals/security/code-admission-gates.tsv) — the only permitted data-to-code transition;
- [`goals/security/class-communication-matrix.tsv`](goals/security/class-communication-matrix.tsv) — every C0–C4 source/target path and authority-transfer rule;
- [`goals/security/controls.tsv`](goals/security/controls.tsv), [`containment-classes.tsv`](goals/security/containment-classes.tsv), and [`containment-tests.tsv`](goals/security/containment-tests.tsv) — the existing release controls and adversarial evidence.
- [`goals/context/application-platforms.tsv`](goals/context/application-platforms.tsv) — concrete application, language-runtime, compatibility, browser, game, remote-UX, fleet, and browser-hosted destinations;
- [`goals/context/landing-zones.tsv`](goals/context/landing-zones.tsv) — the machine-readable join keeping goals, performance domains, applications, security controls, containment classes, roadmap horizon, and claim gates side by side.

`xtask check-assurance-spine` rejects a missing, incomplete, malformed, or disconnected charter catalogue.

## Whole-system steering rule

TinyOS is not optimised as an isolated kernel benchmark. It is steered as four coupled planes:

1. **Goals** state why a capability exists and which destination it serves.
2. **Performance** applies the relevant domains from the 625-test catalogue to the complete workload, including security-denial and hostile-load costs.
3. **Applications** name the real programs, frameworks, runtimes, games, browser, host tools, remote UX, and fleet workloads that must make the architecture useful.
4. **Security** states where each component runs, which authority it may receive, which external bytes it accepts, and how compromise is contained.

A design is incomplete if any plane is missing. A performance optimisation without its application workload and security invariant is not accepted. An application promise without performance and containment selections remains a horizon goal. A security boundary that cannot meet its declared latency, cycle, memory, allocation, queue, and footprint guardrails remains design debt rather than a viable product boundary.

The 8 MiB non-driver image ceiling applies to the minimal core profile, not to every optional application package added together. Chrome-class browsers, language runtimes, Linux compatibility, codecs, games, and GPU stacks are separately signed, separately measured profile components. When unselected they contribute zero linked bytes and zero live authority.

## One lightweight isolation primitive

TinyOS uses a **Protection Domain**, not a heavyweight container or a separate guest OS, as its normal sandbox:

```text
ProtectionDomain
├── private active address-space root and architecture tag
├── kernel-owned capability-space root
├── scheduling context: budget, period, affinity and priority ceiling
├── memory, IPC, queue, grant and action quotas
├── immutable provenance and signer identity
├── lifecycle generation
├── containment class C1–C4
└── fault and teardown endpoint
```

Containment class, capability authority, scheduling criticality, and provenance are independent. The class selects a failure and evidence posture; it grants neither priority nor authority.

The same primitive serves a C2 driver, C3 application, and C4 disposable parser. Flexibility comes from changing explicit capabilities and budgets, not from weakening the boundary or adding a second privileged mechanism.

## Protection Domain invariants

Every runnable non-C0 component obeys all applicable `PD-*` contracts. The load-bearing rules are:

1. **Private active memory.** The scheduler activates the selected domain’s page tables before any of its instructions execute. User mappings cannot address another domain or writable kernel memory.
2. **Kernel-derived identity.** The kernel derives the caller from the currently executing TCB and domain. A caller-supplied PID, address, filename, port, device id, or class is never authority.
3. **Empty authority first.** A new domain begins with an empty capability space. The launcher may install only the rights-reduced intersection of a signed manifest and current policy before first execution.
4. **Executable sealing.** No page or physical alias is writable and executable. User code has no `mprotect`, JIT, debug-write, process-write, or loader primitive capable of creating executable bytes. Only the code-admission service may request a sealed executable mapping.
5. **Mediated communication.** Cross-domain calls use typed, bounded, capability-checked IPC. Cross-process pointers are not an ABI. Ordinary messages transfer data, never authority.
6. **Explicit sharing.** Bulk zero-copy uses participant-bound, rights-sized, generation-safe page grants. Read-only is the default; mutable sharing has one declared writer and deterministic revocation.
7. **Temporal isolation.** CPU execution is constrained by a budget and period, independently of class and static priority. A depleted domain is not runnable until replenishment.
8. **Caller-funded service work.** Work requested from a shared C2 broker is charged to the requester’s scheduling and queue budget, except for a small separately bounded recovery reserve. One client cannot consume another client’s service allowance.
9. **Finite ownership.** Pages, objects, messages, grants, retries, parser depth, model actions, and outstanding calls are charged to a domain quota. C3/C4 cannot consume C0/C1 or admitted RT reserves.
10. **Device isolation.** A C2 driver receives only device-bound, current-generation MMIO, IRQ, and DMA grants. IOMMU enforcement or wiped bounce buffers prevent device access to unrelated memory.
11. **Provenance confinement.** Copy, rename, extraction, conversion, compilation, IPC, or model generation cannot raise trust. Persistent or privacy-relevant state remains origin-partitioned and capability-scoped.
12. **Fault containment.** A user fault stops or reports the faulting domain; it does not silently continue, corrupt a peer, or panic the kernel. Complex hostile formats never enter C1.
13. **Revoke before reuse.** Termination stops scheduling, closes ingress, revokes capabilities/IPC/memory/DMA/IRQ/MMIO, invalidates translations, wipes private state, and advances generations before any resource identifier is reused.
14. **No ambient namespace.** There is no global “open any file,” “inspect any process,” “bind any port,” “load any driver,” or “administer the machine” interface. Access begins from an explicit object or endpoint capability.

## The only remote-data-to-code path

Remote packets, local host traffic, downloads, model output, files, archives, updates, and deploy payloads begin as non-executable C4 data. Transport authentication proves a peer identity; it does not make that peer’s bytes executable and does not grant command authority.

```text
hostile bytes
  ↓ bounded C2 transport service
quarantined immutable C4 object + origin
  ↓ disposable bounded C4 parsing and canonicalisation
content hash + detected type + complete dependency closure
  ↓ signature, trust-path, revocation and anti-rollback verification
signed manifest ∩ deployment policy
  ↓ destroy the inspection domain
fresh private C3 domain with empty capability space
  ↓ sealed RX code / RO data / NX writable data / guard pages
explicit kernel activation with recorded identity and generation
```

Every arrow is a separately testable `RCG-*` gate. Failure at any gate:

- leaves the object quarantined and non-executable;
- creates no partial executable mapping, capability, registration, listener, task, driver, or persistent state;
- records an attributable denial;
- consumes only the attacker’s bounded resources;
- cannot fall back to a less strict loader or development path.

### Mandatory remote-code exclusions

- Network, HBP, WCI, ACI, shell, model, file, and deploy parsers can produce data objects only. None can jump to, map, patch, or register their payload as code.
- Executable permission is not a general memory right. It is a kernel-mediated result available only for an immutable object that completed every `RCG-*` gate.
- Writable-to-executable transition, executable-to-writable transition, writable executable aliases, in-place binary patching, remote debugging writes, process-memory writes, and self-modifying code are absent from production profiles.
- JIT compilation is absent by default. A future profile that needs generated code must use a separate C4 producer and the complete admission chain to create a new immutable object; it receives no in-place W→X exception.
- TXE executables, TON libraries, drivers, boot images, and updates are content-addressed. Every dependency is pinned by hash and signer; name lookup alone cannot select executable content.
- C4 never promotes in place. Validation destroys the inspecting instance and creates a new C3 instance with new mappings, handles, queues, capabilities, and generations.
- Remote deployment can stage an object but cannot activate it. Hot deployment creates a fresh C2/C3 domain after verification; core deployment becomes eligible only through C0 verified boot and monotonic A/B recovery.
- Trust-root enrollment, recovery-policy changes, signer-authority expansion, rollback-counter reset, and audit-key replacement require a local physical/recovery ceremony. No standing remote endpoint can perform them.

These rules block classic injected shellcode and binary substitution. Code-reuse exploitation inside a compromised process is contained by the same capability, memory, scheduling, device, and persistence boundaries: controlling a process does not create a capability it did not already possess.

## Application and language-runtime charter

“Supports an application” means the application passes the same Protection Domain, code-admission, resource, provenance, teardown, and performance gates as native TinyOS code. It never means importing that application's runtime into C1 or treating a language runtime's own permission system as the OS boundary.

### Native and ahead-of-time paths

- Rust applications compile to the versioned TinyOS ABI and enter as signed TXE/TON objects in C3.
- Go applications, including Wails backends, are ahead-of-time compiled and receive no implicit filesystem, network, process, device, clock, or UI authority from the Go runtime.
- Production .NET 10-or-later applications prefer self-contained Native AOT. Native AOT's lack of runtime JIT is compatible with executable sealing, but it does not itself grant trust.
- C# managed memory safety is defence in depth, not process isolation. Reflection-based code generation, `Assembly.LoadFile`, unrestricted P/Invoke, name-only native-library loading, COM/DCOM, remoting, debugger process access, and generic process injection are absent from the production profile.
- Approved native interop targets are signed, hash-pinned TON objects selected in the application's manifest. The broker validates the exact exported ABI and rights; a string naming a DLL or shared object is never authority.

### JavaScript, webview, and generated-code paths

- Node.js, Bun, V8, JavaScriptCore, webviews, and browser engines are optional C3/C4 runtime systems, never kernel facilities.
- Runtime permission flags and framework ACLs are useful inner defences, but TinyOS assumes runtime code may bypass them. The kernel Protection Domain and capabilities remain authoritative.
- Production support begins JITless. Native addons, FFI, WASI, inspector/debug endpoints, child-process spawning, shell execution, worker creation, lifecycle scripts, and dynamic native loading are independently absent unless a signed profile grants a narrower broker operation.
- If a later performance profile requires generated native code, the running application cannot map it. A disposable C4 code producer emits a content-addressed candidate; the candidate traverses every `RCG-*` gate; the producer is destroyed; and a fresh C3 domain receives sealed executable pages. Content-addressed admitted code may be cached read-only, but no in-place W→X transition or writable executable alias exists.
- Wails and Tauri local UI assets may receive only their signed manifest's typed commands. A local webview that navigates to a remote origin loses local application IPC authority; remote web content is a separate C4 renderer, not a trusted continuation of the local frontend.

### Chrome-class browser

A Chrome- or Chromium-class browser is an opt-in compartment system, not one powerful process:

- the browser controller is C3 and has no ambient machine authority;
- each remote site renderer is C4 and receives no direct filesystem, network, device, secret, process, or application-bridge access;
- network, storage, GPU, media/codec, download, certificate, and secret operations are narrow C2 brokers assumed compromisable;
- profiles, cookies, caches, identifiers, service workers, and permissions are origin-partitioned, bounded, inspectable, and expiring;
- downloads remain quarantined C4 objects, and extensions, codecs, native messaging, remote debugging, and generated code are absent unless independently admitted;
- compromise of a renderer, browser controller, codec, GPU service, or network service must still be insufficient for cross-domain memory access, trust enrollment, code admission, persistence, or system takeover.

### Compatibility, host coexistence, and browser hosting

- The **TinyOS Linux Environment (TLE)** is a future C3 compatibility guest or syscall personality. It is not “WSL2 running inside TinyOS,” and it receives private process, storage, network, and device namespaces rather than a Linux-shaped ambient-authority escape hatch.
- **Windows TinyOS Tools (WST)** is the Windows-side HBP integration surface for a simultaneously running TinyOS partition or guest. HBP shared memory, virtio/vsock, or Hyper-V sockets carry typed messages and explicit object grants; WST cannot mount every TinyOS file, inspect every process, forward every port, or write TinyOS memory.
- A TinyOS build may run inside a browser through WebAssembly or emulation for learning, demonstrations, portable conformance, and development. The host browser remains an outer security and scheduling boundary. Such a build cannot satisfy bare-metal interrupt, DMA/IOMMU, verified-boot, hard-RT, or HIL evidence and must not be marketed as equivalent.

### Games, networking, remote UX, and fleets

- Dangerous Dave, DOOM, Quake II, and Quake III are proving workloads for the application ABI, frame pacing, input, audio, graphics, storage, networking, and fault containment. Game data and server downloads are data; mods or native modules do not bypass code admission.
- Multiplayer, TinySpot remote desktop, HBP/WCI, and fleet/data-centre workloads use the same endpoint-capability model. Bind, listen, connect, discover, route, administer, raw-frame, clipboard, input, file-transfer, telemetry, and command rights are separate.
- No TCP/IP implementation creates a default listener. Each C3 application receives a private network namespace and only the endpoints declared by the signed manifest and local deployment policy.
- Spoors coordinate and audit work; they do not grant authority. A remote node, fleet coordinator, browser, host service, language runtime, or model cannot turn a spoor, socket, filename, process ID, or pointer into a capability.

## Hostile takeover resistance

TinyOS assumes an attacker can fully control one C2, C3, or C4 component. The release property is that this is still not system control:

- C1 contains no network, executable, file, document, model, archive, or variable-length device parser.
- A compromised domain cannot name or map peer memory.
- It cannot forge a kernel capability or raise its class, priority ceiling, budget, provenance, or signer.
- It cannot make a listener, route, file, device, DMA window, secret, deploy action, or actuator appear without the corresponding capability.
- It cannot spend another domain’s CPU, queue, memory, retry, or service budget.
- It cannot survive termination through stale mappings, DMA, handles, queues, secrets, or reused identifiers.
- It cannot modify the immutable system image or replace the next boot with an unverified image.
- A single defect outside C0 is insufficient for durable whole-system compromise; seeded-defect campaigns verify this rather than assuming it.

Shared-hardware timing and power side channels require deployment-specific mitigation. Critical C0/C1 and hard-RT profiles may require core partitioning, SMT disablement, cache or memory-bandwidth partitioning, and restricted clocks. TinyOS promises deterministic denial of unauthorised state changes and bounded resource interference; stronger confidentiality claims require explicit hardware evidence.

## Frugality and latency rules

Security mechanisms remain on the performance catalogue rather than outside it:

- Protection Domain tables, capability slots, queues, and grants use fixed-capacity pools.
- Context switching is bounded; architecture tags such as PCID/ASID may preserve safe TLB entries without weakening separation.
- Immutable code and read-only model data may share physical pages; writable state remains private.
- Small control messages copy through bounded IPC; large payloads use explicit page loans.
- Brokers are shared services with narrow endpoints, not duplicated per sandbox.
- Unselected drivers, parsers, protocols, and runtimes are absent from the image.
- Performance evidence fails if low latency was obtained by skipping validation, widening a grant, sharing mutable state, retaining stale authority, or consuming another domain’s reserve.

## Release evidence

A release profile cannot claim this charter until dated raw evidence proves at least:

1. every ungranted cross-domain read, write, execute, remap, IPC, and capability operation fails with victim state unchanged;
2. remote packets and payloads cannot create executable mappings through any parser, protocol, error, recovery, debug, deploy, or compatibility path;
3. signature, dependency, signer-revocation, rollback, manifest, provenance, and policy failures leave zero executable or activated residue;
4. queue, CPU, allocation, parser, grant, retry, and service floods preserve admitted RT work and peer budgets;
5. domain death revokes memory, endpoints, capabilities, DMA, IRQ, MMIO, queue state, and secrets before reuse;
6. a compromised driver and malicious device cannot access ungranted memory or interrupts;
7. unselected components have zero linked bytes and zero runtime registrations or authority;
8. seeded single defects and long-horizon adaptive campaigns do not cross an ungranted class boundary or establish persistence.

Until those Reports exist, the charter is a binding construction contract and the assurance state remains `baseline-debt`, not a security certification.

## Design foundations

TinyOS adopts the object-capability and temporal-isolation principles documented by the [seL4 capability model](https://docs.sel4.systems/Tutorials/capabilities.html) and [MCS scheduling contexts](https://docs.sel4.systems/Tutorials/mcs.html), while retaining its own architecture and evidence requirements. x86_64 memory protection and process-context mechanisms follow the [Intel system-programming architecture](https://www.intel.com/content/www/us/en/developer/articles/technical/intel-sdm.html); device memory is subject to [Intel VT-d](https://www.intel.com/content/www/us/en/content-details/868911/intel-virtualization-technology-for-directed-i-o-architecture-specification.html) where available. CHERI-class hardware is a future optimisation for finer-grained compartmentalisation, not a substitute for this charter’s Protection Domain boundary.

Application-runtime policy is informed by the actual upstream execution models rather than framework names: [.NET Native AOT](https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/) removes runtime JIT but retains explicit native-interop considerations; [Node's permission model](https://nodejs.org/api/permissions.html) explicitly does not claim to contain malicious code; [Tauri](https://v2.tauri.app/security/) and [Wails](https://v3.wails.io/concepts/architecture/) bridge native backends to system webviews; [Chromium](https://www.chromium.org/developers/design-documents/multi-process-architecture/) depends on multi-process renderer isolation and brokers; and [WebAssembly](https://webassembly.org/docs/security/) remains subject to its embedding browser's security policy. TinyOS reuses compatible ideas but always enforces its own Protection Domain and admission contracts underneath them.
