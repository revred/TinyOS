# STORY-P1-03-01 — Per-Task `CR3` Switching in the Context Switch

Status: **Verified (Tier 0 + Host), 2026-07-27** — assurance state `baseline-debt`; mechanism only, not wired into the real dispatch path (see acceptance criterion 1's own scope note)
Feature: [`FEAT-P1-03`](../features/FEAT-P1-03.md)
Introduced in: [`session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md`](../../session/hand-2026-07-26/36-epic-p1-determinism-proof-decomposition.md)
Implemented in: [`session/hand-2026-07-27/10-story-p1-03-01-cr3-switching.md`](../../session/hand-2026-07-27/10-story-p1-03-01-cr3-switching.md)

## Description

Install a task's `exec::AddressSpace` into `CR3`: the TCB gains an address-space handle (`Tcb::address_space`/`Scheduler::set_address_space`), a new `kernel::context::switch_address_space` loads the incoming task's `CR3` (skipping the reload when unchanged, per `hal_x86_64::paging::cr3_reload_needed`'s pure comparison — same-space switches must not pay the TLB cost), and a real Tier 0 fixture proves two genuinely distinct address spaces switch correctly and isolate a task that probes the other's private memory.

**What this Story does not do.** Wiring `switch_address_space` into the real dispatch loop, replacing the boot identity map for every scheduled task, needs W^X-correct kernel mappings *shared* across every task's tree first — building that safely, and demoted-boot-identity-map-as-bootstrap-only, is `STORY-P1-03-02`'s charge. This Story delivers the switching mechanism and proves it against two hand-built address spaces in its own fixture, not a production integration.

## Depends on

`STORY-P1-02-01`/`-02` (hard — both Verified 2026-07-27, unblocking this Story the same day); `STORY-P1-01-01` (D04 baseline, though the measured same-space-vs-cross-space delta itself is deferred — see acceptance criterion 2's own note).

## Acceptance criteria (final)

1. **Two tasks in distinct address spaces each run and switch under Tier 0; a cross-space memory probe from one *faults* and is contained by the `#PF` handler (the other task keeps running) — isolation proven adversarially, not inferred from the mapping tables.** **Met**: `--fixture=address-space-switch` builds two disjoint `exec::AddressSpace` trees (never two sections in one shared tree), confirms their `CR3` values are genuinely distinct, switches into task A (a real `CR3` reload, read back from the register), which faults for real reading task B's wholly-unmapped private address and is terminated exactly as `fixture_fault`'s victims are, then switches into task B (another real, distinct reload) which runs to completion. Scoped narrower than the original draft: this proves the *mechanism*, not a production dispatch-loop integration — see this Story's own Description.
2. **Same-space switches skip the `CR3` reload, and the measured D04 delta between same-space and cross-space switches is recorded against the catalogue budget.** **Met differently than drafted**: the *skip* decision is proven as pure, host-tested logic (`cr3_reload_needed`), independent of hardware; a measured D04 same-space-vs-cross-space delta is **not** produced, since nothing in the real dispatch path installs a per-task address space yet (acceptance criterion 1's scope note) — measuring a switch cost the production system doesn't yet pay would misrepresent Tier 0 fixture overhead as a real scheduling cost. Deferred to whichever Story first wires this mechanism into `kernel::dispatch::run_once`.

## Tests

[`TEST-P1-03-01-A`](../tests/TEST-P1-03-01-A.md) — six clauses. Written alongside implementation, not before it — see that Test document's own process note.

## Reports

- [`REPORT-2026-07-27-08`](../reports/REPORT-2026-07-27-08.md) — the Tier 0 capture, the `AddressSpace::drop` finding, and what remains open.

## Goals verified

G-SEC-2 (active address spaces — mechanism only, not yet the running system's default). G-RT-1 (switch cost) is **not** verified: no measured delta exists (criterion 2).

## Named debt this Story leaves open

Everything acceptance criterion 1's scope note and the linked Test document's clause 6 name: no production dispatch-loop wiring, no W^X enforcement (the fixture's kernel-replica mappings are all-RWX), no teardown (the fixture's two spaces are deliberately leaked via `core::mem::forget`, since `AddressSpace::drop`'s existing teardown is not generation-safe and tearing either tree down mid-fixture would be exactly the bug this Story's own Report records finding). All three are `STORY-P1-03-02`'s charge.
