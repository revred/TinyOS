# Handover 01A — No Python in the TinyTile Stack, Anywhere; Python Calls In Through the C ABI

**Owner ruling, 2026-08-03**, recorded as [`ADR 0014`](../../docs/adr/0014-the-tinytile-stack-contains-no-python-anywhere.md)
and propagated into the three documents that asserted otherwise. Narrows — does not supersede —
[`ADR 0012`](../../docs/adr/0012-device-kernels-are-admitted-code-compiled-ahead-of-time-off-target.md).

Follows [`hand-2026-08-01/03A`](../hand-2026-08-01/03A-the-dashboard-moves-because-the-register-moved.md).
This is a decision record and a documentation change; **no code was written and none was owed.**

## 1. The ruling

> *"There should be no python in tinyTile stack anywhere — it is built for speed."*
> *"However, python can of course use or call into it through C ABI."*

Two sentences that draw one line: **Python may not be a component of TinyTile; Python may be a
consumer of it.** Those are different risks and different dependencies, and `ADR 0014` keeps them
apart deliberately rather than collapsing to "no Python".

## 2. What was already right, and what was not

`ADR 0012` clause 3 had already banned every compiler from the device — *"No Python, no TVM, no
vendor compiler, no IR-to-machine translation on target"*. **That half needed no change.**

The gap was one sentence in clause 2, which left the build host deliberately unconstrained: *"A build
host — CI or a deployment-signing station — runs TileLang, TVM, `nvcc`, `hipcc`, or any other
toolchain it likes."* **TileLang is a Python DSL on TVM.** That sentence therefore put a Python
runtime and TVM's compiler infrastructure inside the path that produces *and signs* kernel artifacts
— the most trust-sensitive position in the system, since `RCG-07` signs whatever the toolchain emits.

The word "anywhere" in the ruling is doing real work: read as device-only it would merely restate
clause 3. It reaches past the device boundary to the whole stack, and that is how it has been
recorded.

## 3. What changed

| Document | Change |
|---|---|
| [`ADR 0014`](../../docs/adr/0014-the-tinytile-stack-contains-no-python-anywhere.md) | **New.** Five clauses: no Python component on or off target; Python permitted as a C-ABI consumer; the off-target compiler is Rust-native; "consumes TileLang or CUDA" re-read into two concrete paths; and the boundary stated so it cannot drift |
| `ADR 0012` | Clause 2 annotated in place, plus a dated amendment section at the foot. Clauses 1, 3, 4, 5 stand untouched |
| `EPIC-P6B` | Four owner-settled constraints → **five**; `FEAT-P6B-03` re-shaped; the exit criterion widened from "zero Python bytes on any device" to the toolchain as well |
| `docs/tinytile-architecture.md` | Constraint preamble, §2's "stays on the build host, always", and the off-target half of the two-places model |

## 4. The consequence, priced honestly rather than discovered later

**`FEAT-P6B-03` was a contract around somebody else's compiler. It is now a compiler.**

It read *"How CI/signing hosts run TileLang/`nvcc` and emit TKAs"* — a wrapper. With TileLang and TVM
excluded, TinyTile needs its own Rust tile front end, layout inference and specialization. That is
materially more work than was scoped on 2026-08-01, and `ADR 0014` says so in its own Consequences
section rather than letting a future session find it.

**It does not all have to arrive at once.** `ADR 0014` clause 4 splits "easily consumes TileLang or
CUDA code" into two paths, and they are very different sizes:

- **(b) Artifact ingestion** — third-party precompiled device code (PTX, cubin, HSACO) arrives as
  **data** and passes `ADR 0012`'s unchanged gate chain. Whatever produced it upstream is outside
  TinyOS's stack and imposes no dependency on it. **This is the smaller increment and the likelier
  first Story.**
- **(a) Rust-native front end** — parse TileLang-shaped tile source and/or CUDA C, lower it in Rust.
  The substantial one, and the one that makes "consume" mean *compile*.

`FEAT-P6B-03` must now state which it delivers first. Neither is scoped here; that is the Feature's
own decomposition.

## 5. What this does not do

- **It does not ban Python bindings.** A Python caller over the C ABI is anticipated, permitted, and
  probably how most AI tooling will meet this layer. It is out of scope for `EPIC-P6B` and is not a
  TinyOS deliverable — but nothing here discourages one, and clause 2 exists so a later session cannot
  misread clause 1 as forbidding it.
- **It does not weaken `ADR 0012`.** Device code is still admitted code; no compiler still runs on
  target; the bounded interpreted fallback and the fail-safe on a missing variant are unchanged.
- **It does not ban non-Python vendor tools.** `nvcc`, `ptxas` and `hipcc` are C++ programs and may
  still be back-end steps off target. The constraint names Python.
- **It changes no code.** `EPIC-P6B` has no implementation yet, which is the cheapest possible moment
  for a constraint like this to arrive — the cost is three documents, not six Features grown on top
  of a dependency nobody could remove.

## 6. One interpretive call the owner may want to correct

Clause 5 draws the boundary at what TinyOS **builds, ships, or requires**. It cannot bind what a
third party ran before handing over a signed artifact — so clause 4(b) lets a customer bring a
TileLang-produced `cubin`, because by then it is data and Python was in *their* pipeline, not ours.

The guard against that becoming a loophole is stated in the same clause: **no documented TinyTile
path may depend on Python existing anywhere**, and no TinyOS CI job, `xtask` subcommand or signing
step may invoke it. If the intent was stricter — that TinyOS should refuse artifacts of Python-derived
provenance outright — that is a different and much harder rule (it needs provenance attestation in the
manifest, not just a signature), and it should be said explicitly before `FEAT-P6B-02` fixes the TKA
manifest schema.

## 7. State

- `main` at the time of writing was level with `origin/main`; the working tree carries only the
  external soak log's appends.
- `check-assurance-spine` green after the change.
- No register rows added: nothing here is a defect or an open end, and `EPIC-P6B`'s Features are
  still `specified`.
