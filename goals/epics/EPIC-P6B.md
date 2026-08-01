# EPIC-P6B — Heterogeneous Compute: the TinyTile Access Layer

Status: **Specified — Epic written, Features enumerated but not decomposed; no Story exists. Implementation queues behind the hardware-evidence sprint (Pi 5 first silicon, 2026-07-30 owner priority) and behind `EPIC-P6`.**
Roadmap phase: 6b — Heterogeneous compute.
Introduced in: [`session/hand-2026-08-01/01A-tinytile-planning-session-mandate.md`](../../session/hand-2026-08-01/01A-tinytile-planning-session-mandate.md) (owner-ordered planning mandate)
Governing rulings: [`ADR 0012`](../../docs/adr/0012-device-kernels-are-admitted-code-compiled-ahead-of-time-off-target.md) (device kernels are admitted code, compiled ahead of time, off target) and [`ADR 0013`](../../docs/adr/0013-zero-copy-buffer-sharing-is-conditional-on-dma-containment-qualification.md) (zero-copy is a per-platform qualified capability)
Depends on: `EPIC-P6` (LLM integration — the flagship caller), and on `EPIC-P0`'s virtual-memory substrate (`FEAT-P0-03` onward) for the UMM's mapping primitives.
Proving board: **Jetson Orin Nano Super** — one conformance target, deliberately not the architectural centre.

## Goal

Let AI and similar throughput workloads reach a GPU or other accelerator efficiently on TinyOS,
through a portable compute substrate — not "TileLang inside TinyOS", and not a CUDA rebuild.

**The gap this Epic fills, stated precisely:** the UMM owns *buffers*
([`docs/inference-architecture.md`](../../docs/inference-architecture.md)); the inference runtime
owns *workloads*. **Nothing owns *kernels*** — how a tile-level computation is expressed, admitted,
scheduled onto a device queue, executed, and accounted for. TinyTile is that layer, and it sits
between the two.

## The four owner-settled constraints (not open for re-litigation)

1. **Entirely Rust, exposing a C ABI.**
2. **Native-fast** — no marshalling layer, no interpreter tax on the hot path.
3. **Easily consumes TileLang or CUDA code** (design inspiration: `tilelang.com`).
4. **This is to be done** — a planned destination, not a thought experiment.

## What already exists and is built on, not re-opened

Fixed by [`docs/inference-architecture.md`](../../docs/inference-architecture.md): the compute
**admission model** (accelerator work is admission-controlled, never scheduler-prioritised; the RT
kernel never blocks on a compute-device submission, fence, or driver call), the **UMM** (typed
ownership-tracked handles, single writer, explicit fenced hand-off, explicit-copy fallback behind
the same handle API), the **mmap/pointer model-load path** with its recorded corrections, and the
rule that **vendor bindings live in `-sys` crates** with safe logic on top.

What is borrowed from TileLang is its **programming model** — tile-level abstraction, explicit
memory scopes (global/shared/fragment), explicit pipeline stages, a small set of composable
primitives with layout inference underneath. What is *not* coming into this kernel: TVM, Python,
`nvcc`, or any JIT ([`ADR 0012`](../../docs/adr/0012-device-kernels-are-admitted-code-compiled-ahead-of-time-off-target.md) clause 3).

## The shape, settled by ADR 0012

```
TileLang / CUDA source ──► off-target compiler (CI or signing station)
                                    │
                     signed Tiny Kernel Artifact (TKA)
                                    │
                 code-admission gates (RCG-07/-10/-12, full chain)
                                    │
             TinyTile C ABI: discover · import buffers · load · dispatch
                                    │
             UMM handles ── queue/fence ── admission controller ── telemetry
                                    │
        CPU reference backend │ HBP-brokered accelerator │ native device backend
```

The on-device runtime is small and auditable. Its whole job: discover device capabilities,
allocate/import typed buffers through the UMM, load admitted kernel artifacts, dispatch work under
declared memory and execution budgets, poll/cancel/reset queues, and return telemetry — time,
energy, bandwidth, temperature, and RT interference. Compilation happens off target, always.

### The Tiny Kernel Artifact

