# TinyOS Performance and Frugality Review — 26 July 2026

Status: **Current-state audit supporting the 625-test performance catalogue**

## Executive finding

TinyOS is compact because it is still a Phase-0 walking skeleton, not because the finished OS has yet proved a compact or low-latency profile. The repository has good foundations for bounded work—`no_std`, fixed-capacity structures, typed failures, no general kernel heap, W^X checks in the PE parser, and a small dependency graph—but it has no timing harness, hardware-counter evidence, real preemption, driver boundary, ACI engine, networking stack, storage path, or inference runtime yet.

Accordingly, all 625 entries in [`catalogue.tsv`](catalogue.tsv) start as **Specified**, not Passed. The catalogue is the guardrail for future implementation; it is not evidence for a present performance claim.

## Measured repository baseline

Measured locally against the pinned `nightly-2026-07-26` toolchain:

- `cargo test --workspace --lib`: **140 host tests passed** across `exec` (47), `hal` (4), `hal-x86_64` (30), and `kernel` (59).
- `cargo test -p xtask`: **15 host tests passed**, including the 625-cell catalogue and mandatory assurance-spine integrity tests.
- Production LOC reported by the existing crate-size gate: `exec` 1,847; `hal` 91; `hal-x86_64` 788; `kernel` 1,796; `xtask` 1,227.
- The release x86_64 bare-metal kernel ELF is **16,032 bytes on disk**. This is not a finished-OS size: it contains the boot/topology path, while most roadmap subsystems do not exist. Its NOBITS runtime memory also includes a 1 MiB boot stack and page-table storage that the disk size does not reflect.
- Existing tests are functional and adversarial. There are currently **zero cycle-count, percentile-latency, queue-residence, allocation-instrumentation, energy, thermal, or comparative baseline reports**.

## Findings that shape the catalogue

### 1. Scheduling is cooperative, not preemptive

`kernel::dispatch::run_once` explicitly depends on a task yielding through `context::switch`. There is no timer interrupt, IDT, APIC timer, forced preemption, periodic WCET attribution, or production watchdog. The current scheduler tests validate bookkeeping and cooperative selection; they do not establish hard-real-time latency.

Catalogue focus: D02–D06, especially G03–G05, G13–G16, G19, G21, and G22.

### 2. Core selection and allocation paths are bounded but not constant-time

`Pool::alloc` linearly scans for the first free slot. `Scheduler::highest_priority_ready` scans every live task. `AddressSpace::validate_sections` is quadratic in section count. PE import RVA lookup repeatedly scans sections, Win32 buffer validation walks every covered page, and the BIOS RSDP fallback scans up to 128 KiB.

Those algorithms are finite, which is better than unbounded growth, but finite O(N) and O(N²) work is not the same as a tight O(1) RT bound. Capacity-edge latency tests must run the worst slot, task count, section count, import count, page span, and firmware scan position—not only small happy-path fixtures.

Catalogue focus: D01, D05, D07–D09, G04, G06–G08, G12, and G21.

### 3. Priority inheritance needs multi-lock and authority hardening

The lock API accepts a caller-supplied `task_priority` instead of deriving the contender's effective priority from the scheduler. The current single-lock bookkeeping restores one saved priority, but no system-wide mechanism tracks multiple locks or several outstanding inherited priorities. Releasing one lock must never deboost a task below the priority still required by another lock.

Catalogue focus: D06-G04, D06-G19, D06-G20, D06-G21, and the capacity/burst variants.

### 4. WCET detection is not yet a one-shot fail-safe transition

`record_tick` detects a crossing, but any later call while the counter remains over budget invokes the handler again. The counter saturates at `u32::MAX`; there is no period source, overrun latch, watchdog transition, task quarantine, or demonstrated safe-state path. This is a functional/safety gap before it is a performance optimization target.

Catalogue focus: D03 and D24 across all guardrails, with zero duplicate fault actions and bounded safe-state entry as required evidence.

### 5. Context state is intentionally incomplete

The switch saves the SysV callee-saved general registers and flags. It does not yet switch CR3/address spaces, FS/GS base, FPU/SIMD/XSAVE state, debug state, TLS, protection keys, or architecture-equivalent ARM64 state. Benchmarking today's narrow switch as if it were a production process switch would understate both cost and security obligations.

Catalogue focus: D04; results must name the state set included and cannot be used for production claims until the required set is complete.

### 6. Early boot deliberately uses a broad RWX identity map

The boot stub identity-maps the first 1 GiB with writable executable 2 MiB pages. The process page-table builder applies W^X at leaves, but constructed page tables are not loaded into CR3. There are no guard pages around the 1 MiB boot stack and no proof that boot-only mappings are removed before untrusted code can run.

Compactness cannot be allowed to turn into an overbroad executable mapping. Security closure is a precondition to accepting boot, paging, or executable-load performance results.

Catalogue focus: D01, D08, D09, G09–G11, G19–G21.

### 7. Static-memory accounting is only an approximation

`kernel::capacities::committed_bytes` counts element payload sizes for two capacities but intentionally omits container tags, padding, stacks, boot page tables, executable address-space pools, and future queues. The current 8 MiB assertion is therefore not a total image or total static-memory gate.

Catalogue focus: G09 and G10 in every domain, with D25 as the authoritative whole-system total.

### 8. Address-space backing now demand-zeros and bounds-checks (`STORY-P0-05-04`)

