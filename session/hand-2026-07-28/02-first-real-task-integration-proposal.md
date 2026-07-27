# Handover 02 — Proposal: "The First Real Task" — Stop Proving Mechanisms in Isolation, Wire Them Together

Follows: [`01-story-p1-03-01-completion-and-reconciliation.md`](01-story-p1-03-01-completion-and-reconciliation.md). Decision-record / scoping proposal, same shape as [`hand-2026-07-27/03-le-09-arm64-pi5-slice-proposal.md`](../hand-2026-07-27/03-le-09-arm64-pi5-slice-proposal.md) — no code changes, a plan for the user to weigh in on.

## The pattern, stated plainly

Every mechanism this project has built since `EPIC-P0` started — scheduler, context switch, pools, IPC, spoor, the PE64 loader, the Win32 shim, real fault handling, and now per-task `CR3` switching — was proven **in its own isolated Tier 0 fixture**, never together, and never in the real boot path. Concretely, checked just now:

- `os/src/kernel/src/main.rs`'s real (non-fixture) `kernel_main` does ACPI topology discovery, PCI enumeration, and halts. **It never creates a task.** No `Scheduler`, no `dispatch::run_once` call, no `AddressSpace`, exists anywhere on the real boot path.
- `kernel::dispatch::run_once` calls `context::switch` — **not** `context::switch_address_space`. `STORY-P1-03-01` built the CR3-switching mechanism and proved it in its own fixture; the actual dispatcher this kernel would use in production has no idea it exists.
- `kernel::spoor_journal::SpoorJournal` appears in exactly one place outside test code: `dispatch.rs`'s own test module. Four Stories (`FEAT-P0-06`, all four) built spoor's encoding, journal, and two adoption sites (`PriorityInheritingLock`, `wcet::record_tick`) — and every one of those adoption sites' own Reports says the same thing: *"no production call site or capacity constant added yet — no dispatcher is wired into `main.rs`."* That sentence is still true today, three EPIC-P1 Features later.
- `exec`'s own `blue-sharc-fixture` already proves a **real** executable — `blue-sharc.exe`, repacked through the real `xtask pack-txe` pipeline, parsed through the real PE64 loader, mapped through the real `AddressSpace::create` — loads correctly under QEMU. It has never been scheduled. It has never had its own `CR3`. It has never faulted and been contained while running as a task rather than as a one-shot fixture body.

None of this is a defect in any individual Story — each one correctly scoped itself to proving one mechanism cleanly, and several Reports (`STORY-P0-06-03`, `-04`; `STORY-P1-03-01`) explicitly flagged the missing production wiring as a named, deliberate gap rather than an oversight. But the accumulation is now the single largest gap between "TinyOS has verified mechanisms" and "TinyOS is an operating system that runs something." `SeedMVP.md`'s founding intent is a system that runs real workloads under real containment; nothing built so far has yet run *anything* end to end outside a hand-built fixture body.

## The proposal

**Make the next unit of work an integration, not another isolated mechanism.** Concretely: extend `STORY-P1-03-02` (already the next-scheduled Story, already about to touch `AddressSpace` teardown and W^X) so that its own Tier 0 fixture *is* the first real task — not one more synthetic two-task probe, but `blue-sharc.exe`, scheduled, running under its own real, W^X-correct, per-task `CR3`, dispatched through the real (now CR3-aware) `dispatch::run_once`, with a real fault from inside it caught by the real fault handler and audited through spoor's first production call site.

This is deliberately framed as **extending the next Story already in the plan**, not inventing a seventh Feature or a new Epic detour — `FEAT-P1-03`'s own exit criteria already require "a task provably cannot touch another task's memory" and "W^X violations fault," and the most honest way to prove both is against a real loaded image with a real capability boundary (the Win32 shim's existing `CapabilityPolicy` allowlist), not another fixture that fabricates its own victim.

### Concretely, what would change