A TKA is a signed, immutable package admitted like any other code
(`ADR 0012` clause 2): target requirements (device class, ISA/SM level), executable hash per
variant, buffer schema, maximum scratch/device-local memory, workgroup limits, expected dtypes,
**mandatory CPU fallback**, and provenance (what compiled it, from what source, when, signed by
whom). A missing variant resolves fail-safe — CPU fallback, bounded interpreter, declared
degradation, or clean ACI refusal — never an on-demand compile (`ADR 0012` clause 5). Wire
envelopes carrying TKAs or their manifests follow the `TOS64-*` prefix convention.

### Vocabulary ruling

This Epic and its children say **compute-device submission** (not "GPU submission") and
**device-local memory** (not "VRAM"); the UMM is defined as a **heterogeneous buffer/fence
contract**, not CUDA-style managed memory. Existing prose migrates opportunistically, not by sweep.

## What talks to the device — faced honestly

"Consume CUDA code" cannot mean "run NVIDIA's Linux driver stack." What is actually known,
verified against public sources on 2026-08-01:

- On the Orin, the integrated Ampere GPU (sm_87) is driven by **`nvgpu`** — NVIDIA's
  source-available Tegra GPU driver (MIT core, GPLv2 Linux glue, published in the Jetson Linux
  kernel sources) — which depends on `host1x` (command/sync) and `nvmap` (memory) rather than on
  the desktop stack. This is the **documented reference for the submission interface**, and its
  availability in source form is materially better than the desktop-GPU situation.
- The **nouveau/NVK/GSP** open path targets discrete Turing-and-later GPUs initialised via GSP
  firmware; it is **not** the Orin iGPU route.
- **TileLang's published validation covers datacenter/desktop devices** (H100/A100/V100, RTX
  4090/3090/A6000, MI250/MI300X). No Tegra device appears on its tested list; emitting sm_87 is
  expected to work and is **unproven publicly**. This is a named unknown, not an assumption.

Therefore the device path is staged, and the stages are Features:

1. **Stage 1 — HBP-brokered accelerator.** The accelerator lives on a Linux host behind a C2
   broker speaking HBP (the same transport pattern `G-AI-7` already blesses). TinyOS proves the C
   ABI, the TKA format, admission, queues, budgets, and telemetry end to end. The broker stage
   *sidesteps* `PD-10`/`ADR 0013` (the broker host's kernel contains the device's DMA, not
   TinyOS), and every piece of evidence from it says so.
2. **Stage 2 — native backend on the Orin**, scoped as a deliberately narrow submission-queue
   subset informed by the `nvgpu` sources, in `-sys` crates. Gated on hardware evidence
   (`LE-09`-class: this project has never initialised this device) and on `ADR 0013` qualification
   for any zero-copy configuration.

If Stage 2's investigation concludes the submission path is not tractable, **that is a legitimate
finding recorded as such**, and TinyTile remains real: Stage 1 plus the CPU backend is a shippable
substrate. An API that assumes a driver nobody has is this Epic's first kill criterion.

## Features (enumerated, not decomposed)

Contract rows in `feature-contracts.tsv`/`story-contracts.tsv` are created at decomposition time,
per the just-in-time rule — none exist yet, deliberately. Any performance domain selected whose
subsystem does not exist is initialised as stated open debt in `open-debt.tsv` (`LE-35`).

| Feature | Summary | Depends on |
|---|---|---|
| `FEAT-P6B-01` | **Tiny Compute ABI + CPU reference backend.** The C ABI surface (`extern "C"`, caller-owned buffers with explicit capacities, integer error codes, no panic across the boundary, versioned from v1) and a `no_std` CPU backend that makes every TKA executable somewhere. Landable earliest; needs no device. | `FEAT-P0-03` substrate |
| `FEAT-P6B-02` | **Tiny Kernel Artifact format + admission path.** Manifest schema, signing, the full gate chain for device code, hostile-artifact parsing tests. | nothing |
| `FEAT-P6B-03` | **Off-target toolchain contract.** How CI/signing hosts run TileLang/`nvcc` and emit TKAs; an `xtask`-style driver or documented external contract. Zero Python on target. | `-02` |
| `FEAT-P6B-04` | **Queue, fence, admission and telemetry runtime.** Dispatch under budgets, poll/cancel/reset, UMM seam, RT non-interference argument and its adversarial tests. | `-01`, `-02` |
| `FEAT-P6B-05` | **HBP compute broker** (Stage 1). Linux-hosted C2 broker; remote accelerator behind the same C ABI. | `-01` – `-04`, HBP |
| `FEAT-P6B-06` | **Native Orin backend** (Stage 2). Narrow `nvgpu`-informed submission subset in `-sys` crates; `ADR 0013` qualification for any zero-copy config. | `-04`, hardware evidence, driver finding |
| `FEAT-P6B-07` | **Vertical proof.** One quantized GEMV/dequantization TKA compiled from a TileLang(-inspired) source, served through the inference runtime (attributed per `G-AI-10` — never presented as blue-sharc.exe being the model runtime), measured on the `G-AI-9` axes: TTFT, prefill, decode, quality, energy, thermal, memory bandwidth, RT interference. | `-05` (Stage 1 suffices) |

