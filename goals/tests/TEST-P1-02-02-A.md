# TEST-P1-02-02-A — A Fault Inside the Fault Path Lands on a Known-Good Stack and Reports

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P1-02-02`](../stories/STORY-P1-02-02.md)
Tier: Host unit tests (TSS layout, GDT/system-descriptor packing, IDT IST index, double-fault audit) **plus** a Tier 0 QEMU escalating-fault fixture that destroys the kernel stack for real, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D02`
Security controls: `SEC-14`, `SEC-19`
Containment classes: `C0`, `C1`
Boundary tests: `BND-04`, `BND-17`, `BND-20`
Protection Domain contracts: `PD-02`, `PD-12`, `PD-13`
Code admission gates: `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## What this test is for

`STORY-P1-02-01` gave this kernel real `#UD`/`#GP`/`#PF` handlers. That closed one gap and opened a sharper one, which its own Report stated plainly: **the fault handler is now new code, and new code can fault.** Today a fault raised while the CPU is delivering a fault has nowhere to push its frame, escalates to `#DF`, finds no usable stack for *that* either, and triple-faults — which under QEMU is a silent machine reset with no output at all, and on the Pi 5 (`LE-09`) would be a silent hang.

`LE-04` has carried this since Handover 32. This Story closes it: a dedicated, known-good stack that the CPU switches to *unconditionally* on `#DF`, selected by hardware (the Interrupt Stack Table) rather than by any code that the compromised fault path might have corrupted.

The double-fault handler is **terminal but reporting**. It does not contain, does not resume, and does not terminate-and-continue: a `#DF` means the primary fault path itself is compromised, so the only honest action is to say so and stop.

## Specification

### 1. A TSS exists, is architecturally exact, and is pinned by host tests

**Given** the 64-bit Task State Segment this Story introduces,
**then** its Rust type is `#[repr(C, packed)]` with `size_of` **exactly 104** and every field offset pinned by a host test against Intel SDM Vol 3A §8.7 — `rsp0` at 4, `ist1` at 36, `ist7` at 84, `iomap_base` at 102.

**And** `iomap_base` is set beyond the segment limit, so no I/O permission bitmap is claimed: an I/O bitmap this kernel never populates but *does* advertise would be the CPU reading 8 KiB of whatever happens to follow the TSS in memory.

**And** `rsp0`/`rsp1`/`rsp2` are zero and documented as such. They only matter on a privilege-level change, and this kernel has none — everything is CPL 0 (`STORY-P1-02-01` clause 8). A non-zero value there would be a claim this Story cannot back.

This is the same discipline `idt.rs` and `paging.rs` already apply: a bit-layout error in a table the CPU parses is invisible to the type system, so the layout is pinned somewhere a host test can reach it.

### 2. The GDT grows a TSS descriptor without disturbing the segments already in use

**Given** `hal_x86_64::boot`'s GDT — null, code, data, in that order —
**then** the GDT this Story installs repeats those three descriptors **byte for byte** (a host test asserts the exact quadwords against the values in `boot.rs`), and appends the 16-byte 64-bit TSS system descriptor after them.

**And** because entries 0–2 are unchanged, `CS`/`DS`/`SS`/`ES`/`FS`/`GS` keep the selectors and cached descriptors they already hold: no far return, no segment reload, no window in which the code segment is briefly undefined. The `lgdt` is additive, and the test states that as the reason it is safe.

**And** the TSS descriptor's own packing — base split across three fields plus a 32-bit upper half, limit 103, type `0x9` (available 64-bit TSS), present, DPL 0, granularity byte — is host-tested by reading each field back out of the packed bytes, mirroring `IdtEntry::handler_address`'s own round-trip style.

### 3. An IDT gate can name an IST slot, and out-of-range slots cannot be expressed

**Given** `Idt::set_handler`,
**then** a second constructor wires a gate with an IST index, and the index's type makes an invalid one unrepresentable: `IstIndex::try_new` accepts `1..=7` and rejects `0` (which means "no IST", and is what the plain `set_handler` already produces) and `8..=255`.

**And** a host test proves the index lands in the descriptor's own `ist` field and that every gate `set_handler` builds still has `ist == 0` — this Story adds an IST to exactly one vector and must not silently move any other.

### 4. `#DF` is captured in the same frame shape, through its own entry point

**Given** a double fault (vector 8),
**then** it reaches an entry stub that builds the identical `FaultFrame` `STORY-P1-02-01` pinned — the layout test already covers it, and a second frame shape would be a second parser.

**And** the stub calls `tinyos_double_fault_entry`, **not** `tinyos_fault_entry`. The primary path is not extended with an eighth case; it is left exactly as it was (clause 7), and the double fault gets its own symbol, because the two paths differ in the one way that matters: the primary one decides, and this one has nothing to decide.

**And** `FaultVector::from_raw(8)` still returns `None`. `FaultVector` enumerates the vectors whose faults the *disposition policy* contains; vector 8 now has a handler but is deliberately not one of them, and the existing host test asserting `None` for vector 8 stays as it is.

**And** `FaultFrame::faulting_address()` returns `None` for a `#DF`, so the `CR2` the stub reads (unconditionally, as for every vector) cannot be reported as if it meant something.

### 5. There is no double-fault disposition, because there is no decision

**Given** a captured double fault,
**then** the kernel emits the audit pair and halts. `Disposition::of` is **not** consulted, is not extended, and does not gain a vector-dependent branch — that would break `STORY-P1-02-01`'s load-bearing invariant that the policy reads exactly one field (which context was running) and never the vector. The existing host test `the_vector_is_recorded_but_never_changes_the_decision` must still pass unmodified.

