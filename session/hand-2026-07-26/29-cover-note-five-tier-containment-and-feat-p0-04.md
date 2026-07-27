# Handover 29 — Cover Note: Five-Tier Containment Model (New Directive) and `FEAT-P0-04` Remainder

Follows: [`28-story-p0-07-02-transactional-grants.md`](28-story-p0-07-02-transactional-grants.md).

This is a mandate for the next session, not a record of work done — no implementation code changed this session. Two threads are queued up together because the user asked for both to be addressed next: finishing `FEAT-P0-04`, and formalizing a new containment-tier model the user gave in full and wants made enforceable, not just architectural prose.

## Thread 1 — New directive: five containment tiers, not privilege rings

The user revised the security model's framing. Recorded here verbatim/near-verbatim so the next session formalizes the actual rule, not a paraphrase of it:

> Five containment classes, not conventional privilege rings. Capabilities must determine authority; the tier determines how strongly a component is isolated and verified.

| Tier | Recommended name | Contains | Default posture |
|---|---|---|---|
| 0 | Root of Trust | Hardware trust anchor, boot verifier, key measurement, earliest assembly | Immutable, minimal, formally specified |
| 1 | Trusted Kernel Core | Scheduler, MMU, IPC, capability enforcement, fault/IRQ routing | No external-format parsing; smallest possible TCB |
| 2 | Isolated System Services | Drivers, network/storage stacks, loader, crypto and device brokers | Assume compromise; user-mode, restartable, narrow MMIO/IRQ/DMA capabilities |
| 3 | Sandboxed Applications | Installed and signed programs, local inference runtime, administrative tools | Empty authority initially; signed manifest requests explicit capabilities |
| 4 | Hostile Content | Downloads, browser renderers, scripts, documents, model output and unknown executables | Quarantined, disposable, zero authority and strict resource budgets |

Two naming corrections the user was explicit about, and why:

- **Tier 2 is not "secure drivers."** Drivers routinely combine unsafe hardware access with hostile device input and must be treated as potentially compromised, not trusted by virtue of running in a driver slot.
- **Tier 4 is not "user level."** A human user is an authenticated principal, not inherently unsafe. Tier 4 means hostile or unverified execution/content specifically — downloads, renderers, scripts, documents, model output, unknown executables — not "anything a user runs."

**The load-bearing rule:** no tier automatically trusts another tier, and no tier number automatically grants authority. Worked example the user gave: a Tier 2 network driver may access one NIC's MMIO/DMA buffers but must not read user files, inspect other processes, install code, or invoke actuators; a Tier 3 real-time control application might hold a narrowly scoped actuator capability while still being unable to reach the network at all. Tier is isolation strength; capability is what's actually permitted — they are independently set, never inferred from each other.

**Four concepts the user requires kept independent** (do not collapse any pair of these into one axis when formalizing this):

1. **Containment tier** — how the component is isolated.
2. **Capabilities** — precisely what it may do.
3. **Scheduling criticality** — its CPU priority and timing budget.
4. **Provenance** — signer, origin, entitlement, and trust history.

**Required cross-tier rules** (these are the enforceable obligations the next session needs to turn into actual gates/tests, not just documentation):

1. No direct Tier 4 → Tier 0/1 calls; only fixed-format, capability-checked IPC.
2. Tier 4 cannot promote itself. Installation means termination, verification, and recreation as a new Tier 3 process — never an in-place privilege change.
3. Tier 3 cannot load drivers or grant itself network, storage, or process authority.
4. Tier 2 drivers receive only device-bound, generation-safe MMIO/IRQ/DMA grants.
5. Tier 1 never parses packets, files, PE images, documents, or model output — any such parsing happens strictly above Tier 1.
6. Every process starts with an empty capability set.
7. Copying or transforming data cannot raise its provenance tier.
8. Termination revokes memory, IPC, DMA, and device grants before resources are reused.

**Directed next step, in the user's own words:** "formalize it in the security spine as an enforceable tier-transition and communication matrix — not merely architectural terminology."

### How this lands on what already exists

This is genuinely new vocabulary — nothing in `docs/security-spine.md`, `goals/security/controls.tsv`, or `goals/security/current-state-review.md` currently uses "tier" or "ring" language; the 20 `SEC-*` controls are capability/invariant-based, which is compatible with (and does not need to be discarded for) this tier model. The work is additive: map each of the 20 existing `SEC-*` controls onto the tier(s) it governs, then encode the 8 cross-tier rules above as machine-checkable obligations analogous to how `goals/assurance/story-contracts.tsv` and `xtask check-assurance-spine` already make performance/security selection mandatory rather than aspirational. Concretely, next session should decide and produce:

