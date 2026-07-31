# TEST-P0-01-04-A — The Harness Is Held to the Discipline It Enforces

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P0-01-04`](../stories/STORY-P0-01-04.md)
Tier: Host unit tests (fixture registry, CI cross-checks, owning-Test resolution) **plus** Tier 0 QEMU runs of `broken-boot` and `idt-apic-unrouted`, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`, `BND-18`
Protection Domain contracts: `PD-02`, `PD-12`, `PD-13`, `PD-14`
Code admission gates: `RCG-05`, `RCG-06`, `RCG-07`, `RCG-12`, `RCG-14`
Assurance state: `baseline-debt`

## What this test is for

Two problems with one shape.

**First, three pieces of assurance tooling shipped in `2da1ccd` with no Test document**: `xtask`'s `FIXTURES` table and `list-fixtures` command, `goals/assurance/loose-ends.tsv` with its spine validation, and the status-header grammar. They are good work and they are well covered by unit tests — every violation case has one. What they lacked is the artifact that makes them *assurance* rather than *code*, and shipping them that way bypassed rule 3 (test-driven, no exceptions) and rule 8 (nothing bypasses the spine). **The cost of leaving a bypass standing is that it licenses the next one**, which is the reason this Story exists at all and the reason it is worth writing a document for code that is already green.

**Second, `broken-boot` and `idt-apic-unrouted` still have the exit-code hole `wcet-trip` closed**, deferred across three mandates. Their documented pass condition is "exits 1" — and every other failure also exits 1, so the assertion cannot distinguish *failed as designed* from *broke*. `TEST-P1-04-02-A` established the fix (capture the serial output and assert on its content) and proved it necessary by falsification: with enforcement removed, `wcet-trip` still exited 1.

**The thing that makes the second problem harder than it looks, and it was discovered by running the fixtures rather than by reading them**: both produce an **empty serial capture**. `broken-boot` panics, and the panic handler is `exit_qemu(QemuExitCode::Failure)` with no output at all. `idt-apic-unrouted` reaches `unhandled_interrupt_handler`, which is likewise a bare `exit_qemu`. **There is nothing to grep.** The hole cannot be closed by strengthening the CI step alone; the system has to say something first.

That reframes the work, and in a direction worth having: **a TinyOS kernel that panics currently does so in total silence, and an unrouted interrupt kills the machine with no diagnostic.** That is not a test-harness defect. It is a system defect that a test-harness gap was hiding.

## Specification

### 1. A panicking kernel says so, on the UART, before it dies

**Given** any TinyOS binary built for the target,
**when** its `panic_handler` runs,
**then** it emits exactly one sentinel line on COM1 — `TOS64-PANIC/1 ` followed by the panic location where one is available — **before** signalling `exit_qemu(QemuExitCode::Failure)`.

**And** the handler remains divergent, fail-closed and allocation-free: it must not take a lock, must not call back into the scheduler, and must reach the exit port even if serial output fails. A panic handler that can itself fail to terminate is worse than a silent one.

**And** the line is emitted **first**, before the exit port is touched. The exit port stops the machine; anything after it is not evidence.

### 2. An unrouted interrupt is diagnosable

**Given** the shared fail-closed default `hal_x86_64::interrupts::unhandled_interrupt_handler`,
**then** it emits exactly one sentinel line — `TOS64-UNROUTED/1 fail_closed=true` — before `exit_qemu(QemuExitCode::Failure)`.

**And** the vector is deliberately **not** named. The handler is one shared stub installed for every unrouted vector and receives no vector argument; naming it would require per-vector trampolines, which is a larger change than this Story's charge and is recorded as a loose end rather than smuggled in.

**And** fail-closed behaviour is unchanged in every other respect. This clause adds a diagnostic to an existing containment action; it must not alter what that action *is*.

### 3. Both fixtures' CI steps assert on content, not on an exit code alone

