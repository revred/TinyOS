# ADR 0012 — Device Kernels Are Admitted Code, Compiled Ahead of Time, Off Target

Status: **Accepted**
Date: 2026-08-01
Introduced in: [`session/hand-2026-08-01/01A-tinytile-planning-session-mandate.md`](../../session/hand-2026-08-01/01A-tinytile-planning-session-mandate.md) §5, which tabled the three candidate shapes and required this ruling before any TinyTile API exists
Governs: [`goals/epics/EPIC-P6B.md`](../../goals/epics/EPIC-P6B.md) and every Feature decomposed from it

## Context

TinyTile — the planned heterogeneous compute access layer (`EPIC-P6B`) — must "easily consume
TileLang or CUDA code" (owner constraint, not open for re-litigation). TileLang's native pipeline is
**Python → TVM IR → CUDA/HIP source → `nvcc`/`hipcc` → device binary**, driven by a `@tilelang.jit`
decorator; CUDA's native pipeline likewise ends in a vendor compiler producing device machine code.
TinyOS is `no_std` Rust with no heap, no Python, no TVM, no vendor compiler — and a Security Charter
whose `RCG-11` denies *"JIT, self-modification, writable executable alias"* by name.

The charter's text was written about CPU mappings. Nobody had ruled on whether it binds code
destined for an accelerator. That question dominates every other TinyTile decision, because the
answer decides where compilation is allowed to happen at all.

## Decision

**1. The code-admission charter binds device code, and it binds harder than it binds CPU code.**

A GPU or NPU kernel is executable code for a bus-mastering processor — the one execution engine on
the board whose instructions TinyOS does not schedule, cannot preempt, does not fault-contain, and
which can reach physical memory the CPU's own page tables would have denied. Every gate in
[`code-admission-gates.tsv`](../../goals/security/code-admission-gates.tsv) applies to bytes that
will execute on an accelerator exactly as to bytes that will execute at `EL1`/ring 0, with no
device-code carve-out. "It only runs on the GPU" is an aggravating circumstance, not a mitigation.

