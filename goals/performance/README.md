# TinyOS Performance Catalogue — 625 Guardrail Tests

Status: **Specified, not verified** — 625 test contracts; thresholds are provisional engineering budgets until measured on the named hardware tiers.

This catalogue turns TinyOS's performance, frugality, compactness, isolation, and fail-safe ambitions into individually addressable tests. It is deliberately compact: **25 performance domains × 25 guardrails = 625 tests**, expanded in [`catalogue.tsv`](catalogue.tsv). Every cell has a stable ID of the form `PERF-Dnn-Gnn`.

The catalogue is part of the mandatory [`../assurance/`](../assurance/) spine. Every Story selects one or more domains in [`../assurance/story-contracts.tsv`](../assurance/story-contracts.tsv); selecting a domain brings all 25 guardrails into that Story's assurance contract. Adding an unmapped Story fails CI.

The same 625 cells also steer product destinations through [`../context/application-platforms.tsv`](../context/application-platforms.tsv). Each concrete runtime, framework, game, browser, remote-UX, compatibility, inference or fleet target selects the domains that must hold for that workload; [`../context/landing-zones.tsv`](../context/landing-zones.tsv) keeps those selections beside its goals and security/class contracts. This creates future evidence obligations without pretending unbuilt applications pass.

The catalogue does not claim that unbuilt subsystems already pass. The `readiness` field distinguishes current Phase-0 prototypes, partial/stand-in surfaces, specified work, and design-only future work. A test becomes Verified only after raw evidence and a report exist; merely appearing here means only that the contract is specified. A Story's functional `Verified` status does not satisfy these contracts.

See [`current-state-review.md`](current-state-review.md) for the repository audit that shaped these domains and priorities.

## Non-negotiable interpretation

Performance never outranks safety, security, or correctness. A fast result that bypasses capability policy, weakens W^X, drops a safety message, hides an overrun, or changes the documented fail-safe state is a failure regardless of its timing.

Guardrails G01–G23 are release gates once the corresponding subsystem exists. G24 and G25 are **claim gates**, not excuses to compromise the design:

- “better than Linux” requires the same hardware, workload contract, build mode, power state, safety checks, and raw data;
- “10× better than most RTOSes” is not a single scientific metric. It may be stated only for a named metric after a same-hardware comparison with at least three current RTOS baselines. If the ratio is below 10×, TinyOS can still ship if its absolute release gates pass, but that marketing claim is blocked.

## Measurement protocol

1. Use the pinned release toolchain, LTO, stripped artifacts, a recorded commit, hardware/firmware revision, profile, driver set, clock policy, and thermal state.
2. Pin measurement and interference tasks to declared cores. Report whether SMT, turbo/boost, C-states, IOMMU, caches, and mitigations are enabled; never silently tune only TinyOS.
3. Subtract calibrated timer/PMU read overhead. Use invariant TSC on x86_64 and the architectural counter on ARM64. Wall-clock and cycle results are both required.
4. For p99.9 and observed maxima, collect at least 1,000,000 operations and retain raw histograms. Use 30 independent runs for run-to-run variation.
5. An observed maximum is not a proof of WCET. RT release requires a documented upper-bound argument plus at least 20% margin over the worst valid measurement matrix.
6. Run cold, warm, capacity-edge, malformed-input, denial, exhaustion, fault-injection, and 90% competing-load cases separately. Never average them together.
7. Shared public CI checks catalogue integrity and benchmark shape; threshold enforcement belongs on controlled QEMU/HIL runners. Noisy shared-host timings are diagnostic only.
8. A threshold can tighten with evidence. Loosening a release threshold requires an ADR, a V&V report, and confirmation that no safety/security invariant changed.
9. Driver and profile results include negative footprint evidence: an opt-out subsystem must contribute no executable bytes, registered interrupts, DMA grants, capabilities, queues, or reachable parser surface.
10. Reports retain raw samples, percentile method, outliers, failures, compiler/link map, energy/thermal data, and comparison harness version.
11. Application/runtime results are end to end. Framework bridges, GC, JITless or admitted-code paths, broker IPC, TCP/TLS, GPU copies, provenance checks, denials and teardown remain inside the measured boundary rather than being subtracted as “platform overhead.”
12. AI results report TTFT, prefill, decode, quality, energy, thermal state and RT interference together. Game/browser results report frame pacing and input-to-photon latency, not only synthetic rendering throughput.

## Domains

Latency budgets are shown as p50/p99/p99.9/observed-max candidate values. Feature image budgets are incremental unless D25 says total image. D22's zero means an unselected driver must add zero bytes; its enabled-driver ceiling is defined by G09.