- A `goals/security/tiers.tsv` (or equivalent) naming the 5 tiers and their default postures, mirroring `controls.tsv`'s own row-per-ID structure.
- A tier-transition/communication matrix — which tier may call which, by what IPC shape, under what capability check — expressed as data an `xtask` gate can validate, not prose alone.
- An explicit mapping of `STORY-P0-07-01`'s `kernel::ipc::Channel` and `STORY-P0-07-02`'s `exec::shared_memory` grants against rule 1 (fixed-format, capability-checked IPC is the *only* Tier 4 → Tier 0/1 path) and rule 4 (device-bound, generation-safe MMIO/IRQ/DMA grants for Tier 2) — `shared_memory`'s new `GrantRegistry` generation-safety (Handover 28) is a plausible starting primitive for rule 4's "generation-safe" requirement, but this needs to be checked against the driver/MMIO case specifically, not assumed to transfer unchanged from the task-to-task memory-sharing case it was built for.
- A read against `goals/security/current-state-review.md`'s existing blocker list (no IDT, no per-task `CR3` switch, no task-exit teardown) — several of those blockers are *exactly* what rules 3/6/8 above need to be enforceable at runtime, so this tier formalization and that already-tracked isolation work are the same underlying gap, not two separate backlogs.

## Thread 2 — `FEAT-P0-04` remainder (unrelated thread, also queued for next session)

`FEAT-P0-04` is 1/3 Stories Verified (`STORY-P0-04-01`, ACPI→topology parsing). Two Stories remain, both still at "draft acceptance criteria, not yet started":

- **`STORY-P0-04-02` — Interrupt controller (APIC) bring-up.** Local APIC + I/O APIC using `STORY-P0-04-01`'s MADT routing, replacing the legacy 8259 PIC. Draft AC: a timer interrupt through the local APIC fires at a bounded, QEMU-measured interval (not assumed from datasheet timing); spurious/unrouted interrupts get an explicit default handler, never silently dropped. This is also the IDT/interrupt-routing work the security-spine audit (Handover 27) already named as the largest outstanding isolation gap, and it directly gates cross-tier rules 3/6/8 above (fault containment, task-exit teardown, and any preemptive enforcement all need a working IDT first) — so this Story is now load-bearing for both `FEAT-P0-04`'s own exit criteria and the tier-formalization work in Thread 1.
- **`STORY-P0-04-03` — Minimal bus-enumeration pass.** Read-only PCI(e) config-space walk recording discovered devices into the topology model, groundwork for `EPIC-P3`'s class drivers — no driver bring-up, no device-state mutation. Draft AC: under QEMU's `q35` model, enumerate at least the host bridge and whatever the default machine exposes, recorded with vendor/device ID and bus/device/function position.

Both depend only on the already-Verified `STORY-P0-04-01`, need Tier 0 QEMU verification, and have no Test documents written yet (acceptance criteria are explicitly draft, per this project's "decompose just-in-time, finalize at pickup" convention). Note: this session's own environment had no QEMU binary available (see Handover 28's verification section) — if that's still true next session, `STORY-P0-04-02`/`-03` cannot be host-tested around; both Stories' acceptance criteria specifically require a real Tier 0 QEMU pass (measured interrupt timing, real PCI config-space enumeration), so QEMU availability should be confirmed first before committing to implementing either this session.

## Suggested order for next session

1. Confirm QEMU is available in the working environment (both threads below need it).
2. `STORY-P0-04-02` (APIC/IDT bring-up) — pick this first: it closes a `FEAT-P0-04` Story *and* directly unblocks the fault-containment/task-exit-teardown prerequisites Thread 1's cross-tier rules and Handover 27's own "activate the isolation that is currently only data" priority both already depend on. Doing it once, framed correctly, serves both backlogs instead of being redone later under tier-formalization pressure.
3. `STORY-P0-04-03` (bus enumeration) — independent of the above, can be done in either order.
4. Formalize Thread 1: `goals/security/tiers.tsv` (or equivalent) plus the tier-transition/communication matrix, wired into `xtask check-assurance-spine` or a new sibling gate, per the concrete deliverables list above.

## What this does not claim

- No code changed this session. `FEAT-P0-04` is still 1/3 Verified; the tier model is a directive received, not yet reflected anywhere in `goals/security/` or `docs/security-spine.md`.
- The suggested ordering above is a recommendation, not a decision the user has made — confirm before committing significant implementation time, especially since Story 2 below is larger than a typical single-session Story (IDT bring-up plus its downstream interactions with the scheduler's WCET/dispatch code, which have both been silently assuming no real timer interrupt exists yet).