**2. Kernels are compiled ahead of time, off target** (the mandate's shape A). A build host — CI or
a deployment-signing station — runs TileLang, TVM, `nvcc`, `hipcc`, or any other toolchain it
likes. **Narrowed 2026-08-03 by [`ADR 0014`](0014-the-tinytile-stack-contains-no-python-anywhere.md):
no part of TinyTile's toolchain may be Python, so TileLang and TVM are out; the off-target compiler
is Rust-native, and a non-Python vendor assembler may still be a back-end step. See the amendment
section at the foot of this ADR.** What reaches a TinyOS device is a **signed, immutable kernel
artifact**: precompiled device
binaries plus a declarative manifest (target requirements, executable hash, buffer schema, memory
ceilings, workgroup limits, dtypes, CPU fallback, provenance). The artifact is admitted through the
same gate chain as any PE64/TXE code — `RCG-07` (signature), `RCG-10` (immutability), `RCG-12`
(sealing) and the rest — and is never writable and executable in any address space, CPU or device,
at any time.

**3. No compiler runs on a TinyOS device.** No Python, no TVM, no vendor compiler, no IR-to-machine
translation on target, in any production configuration. "Consume TileLang or CUDA" is satisfied at
the toolchain boundary: the off-target tool consumes those languages; the device consumes admitted
artifacts. On-target compilation in any form is shape B, is denied by `RCG-11` as written, and
would require superseding this ADR — not amending it in passing.

**4. A bounded interpreted fallback is permitted, as data.** A `no_std` Rust executor walking a
validated tile IR (the mandate's shape C) creates no executable memory and therefore needs no code
admission. It may serve cold or rare shapes for which no precompiled variant was shipped — under
its own declared time and memory budget, never on the hot path, and never silently: selecting it is
a reportable degradation, not an equivalence.

**5. A missing kernel variant resolves fail-safe.** The outcomes are: the artifact's mandatory CPU
fallback, the bounded interpreter of clause 4, a declared degraded configuration, or a clean refusal
through the ACI. Never an on-demand compile, never a fetch-and-run of a variant that did not pass
admission, never an unbounded retry against a deadline.

## Rationale

- **It is the only shape that satisfies the charter without amending it.** Shape B requires a
  charter amendment and puts a compiler — the largest, least auditable component in the entire
  stack — inside the trusted path. Shape C alone almost certainly fails the owner's "native fast"
  constraint on the dense kernels that are the point of the layer.
- **It matches how TinyOS already admits code.** Signed immutable PE64/TXE artifacts through the
  gate chain is the existing, tested model; kernel artifacts reuse it rather than inventing a
  parallel weaker one.
- **Enumerating shape/dtype variants at build time is a real cost, honestly priced.** It is the
  cost of not shipping a JIT, and clause 4 plus clause 5 are the pressure valve. If variant
  explosion ever makes this untenable, the answer is a better off-target specializer, not a
  compiler on the device.
- **Security before performance.** The priority ordering resolves this trade-off in one direction
  only.

## Consequences

- `EPIC-P6B`'s Feature decomposition follows this shape: an artifact format and admission path, an
  off-target toolchain contract, and a dispatch runtime — not a compiler port.
- The artifact cache must not become a JIT by drift: a variant generated on demand by an off-board
  service and fetched at runtime is **remote code admission** and passes every gate, every time,
  or does not run. Convenience paths that shortcut this are the exact hole rule 9 exists to close.
- The interpreted fallback, if built, is a C3-resident executor whose input is C4 data; its IR
  validator is a parser of hostile bytes and carries the corresponding adversarial-test obligation.
- Nothing here weakens [`SECURITY_CHARTER.md`](../../SECURITY_CHARTER.md); clause 1 extends its
  reach to device code explicitly.

## Alternatives considered and rejected

- **Shape B, on-target JIT.** Denied by `RCG-11` as written; would trade the charter's sharpest
  rule for runtime specialization the workloads have not yet demonstrated they need.
- **Shape C as the primary path.** No code admission at all is attractive, but an interpreter
  walking tile IR cannot plausibly match compiled tensor-core/matrix-core kernels on the dense
  GEMM/GEMV work that motivates the layer. It survives as clause 4's bounded fallback.
- **Ruling that device code is outside the charter.** Rejected as the least defensible position in
  the table: it would make the least containable processor on the board the least regulated one.

## Amended 2026-08-03 by [`ADR 0014`](0014-the-tinytile-stack-contains-no-python-anywhere.md) — clause 2's build host is no longer unconstrained

**Nothing above is withdrawn.** Clauses 1, 3, 4 and 5 stand exactly as written, and clause 3's
device-side rule — no Python, no TVM, no vendor compiler, no translation on target — was already the
strictest half of this ADR.

What changed is clause 2's *permissiveness about the build host*. It said the off-target toolchain
could be "TileLang, TVM, `nvcc`, `hipcc`, or any other toolchain it likes", and TileLang is a Python
DSL on TVM — so that sentence placed a Python runtime and TVM's compiler infrastructure inside the
path that produces and signs kernel artifacts. The owner constraint of 2026-08-03 reaches past the
device boundary to the whole stack, and `ADR 0014` closes it:

- **TinyTile's off-target compiler is Rust-native.** A non-Python vendor assembler (`nvcc`, `ptxas`,
  `hipcc`) may still be invoked as a back-end step; TileLang and TVM may not.
- **Python remains welcome as a *consumer*** calling in through the C ABI. That is outside the
  stack and is explicitly permitted.
- **"Easily consume TileLang or CUDA" survives** in two forms — a Rust-native front end that parses
  and lowers that source, or ingestion of third-party precompiled artifacts as data through this
  ADR's unchanged gate chain. `ADR 0014` clause 4.

The consequence this ADR should have priced and could not: `EPIC-P6B`'s off-target toolchain Feature
stops being a contract around someone else's compiler and becomes a compiler of its own. That cost
belongs to `ADR 0014`, and is recorded there.
