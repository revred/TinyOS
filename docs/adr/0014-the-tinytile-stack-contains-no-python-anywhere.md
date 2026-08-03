# ADR 0014 — The TinyTile Stack Contains No Python Anywhere; Python Is a Consumer Through the C ABI

Status: **Accepted**
Date: 2026-08-03
Introduced in: [`session/hand-2026-08-03/01A-no-python-in-the-tinytile-stack.md`](../../session/hand-2026-08-03/01A-no-python-in-the-tinytile-stack.md)
Governs: [`goals/epics/EPIC-P6B.md`](../../goals/epics/EPIC-P6B.md), [`docs/tinytile-architecture.md`](../tinytile-architecture.md)
Amends: [`ADR 0012`](0012-device-kernels-are-admitted-code-compiled-ahead-of-time-off-target.md) clause 2 — **narrowing it, not superseding it**

## Context

Owner constraint, 2026-08-03: **"there should be no python in tinyTile stack anywhere — it is built
for speed"**, with the clarification that **"python can of course use or call into it through C ABI"**.

`ADR 0012` had already banned every compiler from the device (clause 3: *"No Python, no TVM, no
vendor compiler, no IR-to-machine translation on target"*). That half is unaffected and remains
exactly as ruled.

What `ADR 0012` also did, in clause 2, was leave the *build host* unconstrained: *"A build host — CI
or a deployment-signing station — runs TileLang, TVM, `nvcc`, `hipcc`, or any other toolchain it
likes."* Since TileLang **is** a Python DSL on TVM, that clause put Python squarely inside TinyTile's
own toolchain. The new constraint reaches further than the device boundary and closes that.

The distinction the constraint draws is between Python as a **component** and Python as a
**consumer**, and the two are not the same risk or the same dependency.

## Decision

**1. No Python is a component of the TinyTile stack, on target or off it.** Nothing TinyOS builds,
ships, tests, gates, or *requires in order to produce a kernel artifact* may be written in Python or
depend on a Python runtime. This extends `ADR 0012` clause 3's device rule to the whole stack,
including CI steps, signing stations, and any `xtask`-side driver.

**2. Python may call into TinyTile through the C ABI, without restriction.** A Python host — a
notebook, a PyTorch-style framework, an orchestration script — is a legitimate and expected
consumer. The C ABI exists precisely so callers in any language can reach the layer at native speed.
A consumer is not part of the stack, takes no part in producing artifacts, and imposes no build
dependency on TinyOS. Nothing in clause 1 discourages this.

**3. TinyTile's off-target compiler is Rust-native.** `ADR 0012` clause 2 is narrowed: the build host
runs **TinyTile's own Rust toolchain**, not TileLang and not TVM. A vendor *assembler/compiler* that
is not Python (`nvcc`, `ptxas`, `hipcc`) may still be invoked as a back-end step off target — the
constraint names Python, and those are C++ programs — but the tile front end, layout inference,
specialization and artifact emission are Rust.

**4. "Easily consumes TileLang or CUDA code" is re-read, not withdrawn.** The owner constraint from
`EPIC-P6B` stands. With clause 1 in force it can be satisfied in exactly two ways, and the Feature
work must say which it is building:

- **(a) Rust-native front end.** TinyTile ingests TileLang-*shaped* tile source and/or CUDA C kernel
  source and lowers it with Rust. "Consume" means *parse and compile*, and the tile programming
  model — which `ADR 0012` and the architecture document already establish is what was worth
  borrowing — is expressed in a Rust surface (macro or builder) rather than a Python DSL.
- **(b) Artifact ingestion.** A third party's already-compiled device code (PTX, cubin, HSACO)
  arrives as **data** and is admitted through the unchanged `ADR 0012` gate chain. Whatever produced
  it upstream is outside TinyOS's stack and outside its dependencies.

**5. The boundary, stated so it cannot drift.** The constraint binds what TinyOS *builds, ships, or
requires*. It cannot bind what a third party ran before handing over a signed artifact, and clause
4(b) does not become a loophole for clause 1: **no documented TinyTile path may depend on Python
existing anywhere**, and no TinyOS CI job, `xtask` subcommand, or signing procedure may invoke it.

## Rationale

- **It removes a dependency, and dependencies are the thing this project is most careful about.**
  TileLang/TVM would have made a Python runtime, a C++ compiler infrastructure, and their transitive
  supply chain load-bearing for producing any TinyOS kernel artifact — inside the *signing* path,
  which is the most trust-sensitive position in the whole system. `RCG-07` signs what the toolchain
  emits; a smaller toolchain is a smaller thing to trust.
- **"Built for speed" is a design instruction about the whole pipeline, not just the hot path.**
  Build-time specialization, autotuning and variant enumeration are where `ADR 0012` clause 2 put the
  cost of not shipping a JIT. Those are compile-heavy, repeated, CI-resident workloads, and Rust is
  the right tool for them by the same reasoning that put the runtime in Rust.
- **One language across the stack is a stated project value**, not a preference invented here — the
  language policy already confines non-Rust to `-sys` binding crates.
- **The consumer allowance costs nothing and buys reach.** Python calling in through the C ABI is how
  most AI tooling will actually meet this layer. Permitting it explicitly prevents a future session
  reading clause 1 as a ban on Python bindings, which it is not.

## Consequences

- **`FEAT-P6B-03` changes shape and grows.** It was *"how CI/signing hosts run TileLang/`nvcc`"* — a
  contract around someone else's compiler. It becomes **TinyTile's own Rust tile compiler**, which is
  materially more work, and that cost is **honestly attributable to this ADR** rather than discovered
  later. The Feature must state which of clause 4's two paths it delivers first; **(b) is the smaller
  increment and the likelier first Story**, with (a) as the substantial one.
- **A Python binding over the C ABI is now an anticipated artifact**, and may be built by anyone. It
  is out of scope for `EPIC-P6B` itself and is not a TinyOS deliverable.
- **The negative-footprint evidence widens.** `EPIC-P6B`'s exit criterion was zero Python bytes *on
  any TinyOS device*; it now covers the toolchain too — no Python in any CI job, `xtask` subcommand,
  or signing step that produces or gates a kernel artifact.
- **`ADR 0012` is otherwise untouched.** Clauses 1, 3, 4 and 5 — device code is admitted code, no
  compiler on target, the bounded interpreted fallback, fail-safe on a missing variant — all stand.

## Alternatives considered and rejected

- **Reading the constraint as device-only.** That is what `ADR 0012` clause 3 already said; the
  constraint would then be a restatement rather than a ruling, and the word "anywhere" would be doing
  no work. Rejected as a reading that ignores what was actually asked for.
- **Banning Python as a consumer too.** Not what was asked, and it would cut TinyTile off from the
  ecosystem it exists to serve, for no security gain: a caller through a C ABI is on the outside of
  the boundary, holding whatever authority the ACI granted it and no more.
- **Keeping TileLang off-target "just for now".** A load-bearing dependency in the signing path is
  not a temporary convenience; it would be exactly the kind of thing a later session finds impossible
  to remove because six Features grew on top of it.