**Given** the `broken-boot` and `idt-apic-unrouted` CI steps,
**then** each runs with `--serial-capture=`, asserts the exit code is 1, **and** greps the capture for its own sentinel — matching the shape `wcet-trip` and `os-runaway` already use.

**And** a fixture that fails for any other reason must fail the step. This is the whole point: the exit code alone cannot tell the two apart, which is why `TEST-P1-04-02-A` recorded a falsification in which a broken `wcet-trip` still exited 1 and still looked green.

### 4. Every fixture's declared owning Test exists

**Given** the `FIXTURES` table, in which each row declares an `owning_test`,
**then** every declared `TEST-*` resolves to a real document under `goals/tests/`.

A registry that names an owning Test which does not exist is worse than one that names none: it reads as traceability and provides none. This is the same failure class the assurance spine already rejects for Stories and Features, applied to the table that claims to be the fixture set's source of truth.

### 5. Every fixture is actually run somewhere

**Given** the `FIXTURES` table and `.github/workflows/ci.yml`,
**then** every fixture in the table is invoked by at least one CI step, or is explicitly listed as deliberately not run with a stated reason.

The existing drift guard checks CI → table (a step naming a fixture the table lacks). **The reverse direction is unguarded**, and it is the more dangerous one: a fixture that exists, compiles, and is never run is an unverified fixture that looks verified. That is `LE-07` — CI unobserved for thirty handovers — in a new place.

### 6. The tooling shipped in `2da1ccd` has its assertions written down

**Given** the loose-ends register, the status grammar and the fixture registry,
**then** this document records the properties they enforce, so a future change that weakens one is visibly a change to a specified behaviour rather than an edit to an unowned file:

- **Loose ends**: a register gap (an `LE-N` referenced by a document with no row) is rejected; a row marked `closed` with no closing evidence is rejected; a row marked `open` that claims closure is rejected; an out-of-vocabulary `state` or `ownership` value is rejected; and `LE-` tokens are extracted without false positives from surrounding prose.
- **Status grammar**: every committed Feature/Story/Test/Report carries a parseable `Status:` header; a state word outside the vocabulary is rejected; an unbolded status is rejected; a state word must be terminated, so `Complete` does not match `Completely rewritten`; and `Functionally Verified` is never truncated to `Verified`.
- **Fixture registry**: names are unique; the no-fixture default boot resolves; an unknown fixture is rejected; and exactly three fixtures declare failure as their pass condition, so adding a fourth is a deliberate act.

**And** the existing unit tests that already establish these are enumerated in the Story's Report rather than rewritten. Back-filling a document over green code is honest only if it says that is what it is doing.

### 7. What this test explicitly does **not** establish

- **No claim that a Test document written after the code is TDD.** Clauses 1–5 are genuinely Red-first; clause 6 is retrospective and is labelled as such. The distinction is stated rather than blurred, because blurring it is how the discipline erodes.
- **No hardware tier.** Tier 0 only; `LE-09` untouched.
- **No claim that the panic path is safe under all conditions.** It writes to a UART from a context that may hold arbitrary broken state. It is best-effort by construction, and clause 1 requires only that failure to emit cannot prevent termination.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/xtask/src/main.rs`) for owning-Test resolution and the CI coverage direction; Tier 0 QEMU runs of `broken-boot` and `idt-apic-unrouted` with `--serial-capture=`, asserted in CI by content.

## Implementation location

- `os/src/kernel/src/main.rs` — the `panic_handler` sentinel.
- `os/src/hal-x86_64/src/interrupts.rs` — the `unhandled_interrupt_handler` sentinel.
- `os/src/xtask/src/main.rs` — the two new registry cross-checks.
- `.github/workflows/ci.yml` — both steps re-shaped to assert on content.

## Reports

- [`REPORT-2026-07-28-07`](../reports/REPORT-2026-07-28-07.md) — the empty-capture finding, the Red runs, and the two fixtures' new pass conditions.
