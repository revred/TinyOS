# TinyOS Security Current-State Review — 26 July 2026

Status: **Architecture and prototype audit; no security certification.**

## Executive finding

TinyOS has several good security primitives for an early Phase-0 skeleton: Rust-first `no_std` code, typed errors, finite data structures, a PE parser that rejects malformed inputs, W^X validation, page-table construction, transactional generation-stamped shared-memory grants, bounded local IPC prototypes, capability-policy traits, fixed-size spoors, a small image, and no implemented general network/browser/storage/driver stack to expose yet. The latest boot path also installs a complete IDT, retires the legacy PIC, arms a local-APIC timer, and routes every unserviced vector to a fail-closed non-returning handler.

Those are foundations, not the claimed security system. The governing [`Security Charter`](../../SECURITY_CHARTER.md), 14 Protection Domain contracts, 14 remote-code admission gates, 25-pair C0–C4 matrix, 19 application/platform targets, and nine whole-system landing zones are now machine-checked design constraints, but they are not runtime enforcement. The current boot identity-map is broad RWX; process page tables are built but never activated with a per-task CR3 switch; the IDT has no domain-aware `#PF`/`#GP` handling or TSS/IST and its default cannot terminate and reclaim one task; TXE files are unsigned; `AllowAllPolicy` stand-ins exist; there is no real ACI, verified boot, IOMMU programming, origin/entitlement metadata, storage namespace, network namespace, quarantine, immutable update path, secret store, sandbox teardown, or AI-campaign harness.

Accordingly, every current functional-Verified Story remains assurance `baseline-debt`.

## Security Charter activation audit

| Charter surface | Contracted target | Current blocking reality |
|---|---|---|
| `PD-01..PD-14` | Private active memory, unforgeable least authority, temporal/resource/device isolation, contained faults, complete teardown | Page-table data structures, bounded IPC/grants, pools, WCET bookkeeping and a boot-path IDT/APIC timer exist; active CR3, real capability space, scheduler tick consumption, domain-aware fault termination, TSS/IST, IOMMU and exit teardown do not |
| `RCG-01..RCG-14` | The only path from remote/external data to sealed executable pages and a fresh C3 domain | PE/TXE parsing and W^X validation exist; quarantine, signatures, dependency identity, revocation, anti-rollback, manifest-policy admission, executable sealing and actual scheduled C3 execution do not |
| 25-pair class matrix | Every C0–C4 path denied, one-shot, internal, or capability-mediated exactly as declared | The matrix is CI-enforced design data; runtime class identity and a production policy/capability engine do not exist |

No release may interpret catalogue validity as prevention of remote code injection. That claim requires exhaustive mapping/activation-path tests, negative signature and rollback corpora, active fault-contained domains, and proof that no debug, deploy, compatibility, recovery, or error path bypasses the admission chain.

## Five-class containment audit

The C0–C4 model is now present in the machine-checked Feature/Story/security contracts, but no runtime class claim is active yet:

| Class | Contracted target | Current blocking reality |
|---|---|---|
| C0 Root of Trust | Authenticated measured handoff with no runtime re-entry | No hardware trust anchor, signature verification, measurement, anti-rollback, or proven removal of boot mappings |
| C1 Trusted Kernel Core | Minimal scheduler/MMU/IPC/capability/fault core with no hostile-format parser | IDT and local-APIC timer are active, but the broad RWX identity map remains; per-task CR3, domain-aware exception recovery/termination, TSS/IST and teardown are inactive; ACPI and executable parsing still run in their containing trust context |
| C2 Isolated System Service | Restartable user-mode drivers and service brokers with narrow grants | No driver/service process model, IOMMU, device grant lifecycle, network/storage stack, or restart/revocation path exists |
| C3 Sandboxed Application | Fresh signed process with empty authority and policy-subset capabilities | Address spaces are constructed but not scheduled active; TXE is unsigned; no manifest, ACI, sandbox lifecycle, or teardown exists |
| C4 Hostile Transient Domain | Disposable zero-authority parser/content/model sandbox with no in-place promotion | No quarantine, disposable process, brokered result channel, trust-preserving promotion, parser kill/recreate, or campaign harness exists |

