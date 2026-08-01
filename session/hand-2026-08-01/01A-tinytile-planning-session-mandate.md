# Handover 01A — Cover Note: Plan **TinyTile**, the Heterogeneous Compute Access Layer

**The start-here document for the session that plans TinyTile.** It authorises and orders that work; it
does **not** design it, and the session it commissions must not implement it either. Owner-ordered
2026-08-01.

Follows [`hand-2026-07-31/01A`](../hand-2026-07-31/01A-tos64-protocol-prefix-rename.md). `main` carried
unpushed work when this was written — check `git log origin/main..HEAD` before assuming a clean base.

## 1. The charge, and the four things already settled

**Plan a compute-access layer, named `TinyTile`, that lets AI and similar throughput workloads reach a
GPU or other accelerator efficiently on the TinyOS platform.**

Four things are **owner-decided and not open for the session to re-litigate**:

1. **TinyTile is written entirely in Rust**, exposing a **C ABI**.
2. **The API must be native-fast** — no marshalling layer, no interpreter tax on the hot path.
3. **It must easily consume TileLang or CUDA code.** Design inspiration: `tilelang.com`.
4. **This is to be done.** It is a planned destination, not a thought experiment.

Everything else below is a question for the planning session, and §5 is the one that decides whether the
rest is even coherent.

## 2. What TileLang actually is, stated before anyone designs against a slogan

Read this before the API docs, because the shape of the thing is the whole problem:

- A **Python DSL** built on **TVM**, working at *tile/block* level rather than per-thread. Primitives:
  `T.Kernel`, `T.Parallel`, `T.alloc_shared`, `T.alloc_fragment`, `T.copy`, `T.gemm`, `T.Pipelined`,
  `T.use_swizzle`.
- Its pipeline is **Python → TVM IR → backend codegen → CUDA/HIP source → `nvcc`/`hipcc` → device
  binary**, driven by a `@tilelang.jit` decorator.
- Backends: NVIDIA CUDA, AMD HIP, experimental Metal and WebGPU.

So the thing being "consumed" is, natively, **a Python program that JITs C++ and shells out to a vendor
compiler**. TinyOS is `no_std` Rust with no heap, no Python, no TVM, no `nvcc`, and a charter that denies
JIT by name. **The real design work is entirely in what "consume" is allowed to mean** — §5.

What *is* genuinely borrowable, and probably should be, is the **programming model**: tile-level
abstraction, explicit memory scopes (global/shared/fragment), explicit pipeline stages, and a small set
of composable primitives with layout inference underneath. That model is language-agnostic, and a Rust
`no_std` expression of it is a real and attractive design.

## 3. What already exists and must not be re-invented

[`docs/inference-architecture.md`](../../docs/inference-architecture.md) already fixes four things. The
session **builds on these and does not re-open them**:

- **The compute admission model.** GPU work is admission-controlled, never scheduler-prioritised, and
  *"the RT kernel's own execution is never blocked waiting on a GPU submission, fence, or driver call"*.
- **The Unified Memory Manager (UMM).** Typed ownership-tracked handles, single writer, explicit fenced
  hand-off, and a fallback explicit-copy path behind the same handle API (`G-HW-6`).
- **The mmap/pointer model-load path**, including the correction already recorded there about what
  `mmap` does and does not buy.
- **Vendor bindings live in `-sys` crates**; the safe logic sits on top.

`EPIC-P6B` — *Phase 6b, Heterogeneous compute*, goals `G-AI-6`/`G-HW-6`, proving board **Jetson Orin
Nano Super**, depends on `EPIC-P6` — **exists in [`goals/epics/backlog.md`](../../goals/epics/backlog.md)
and has no Epic document.** TinyTile is that Epic's substance, and writing `EPIC-P6B.md` is probably the
session's largest single artifact.

**The gap TinyTile fills, stated precisely:** the UMM owns *buffers*; the inference runtime owns
*workloads*. **Nothing owns *kernels*** — how a tile-level computation is expressed, admitted, scheduled
onto a device queue, and executed. That layer is TinyTile, and it sits between the two.

