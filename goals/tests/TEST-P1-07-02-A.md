# TEST-P1-07-02-A — A Fault Announces Itself, or the Rest of This Feature Is Undebuggable

Status: **Specified — written before implementation, per the TDD mandate**
Story: [`STORY-P1-07-02`](../stories/STORY-P1-07-02.md)
Tier: Host unit tests (`ESR_EL1` decoding, vector-table completeness, spoor encoding) **plus** a Tier 1 hardware fault-injection run on a Raspberry Pi 5, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D02`
Security controls: `SEC-14`, `SEC-19`
Containment classes: `C0`, `C1`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: `PERF-D02-G01`, `PERF-D02-G04`, `PERF-D02-G21` — exception entry latency, its WCET bound, and fault containment completion. This Story installs the path those guardrails will eventually be measured on; it closes none of them.

## What this test is for

`TEST-P1-02-01-A` opened by naming what a missing fault handler cost on x86_64: a `#UD` from missing SSE enablement produced a triple fault with no diagnostic, and two debugging cycles. **On a Raspberry Pi 5 the same mistake is strictly worse**, because there is no `isa-debug-exit`, no QEMU monitor and no `-d int,cpu_reset` log. A fault with no vector table is a silent hang with no output whatsoever — indistinguishable from a dead adapter, a rejected image, or a board that never started.

That is the whole argument for this Story's position in the order. `-03`'s MMU and `-04`'s timer are the two easiest things in the Feature to get subtly wrong, and the first symptom of either is an exception. This Story is what turns that symptom into a sentence.

## Specification

### 1. All sixteen vectors are present

**Given** the AArch64 vector table installed at `VBAR_EL1`,
**then** every one of the sixteen entries is filled before the table is installed, and a host test asserts it — the direct analogue of `Idt::every_entry_present`.

**And** entries this Story does not decode reach one shared fail-closed default that reports and halts, exactly as `STORY-P0-04-02`'s default does on x86_64. This Story narrows the set of faults that are terminal for the whole system; it does not widen the set that is silent, which stays empty.

**And** the table's 128-byte-per-entry alignment requirement is asserted at build time, not discovered at run time. A misaligned `VBAR_EL1` write is architecturally ignored, which means the failure presents as "no handler ran" — the exact symptom this Story exists to eliminate.

### 2. A deliberate fault is mandatory

**Given** a fixture that deliberately triggers a synchronous exception,
**when** it runs on the board,
**then** the handler prints the exception class, the faulting address (`FAR_EL1`) and a decoded `ESR_EL1`, and the serial capture is quoted verbatim in this document.

**And there is no version of this Story that passes without inducing a fault.** Its entire value is that failure becomes visible, and a claim that failure is visible, tested only against code that does not fail, is not a test. `fixture-broken-boot` established this discipline for boot; it applies with more force here, because the thing being proven is a diagnostic channel.

### 3. `ESR_EL1` is decoded by pure, host-tested functions (`SEC-19`)

**Given** an `ESR_EL1` value,
**then** the exception class, the instruction-length bit and the class-specific ISS fields decode on the dev host, with no `unsafe`, no board, and a case per class this Story claims to name.

**And** an unknown exception class is reported as unknown with its raw value, never decoded as though it were a known one.

### 4. A fault frame is evidence, never authority (`PD-12`, `BND-04`-shaped)

**Given** a decoded fault,
**then** nothing decoded from it widens any decision. The disposition depends only on **where** the fault happened, never on what the faulting context claimed about it — the invariant `TEST-P1-02-01-A` clause 2 established on x86_64, restated here because a second architecture is where an invariant like that either holds or turns out to have been arch-shaped.

### 5. The handler is bounded and non-reentrant

**Given** the handler,
**then** it takes no lock, allocates nothing, contains no unbounded loop, and runs with interrupts masked so it cannot be re-entered mid-decision.

**And** a fault *inside* the fault handler is **not survivable by this Story and must not be claimed to be.** There is no AArch64 counterpart of `STORY-P1-02-02`'s TSS/IST work in this Feature. Stated here so that no reader infers more containment than exists.

### 6. Every fault is a spoor (`SEC-14`, `PD-12`, `BND-17`)

**Given** any captured fault,
**then** a spoor is emitted carrying category, actor, action, outcome and the faulting context — and carrying **no** register content and **no** faulting address. `PD-12` scopes a fault record to class/actor/action/outcome; an audit atom is not a debugging channel.

**And** the full frame goes to the serial report, which is bounded and explicitly not the audit log. The two channels answer different questions and merging them on a board where serial is the only output is the tempting mistake this clause forbids.

### 7. What this test explicitly does **not** establish

- **No nested-exception or double-fault survival.**
- **No `EL0`.** Everything runs at `EL1`, so "which context faulted" means which execution context, not a privilege transition.
- **No timing.** The MMU is still off (`STORY-P1-07-03`), so no latency observed here is meaningful, and `PERF-D02-*` stays unclosed.
- **`LE-09` stays open.** A decoded fault is diagnostics, not a measurement.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/hal-arm64/src/`) plus a Tier 1 hardware fault-injection fixture.

## Implementation location

- `os/src/hal-arm64/` — vector table, synchronous-exception handler, `ESR_EL1`/`FAR_EL1` decoders.
- `os/src/kernel/` — the deliberate-fault fixture and its spoor emission.

## Reports

To be filed when the Story goes Green.