| ID | Domain | Phase | Tier | Readiness | Latency budget | Image | Working memory | Throughput |
|---|---|---|---|---|---:|---:|---:|---:|
| D01 | Boot and topology discovery | P0 | T0+T1+T2 | prototype | 500/1000/1500/2000 us | 96 KiB | 128 KiB | 500/s |
| D02 | Interrupt entry and exit | P1 | T1+T2 | unbuilt | 0.25/0.5/0.75/1 us | 16 KiB | 8 KiB | 1000000/s |
| D03 | Timer tick and deadline accounting | P1 | T0+T1+T2 | partial | 0.15/0.3/0.5/0.75 us | 16 KiB | 8 KiB | 2000000/s |
| D04 | Context switch | P0 | T0+T1+T2 | prototype | 0.3/0.6/0.9/1.2 us | 12 KiB | 8 KiB | 1000000/s |
| D05 | Ready queue and dispatch | P1 | T0+T1+T2 | prototype-cooperative | 0.2/0.4/0.6/0.8 us | 24 KiB | 16 KiB | 1500000/s |
| D06 | Priority inheritance lock | P1 | Host+T0+HIL | prototype | 0.2/0.5/0.8/1 us | 12 KiB | 8 KiB | 1000000/s |
| D07 | Static pool allocation | P0 | Host+T0+HIL | prototype | 0.05/0.1/0.15/0.25 us | 16 KiB | 8 KiB | 5000000/s |
| D08 | Paging and address spaces | P0 | Host+T0+T1+T2 | prototype-inactive | 0.3/0.7/1/2 us | 48 KiB | 64 KiB | 250000/s |
| D09 | PE64 loading and import validation | P0 | Host+T0 | prototype | 50/100/200/500 us | 128 KiB | 256 KiB | 2000/s |
| D10 | ACI capability decision | P5 | Host+T0+HIL | stand-in-only | 0.3/0.8/1.2/2 us | 128 KiB | 64 KiB | 500000/s |
| D11 | Spoor stamp and journal | P0 | Host+T0+HIL | prototype | 0.03/0.06/0.1/0.15 us | 32 KiB | 64 KiB | 10000000/s |
| D12 | Local IPC message channel | P0 | Host+T0+HIL | specified | 0.2/0.5/0.8/1.5 us | 64 KiB | 128 KiB | 1000000/s |
| D13 | Shared-memory grant | P0 | Host+T0+HIL | specified | 0.5/1/2/4 us | 64 KiB | 128 KiB | 250000/s |
| D14 | Storage and file access | P3 | T0+T1+T2 | stand-in-only | 0.4/1/2/5 us | 256 KiB | 512 KiB | 200000/s |
| D15 | Model mmap and page access | P6B | T1+T2 | design | 0.1/0.3/0.5/1 us | 256 KiB | 512 KiB | 2000000/s |
| D16 | Local LLM token delivery | P6 | T1+T2 | design | 2000/5000/10000/20000 us | 512 KiB | 1024 KiB | 500/s |
| D17 | GPU UMM and admission | P6B | T1+T2 | design | 0.8/2/5/10 us | 512 KiB | 1024 KiB | 100000/s |
| D18 | HBP shared-memory transport | P4 | T0+T2 | design | 0.5/2/5/10 us | 128 KiB | 256 KiB | 500000/s |
| D19 | WCI secure command transport | P4 | T0+T1 | design | 50/100/250/500 us | 512 KiB | 1024 KiB | 20000/s |
| D20 | TCP IP data path | P3 | T0+T1+T2 | design | 5/15/30/50 us | 512 KiB | 1024 KiB | 100000/s |
| D21 | CAN USB and field I O | P3 | T0+HIL | design | 10/50/100/250 us | 512 KiB | 1024 KiB | 50000/s |
| D22 | Opt-in driver lifecycle | P7 | T0+HIL | design | 100/500/1000/2000 us | 0 KiB | 256 KiB | 1000/s |
| D23 | Shell config and audit query | P2 | Host+T0+T1+T2 | design | 20/50/100/250 us | 512 KiB | 1024 KiB | 10000/s |
| D24 | Watchdog fault and deploy recovery | P1.5 | T0+HIL | partial | 50/100/250/500 us | 256 KiB | 512 KiB | 1000/s |
| D25 | Combined system and footprint | cross | T1+T2 | design | 50/100/250/500 us | 8192 KiB | 4096 KiB | 20000/s |

## Guardrails

Each guardrail is applied to every domain's work unit. Domain-specific numeric targets are materialized in `catalogue.tsv`.

