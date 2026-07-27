# TinyOS Security Control Catalogue

Status: **Specified; implementation evidence is incomplete.** These controls are release contracts, not claims that the current Phase-0 skeleton already satisfies them.

TinyOS does not inherit the general-purpose-OS assumption that every installed program, driver, file, port, parser, or user session participates in one ambient environment. Its default is the opposite: no executable authority, no shared memory, no network endpoint, no driver, and no trust transfer unless a narrow, revocable capability explicitly grants it.

[`../../SECURITY_CHARTER.md`](../../SECURITY_CHARTER.md) is the governing security contract. Its machine-readable spine consists of 14 [`Protection Domain invariants`](protection-domain-contracts.tsv), 14 mandatory [`code-admission gates`](code-admission-gates.tsv), and the complete 25-pair [`C0–C4 communication matrix`](class-communication-matrix.tsv), joined to the canonical 20 [`controls`](controls.tsv), five [`containment classes`](containment-classes.tsv), and 20 [`boundary contracts`](containment-tests.tsv). The expanded architecture and adversary model are in [`../../docs/security-spine.md`](../../docs/security-spine.md). Every Feature and Story is connected through [`../assurance/`](../assurance/).

The Charter now also governs the 19 application/platform targets and nine whole-system landing zones under [`../context/`](../context/). CLR, Go, Node, Bun, V8, JavaScriptCore, webviews, Chromium, WebAssembly, Linux compatibility, games and host tools remain inside OS Protection Domains; their own managed-memory, permission, ACL or sandbox mechanisms are defence in depth.

The honest code-level gap audit is [`current-state-review.md`](current-state-review.md).

## Containment classes are not privilege rings

- **C0 Root of Trust** verifies, measures, recovers, and transfers control. It has no reusable runtime command surface.
- **C1 Trusted Kernel Core** schedules, switches address spaces, mediates fixed-format IPC/capabilities, routes faults/interrupts, and enforces budgets. It parses no complex hostile format.
- **C2 Isolated System Service** contains drivers and device/network/storage/loader brokers. C2 is assumed compromisable and receives only narrow restartable grants.
- **C3 Sandboxed Application** contains signed installed applications. Every launch starts empty and receives only the intersection of manifest request and policy.
- **C4 Hostile Transient Domain** contains downloads, unknown executables, parsers, renderers, documents, scripts, and model output. It is disposable, quarantined, zero-authority, and never promoted in place.

Capabilities, scheduling criticality, provenance, and containment class are independent. A signed C2 driver is not trusted C1; a C3 RT program does not gain ambient authority from priority; a human is an authenticated principal rather than a containment class.

## Meaning of “secure”

“Defeats all attacks” and “zero attack surface” are not testable engineering claims. TinyOS instead uses falsifiable invariants:

- an unsigned or substituted image never becomes executable;
- one process cannot address another process’s memory;
- shared memory requires an explicit, rights-sized, revocable grant;
- untrusted objects retain origin and entitlement labels;
- network endpoints, drivers, parsers, and executable content are absent unless selected;
- compromise of one sandbox has a bounded resource and authority blast radius;
- destructive behavior is attributable, rate-bounded, reversible where storage permits, and unable to persist through verified boot;
- model output is always treated as attacker-controlled input, regardless of which model produced it.

The “near-zero attack surface” claim is therefore measured as **negative presence evidence**. For an unselected component, reports must prove zero linked executable bytes, zero registered interrupts, zero DMA/MMIO grants, zero ports, zero capabilities, zero queues, and zero reachable parser entry points. Idle-but-present does not count as absent.

Remote-code prevention has an equally concrete meaning: an exhaustive path audit finds no executable mapping or activation route outside `RCG-01..RCG-14`; each gate's negative corpus leaves zero executable residue; and fully attacker-controlled admitted code remains bounded by all applicable `PD-01..PD-14` invariants. Transport authentication, a valid signature, installation, or functional execution is never accepted as that evidence by itself.

## Release rule

A functional Story may be locally Verified while its assurance state remains `baseline-debt`. That distinction is intentional. No release or performance comparison may treat functional verification as proof of the mapped security controls.

For a Story to become assurance `verified`, its Report must include:

1. every mapped `SEC-*` control and its adversarial result;
2. every applicable `PERF-Dnn-Gnn` result or a justified, release-blocking deferral;
3. raw evidence, environment, toolchain, commit, capabilities, driver profile, and failure-state observations;
4. proof that the safety and security invariant held at the measured high-performance load.

The assurance integrity gate checks the Charter, Protection Domain/code-admission/communication catalogues, class/control/test catalogues, Feature and Story coverage, and references:

```text
cd os
cargo run -p xtask -- check-assurance-spine
```

It deliberately does not turn missing runtime evidence into a fake pass. Current debt remains visible until real reports close it.