1. **`kernel::dispatch::run_once` gains `CR3` awareness.** Today it calls `switch`; it needs a variant (or a parameter) that reads `Tcb::address_space` and calls `switch_address_space` when the selected task has one, falling back to the plain `switch` when it doesn't (`None`, the default every existing task still gets — no behavior change for anything that doesn't opt in). This is the one piece of new *mechanism* code this proposal needs; everything else is wiring already-built pieces together.
2. **`STORY-P1-03-02`'s W^X kernel mappings become the thing every task's tree shares**, replacing the `address-space-switch-fixture`'s own explicit, documented stand-in (an 8 MiB all-RWX identity replica built fresh per space). This was already `STORY-P1-03-02`'s job; this proposal just names the real consumer that makes "shared, not duplicated" load-bearing rather than a nice-to-have.
3. **The real boot path creates at least one real task**: parse `blue-sharc.exe` (already TXE-packed via `xtask pack-txe`), build its `AddressSpace` (the real loader pipeline, not a fixture's hand-built section list), attach its `CR3` via `Scheduler::set_address_space`, and dispatch it via the now-CR3-aware `run_once` — replacing (or running alongside) today's "discover ACPI/PCI, then halt" real boot path.
4. **A real fault from inside that task is contained live**, using the Win32 shim's existing capability boundary (an out-of-allowlist call, or a buffer bounds violation `STORY-P0-05-03` already defends against) as the *real* adversarial trigger — not a hand-placed `ud2` or a wholly-unmapped probe address, which is what every fault fixture so far has used. This is a materially harder and more honest claim: the first fault this project catches that comes from a real workload's real defect shape, not a fixture built to fault on cue.
5. **Spoor gets its first production call site.** The contained fault (already spoored by `kernel::fault::audit`) and the dispatch round that ran the task are the natural first candidates — wiring one of them in finally closes the sentence four Reports have now repeated.

### What this is not

Not a rewrite of the real boot path's ACPI/PCI discovery (that stays; a real task boots *alongside* or *after* it, not instead of it — topology discovery is still how the kernel would find real hardware). Not `FEAT-P1-03-02`'s W^X/teardown work replaced — this proposal needs that work's real output (shared, correct kernel mappings) as its own prerequisite, not a substitute for it. Not full preemption (`FEAT-P1-04`'s charge; this is still cooperative dispatch, per `dispatch.rs`'s own scope note). Not `FEAT-P1-06`'s flagship decision-to-actuation bound — this is the integration `FEAT-P1-06` will eventually need something real to actuate *from*, not that Feature itself.

## Why this, and why now

Every prerequisite this needs already exists as proven, Verified mechanism: real fault containment (`FEAT-P1-02`, complete), real per-task `CR3` switching (`STORY-P1-03-01`, complete), a real loader for a real executable (`FEAT-P0-05`, complete, with `blue-sharc-fixture` as living proof), a real capability-scoped compatibility shim (`STORY-P0-05-03`, complete), and a real spoor atom with nowhere left to actually stand (`FEAT-P0-06`, complete but production-inert). The only genuinely new code this proposal needs is `dispatch::run_once`'s `CR3` awareness — everything else is connecting wires that are already built and already tested, just never plugged into each other. That is exactly the shape of work most likely to surface real integration bugs cheaply (the `AddressSpace::drop` finding two handovers ago is the pattern: isolated pieces each pass their own tests and the bug lives entirely in the seam between them) — and it is the most direct answer to "make the OS real" available without inventing new scope.

## What this would prove, stated as claims

- TinyOS can load a real, externally-authored executable into its own protected address space and run it as a scheduled task.
- A real defect in that task's own code — not a fixture's staged trigger — faults, is captured, and is contained without taking the rest of the system down.
- The audit trail for that event (spoor) actually exists in the running system, not only in a test double.

That is a materially different, stronger claim than anything filed so far, and it is the first point at which "TinyOS boots and does something" stops being a sentence about isolated fixtures.

## Decided (user, 2026-07-28)

1. **Fold into `STORY-P1-03-02`.** The real-task integration is this Story's own subject, not a separate claim — its exit criteria already implied a real thing to test W^X/teardown against.
2. **Out-of-allowlist Win32 API call** is the adversarial fault trigger — exercises `STORY-P0-05-03`'s closed-allowlist `CapabilityPolicy` directly under a real, un-staged call.
3. **The real boot path gains a second branch.** ACPI/PCI topology discovery stays exactly as it is; real task scheduling runs as an additional step afterward, so `TEST-P0-04-01-A`/`TEST-P0-04-03-A`'s existing assertions are untouched.

This handover proposed; the user decided the same session. `STORY-P1-03-02.md`'s acceptance criteria are finalized against these three decisions before implementation starts.
