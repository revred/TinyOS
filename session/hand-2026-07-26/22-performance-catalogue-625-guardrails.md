# Handover 22 — 625-test performance, frugality, compactness, and safety catalogue

Follows: [`21-story-p0-06-02-spoor-journal-implementation.md`](21-story-p0-06-02-spoor-journal-implementation.md).

## Direction

The user made performance, frugality, compactness, real-time latency, local-LLM token delivery, secure execution, and opt-in drivers the OS's key USP, and requested 625 performance-related tests covering memory, spoors, CPU cycles, access time, queue time, allocations, and safety under load. The desired competitive direction is better than a full Linux system and, where evidence supports a named metric, 10× better than other RTOS systems.

## What this work delivered

1. **A 625-row machine-readable catalogue** at [`goals/performance/catalogue.tsv`](../../goals/performance/catalogue.tsv), defined as the complete cross-product of:
   - 25 OS domains: boot, IRQs, timers/WCET, switching, dispatch, locks, pools, paging, PE loading, ACI, spoor, IPC, shared memory, storage, model mmap, token delivery, GPU/UMM, HBP, WCI, TCP/IP, field I/O, opt-in drivers, shell/config/audit, recovery, and combined-system performance;
   - 25 guardrails: latency percentiles/max/jitter, cycles and PMU efficiency, image/working memory, allocation count/cost, queue residence/service, throughput/backpressure, cold/warm start, load isolation, denial cost, fault containment, soak drift, spoor overhead, Linux comparison, and the 10× RTOS claim gate.
2. **The human runbook and threshold model** at [`goals/performance/README.md`](../../goals/performance/README.md), including measurement controls, sample sizes, HIL requirements, raw-evidence requirements, and the rule that safety/security/correctness failures override any fast number.
3. **A current codebase audit** at [`goals/performance/current-state-review.md`](../../goals/performance/current-state-review.md). It records the Phase-0 baseline and major gaps before performance claims are credible: cooperative rather than preemptive dispatch, linear hot-path scans, incomplete context state, no timer/watchdog, broad boot RWX mapping, approximate static-memory accounting, inactive process page tables, and unbuilt driver/ACI/network/storage/inference paths.
4. **Claim honesty is structural.** G01–G23 are eventual release gates. G24/G25 are claim gates:
   - Linux comparisons require identical hardware, clocks, workload contract, compiler/build mode, and safety behavior.
   - “10× RTOS” requires a named metric, at least three current same-hardware RTOS baselines, equivalent safety, raw data, and a measured ratio of at least 10.0×. If evidence falls short, the absolute TinyOS gates may still pass, but the comparative claim is blocked.
5. **An executable integrity gate** in `xtask`: `cargo run -p xtask -- check-performance-catalogue`. It rejects missing cells, duplicates, malformed IDs/rows, empty required fields, bad gates/status, and anything other than exactly D01..D25 × G01..G25.
6. **Five validator unit tests** cover a valid full cross-product, a missing cell, duplicate ID, empty field, and the committed catalogue itself.
7. **CI integration** in `.github/workflows/ci.yml`, under the existing governance job.
8. **Goals dashboard/traceability links** were added without inflating Story/Test verification counts. All 625 rows begin `specified`; none is called Passed merely because its contract now exists.

## Baseline and verification

- Catalogue structure: **625 rows, 625 unique IDs, 25 domains, 25 guardrails**.
- Gate split: **575 release contracts, 50 comparative claim contracts**.
- `cargo run -p xtask -- check-performance-catalogue`: pass.
- `cargo test -p xtask`: 5/5 pass.
- `cargo test --workspace --lib`: 110/110 pass.
- `cargo fmt --all -- --check`: pass.
- `cargo clippy --workspace --lib -- -D warnings`: pass.
- `cargo clippy -p xtask --all-targets -- -D warnings`: pass.
- Crate-size gate: pass; `xtask` is 659 production LOC, all crates well below 20,000.
- Normal boot plus context-switch, address-space, and Win32-shim QEMU fixtures: pass.
- Broken-boot fixture: expected distinguishable exit code 1.
- Current release kernel ELF file size: 16,032 bytes on disk. This is explicitly not treated as a finished-OS footprint claim because most roadmap subsystems do not exist and NOBITS runtime storage is not represented by the file length.

The broader Windows-host command `cargo clippy --workspace --all-targets` still fails on the existing `kernel`/`exec` no-std fixture binaries because `hal_x86_64::boot` and `qemu_exit` are intentionally gated out on Windows while those host bin targets import them. This predates the catalogue work; the library and `xtask` lint surfaces are clean, and Linux CI remains the authoritative all-target host.

## Immediate next steps

1. Land a calibrated monotonic-clock/PMU measurement ABI and raw result record format before implementing timing thresholds.
2. Decompose Phase 1 around real timer/IDT preemption, WCET-to-watchdog handoff, and O(1) ready selection.
3. Add exact linked-image, per-feature delta, stack, pool, page-table, and runtime-static-memory accounting.
4. Make local IPC and production spoor emission the low-overhead measurement substrate.
5. Add controlled Tier 1/2 runners; use public CI for benchmark shape and catalogue integrity, not noisy timing thresholds.
6. Build Linux/RTOS comparison harnesses only after absolute TinyOS release gates are stable.
