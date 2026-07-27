# TEST-P1-07-02-A — A Fault Announces Itself, or the Rest of This Feature Is Undebuggable

Status: **Partially Verified (Host), 2026-07-28** — clauses 1, 3, 4, 5 and 6 Green on the host; clause 2 is untouched and needs hardware. **Specification unchanged since it was written before implementation** — see "What was and was not run", below.
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

## What was and was not run, 2026-07-28

**No clause was edited to fit what happened.** The specification above is the
one written before implementation; this section is added below it, per the
precedent `TEST-P1-07-01-A` and `TEST-P1-01-04-A` clause 4 set.

| Clause | State | Evidence |
|---|---|---|
| 1 — all sixteen vectors present, 128-byte alignment asserted at build time | **Green.** Sixteen slots modelled and tested on the host, including per-slot "leave exactly one unrouted" rejection; the assembled table is exactly `0x800` bytes with one branch relocation per entry, and the stride is enforced by `.org`, not by a comment. | `hal_arm64::vectors` host tests; `llvm-objdump -h` on the AArch64 rlib; the negative test below |
| 2 — a deliberately-triggered fault prints a decoded `ESR_EL1` | **Not run.** This is the Green, and it needs the board. `hal_arm64::fault::deliberate_breakpoint` and `deliberate_alignment_fault` exist, are compiled for AArch64, and have never executed. | — |
| 3 — `ESR_EL1` decoding: class, IL bit, class-specific ISS | **Green.** Class from bits `[31:26]` against noise in every other bit, `IL` from bit 25 alone, `ISS` from `[24:0]`, data- and instruction-abort syndromes decoded separately and proven not cross-wired, every one of the 64 representable `EC` values total. | `hal_arm64::esr` host tests |
| 4 — a fault frame is evidence, never authority | **Green.** No function in `hal_arm64::fault` turns a frame into a decision, and `kernel::fault::Disposition` — the x86_64 policy — is run **unmodified** against AArch64 frames across every slot × a matrix of `ESR_EL1` values, reaching one disposition. | `hal_arm64::fault` host tests |
| 5 — the handler is bounded, allocation-free, non-reentrant | **Green for what a host can establish.** The report allocates nothing, takes no lock, has no unbounded loop, and a wedged UART makes it return `TransmitTimeout` rather than stall. Non-reentrancy is `DAIF` masked on entry and never cleared, plus a handler that cannot return — reviewed, not executed. | `hal_arm64::fault` host tests |
| 6 — every fault is a spoor, carrying no register content and no faulting address | **Green for the encoding and the policy; the emission is blocked with clause 2.** `kernel::fault::audit` consumes an AArch64 fault unmodified, and two faults at one slot audit *identically* however different their frames — which is the strongest available statement that no `ESR_EL1` or `FAR_EL1` is smuggled into the atom. | `hal_arm64::fault` host tests |
| 7 — what this test does not establish | **Unchanged, and still true.** No nested-fault survival, no `EL0`, no timing, `LE-09` open. | — |

`hal-arm64` went from **64 host tests to 115**, and the workspace suite from
**498 passing to 549**. `crate-size-check` reports `hal-arm64` at **2,216 lines** against the
20,000-line ceiling — the pre-commit hook's own figure, quoted rather than a
hand count, because this repository has an authoritative measurement and a
second one only invites the two to disagree.

### Clause 6's spoor emission does not live where this document said it would

Stated in the open rather than quietly relocated. "Implementation location"
above names `os/src/kernel/` for "the deliberate-fault fixture and its spoor
emission". The fixture half is still correct and still unbuilt (see clause 2).
The **spoor emission cannot be there**, and the reason is structural rather
than a preference:

`kernel` depends unconditionally on `hal-x86_64`. Building `kernel` for
`aarch64-tinyos` would mean building the x86_64 HAL for AArch64, which cannot
work. So no code that runs on the Raspberry Pi can call into `kernel` as this
workspace is arranged today.

What was done instead is the thing that keeps the claim honest: `kernel::fault`
is exercised **from `hal-arm64`'s host tests** (`kernel` is already a
dev-dependency, for `TEST-P1-01-03-A` clause 5's own reasons), so the audit path
an AArch64 fault would take is proven against the real policy and the real
`Spoor` encoding rather than against a second copy of them. What is *not* proven
is that a board emits one, and that is recorded as blocked with clause 2 rather
than counted as Green.

