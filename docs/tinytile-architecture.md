# TinyTile Architecture — the Heterogeneous Compute Access Layer

Status: **Specified — design document only. No crate, no `extern "C"` shim, no device code exists;
implementation queues behind the hardware-evidence sprint (Pi 5 first silicon) per
[`EPIC-P6B`](../goals/epics/EPIC-P6B.md)'s own Status header.**
Parent Epic: [`EPIC-P6B`](../goals/epics/EPIC-P6B.md) (Phase 6b — Heterogeneous compute)
Governing rulings: [`ADR 0012`](adr/0012-device-kernels-are-admitted-code-compiled-ahead-of-time-off-target.md)
(device kernels are admitted code, compiled ahead of time, off target) and
[`ADR 0013`](adr/0013-zero-copy-buffer-sharing-is-conditional-on-dma-containment-qualification.md)
(zero-copy is a per-platform qualified capability; the copy path is the default)
Commissioned by: [`session/hand-2026-08-01/01A`](../session/hand-2026-08-01/01A-tinytile-planning-session-mandate.md) §7,
delivered under [`01C`](../session/hand-2026-08-01/01C-next-steps-after-tinytile-planning.md) Step 2

Everything normative in this document is downstream of the two ADRs above and of
[`docs/inference-architecture.md`](inference-architecture.md)'s fixed decisions (compute admission
model, UMM, mmap model-load path, `-sys` crate rule). Where a number below is a v1 *proposal* rather
than a settled ruling, it says so; `FEAT-P6B-02`'s test-first decomposition pins the artifact format
byte-for-byte, and `FEAT-P6B-01`'s pins the ABI. Nothing here re-opens shapes B or C-as-hot-path —
that door is a superseding ADR, not this document.

---

## 1. Position in the system

The gap TinyTile fills, restated from the Epic: the **UMM owns buffers**, the **inference runtime
owns workloads**, and **nothing owns kernels** — how a tile-level computation is expressed,
admitted, scheduled onto a device queue, executed, and accounted for. TinyTile is that layer, and it
sits between the two.

```
        inference runtime (C3, budgeted)          any other admitted caller (C3)
                     │                                        │
                     └────────────── TinyTile C ABI ──────────┘
                                          │
              ┌────────────┬──────────────┼───────────────┬──────────────┐
              │ discover   │ buffers      │ artifacts     │ dispatch /   │ telemetry
              │            │ (UMM seam)   │ (admitted)    │ queue mgmt   │
              └────────────┴──────────────┴───────────────┴──────────────┘
                                          │
                          admission controller · budgets · fences
                                          │
        ┌───────────────────────┬─────────┴──────────────┬────────────────────────┐
        │ CPU reference backend │ HBP compute broker     │ native device backend  │
        │ (always present)      │ (Stage 1, remote C2)   │ (Stage 2, gated)       │
        └───────────────────────┴────────────────────────┴────────────────────────┘
```

Four owner-settled constraints bind every section below: entirely Rust exposing a C ABI;
native-fast (no marshalling layer, no interpreter tax on the hot path); easily consumes TileLang or
CUDA code — *at the toolchain boundary, per `ADR 0012` clause 3*; and this is a planned
destination, not a thought experiment.

## 2. The tile programming model, in Rust `no_std` terms

What is borrowed from TileLang is its **programming model**, not its machinery: tile-level
abstraction (the unit of reasoning is a tile of a tensor, not a thread), explicit memory scopes
(global / shared / fragment), explicit pipeline stages, and a small set of composable primitives
with layout inference underneath. TVM, Python, `nvcc`, and the `@tilelang.jit` decorator stay on
the build host, always.

The model therefore lives in **two places with an artifact between them**:

- **Off target** (CI or a signing station): the full authoring surface. TileLang or CUDA source,
  vendor toolchains, layout inference, shape/dtype specialization, autotuning — anything the build
  host likes. Its output is a Tiny Kernel Artifact (§4), and that is the *only* thing that crosses
  toward a device.