| ID | Guardrail | Metric | Gate | Method | Cadence |
|---|---|---|---|---|---|
| G01 | median latency | latency_us_p50 | release | Measure end-to-end work-unit latency after warm-up with a pinned monotonic clock | PR-shape and HIL-threshold |
| G02 | p99 latency | latency_us_p99 | release | Measure at least 100000 operations and retain the raw latency histogram | HIL per merge |
| G03 | p99.9 tail latency | latency_us_p99_9 | release | Measure at least 1000000 operations with IRQ and cache state recorded | HIL per merge |
| G04 | observed maximum and WCET bound | latency_us_max | release | Run adversarial input and load matrices; observed maxima do not substitute for a proven WCET bound | HIL per RT change |
| G05 | jitter envelope | latency_jitter | release | Compare 30 independent runs with fixed power and affinity controls | HIL per merge |
| G06 | median CPU cycles | cpu_cycles_p50 | release | Use invariant TSC or architectural counter with measurement overhead subtracted | HIL per merge |
| G07 | p99 CPU cycles | cpu_cycles_p99 | release | Collect PMU cycles on the same binary and hardware used for latency results | HIL per merge |
| G08 | microarchitectural efficiency | instructions_branch_cache | release | Collect instructions branches branch-misses and cache misses with perf-equivalent PMU counters | scheduled HIL |
| G09 | image and feature footprint | image_bytes | release | Compare stripped release map files and section sizes against the parent commit | every PR |
| G10 | peak working memory | working_memory_kib | release | Account static pools stacks page tables queues and transient buffers at peak occupancy | every PR plus HIL |
| G11 | steady-state allocation count | allocations_per_operation | release | Instrument every allocator and assert a zero delta across the measured operation | every PR |
| G12 | allocation and reclamation latency | allocation_latency_us_max | release | Measure best middle last free slot exhaustion and recovery paths separately | every allocator change |
| G13 | queue residence p99 | queue_wait_us_p99 | release | Timestamp enqueue and service start without allocating or perturbing queue priority | HIL per merge |
| G14 | queue processing maximum | queue_service_us_max | release | Exercise smallest largest malformed denied and cancellation work items | HIL per change |
| G15 | sustained throughput floor | throughput_per_second | release | Hold offered load at 80% saturation for 15 minutes and report useful completed work | scheduled HIL |
| G16 | burst and backpressure safety | burst_recovery | release | Overdrive producers while verifying explicit drop reject or throttle policy and recovery | HIL per queue change |
| G17 | cold start latency | cold_start_ms | release | Run 30 cold trials and record firmware media authentication and model costs separately | scheduled HIL |
| G18 | warm restart and reuse latency | warm_start_us | release | Measure re-entry after clean teardown with validated reusable state | HIL per lifecycle change |
| G19 | isolation under competing load | loaded_degradation | release | Run at 90% admitted inference network driver and memory load on other partitions or cores | HIL per merge |
| G20 | security denial cost and safety | denial_latency_and_state | release | Benchmark authentication replay capability bounds and parser rejection adversarial cases | every security change |
| G21 | exhaustion and fault containment | fault_completion | release | Exhaust each finite resource and inject task driver transport and data faults | every relevant PR |
| G22 | 72-hour soak stability | soak_drift | release | Run representative load continuously with raw histograms and spoor sequence retained | continuous HIL |
| G23 | spoor observability overhead | observability_overhead | release | A B identical binaries with stamping disabled and enabled; include journal wrap pressure | every instrumentation change |
| G24 | same-hardware Linux advantage | linux_comparison_ratio | claim | Run the same source-level workload contract on identical hardware clocks compiler options and safety checks | release claim audit |
| G25 | same-hardware ten-times RTOS claim | rtos_comparison_ratio | claim | Publish raw data versions configs and confidence intervals; otherwise the 10x claim is blocked | release claim audit |

## Evidence and status

The TSV is the canonical machine-readable expansion. Its `status` is initially `specified` for all 625 rows. Results belong in dated reports and must include:

- test ID, commit, toolchain, target/profile, hardware and firmware;
- driver and capability manifest, core affinity and competing load;
- sample count, warm-up, raw histogram or raw counter series;
- observed value, threshold, pass/fail, and safety invariant result;
- baseline OS/RTOS identity and configuration for G24/G25 only.

A performance failure never gets hidden by a functional pass. Conversely, an unavailable future subsystem is **not failed and not passed**; it remains specified until its implementing Story is ready.

Which cells actually carry dated evidence is recorded in [`../assurance/guardrail-evidence.tsv`](../assurance/guardrail-evidence.tsv) and machine-checked by `check-assurance-spine`. It is a record of evidence, never a score: a cell absent from it is *unevidenced*, which is what it is, and never *passed*.

**Not every guardrail waits on hardware, and the `Cadence` column above says which.** `G09` and `G11` run *every PR*; most of the rest are HIL. The first cells recorded were `G11` across ten domains ([`REPORT-2026-07-28-08`](../reports/REPORT-2026-07-28-08.md)), and they are not measurements: `G11` asks for zero heap allocations per steady-state work unit, and this system has **no heap at all** — every shipped crate is `#![no_std]` with no `#[global_allocator]`, which Rust enforces at compile time. That is stronger than the guardrail's own wording asks for, and independent of which CPU runs it. Ten domains, not the seventeen currently selected by Stories: **a guardrail cannot be closed for a subsystem that does not exist**, so anything whose `readiness` is `design`, `specified`, `stand-in-only` or `unbuilt` was excluded by name.

## Integrity and spine gates

From `os/`:

```text
cargo run -p xtask -- check-performance-catalogue
cargo run -p xtask -- check-assurance-spine
```

The first command rejects missing/duplicate IDs, malformed rows, unknown axes, empty evidence fields, and anything other than the complete D01..D25 × G01..G25 cross-product. The second also enumerates all Story/Feature files and verifies their performance-domain and security-control mappings.