Contract coverage is therefore a design/development guardrail, not runtime evidence. No Report may claim a class boundary implemented merely because its `C*` or `BND-*` row validates.

## Control-by-control audit

| Control | Current repository evidence | Blocking gap | State |
|---|---|---|---|
| SEC-01 verified boot | A/B and signing are specified; broken-boot fixture distinguishes failure | No boot signature verification, hardware trust anchor, measurement, revocation, or recovery proof | Unbuilt |
| SEC-02 signed TXE/TON | Deterministic TXE packer and hostile PE parsing exist | TXE has no content signature, origin, entitlement, anti-rollback counter, or revocation; TON is only a reserved name | Prototype debt |
| SEC-03 process isolation | `hal_x86_64::paging` constructs 4-level tables; loader rejects W+X sections and kernel-region collisions; the real boot path installs a fail-closed IDT | No live CR3 switch; boot retains a broad RWX identity map; context omits FS/GS, XSAVE, protection state and guard pages; `#PF`/`#GP` cannot yet terminate and reclaim one domain; no TSS/IST | Prototype inactive |
| SEC-04 shared-memory grants | `STORY-P0-07-02` is functionally Verified; `exec::shared_memory` checks owner mapping, target vacancy, kernel collision, permission non-escalation, owner-only revoke, and has a Tier-0 fixture | No generation-safe handle, grant registry, participant identity in token, zero-page rejection, atomic rollback on mid-map allocation failure, task-exit revocation, or real active address spaces | Baseline debt |
| SEC-05 sandbox-first authority | `CapabilityPolicy`/`ChannelPolicy` traits demonstrate dependency inversion | No sandbox lifecycle or ACI; `AllowAllPolicy` is present as a standalone default; loaded code never runs in a fault-contained process | Stand-in only |
| SEC-06 origin/entitlement | Spoor can carry actor/action/target attribution | No immutable object origin, signer, trust, derivation, quarantine, or entitlement labels and no propagation engine | Unbuilt |
| SEC-07 storage namespaces | Win32 shim bounds-checks buffers and gates representative calls through a policy trait | File calls are stand-ins; no filesystem, object capability, mount namespace, traversal/link-race defense, or storage policy | Unbuilt |
| SEC-08 network/port isolation | Phase-0 local IPC deliberately creates no socket; no production TCP/IP stack exists | No network namespace, endpoint capability, firewall, parser, authentication, flood reserve, or zero-listener HIL scan | Absent by current scope |
| SEC-09 parser/browser isolation | PE and ACPI parsing use bounded validation and hostile fixtures | Parsers still run in their containing trust context; no disposable parser process, browser profile, JIT policy, kill/recreate path, fuzz corpus gate, or active-content boundary | Partial parser hygiene |
| SEC-10 download quarantine | No download or install subsystem exists | No quarantine bit, type detection, archive limits, verified promotion, consent, or installer authority model | Unbuilt |
| SEC-11 privacy/tracking | No browser/cookie stack exists | No formal opt-in profile, origin partitioning, expiry, inspection, device-identity restriction, or privacy conformance tests | Absent by current scope |
| SEC-12 ransomware/worm containment | Fixed capacities fail closed; future A/B rollback is specified | No immutable runtime system volume, snapshot store, mutation-rate detection, autorun denial test, lateral-propagation test, or implemented signed recovery | Unbuilt |
| SEC-13 opt-in drivers | Universal Driver Model and negative-footprint performance contract are specified | No driver crate/process/manifest/signature/profile link selection; current absence is not yet backed by automated zero-surface evidence | Architecture only |
| SEC-14 unified policy/spoor | Eight-byte typed `Spoor` and fixed-capacity journal exist; lock and WCET prototypes stamp events | No production caller, tamper-evident persistence, monotonic cross-core sequence, real ACI, actor authentication, critical-event retention policy, or report query path | Prototype debt |
| SEC-15 key isolation | HBP/WCI credential rules are specified | No key store, hardware-backed non-exportability, purpose binding, rotation/revocation implementation, TLS stack, or crash-dump proof | Unbuilt |
| SEC-16 Fable-class resistance | Bounded queues/pools and spoor primitives are useful foundations | No ACI action budget, campaign harness, cross-attempt correlation, honey capability, adaptive throttle, policy-kill path, or hundreds-turn red-team evidence | Unbuilt |
| SEC-17 signed atomic update | Deploy protocol specifies A/B recovery; `check-image-size` measures current kernel | No implemented deploy/update path, signature, anti-rollback monotonicity, revoked-key handling, atomic power-loss proof, or remote attestation | Unbuilt |
| SEC-18 DMA/IOMMU isolation | Page-table traits separate mapping logic from allocation | No IOMMU discovery/programming, DMA grant table, bounce-buffer fallback, IRQ ownership, hot-unplug generation, MMIO capability, or malicious-device test | Unbuilt |
| SEC-19 memory-safe core | Rust-first code, application `unsafe` prohibition, narrow HAL `unsafe`, typed parser errors, clippy/missing-doc gates | No Miri/sanitizer/fuzz/mutation CI, unsafe inventory gate, complete integer-boundary campaign, or proof for live paging/context behavior | Partial |
| SEC-20 exhaustion containment | Fixed pools, bounded IPC messages/queues, finite parser ceilings, typed Full/TooLong errors | Several capacity-zero cases can panic/modulo by zero; no global admission controller, RT reserve, decompression bomb corpus, retry budget, priority starvation proof, or bounded system recovery | Partial |

