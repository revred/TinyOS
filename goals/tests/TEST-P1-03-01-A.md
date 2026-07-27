# TEST-P1-03-01-A — Two Real `CR3`s, Switched Per Task, Isolated by a Real Fault

Status: **Verified (Tier 0 + Host) — written alongside implementation, per this Story's own process note below**
Story: [`STORY-P1-03-01`](../stories/STORY-P1-03-01.md)
Tier: Host unit tests (the reload decision, the `AddressSpace::cr3` accessor, `Tcb` address-space bookkeeping) **plus** a Tier 0 QEMU fixture building two genuinely distinct address spaces and switching a real `CR3` between them, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D04`, `D08`
Security controls: `SEC-03`, `SEC-19`
Containment classes: `C0`, `C1`, `C3`
Boundary tests: `BND-04`, `BND-05`, `BND-20`
Protection Domain contracts: `PD-01`, `PD-04`, `PD-13`
Code admission gates: `RCG-10`, `RCG-11`
Assurance state: `baseline-debt`

## What this test is for

`exec::AddressSpace` has built and verified real x86_64 page-table trees since `STORY-P0-05-02` — but loading one into `CR3`, making it the CPU's live address space, was explicitly out of scope until a real fault handler existed to contain what a live per-task mapping could do wrong (`FEAT-P1-02`, now functionally complete). This Story closes that gap: a task's own `AddressSpace` becomes installable, `CR3` reloads only when the incoming task's space actually differs from the one already loaded, and a task confined to its own tree provably cannot read another task's private memory — the attempt faults for real and is contained by the same `kernel::fault` machinery `STORY-P1-02-01`'s fixture already proves, not a second copy of it.

## Specification

### 1. The `CR3` reload decision is a pure, host-tested comparison

**Given** the physical address currently loaded in `CR3` and the physical address the incoming task's space would load,
**then** `hal_x86_64::paging::cr3_reload_needed` returns `false` when they are equal and `true` when they differ — independent of the two functions that actually touch the register, so the decision is exercised on the host without any target-specific instruction.

### 2. `AddressSpace` exposes its own `CR3` value

**Given** a constructed `AddressSpace`,
**then** `cr3()` returns the physical address of the caller-owned `pml4` it borrows — this kernel's current no-higher-half-split memory model means that address already **is** the physical address a caller would load into `CR3`, pinned by a host test against the `pml4` binding's own address.

### 3. A task can carry a dedicated address space

**Given** a `Scheduler`,
**then** `set_address_space(task, cr3)` attaches a `CR3` value to `task`'s `Tcb`, `Tcb::address_space()` reports it back, a newly created task defaults to `None` (no dedicated space — the default that leaves every pre-existing Story's tasks, none of which call `set_address_space`, running exactly as before), and the setter fails closed (`None`, no side effect) against an unknown task.

### 4. `switch_address_space` reloads `CR3` only when needed, before the register swap

**Given** `kernel::context::switch_address_space(prev, next, next_cr3)`,
**then** it reads the current `CR3`, reloads only if `cr3_reload_needed` says so, and only *then* calls `switch` — the address space must be live before the incoming task's suspended execution resumes into it, exactly mirroring how a live system attributes a fault to the *incoming* task's mappings, never the outgoing one's.

### 5. Tier 0: two genuinely distinct trees, a real switch, and a real adversarial fault

**Given** two tasks, each with its own `exec::AddressSpace` built from a separate `PML4`/frame-pool pair (never two sections inside one shared tree),
**when** each space also identity-maps a low kernel-replica region (see the fixture's own doc comment for why, and for what this does and does not claim about the production design) and is loaded into `CR3` for the first time,
**then**: the two `CR3` values are confirmed distinct; switching into task A really changes `CR3` to task A's value (read back from the register, not assumed); task A's deliberate read of task B's private virtual address — an address wholly absent from task A's own page tables — raises a real `#PF`, captured and contained (task A marked `Finished`, terminated exactly as `fixture_fault`'s victims are); and task B, scheduled afterward under its own distinct `CR3`, runs to completion, proving the fault stayed confined to task A alone.

**And** the fixture reports its verdict through the `TINYOS-RESULT/1` sentinel like every other Tier 0 fixture.

### 6. What this test explicitly does **not** establish

- **No hardware tier.** Tier 0 QEMU only; `LE-09` stays open.
- **No production dispatch-loop wiring.** Nothing in the real boot path or `kernel::dispatch::run_once` calls `switch_address_space` yet — every task the real system schedules still has `address_space() == None` and runs on the boot identity map, unchanged. Wiring a live per-task `CR3` into the general dispatch loop needs W^X-correct, *shared* kernel mappings present in every space (this fixture's own low-memory replica is a Tier 0 bootstrap, explicitly not that), which is `STORY-P1-03-02`'s charge.
- **No W^X enforcement.** The fixture's kernel-replica mappings are all-RWX, matching `boot.rs`'s own current identity map exactly. A write to executable memory or an execute of writable memory is not attempted here.
- **No teardown.** The fixture's two `AddressSpace` values are deliberately leaked (`core::mem::forget`) rather than dropped, because `AddressSpace::drop` tears a tree down immediately and this fixture needs both trees to outlive the scope that built them. Generation-safe teardown is `STORY-P1-03-02`'s charge; nothing here claims it exists.
- **No measured `D04` same-space-vs-cross-space delta.** The draft acceptance criteria for this Story asked for one; it is deferred to when the production dispatch path actually installs per-task address spaces, since measuring a switch cost nothing yet exercises in production would misrepresent Tier 0 fixture overhead as a real scheduling cost.

## Process note: how strictly TDD was followed here

The Test document did **not** precede the code, unlike this Epic's earlier Stories. The pure decision logic (`cr3_reload_needed`) and the accessor/bookkeeping additions (`AddressSpace::cr3`, `Tcb::address_space`) were written with their host tests alongside their implementations, and the Tier 0 fixture was debugged interactively against a real triple fault (`AddressSpace::drop` silently tearing down both trees the instant they went out of scope, before their `CR3` values were ever loaded — caught by adding progress checkpoints and a real `qemu -d int,cpu_reset` capture, the same debugging discipline `STORY-P1-02-02` used to find its own `#GP` selector coupling). Recorded here rather than presented as a clean Red run it was not.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-x86_64/src/paging.rs`, `os/src/exec/src/address_space.rs`, `os/src/kernel/src/sched.rs`) plus a Tier 0 QEMU fixture (`cargo run -p xtask -- qemu-x86_64 --fixture=address-space-switch`).

## Implementation location

- `os/src/hal-x86_64/src/paging.rs` — `cr3_reload_needed`, `read_cr3`, `write_cr3`.
- `os/src/exec/src/address_space.rs` — `AddressSpace::cr3`.
- `os/src/kernel/src/sched.rs` — `Tcb::address_space`, `Scheduler::set_address_space`.
- `os/src/kernel/src/context.rs` — `switch_address_space`.
- `os/src/exec/src/fixture_address_space_switch_main.rs` — the Tier 0 two-address-space fixture.
- `os/src/exec/Cargo.toml` — the `address-space-switch-fixture` binary target.
- `os/src/xtask/src/main.rs` — the `--fixture=address-space-switch` mapping.
- `.github/workflows/ci.yml` — the CI step running it.

## Reports

- [`REPORT-2026-07-27-08`](../reports/REPORT-2026-07-27-08.md) — the Tier 0 capture, the `AddressSpace::drop` finding, and what remains open.