Crate-split candidates (`tinytile-abi`, `tinytile-artifact`, `tinytile-queue`, `tinytile-cpu`,
device `-sys` crates) are recorded in [`docs/mvp-delivery-strategy.md`](../../docs/mvp-delivery-strategy.md)'s
crate map at decomposition time — one crate will not fit under the 20,000-line ceiling and is not
going to try.

## Registers: the four-plane join

TinyTile's product destination is **already registered**: `LZ-02` (*Quality-adjusted AI velocity*)
carries `G-AI-6`, and `APP-03` declares the "budgeted C3 inference supervisor uses **C2 GPU and
storage brokers**" seam that TinyTile implements. This Epic therefore joins existing rows rather
than adding one. **Trigger recorded:** if decomposition exposes TinyTile as a caller-facing compute
service beyond `APP-03`'s declaration (third-party compute jobs, not inference), an `APP-20` row
becomes mandatory before that Feature's implementation — and it entails bumping xtask's
`APPLICATION_PLATFORM_COUNT` (test-first) and sweeping the "19 application/platform targets" prose
in `agent.md`/`README.md`/the dashboard. That obligation is named here so it cannot be discovered
late.

## "Native fast", measured honestly

Mechanism evidence first. No performance claim from an emulator (`LE-09`'s rule), no bound except
from a qualified platform (`ADR 0005`'s discipline), no zero-copy figure without an `ADR 0013`
qualification record, and every figure names its path (copy vs zero-copy) and stage (brokered vs
native). `G-AI-9` defines the reporting axes; quality-adjusted, never marketing tokens/second.

## Exit criteria

- All seven Features Verified with contract rows, Tests, and dated Reports — or explicitly closed
  as findings (a recorded "Stage 2 not tractable on this board" can close `FEAT-P6B-06` honestly).
- `FEAT-P6B-07`'s vertical proof: tokens served through the inference runtime using at least one
  TKA kernel, on at least one real accelerator (brokered counts), with the full `G-AI-9` axis set
  reported.
- The `ADR 0013` question answered for the Orin: a DMA-containment qualification record, or a
  recorded statement of why one cannot yet exist.
- Zero Python, TVM, or vendor-compiler bytes on any TinyOS device, demonstrated by the same
  negative-footprint evidence style `LZ-09` requires.

## Explicitly out of scope

- **Running NVIDIA's driver stack, CUDA runtime, or any vendor userspace on TinyOS.**
- **On-target compilation in any form** — shape B is denied by `RCG-11`; superseding `ADR 0012` is
  the only door, and it is not this Epic's to open.
- **A general-purpose GPU display/graphics stack.** TinyTile is compute; display belongs to its
  own roadmap work.
- **Operator libraries at CUDA breadth.** One proven kernel family at a time; breadth is earned by
  the toolchain, not shipped as a library port.
- **Motion/RT timing guarantees.** TinyTile's obligation to the RT core is non-interference,
  proven adversarially — never a latency promise of its own.

## Kill criteria — what would make this Epic dishonest

Inherited from the mandate §8, kept in force verbatim: an API that assumes a driver that does not
exist; a zero-copy claim on hardware whose DMA is unconstrained; any wording implying on-target
compilation without its ADR; a performance claim from an emulator; "inspired by TileLang" used to
import its architecture rather than its programming model.