## Immediate release-blocking technical findings

1. **Activate the isolation that is currently only data.** Extend the now-active IDT with domain-aware `#PF`/`#GP` termination and TSS/IST, then add per-task CR3 switching, full security context save/restore, guard pages, teardown, and removal of broad boot RWX mappings before any untrusted entry point.
2. **Make shared-memory grant creation transactional.** Validate `pages > 0`; allocate/map through a rollback-capable transaction; bind owner and sharee generations; register grants for task-exit revocation; prove stale tokens cannot affect a reused task slot.
3. **Remove ambient allow-all from production composition.** Test-only allow policies can remain fixtures, but a production build must have no way to instantiate a process/channel without a real default-deny policy.
4. **Define signed executable metadata before executing TXE.** Content hash, signer, revocation, anti-rollback, origin/entitlement, capability manifest, imports, section digest, and deterministic verification result are prerequisites to jumping to a TXE entry point.
5. **Make critical spoors non-losable by ordinary ring pressure.** Define severity, reserved capacity, sequence, persistence, tamper evidence, cross-core ordering, and fail-safe behavior if the audit sink cannot accept a required event.
6. **Keep optional stacks physically absent.** Add automated link-map/symbol/registration evidence now, before drivers, TCP/IP, storage, browsers, codecs, and inference dependencies arrive.

## Whole-system destination audit

[`../context/application-platforms.tsv`](../context/application-platforms.tsv) and [`../context/landing-zones.tsv`](../context/landing-zones.tsv) now prevent framework, game, browser, managed-runtime, host-integration, fleet, and browser-hosted ambitions from bypassing the performance/security spine. They are design context only:

- Wails, Tauri, .NET, Node, Bun, games, Chrome, TinySpot, TLE, WST, fleet/data-centre workloads and browser-hosted TinyOS are not currently implemented or supported.
- Their selected `Dnn` domains expand to the existing 25 guardrails per domain; this creates future evidence obligations, not passes.
- The Security Charter makes the Protection Domain the boundary below CLR, Go, V8, JavaScriptCore, webviews, Chromium, WebAssembly and compatibility guests.
- Large optional application/runtime bundles remain outside the 8 MiB base image but must publish their own footprint and zero-presence evidence when unselected.

## What may be claimed today

- The repository has bounded prototype primitives and a small current kernel image.
- The committed performance and security catalogues are structurally complete and every current Story is mapped.
- Local IPC is designed not to create a network listener.
- PE mapping rejects writable-executable sections in the prototype path.

It may **not** yet be claimed that TinyOS is sandboxed, verified-booted, ransomware-proof, immune to cross-process tampering, near-zero attack surface as a finished OS, Fable-class resistant, better than Linux, or 10× better than RTOS baselines. Those are now testable release targets with visible debt.