- **On target**: no source, no IR on the hot path, no inference of anything. The runtime sees a
  kernel as its **declared interface**: a buffer schema, launch-geometry limits, memory-scope
  ceilings, an execution budget, and one or more admitted executable variants. The memory scopes
  survive on target only as *declared, admission-checked quantities* — device-local scratch bytes,
  shared bytes per workgroup, fragment/register pressure class — never as types the runtime
  manipulates.

In `no_std` Rust terms, the on-target model is:

- **Plain-old-data descriptors with compile-time bounds.** Kernel, buffer, and dispatch descriptors
  are fixed-layout structs with bounded tables (v1 proposal: ≤ 16 buffers per kernel, ≤ 8 variants
  per artifact, ≤ 3 launch dimensions — pinned by `FEAT-P6B-02` tests). No descriptor owns heap
  memory because there is no heap: every shipped crate is `no_std` with no `global_allocator`, the
  standing `PERF-Dnn-G11` evidence.
- **Caller-supplied and pool storage only.** Queue slots, fence records, and telemetry rings come
  from fixed-capacity pools sized at admission time out of the caller's own quota (charter
  Protection Domain rule 9, finite ownership). A caller that wants a deeper queue asks for it in
  its admission request and is granted, throttled, or refused — it cannot grow one at runtime.
- **Buffers are UMM handles, never raw pointers.** Typed, ownership-tracked, single-writer,
  explicitly fenced (`inference-architecture.md`); whether a handle is backed by a zero-copy
  mapping or an explicit-copy/bounce path is a platform-qualification outcome (`ADR 0013`), not an
  API distinction — callers observe it only through telemetry.
- **The bounded interpreter is data end to end.** `ADR 0012` clause 4's fallback executor walks a
  *validated* tile IR under its own declared time and memory budget. Selecting it is a reportable
  degradation. Its IR validator is a parser of hostile bytes and carries the corresponding
  adversarial-test obligation; the IR blob travels inside the TKA (§4.3) and is admitted as data,
  creating no executable memory.

## 3. The C ABI surface (v1)

Every `extern "C"` entry is a trust boundary and an `unsafe` boundary. The rules below are the
contract; the entry-point families follow. This section defines the surface — it is deliberately
*not* a header file, and no shim exists until `FEAT-P6B-01` opens with its tests.

### 3.1 Boundary rules (all normative, v1)

1. **Caller-owned buffers, explicit capacities.** Every out-parameter is caller-allocated storage
   passed with its capacity in bytes (or element count); the callee writes at most that much and
   returns the length actually written (or required, on overflow — see error model). The library
   never allocates on the caller's behalf and never returns a pointer it expects the caller to free.
2. **Integer error codes.** Every entry returns `int32`: `0` is success; negative values are error
   codes from a single stable enumeration (v1 families proposed: argument/handle errors,
   admission/budget refusals, missing-variant outcomes, device/backend faults, revocation/staleness).
   Codes are never renumbered and never reused; new codes may be added. No stringly-typed errors
   cross the boundary; a bounded diagnostic-string lookup (`code → static message`) exists for
   humans and is not part of any control flow.
3. **No panic across the boundary.** Every ABI function is total: all failure modes surface as
   error codes. A panic reaching an ABI frame is a defect, and its handling is the kernel's
   fail-safe fault path (stop and report the domain, per charter PD rule 12) — never an unwind into
   C. This is a stated adversarial-test target, not an assumption.
4. **Handles are opaque, generation-tagged 64-bit values.** Device, artifact, buffer, queue, and
   submission handles carry a generation; any use after revocation or reset fails with a staleness
   error (revoke-before-reuse, charter PD rule 13). Handles are per-domain capabilities — they do
   not travel between domains except through the UMM's own fenced hand-off.
