# Handover 32 — `STORY-P0-04-02` Implemented and Verified: Real IDT / Local-APIC Timer Bring-Up

Follows: [`31-security-charter-and-remote-code-exclusion.md`](31-security-charter-and-remote-code-exclusion.md) (concurrent containment/Security-Charter formalization work) and [`29-cover-note-five-tier-containment-and-feat-p0-04.md`](29-cover-note-five-tier-containment-and-feat-p0-04.md) (this Story's own mandate).

## Direction received

The user reviewed Handover 29's cover note and confirmed: the five containment classes and 20 boundary tests are now integrated into the assurance spine (concurrent work, Handovers 30/31); the one remaining contractual gap is a machine-readable transition/communication matrix; runtime containment (IDT/fault handling, active per-task `CR3`, task-exit revocation) remains the accurately-documented blocker; `FEAT-P0-04` is 1/3 Verified; and the mandated next implementation is `STORY-P0-04-02` (IDT/APIC timer bring-up, measured interrupt bounds, fail-closed default handler), with `STORY-P0-04-03` (PCIe enumeration) to follow. Given a choice between that implementation work and the still-missing transition matrix, the user chose `STORY-P0-04-02`.

## What landed

**`STORY-P0-04-02` implemented and Verified**, scoped to the local APIC (not the I/O APIC — see the Story's own scope-correction note for why):

1. **`hal_x86_64::idt`** — a full 256-entry x86_64 long-mode IDT: 16-byte gate-descriptor bit-packing, `set_handler`/`entry`, and `every_entry_present` (the structural half of "no vector is ever silently unhandled"). Pure data, host-testable, 7 new tests, no dependency on `interrupts.rs`.
2. **`hal_x86_64::interrupts`** — `init(initial_count)` orchestrates: build the IDT (every vector defaults to a bare, non-returning `unhandled_interrupt_handler`; `TIMER_VECTOR`/`SPURIOUS_VECTOR` get real save-everything/`iretq` assembly stubs), `lidt`, retire the legacy 8259 PIC (remap off the CPU-exception range, then mask), enable the local APIC, arm its timer for periodic delivery, `sti`. Target-only (gated `not(target_os = "windows")`, matching `boot`/`qemu_exit`'s existing precedent) — everything in it only means anything on a real CPU.
3. **Two new Tier 0 QEMU fixtures** (`kernel`'s `fixture-idt-apic-timer`/`fixture-idt-apic-unrouted`): the first arms the timer and asserts 5 real ticks land within a self-consistent bound; the second deliberately executes `int 0x21` and asserts the fail-closed default handler is reached — this fixture's *correct* result is a QEMU **Failure** exit code, mirroring `fixture-broken-boot`'s established precedent.
4. **`interrupts::init` wired into the real (non-fixture) `kernel_main` boot path** — an armed IDT with a fail-closed default for every vector is now this kernel's actual boot-time state, not just a fixture demonstration, though nothing yet consumes `tick_count()` (no scheduler dispatch loop exists to).
5. **V&V**: `STORY-P0-04-02.md` rewritten to Verified with final acceptance criteria, new `TEST-P0-04-02-A`, `REPORT-2026-07-26-27`; `FEAT-P0-04` now 2/3 Stories Verified; `story-contracts.tsv` row moved `specified` → `baseline-debt`.

## A real bring-up bug, found and fixed (not glossed over)

The local APIC is architecturally relocatable via `IA32_APIC_BASE` (Intel SDM Vol 3A §11.4.1). The first working version of this Story relocated it into `boot.rs`'s existing first-1GiB identity map, to avoid touching boot assembly. **QEMU's own APIC device model does not honor that relocation** — every local-APIC register, including the read-only, hardware-answered APIC ID register, read back all-zero after relocating, proving the MMIO access was landing on ordinary backing memory rather than being intercepted by the APIC at all. Diagnosed via a temporary debug-serial hex dump (COM1, removed before landing) of `IA32_APIC_BASE`/SVR/LVT/ID/current-count immediately after `init` — the MSR write itself was confirmed correct (enable bit set, requested address field present), narrowing the fault to the MMIO decode path specifically. Fixed by targeting the real, non-relocated default (`0xFEE00000`) instead: `boot.rs` gained `boot_pd_gib3`, a second `PDPT[3]`-rooted `PD` identity-mapping `0xC0000000-0xFFFFFFFF`, since the default address sits outside the original first-1GiB map. This is a shared-boot-path change — verified via a full regression sweep of all nine pre-existing QEMU fixtures (default boot, `broken-boot`, `context-switch`, `address-space`, `win32-shim`, `blue-sharc`, `blue-sharc-broken`, `shared-memory`, plus this Story's own two) that none regressed.

A second bug surfaced the same way: `fixture-idt-apic-unrouted`'s first version armed the timer with reload count `1` ("irrelevant, this fixture never waits on a tick" — true of the *read*, false of the *arm*, since `init` unconditionally enables the timer regardless of whether the caller consumes ticks). Count `1` fires near-continuously, starving the CPU of forward progress before it ever reached the fixture's own deliberate `int 0x21` — observed as a 15-second QEMU boot-timeout hang, not a crash, which made it look at first like a completely different (and initially more alarming) class of bug than it actually was. Fixed with `u32::MAX` as the reload count.

## Verification

- `cargo test --workspace --lib`: **152/152 passed** (`exec` 51, `hal` 4, `hal-x86_64` 37 [+7], `kernel` 60).
- All nine pre-existing QEMU fixtures plus this Story's own two: **11/11 correct results** (nine unchanged passes, `idt-apic-timer` passes, `idt-apic-unrouted` correctly reports Failure).
- `cargo fmt --check` / `cargo clippy --workspace --lib -- -D warnings`: clean. Also clippy'd against the real target for `hal-x86_64` and `kernel` (default + both new fixture features) specifically, since `interrupts.rs` and the fixtures are gated off the host build entirely.
- `cargo run -p xtask -- check-crate-sizes`: `hal-x86_64` 1,404 lines (from 788), `kernel` 2,012 (from 1,804) — both far under the 20,000-line ceiling.
- `cargo run -p xtask -- check-image-size`: kernel release image 17,288 bytes (from 16,032) — real production growth (`interrupts::init` is now boot-path code), still far under the 8 MiB ceiling.
- `cargo run -p xtask -- check-assurance-spine`: passes; `STORY-P0-04-02` now `baseline-debt`.

## What this does not claim

- **No I/O APIC / device-IRQ routing.** This Story's own draft description over-scoped itself; neither acceptance criterion ever needed it, and there is still no device driver to route a line for. Deferred to `STORY-P0-04-03` or a later driver Story, not silently dropped.
- **No TSS/Interrupt Stack Table.** A `#DF`/`#MC` whose own stack is already invalid can still fault a second time while the CPU pushes that vector's frame — `idt::Idt`'s own doc comment states this as a real, general, unresolved limitation.
- **No production tick consumer.** The IDT/timer are armed on the real boot path, but nothing reads `tick_count()` — no scheduler dispatch loop exists yet.
- `STORY-P0-04-02` is functionally Verified, not assurance `verified` — no raw `D02`/`D03` interrupt-latency evidence, and `SEC-03`/`SEC-18`/`SEC-19`/`SEC-20` remain `baseline-debt` per the assurance spine's own lifecycle rules.
- The transition/communication matrix gap Handover 29 flagged (and this session's own kickoff question deferred in favor of this Story) is **still open** — concurrent work landed the containment-class/boundary-test catalogue and the Security Charter's PD/RCG obligations and class-communication-pair enumeration (Handovers 30/31), but whether that fully satisfies "an explicit machine-readable source→destination transition/communication matrix" as originally posed is for the next session (or the user) to confirm, not asserted here.

## Immediate next steps

1. `STORY-P0-04-03` (read-only PCIe enumeration under QEMU `q35`) — the last `FEAT-P0-04` Story, independent of this one.
2. This Story's own IDT now exists as a real, working foundation — the next natural step toward closing "no IDT/fault containment" (Handover 27's largest-named gap) is wiring a *real* CPU exception handler (not just the fail-closed diverge-and-report default) for at least `#PF`/`#GP`, and only then active per-task `CR3` switching, since a live page-table switch with no fault handler behind it is strictly more dangerous than the current all-RWX identity map.
3. Confirm with the user whether the concurrent PD/RCG/class-communication-pair work (Handover 31) already closes the "transition/communication matrix" gap, or whether a distinct artifact is still wanted.