## 4. Where it must join the spine before any code exists

Non-negotiable, per [`agent.md`](../../agent.md) rules 8 and 10:

- `EPIC-P6B` document; Feature(s) with rows in `feature-contracts.tsv` (implementation/subject classes,
  authority posture, hostile inputs, `BND-*`, `PD-*`, `RCG-*`); Story rows in `story-contracts.tsv`.
- **[`application-platforms.tsv`](../../goals/context/application-platforms.tsv) and
  [`landing-zones.tsv`](../../goals/context/landing-zones.tsv) updated before implementation** — rule 10
  is explicit that a new runtime updates both with goal, performance, security, class, horizon and
  claim-gate selections. `LZ-02` (*Quality-adjusted AI velocity*) is the obvious join.
- Any performance domain selected whose subsystem does not exist yet is **initialised as stated open
  debt** in `open-debt.tsv` at the moment of selection (`LE-35`'s rule).

## 5. The decision that dominates everything: "consume CUDA" versus the code-admission charter

**A GPU kernel is executable code for a bus-mastering processor.** Rule 9 says remote bytes are data,
never code, and no path may create executable memory except through every gate in
[`code-admission-gates.tsv`](../../goals/security/code-admission-gates.tsv). `RCG-11` denies *"JIT,
self-modification, writable executable alias"* by name.

**The charter's text is written about CPU mappings. Nobody has yet decided whether it binds device
code.** The session must decide, and the honest starting position is that **it binds harder, not
softer**: an accelerator is the one execution engine on the board whose code TinyOS does not schedule,
cannot preempt, does not fault-contain — and which can reach memory the CPU's own page tables would have
denied it.

Three candidate shapes. **Pick one and say why the other two were rejected:**

| | Where kernels are compiled | Charter position | Cost |
|---|---|---|---|
| **A. Ahead-of-time, off-target** | A build host runs TileLang/`nvcc`; TinyOS admits the artifact as signed, immutable, non-writable device code through `RCG-07`/`RCG-10`/`RCG-12` | Clean fit; no JIT on target | No runtime specialisation — shape/dtype variants enumerated at build time |
| **B. On-target JIT** | TinyOS compiles at runtime | **Denied by `RCG-11` as written.** Needs a charter amendment and an ADR, and puts a compiler in the trusted path | Most flexible, most expensive, worst security story |
| **C. Interpreted tile IR** | A Rust `no_std` executor walks a validated IR; no code created anywhere | No code admission at all — data end to end | Almost certainly fails "native fast" on the dense kernels that matter |

**Recommendation: A, with a bounded C.** Ahead-of-time is the only shape that satisfies the charter
without amending it, and it matches how this project already admits PE64 artifacts. A small interpreted
path may still earn its place for cold or rare shapes where a missing precompiled variant would
otherwise be a hard failure — but as a documented fallback with its own budget, never the hot path.

**Whatever is chosen, state what happens when a kernel variant is missing.** *Fail-safe over
keep-trying* (rule 6): it resolves to a declared safe outcome — CPU fallback, degraded configuration, or
a clean refusal through the ACI — never an on-demand compile that was not admitted, and never an
unbounded retry against a deadline.

## 6. The second-order decisions, each of which can invalidate the design

1. **`PD-10` versus `G-HW-6`, and this one is sharp.** `PD-10` requires device DMA to be constrained
   *"by IOMMU or wiped bounce buffer"*. `G-HW-6` promises **zero-copy** CPU/GPU buffer sharing. On
   hardware with no IOMMU/SMMU available to TinyOS, `PD-10` forces exactly the copy the UMM exists to
   remove. **Zero-copy and DMA containment are in direct tension** — most likely resolution: zero-copy
   is conditional on a qualified IOMMU and the copy path is the honest default. Same shape as
   `ADR 0005`'s "conditional on qualification" ruling, and it probably wants its own ADR.
2. **The vendor-driver problem, faced honestly.** "Consume CUDA code" cannot mean "run NVIDIA's driver
   stack" — that is a Linux kernel module, and TinyOS is not Linux. Say **what actually talks to the
   device**: an open-stack path, a vendor firmware interface, a Jetson-specific route, or a deliberately
   narrow submission-queue subset. **If the answer is "unknown", that is a legitimate finding and must be
   recorded as one**, not papered over with an API sketch that assumes a driver nobody has.
3. **The C ABI is an `unsafe` boundary.** Every `extern "C"` entry is a trust boundary: caller-owned
   buffers with explicit capacities, integer error codes (no stringly-typed errors), no panic across the
   boundary, and a stated versioning rule. It is also a *stability promise* — decide what "TinyTile ABI
   v1" commits to before it has callers.
4. **No heap.** Every shipped crate is `no_std` with no `global_allocator`, and `check-assurance-spine`
   enforces it — that is the standing evidence behind every `PERF-Dnn-G11` row. A GPU runtime that cannot
   allocate is a real constraint: fixed-capacity pools, caller-supplied storage, compile-time bounds.
5. **The 20,000-LOC crate ceiling** (rule 4). A tile runtime plus a device backend plus an artifact
   loader will not fit in one crate. Decide the split up front and put it in
   [`docs/mvp-delivery-strategy.md`](../../docs/mvp-delivery-strategy.md)'s crate map.
6. **What "native fast" is measured against.** Which performance domain, which guardrails, which tier —
   and remember `ADR 0005`: a *bound* is quotable only from a qualified platform, and zero platforms hold
   a record. Plan for mechanism evidence first, and say so rather than discovering it at Report time.
7. **Where TileLang consumption physically happens.** An offline `xtask`-style tool, a separate
   repository, or a documented external toolchain contract? This decides how much Python ever comes near
   this project. The answer on target should be "none".

## 7. What the session must produce

- `goals/epics/EPIC-P6B.md`, and the Feature/Story decomposition that follows from §5's decision.
- `docs/tinytile-architecture.md` — programming model, C ABI surface, artifact format, admission path,
  device-backend seam, and the RT non-interference argument.
- **An ADR for §5**, and probably a second for §6.1. Both are charter-adjacent rulings later sessions
  will need to cite.
- The register updates in §4.
- An honest **feasibility statement**: what can be proven on the Jetson Orin Nano Super, what needs
  hardware nobody has, and what is blocked behind a driver question.

## 8. Kill criteria — what would make this plan dishonest

- **An API that assumes a driver that does not exist.** A beautiful C ABI over an imaginary submission
  path is a design nobody can falsify. If §6.2 has no answer, the plan says so in its own summary.
- **A zero-copy claim on hardware whose DMA is unconstrained.** That is `PD-10` violated, and it is a
  safety claim, not a performance one.
- **Any wording implying on-target compilation** without the ADR that permits it.
- **A performance claim from an emulator.** `ADR 0005` and `LE-09` apply here exactly as on the RT path.
- **"Inspired by TileLang" used to import its architecture rather than its programming model.** TVM,
  Python and a JIT are not coming into this kernel; the tile abstraction is what is worth having.

## 9. What the session must not do

- **Do not write TinyTile.** No crate, no `extern "C"` shims, no device code. Contracts before code; this
  session's output is contracts and decisions.
- **Do not touch `EPIC-P1`'s or `EPIC-P6`'s open work** to make room for this.
- **Do not amend the Security Charter in passing.** If §5 needs an amendment, that is an ADR with its own
  reasoning, not a sentence added to a spec.

## 10. Traps

1. **The tempting order is backwards.** Designing the C ABI first is the most enjoyable part and the
   least useful: it is downstream of §5 and §6.2, and either can invalidate it entirely.
2. **`no_std` will be discovered late if it is not confronted early.** Most GPU-runtime prior art assumes
   an allocator, threads and a filesystem. TinyTile has none of the three.
3. **This repository's own record is the best guide to what "planned" means here.** Read `05B` and `08C`
   in [`hand-2026-07-29/`](../hand-2026-07-29/) for the house style of a mandate that scopes honestly —
   including how each names what it *cannot* claim.