5. **Calling context.** The ABI is callable from budgeted non-RT task contexts only. No entry may
   be reached from an RT task, an interrupt context, or the kernel's own RT paths; §7's NI-1 states
   this as a falsifiable claim with its enforcement and its refutation test.
6. **v1 stability statement.** From the moment `FEAT-P6B-01` ships v1: existing function
   signatures, struct layouts, error-code meanings, and success semantics are frozen. Evolution is
   additive only — new functions, new error codes, new optional capability flags. Every versioned
   struct begins with its own size field; the library rejects sizes it does not recognise rather
   than guessing. An incompatible change is v2, a new symbol prefix, and a migration note — not a
   quiet edit. `tt_abi_version` reports `(major, minor)`; callers pin major.

### 3.2 Entry-point families

| Family | Entries (v1 names, indicative) | Contract |
|---|---|---|
| Version & discovery | `tt_abi_version`, `tt_device_count`, `tt_device_info` | Enumerate admitted backends and their capability records: device class, ISA/SM level, memory ceilings, queue depths, buffer-sharing path (copy vs qualified zero-copy, with the `ADR 0013` qualification-record reference or its absence). Discovery never touches hardware it hasn't been granted. |
| Buffers (UMM seam) | `tt_buffer_alloc`, `tt_buffer_import`, `tt_buffer_release`, `tt_buffer_fence_handoff` | Thin, typed veneer over UMM handles — TinyTile adds no second buffer model. Single writer, explicit fenced hand-off, dtype/extent tags checked against kernel schemas at dispatch. Import of an existing UMM handle (e.g. a demand-paged model-weight region) transfers no ownership and raises no trust. |
| Artifacts | `tt_artifact_load`, `tt_artifact_query`, `tt_artifact_release` | `load` accepts only a reference to an **already-admitted** TKA (§5) — a post-`RCG-12` object with kernel-recorded provenance. Raw bytes are not an input to this ABI, on any path, ever. `query` reports the manifest's declared interface so callers bind buffers without re-parsing anything. |
| Dispatch | `tt_dispatch` | Binds buffers to a named kernel + variant selection policy, supplies launch geometry within manifest limits, and charges the submission against the caller's admitted budget. Returns a submission handle immediately; never blocks on the device. Refusal reasons are distinct codes: over-budget, missing variant (with the fail-safe resolution actually taken, per `ADR 0012` clause 5), geometry out of range, schema mismatch, backend fault. |
| Completion & queues | `tt_poll`, `tt_wait_bounded`, `tt_cancel`, `tt_queue_reset` | `poll` is non-blocking. `wait_bounded` takes a mandatory timeout and is refused wholesale in contexts where waiting is illegal; there is no unbounded wait in the surface at all. `cancel` and `reset` are the fail-safe levers: reset revokes in-flight submissions, advances the queue generation, and reports what was lost. |
| Telemetry | `tt_telemetry_read` | Per-submission and per-queue: time, energy where the platform meters it, bytes moved, path taken (copy / zero-copy / brokered / CPU-fallback / interpreter), degradation events, and the RT-interference counters §7 defines. Every figure a Report quotes traces to this surface with its stage and path named (`ADR 0013` clause 4). |