**And** the audit pair records the faulting context for *attribution* (which task was running) while its disposition spoor's outcome is `Failed` in **both** contexts — task and kernel alike. A host test asserts exactly that: a double fault that happened inside a task still says "not contained", because it was not.

**And** the spoors carry no address, no error code and no register content, per `PD-12`, exactly as `STORY-P1-02-01`'s do.

### 6. Tier 0: a deliberately destroyed kernel stack reaches the IST handler

**Given** a Tier 0 fixture whose task sets `RSP` to a canonical-but-unmapped address and then pushes,
**when** the resulting `#PF` cannot have its own frame pushed either and escalates,
**then** the `#DF` handler runs, and the fixture proves it ran **on the IST stack** by checking that the handler's own stack pointer lies inside the IST stack's address range — not by observing that it merely produced output, which a lucky non-IST handler could also do.

**And** the frame's saved `RSP` is reported and checked against the destroyed value, so the evidence shows the fault really did originate from the broken stack rather than from somewhere incidental.

**And** the fixture reports over COM1 and exits through `isa-debug-exit`, so its verdict travels on `STORY-P1-01-02`'s `TOS64-RESULT/1` line like every other fixture's.

**And** the *contrast* is recorded, not assumed: what this same fixture does with the IST removed (a triple fault — QEMU resets and never reaches the debug-exit port, so the harness sees no kernel verdict at all) is observed and written into the Report. "It passes now" is not evidence that the IST is what made it pass.

### 7. The primary fault path is demonstrably unaffected

**Given** `STORY-P1-02-01`'s fixture,
**then** `--fixture=fault` still passes with the TSS installed and the GDT replaced: three faults, three contained terminations, survivor runs three times.

**And** every other Tier 0 fixture (`qemu-x86_64` with no fixture, `context-switch`, `idt-apic-timer`, `idt-apic-unrouted`, `pci-enumeration`, `pool-bench`, `measure`) still produces its own established result, since `interrupts::init` now installs a GDT and a TSS on the real boot path too — not only in a fixture. A safety net wired only into the fixture that tests it would be theater.

**And** one coupling `STORY-P1-02-01` left behind is removed rather than worked around: its `#GP` victim loaded selector `0x18` *because* the boot GDT held exactly three descriptors, so growing the GDT would have quietly turned that victim into a TSS-selector load. The victim now uses a selector far past any GDT this kernel will plausibly install, and the fixture says why.

### 8. The IST stack is a named, budgeted capacity

**Given** the IST stack,
**then** its size is a named constant with its rationale written down (not a bare number at the definition site), and it is counted in `kernel::capacities::committed_bytes()` against `STATIC_MEMORY_BUDGET_BYTES` — the same compile-time budget check every other capacity in this kernel passes.

**And** exactly one IST slot is populated. Six unused slots stay zero, and the test asserts that: an IST stack wired for a vector with no handler that uses it is unexercised memory in a fault path.

### 9. What this test explicitly does **not** establish

- **No hardware tier.** Tier 0 QEMU only. `LE-09` stays open, and `STORY-P1-02-01`'s finding — that a Tier 0 fixture can only ever check what the *emulator* does — applies here with equal force.
- **No `#MC` (machine check) survival.** Vector 18 gets no IST in this Story. It is a genuine second IST consumer and it has no Tier 0 way to be triggered, so wiring it would be untested code in a fault path.
- **No triple-fault survival.** Nothing survives a fault inside the `#DF` handler itself; that is what "terminal but reporting" means, and it is the end of the escalation chain, not another rung.
- **No privilege boundary.** `RSP0` is zero and unused; everything is still CPL 0 in one identity-mapped address space.
- **No fault-latency baseline** (`LE-17`). `FEAT-P1-02`'s exit criteria still want one, and this Story does not provide it.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-x86_64/src/tss.rs`, `os/src/hal-x86_64/src/gdt.rs`, `os/src/hal-x86_64/src/idt.rs`, `os/src/kernel/src/fault.rs`, `os/src/kernel/src/capacities.rs`) plus a Tier 0 QEMU fixture (`cargo run -p xtask -- qemu-x86_64 --fixture=double-fault`).

## Implementation location

- `os/src/hal-x86_64/src/tss.rs` — `TaskStateSegment`, the IST stack and its budgeted size constant, the IST stack range predicate.
- `os/src/hal-x86_64/src/gdt.rs` — the GDT carrying `boot.rs`'s three descriptors plus the 64-bit TSS system descriptor, and `lgdt`/`ltr`.
- `os/src/hal-x86_64/src/idt.rs` — `IstIndex` and the IST-bearing gate constructor.
- `os/src/hal-x86_64/src/fault.rs` — `DOUBLE_FAULT_VECTOR` and the `#DF` entry stub.
- `os/src/hal-x86_64/src/interrupts.rs` — GDT/TSS installation and the IST-backed vector-8 gate, on both the fixture and the real boot path.
- `os/src/kernel/src/fault.rs` — `audit_double_fault`.
- `os/src/kernel/src/capacities.rs` — the IST stack's line in the static budget.
- `os/src/kernel/src/fixture_double_fault.rs` — the Tier 0 escalating-fault fixture.
- `os/src/kernel/src/main.rs` — the default `tinyos_double_fault_entry` for every non-fixture build.

## Reports

- [`REPORT-2026-07-27-06`](../reports/REPORT-2026-07-27-06.md) — the Red runs, the Tier 0 capture, the observed no-IST triple fault it is contrasted against, and what remains open.