Resolved 2026-07-26, surfaced by the real `blue-sharc.exe` binary itself (its `.data` section has a genuine 352-byte `.bss` tail, and its default 512-byte `FileAlignment` broke the original zero-copy mapping's page-alignment assumption entirely). `AddressSpace::create` now copies each section's bytes into caller-supplied, page-aligned `staging` storage — zero-filling every mapped page first, then copying whatever falls within `[0, file_size)` — so a `.bss` tail is simply the case where nothing further is left to copy, and `file_offset + file_size` is explicitly re-validated against the supplied byte slice (`AddressSpaceError::SectionDataOutOfBounds`) rather than trusted from a caller that may have bypassed the PE parser.

Catalogue focus: D08, D09, G20, and G21; no timing result passes if the mapping reads beyond validated backing.

### 9. The Win32 shim is a policy stand-in, not the ACI

`AllowAllPolicy` and two no-op-shaped file calls are sufficient for the current Story fixture, but they are neither a real capability registry nor real console/storage I/O. A zero-length buffer at any address is accepted, and page-walk validation cost grows with buffer span. Future measurements must use the actual ACI and actual I/O path without an ambient allow-all default.

Catalogue focus: D10, D14, G13–G16, and G20.

### 10. Spoor has the right record size but no proven RT publication path

A spoor is exactly eight bytes and the in-memory journal is fixed-capacity. `kernel::lock`'s boost/restore events (`STORY-P0-06-03`) and `kernel::wcet`'s overrun/reset events (`STORY-P0-06-04`) now both stamp through this path, proving the API end to end via host tests across two subsystems — but no *production* path emits it yet, since no dispatcher/timer loop is wired into `main.rs` to drive either of them at boot. A capacity-zero journal would divide by zero on append/iteration, and overwrite-oldest behavior needs a safety policy for events that must outlive ring wrap.

Catalogue focus: D11 and G23 in all domains. “Observability enabled” must remain bounded, allocation-free, and incapable of hiding the newest critical event.

### 11. Local IPC is functionally Verified, but remains assurance debt

`kernel::ipc` now contains a fixed-capacity directional message channel with endpoint checks, a policy trait, FIFO/full/empty behavior, and eight passing host tests. `exec::shared_memory` contains page-grant/revoke logic with permission non-escalation, owner-only revocation, and seven passing host tests.

`STORY-P0-07-01`/`-02`, their Tests and Reports, and the shared-memory Tier-0 fixture are now functionally Verified. That status does not provide active per-task address spaces, real ACI, or raw D12/D13 evidence. The shared-memory prototype has no task-generation-safe token or task-exit revocation registry, and a page-table allocation failure after one or more mappings can leave a partial grant because mapping is not transactional. The channel's `AllowAllPolicy` is a stand-in, not a production default-deny policy.

Catalogue focus: D12, D13, G04, G11–G16, G19–G21, and G23. No performance result can close while atomic rollback, stale-handle resistance, and real policy/active-address-space prerequisites remain open.

### 12. Opt-in drivers and near-zero attack surface are architectural only

No driver crate, signed manifest, DCI, DMA/IRQ/MMIO grant mechanism, driver process, profile linker selection, or driver restart path exists yet. The desired property is stronger than “driver idle”: an unselected driver must contribute zero executable bytes, registered IRQs, DMA grants, capabilities, queues, and reachable parser surface.

Catalogue focus: D21, D22, D25, especially G09, G10, G17–G21.

### 13. Local LLM token delivery is architectural only

There is no storage stack, mmap primitive, demand paging, UMM, GPU admission controller, inference runtime, token queue, HBP/WCI delivery path, or actual ACI. Warm mapped access and cold media page-in must be measured separately; neither may be described as “nanosecond SSD access.”

Catalogue focus: D14–D20 and D25. D16's memory budgets exclude model weights and vendor runtime only where the report states those exclusions explicitly.

### 14. Build compactness is not yet optimized or continuously enforced

The release profile enables LTO and panic abort, but does not yet declare stripping, codegen-unit, or size/speed optimization policy. CI enforces per-crate source LOC but not the 8 MiB linked non-driver image, runtime static memory, per-feature deltas, exported symbols, dependency reachability, or opt-out absence.

Catalogue focus: G09 and G10 across all domains.

### 15. CI has no controlled performance runner

The QEMU harness uses a 15-second boot timeout and 100 ms host polling, which is a hang detector, not a boot benchmark. Public shared runners are too noisy for release timing thresholds. QEMU remains useful for instrumentation shape, invariants, and deterministic virtual-time tests; real thresholds require controlled Tier 1/2 hardware.

Catalogue focus: every row's `tier` and the measurement protocol in [`README.md`](README.md).

## Recommended execution order

1. Make catalogue integrity a CI gate so additions, removals, IDs, and required evidence fields cannot drift.
2. Before publishing latency numbers, land a monotonic clock/PMU measurement ABI that itself has calibrated bounded overhead and emits raw records.
3. Close safety prerequisites: timer/IDT/watchdog, complete context state, active address spaces, boot-map tightening, nested inheritance, and exact static-memory accounting.
4. Replace linear hot-path selection/allocation where measured worst-case bounds cannot meet D05/D07.
5. Add stripped-image/link-map and runtime-memory gates before adding large subsystems.
6. Build local IPC and spoor publication next; they become the measurement and observability substrate for drivers, storage, HBP/WCI, and inference.
7. Establish same-hardware Linux and RTOS harnesses only after the absolute TinyOS gates are stable. Comparisons should validate a mature implementation, not steer the kernel toward unsafe shortcuts.