**What is deliberately absent from v1:** kernel compilation of any kind (`ADR 0012` clause 3),
raw-pointer buffer entry points, callback registration into caller code from completion context
(completion is poll/bounded-wait only — no foreign code runs in TinyTile's contexts), and any
graphics/display surface (out of the Epic's scope).

## 4. The Tiny Kernel Artifact (TKA), field by field

A TKA is a signed, immutable package admitted like any other code (`ADR 0012` clause 2). One
artifact carries one kernel's declared interface plus one or more executable variants. All integers
are little-endian, all offsets are from byte 0 of the container, all regions are non-overlapping
and in file order, and every offset/size pair is validated against `total_size` before any use —
the parse happens in a disposable zero-authority C4 domain (`RCG-03`) and this container exists to
be parsed hostilely. Field widths below are the **v1 proposal `FEAT-P6B-02` pins with tests**.

### 4.1 Container header (fixed 64 bytes)

| Offset | Size | Field | Meaning / rejection rule |
|---|---|---|---|
| 0 | 8 | `magic` | ASCII `TOS64TKA`. Anything else: not a TKA. |
| 8 | 2 | `format_major` | v1 = 1. Unknown major: reject (no forward parsing). |
| 10 | 2 | `format_minor` | Additive changes only within a major. |
| 12 | 2 | `header_size` | Must be 64 in v1. |
| 14 | 2 | `hash_alg` | Enum; v1 defines exactly one (SHA-256). One algorithm per artifact; unknown value: reject. |
| 16 | 8 | `total_size` | Exact container byte length. Mismatch with transport-delivered length: reject. |
| 24 | 8 + 8 | `manifest_off`, `manifest_size` | Canonical manifest region (§4.2). |
| 40 | 8 + 4 | `variant_table_off`, `variant_count` | §4.3; `variant_count` ≥ 1 and ≤ 8 (v1 bound). |
| 52 | 4 | `flags` | v1: bit 0 = interpreter-IR blob present. Undefined bits set: reject (no ignore-and-hope). |
| 56 | 8 | `signature_off` | Signature block (§4.4) runs from here to `total_size`. |

The payload region (device binaries and optional IR blob) occupies the space the variant table's
entries point into, between the variant table and the signature block. There is no field two
parsers can disagree about (`RCG-04`'s anti-polyglot rule): every byte of the container belongs to
exactly one declared region, and slack bytes are zero or the artifact is rejected.

### 4.2 Manifest (canonical, hashed, bounded)

The manifest is a fixed-order, fixed-width structure — not a free-form encoding — so its canonical
form *is* its wire form and `RCG-04`'s canonicalisation step has nothing to normalise.

| Field | Type / bound | Meaning |
|---|---|---|
| `artifact_id` | 32 bytes | The container's content hash with `signature_off..total_size` zeroed; recomputed at admission, mismatch rejects. |
| `kernel_name` | UTF-8, ≤ 64 bytes, NUL-padded | Human/telemetry name; no control flow hangs on it. |
| `rollback_counter` | u64 | Monotonic per `kernel_name`+signer; `RCG-06` anti-rollback input. |
| `provenance` | fixed block | Toolchain id + version (≤ 48 bytes), source-tree hash (32 bytes), build UTC timestamp (u64 seconds), builder identity hash (32 bytes). Who compiled what, from what, when — `ADR 0012` clause 2's provenance, machine-checkable. |
| `buffer_schema[]` | ≤ 16 entries × fixed width | Per buffer: index, dtype (closed enum), rank ≤ 4, per-dim extent bounds (min/max, u32; equal = static shape), access (`r` / `w` / `rw`), required placement (any / device-local / host-visible). Dispatch-time bindings must satisfy every row exactly. |
| `launch_limits` | fixed block | Max grid dims (3 × u32), max workgroup dims (3 × u32), max workgroups in flight. |
| `memory_ceilings` | fixed block | Device-local scratch bytes, shared bytes per workgroup, fragment/register pressure class (enum), total device-footprint ceiling. Admission refuses a device that cannot honour them; the runtime refuses a dispatch that would exceed them. |
| `execution_budget` | fixed block | Declared worst-case single-dispatch wall time (µs, u32) and max outstanding submissions. Inputs to `RCG-08`'s budget intersection and to §7's watchdog arming. |
| `cpu_fallback_variant` | u32 index | **Mandatory.** Must name a valid variant table row whose `target_class` is the CPU reference backend. Absent or dangling: reject — an artifact with no fail-safe resolution is inadmissible by construction. |
| `ir_blob` | offset + size, may be 0/0 | Present iff header flag bit 0: the validated tile IR for `ADR 0012` clause 4's bounded interpreter. Data, never mapped executable, own validator, own budget. |

### 4.3 Variant table (per entry, fixed width)

| Field | Meaning |
|---|---|
| `target_class` | Closed enum: `cpu-ref`, `sm_87`, … — one entry per admitted architecture; unknown value rejects the artifact, not just the variant. |
| `isa_level` | Target-class-specific (e.g. SM/compute-capability level); admission matches it against the device's capability record. |
| `exec_off`, `exec_size` | The device binary within the payload region. |
| `exec_hash` | 32 bytes over exactly `exec_off..exec_off+exec_size`; recomputed at admission; the hash executable state is named by at `RCG-11`. |
| `entry_index` | Which entry point within the binary (v1: single-entry binaries, must be 0). |
| `required_caps` | Bitmask matched against the device capability record (e.g. dtype units, shared-memory size class). |
| `actuals` | This variant's actual scratch/shared usage and geometry constraints — each must be ≤ the manifest ceilings or the artifact is internally inconsistent: reject. |
| `dtype_spec` | Which schema dtypes this variant specializes (a variant is picked, never adapted). |

### 4.4 Signature block

Purpose-bound signature (`RCG-05`: device-kernel signing is its own purpose; a code-signing key for
PE64 does not sign TKAs and vice versa) over bytes `0..signature_off` — i.e. header, manifest,
variant table, and payload as laid on disk. Signer identity, algorithm id, and signature bytes in
fixed-width fields; trust chains to the C0-rooted signer set; a name certificate or an
authenticated transport alone is insufficient, exactly as the gate table already states.

### 4.5 Transport: the `TOS64-TKA/1` envelope

Wire envelopes carrying TKAs or their manifests follow the repository's `TOS64-*` convention
(single `key=value` header line, versioned suffix — the `TOS64-RESULT/1` family's shape). A TKA
transfer announces itself as:

```
TOS64-TKA/1 artifact=<artifact_id hex> size=<total_size> variants=<n> targets=<class list> signer=<id hex> rollback=<counter>
```

followed by the binary container on the transport's binary path. The envelope is a courtesy for
routing, logging, and early refusal (a node can refuse an over-size or wrong-target artifact before
buffering it); it carries **zero trust**. Every field is re-derived from the container at
admission, disagreement between envelope and container is itself a rejection, and every arriving
TKA enters at `RCG-01` as hostile bytes regardless of source — including one fetched from an
off-board variant-building service, which is remote code admission and passes every gate, every
time, or does not run (`ADR 0012`'s cache-drift consequence, restated on the wire path where the
drift would happen).

## 5. The admission path, gate by gate

The chain is [`code-admission-gates.tsv`](../goals/security/code-admission-gates.tsv), unmodified —
`ADR 0012` clause 1 extends its reach to device code with no carve-out. What follows is what each
gate concretely means for a TKA; none of it weakens or reorders the table.

| Gate | For a TKA, concretely |
|---|---|
| `RCG-01` — data-only ingress | A TKA arrives (HBP/WCI/file/deploy) as a bounded non-executable object. The `TOS64-TKA/1` envelope's declared size caps buffering; no ingress endpoint can map or activate anything. |
| `RCG-02` — quarantine & origin | Content hash, channel, peer identity, time, detected type (`TOS64TKA` magic), declared type recorded before any interpretation. |
| `RCG-03` — disposable hostile parsing | The §4 container rules are enforced by a parser in a zero-authority bounded C4 domain. Malformed offset, undefined flag, over-bound table, slack bytes: the parser domain is killed and discarded, nothing promoted. This parser is `FEAT-P6B-02`'s named highest-value security surface and carries the full hostile-artifact test obligation. |
| `RCG-04` — canonical object & closure | Recompute `artifact_id` and every `exec_hash`; verify the single-region-ownership property (no polyglot reading); enumerate the dependency closure — which for a TKA is deliberately trivial: variants import nothing, link nothing, and name no external symbol. A variant that wants a dependency is not a valid v1 TKA. |
| `RCG-05` — signature & trust path | §4.4's purpose-bound signer, rooted in C0, over the exact bytes. |
| `RCG-06` — revocation & anti-rollback | Signer revocation state and `rollback_counter` monotonicity per kernel/signer; a re-signed downgrade of a withdrawn kernel is rejected replay. |
| `RCG-07` — signed manifest, minimal surface | The §4.2 manifest **is** the signed manifest: buffer schema, memory ceilings, launch limits, device request (target classes present). Undeclared anything — a variant for a target class the manifest doesn't list, an IR blob the flag doesn't declare — rejects with zero partial registration. |
| `RCG-08` — policy intersection & budget | The caller's requested queue depth, footprint, and submission rate intersect the manifest's declared budgets, current policy, and available reserves. Admission that would endanger existing RT reservations is refused here — this is where the compute admission model (`inference-architecture.md`) is enforced, not advised. |
| `RCG-09` — destroy inspect domain, recreate | The C4 parse instance is torn down; what survives is a fresh admission record and, for the CPU-fallback/interpreter paths, a fresh C3 domain with empty authority. No relabel-in-place. |
| `RCG-10` — exact fresh mapping | Admitted variant bytes map immutable and executable *for their target processor* only: CPU-fallback code as RX in its C3 domain; device-destined bytes staged in memory that is never simultaneously CPU-writable and device-executable. On Stage 1 the device-visible mapping is performed by the broker host's kernel — TinyOS runs gates 01–09 and releases the artifact to the broker only post-admission, and the evidence says which kernel did the mapping. |
| `RCG-11` — executable seal | No W-to-X transition, no writable-executable alias **in any address space, CPU page tables or device/IOMMU mappings alike**; executable state names `exec_hash`. On-target compilation in any form remains shape B, denied. |
| `RCG-12` — attributable activation | First dispatch authorization for an artifact records kernel-derived identity, class, provenance, generation, budgets, and exact capability set. `tt_artifact_load` accepts only objects that have reached this state (§3.2) — the ABI is structurally incapable of accepting bytes. |
| `RCG-13` — runtime blast radius | Even a fully attacker-controlled admitted kernel is contained: budgets bound its device time, quotas bound its queue slots, `PD-10` bounds its DMA reach (§6, `ADR 0013`), fences time out, and the telemetry §7 relies on is kernel-side, not kernel-supplied. |
| `RCG-14` — termination & non-persistence | Queue reset / domain teardown revokes submissions, wipes staging, advances generations before any handle value is reused. An artifact is re-admitted, never resurrected. |

**Fail-safe resolution of a missing variant** (`ADR 0012` clause 5), in the order the runtime
attempts it: the manifest's mandatory CPU-fallback variant → the bounded interpreter, if the
artifact ships IR and policy permits → a declared degraded configuration → a clean, attributable
refusal through the ACI. Each step is a distinct telemetry event and a distinct error code; none is
silent, and no step compiles, fetches, or retries unboundedly.

## 6. The device-backend seam

One seam, three implementations, selected by deployment policy — never silently, never per-call.
The seam is a Rust trait boundary (described here, defined with `FEAT-P6B-04`'s tests) with these
obligations:

- **Capability report** — device class, ISA level, memory sizes, queue depths, metering support,
  and the buffer-sharing path with its `ADR 0013` qualification-record reference or the honest
  `not qualified` default.
- **Buffer import/export** — exclusively via UMM handles; the backend never sees a raw caller
  pointer. Whether satisfying an import means establishing a qualified zero-copy mapping or
  operating the copy/wiped-bounce path is the backend's per-platform truth, reported in telemetry.
- **Artifact activation** — accepts post-`RCG-12` variants only, matched to its capability report.
- **Submit / poll / cancel / reset** — non-blocking submit; bounded everything; reset is mandatory
  and total (`RCG-14` semantics at queue scope).
- **Telemetry** — the §3.2 axes, produced on the TinyOS side of the seam wherever the platform
  allows, so a compromised device or broker can lie about its work but not about what TinyOS
  observed of it.

The three backends:

1. **CPU reference backend** (`FEAT-P6B-01`, always present). Executes each artifact's mandatory
   CPU-fallback variant in a budgeted C3 domain, `no_std`, no allocator. It is the reason every
   TKA is executable somewhere, the fail-safe floor of clause 5, and the conformance oracle the
   other backends are tested against (same-input/same-output within declared numeric tolerance).
2. **HBP compute broker** (`FEAT-P6B-05`, Stage 1). The accelerator lives on a Linux host behind a
   C2 broker speaking HBP with `TOS64-*` envelopes on the established compute-lane pattern.
   TinyOS-side admission runs in full before any artifact leaves the device; the broker host's
   kernel contains the device's DMA, so this stage **sidesteps** `PD-10`/`ADR 0013` rather than
   satisfying it, and every piece of Stage 1 evidence is labelled `stage=brokered` for exactly the
   reason `01C` trap 3 names: broker numbers include a Linux kernel, its scheduler, and its driver
   stack, and they prove the ABI/artifact/admission path — not TinyOS-native performance.
3. **Native Orin backend** (`FEAT-P6B-06`, Stage 2, gated). A deliberately narrow
   submission-queue subset informed by the published `nvgpu`/`host1x`/`nvmap` sources, in `-sys`
   crates with safe logic on top. Gated on hardware evidence (this project has never initialised
   this device) and on an `ADR 0013` qualification record for any zero-copy configuration —
   today's qualified-platform count is zero, so Stage 2 plans copy-path evidence first. If the
   investigation concludes the submission path is intractable, that finding closes the Feature
   honestly and TinyTile remains real on backends 1 + 2.

## 7. The RT non-interference argument, stated to be attacked

The obligation (fixed by `inference-architecture.md`, restated by the Epic's out-of-scope list):
TinyTile owes the RT core **non-interference, proven adversarially — never a latency promise of
its own**. An argument that cannot fail is an assertion, so this section is written as numbered,
falsifiable claims. Each names its interference channel, the mechanism that is supposed to close
it, and **the experiment that would refute it** — which becomes a `BND-*`/`TEST-*` obligation at
`FEAT-P6B-04`'s decomposition, on the `G-AI-9` RT-interference axis. Two channels (NI-5, NI-6) are
*not* closed by construction and are claimed only as measurable, boundable, and honestly reported.

**NI-1 — No RT context ever executes TinyTile code.** Mechanism: the ABI is callable only from
budgeted non-RT task contexts (§3.1 rule 5); no TinyTile symbol is reachable from the RT
scheduler, an interrupt handler, or any RT task's call graph, enforced structurally (crate
dependency direction + policy refusal at the ACI seam), not by convention. *Refutation:* static
reachability analysis finding any path from an RT entry point into a TinyTile crate; or a runtime
probe demonstrating an ABI call admitted from an RT-classified context.

**NI-2 — The RT kernel never blocks on a compute-device submission, fence, or driver call.**
Mechanism: submits are non-blocking; the only wait in the surface is `tt_wait_bounded` with a
mandatory timeout, illegal in RT contexts by NI-1; fence state lives in memory the RT side never
takes a lock on. *Refutation:* under a dispatch storm with a deliberately wedged backend (a hostile
variant that never completes, or a broker that stops answering), any RT task deadline miss
attributable to a TinyTile lock, fence, or queue — the wedged-device case must resolve through
watchdog → cancel → `tt_queue_reset` while the RT deadline harness records zero misses.

**NI-3 — Admission refuses; queues never grow past their grant.** Mechanism: `RCG-08` budget
intersection; fixed-capacity pools charged to the caller's quota (§2); over-budget dispatch fails
with a distinct code rather than queueing. *Refutation:* a caller demonstrating unbounded queue
growth, quota escape (charging another domain's pool), or an admission that shrinks existing RT
reservations.

**NI-4 — Device interrupts do work bounded in RT terms.** Mechanism: completion interrupts (native
backend) or lane events (broker) run bounded handlers that set state and wake non-RT waiters; all
processing beyond acknowledgment happens in budgeted task context. *Refutation:* interrupt-storm
injection (hostile broker flooding the lane; device raising completion storms) producing measured
RT jitter above the platform's stated interrupt-handling bound.

**NI-5 — Memory-bandwidth and cache interference is measured and bounded per platform, not
presumed absent.** Stated honestly: on unified-memory hardware a compute device competes for DRAM
bandwidth and shared cache with the CPU running RT tasks, and **no admission policy closes this
channel** — TinyOS does not schedule the device's memory traffic. Mechanism offered: per-platform
adversarial measurement (a bandwidth-hammer TKA saturating device memory traffic while the RT
deadline/jitter harness runs), throttling policy driven by that measurement, and refusal to state
any RT guarantee on a platform whose measured interference under hammer exceeds the RT margin.
*Refutation:* an RT deadline miss under the hammer on a platform whose TinyTile configuration was
claimed compatible with its RT tier — one miss voids the claim for that platform + configuration,
`ADR 0005`-style. Until a platform holds such a measurement, the honest default is that running
TinyTile concurrently with hard-RT work is **unqualified**, exactly parallel to `ADR 0013`'s
zero-copy default.

**NI-6 — DMA cannot reach RT memory.** Mechanism: `PD-10` (IOMMU enforcement or wiped bounce
buffers) as ruled by `ADR 0013`; on the copy path the device only ever sees dedicated staging;
zero-copy exists only under a current qualification record with a positive control. *Refutation:*
the qualification record's own instrument — a deliberately out-of-bounds device access that must
fault; on an unqualified platform, any configuration found handing a device a mapping into
non-staging memory.

**NI-7 — Thermal and power coupling is reported, never absorbed.** A saturated accelerator can
throttle the SoC's CPU clocks and erode RT margins from below. Mechanism: telemetry carries
temperature and (where metered) power per §3.2; platform policy may cap sustained device budgets;
no RT claim is quoted for a thermal envelope that was not measured with the device loaded.
*Refutation:* sustained-load soak demonstrating CPU frequency capping or deadline erosion within a
claimed-compatible envelope.

The composite claim — "TinyTile does not interfere with the RT core" — is therefore **never quoted
unqualified**: it decomposes into NI-1..NI-4 (closed by construction, each with a standing
adversarial test) plus NI-5..NI-7 (per-platform measured bounds with named instruments and void
conditions). A Report that asserts non-interference names which claims it rests on and, for the
measured ones, the platform, configuration, and dated evidence — the same discipline `ADR 0005`
and `ADR 0013` already impose on this repository's other conditional capabilities.

## 8. What this document does not claim

- **No driver exists and none is assumed.** The native submission path is a Stage 2 finding to be
  earned from the `nvgpu` sources and hardware evidence, or honestly closed (`EPIC-P6B`'s first
  kill criterion). Nothing in §3–§6 depends on it: every surface is exercisable against the CPU
  reference backend and the Stage 1 broker.
- **No performance number.** No figure exists; when figures exist they carry stage (brokered vs
  native), path (copy vs zero-copy), platform, and qualification per `ADR 0013` clause 4 and
  `ADR 0005` — mechanism evidence first, and `LE-09`'s no-emulator rule stands.
- **sm_87 emission is still a named unknown.** TileLang's published validation covers no Tegra
  device; `01C` Step 3.2's off-target probe retires it when the owner opts in.
- **The byte-level widths in §4 are the v1 proposal**, pinned (or amended, with reasons) by
  `FEAT-P6B-02`'s failing-tests-first decomposition — which, like everything else here, waits for
  the implementation queue to open behind the Pi 5 hardware-evidence sprint.