Wiring a real emission needs a crate that both architectures can depend on — a
`kernel-fault`-shaped split, or the AArch64 binary crate `STORY-P1-07-05` will
have to introduce anyway. **That is a real decision and it is deliberately not
made here**, per `FEAT-P1-07` §6 and this Feature's standing rule that a seventh
concern means re-decomposing rather than extending.

### The invariant that could have been arch-shaped, and was not

Clause 4 says a second architecture is where an invariant like "the disposition
depends only on where the fault happened" either holds or turns out to have been
x86-shaped. It held, and more completely than the clause required:
`kernel::fault::Disposition`, `FaultReport` and `audit` needed **no change at
all** to consume an AArch64 fault. `FaultReport`'s `vector` field carries the
vector slot index in place of an x86 vector number, and since the policy never
reads it, nothing else had to move.

That is worth stating as a *result*, not a convenience: the policy was written
without a second architecture in sight, and the reason it survived contact with
one is that it reads exactly one field. A policy that had consulted an error
code would have needed an AArch64 branch, and the invariant would have been
discovered to be x86-shaped at precisely the moment it was hardest to fix.

### Three things this specification's own tests and gates caught

**A raw pointer in the exception entry point, found only by target-only
clippy.** The first implementation had each vector entry build a `FaultFrame` on
the stack and pass `*const FaultFrame` to the Rust entry point.
`cargo clippy -p hal-arm64 --all-targets -- -D warnings` — the host command —
passes on that code, because the entry point is `cfg(target_arch = "aarch64")`
and the host never compiles it. The AArch64 clippy invocation rejected it with
`clippy::not_unsafe_ptr_arg_deref`. That is `LE-12` paying for itself: the
second lint command exists because target-only code is otherwise unlinted, and
the first target-only Story to add code after it was added is the one it caught.

The fix is better than the code it replaced, which is why it is recorded here
rather than in a commit message. Each entry now loads the slot index and the
four describing registers into `x0`-`x4` and branches; the entry point takes
five `u64` arguments. That removes the raw pointer, the `#[repr(C)]` layout
invariant between assembly and Rust, and — the part that matters most — **the
store to the stack**, on a path one of whose causes is a stack that is no longer
valid.

**The 128-byte guard was proven able to fail.** `.org` enforcing the stride is
worth nothing if it silently accepts an over-long entry, so the entry body was
temporarily padded past 128 bytes and the build was run:

```
error: invalid .org offset '128' (at offset '204')
1 | .org tinyos_vector_table + 0x80 * 1
```

The padding was removed. This is the discipline `fixture-broken-boot` set for
the boot path, applied to a build-time assertion: a gate nobody has watched fail
is a gate nobody has tested.

**A test of my own was wrong on first run.** `the_exception_class_comes_from_bits_31_26_and_nothing_else`
built its "every bit outside `EC` set" mask as `0xFFFF_FFC3_FFFF_FFFF`, which
sets bits the field occupies; the correct complement is
`0xFFFF_FFFF_03FF_FFFF`. The decoder was right and the test was wrong. Noted
because this repository's convention is that a failing assertion gets recorded
rather than quietly restated.

### What was deliberately not done

- **No `kernel8.img`, and no fixture that runs one.** Clause 2's fault needs an
  image, and producing one is `STORY-P1-07-05`. The *triggers* are written so
  that a single board session can close `STORY-P1-07-01`'s capture and this
  Story's fault injection together; the harness that calls them is not this
  Story's, and growing one in passing is what `FEAT-P1-07` §6 forbids.
- **No `CPACR_EL1.FPEN` change.** `ExceptionClass::SimdFpAccessTrap` is decoded
  because it is the fault this board is most likely to take from code nobody
  wrote. Decoding it is diagnostics; enabling FP/SIMD would be a scope change
  with no test behind it.
- **No nested-fault handling, no IST equivalent, no `EL0`.** Clause 7, unchanged.
- **No measurement.** The MMU is still off. Nothing here closes `PERF-D02-*`,
  and `LE-09` is untouched.

## Reports

To be filed when the Story goes Green. **Nothing here closes a `PERF-*`
guardrail and `LE-09` remains open**: a decoded fault is diagnostics, not a
measurement, and no fault has been decoded on hardware.
