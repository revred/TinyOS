# Handover 07 — `STORY-P1-02-02`: The Fault Path Gets Its Own Safety Net

Follows: [`06-story-p1-02-01-fault-handling.md`](06-story-p1-02-01-fault-handling.md). Evidence: [`REPORT-2026-07-27-06`](../../goals/reports/REPORT-2026-07-27-06.md). Completes both Stories of `FEAT-P1-02` — which still does not exit.

## What this session did

Delivered `STORY-P1-02-02`: a TSS with an Interrupt Stack Table, and `#DF` wired through an IST-bearing gate. Handover 06 named this the sharpest item in the register, and for the reason it gave — `STORY-P1-02-01` made the fault handler real, which made the fault handler *new code that can fault*. Until today, a fault inside it triple-faulted: QEMU resets, no output, no diagnostic. `LE-04` has carried that since Handover 32.

- **Test document first** ([`TEST-P1-02-02-A`](../../goals/tests/TEST-P1-02-02-A.md)), nine clauses.
- **`hal_x86_64::tss`**: the 104-byte TSS with `size_of` and every offset pinned by host tests, `IstIndex` as a newtype that cannot express slot 0 or slot 9, and the 16 KiB `#DF` stack.
- **`hal_x86_64::gdt`**: `boot.rs`'s three descriptors repeated *byte for byte* (asserted by a host test, because the two copies cannot be shared) plus the 64-bit TSS system descriptor, then `lgdt` and `ltr`.
- **`Idt::set_handler_with_ist`**, and a test that all 255 other vectors still carry `ist == 0`.
- **A separate `#DF` entry stub and a separate kernel symbol** (`tinyos_double_fault_entry`), so the primary path gained no eighth case.
- **Installed on the real boot path**, not only in the fixture — both `init_faults_only` and `init` stand up the GDT and TSS.
- **Tier 0 fixture** (`--fixture=double-fault`), wired into CI the same day.

## The load-bearing evidence

```
fixture-double-fault ist stack 0x11f090..0x123090 installed_top=0x123090
fixture-double-fault captured #DF vector=8 error_code=0x0 rip=0x10349e faulting_rsp=0x8000000000
fixture-double-fault handler_rsp=0x122d58 ist_stack=0x11f090..0x123090 on_ist_stack=true
fixture-double-fault double_fault_spoors=2 attributed_to_task=true terminal=true
TINYOS-RESULT/1 fixture=double-fault ok=true
```

Two numbers carry the claim. `faulting_rsp=0x8000000000` is the unmapped 512 GiB the victim installed as its stack pointer, so the fault demonstrably came from the destroyed stack. `handler_rsp=0x122d58` lies inside `0x11f090..0x123090`, the reserved `#DF` stack — hardware loaded `RSP` from the TSS before pushing anything. A handler that merely produced output would have looked identical.

## The contrast, run rather than assumed

"It passes now" proves nothing about *why*. So the same fixture was rebuilt with one line changed — vector 8's gate back to `set_handler`, no IST — and run under `qemu -d int,cpu_reset`:

```
     0: v=0e ... SP=0010:0000008000000000 CR2=0000007ffffffff8
check_exception old: 0xe new 0xe
     1: v=08 ... SP=0010:0000008000000000
check_exception old: 0x8 new 0xe
Triple fault
```

`#PF` on the push, `#PF` again while delivering it (hence `#DF`), `#PF` a third time while delivering *that*, and the CPU gives up. Serial output stops mid-fixture. QEMU exits **0** — not the success code 33, not the failure code 35, but a reset — so `xtask` reports a harness error rather than a kernel verdict. That is exactly what `LE-04` has meant since Handover 32, seen directly.

This is also the honest answer to Handover 06's confessed process gap. There is still no host Red count (the pure-data modules' unit tests were written alongside their implementations again), but there **is** a recorded, deliberate Tier 0 Red, which is what the mandate is actually for: proof that the fixture can fail and that the mechanism under test is what makes it pass.

## Three design decisions worth defending

**The new GDT is additive, and that is the whole safety argument.** Entries 0–2 are `boot.rs`'s null/code/data descriptors verbatim, so `CS`/`DS`/`SS` keep the selectors *and* cached descriptors they already hold. No far return through a hand-built frame to reload `CS`, no window where the code segment is undefined. Reloading segments is exactly the kind of code that has no business running on a path whose purpose is making faults survivable. The two copies can't be shared — `boot.rs`'s lives in 32-bit `global_asm!` that runs before any Rust — so a host test asserts the three quadwords instead, making drift a test failure rather than a mystery triple fault.

**There is no `DoubleFaultDisposition`, for the same reason there was no `Resume`.** `Disposition::of` was not extended and gained no vector-dependent branch; its load-bearing invariant is that it reads exactly one field, which context was running. A double fault means the machinery that would do the containing is what just broke — there is no arm to choose between, and an enumeration with one variant is a decision that isn't one. The audit still records *which task* was running, for attribution, and stamps `Failed` on the disposition spoor in **both** contexts: a double fault inside a task is not a contained fault and must never audit as one.

**One IST slot, not seven.** `#MC` (vector 18) is the obvious second consumer and is deliberately unwired — there is no Tier 0 way to raise a machine check, so it would be an unexercised gate and unexercised memory in a fault path. Same argument `STORY-P1-02-01` used against the resume arm.

## A coupling paid off, and a small trap found

Handover 06 warned that the `#GP` fixture depended on the boot GDT holding exactly three descriptors. It did — index 3 is precisely where the TSS descriptor now sits, so that victim would have silently become a TSS-selector load: still a `#GP`, different reason, different error code, nothing saying so. The victim now uses index 511 (`0xFF8`), far past any GDT this kernel will plausibly install. A fixture that picks a value by counting what exists today breaks the next time something is added; the fix was to stop counting.

Related, and found by a test rather than by reasoning: `Gdt` is aligned to 8, not 16 like `Idt`. At 40 bytes a 16-byte alignment rounds `size_of` up to 48, and the limit derived from it would advertise eight bytes of tail padding as a sixth descriptor slot — turning an out-of-range selector into a *zeroed* one, which is a quieter failure than the `#GP` hardware should raise.

## A side effect worth having: every fixture feature now passes target clippy

Verifying this Story meant building the kernel binary against the real `x86_64-tinyos` target, which made running clippy there nearly free. So it was run once per fixture feature — `LE-12`'s subject — and the debt turned out to be concrete: `fixture-context-switch` (7 lints), `fixture-fault` (7) and this Story's own `fixture-double-fault` (3) all failed `-D warnings`, every one of them the same missing `static_mut_refs` allow that `interrupts::init` has carried since `STORY-P0-04-02`. All fixed; one lint was a genuine simplification and was taken rather than allowed. `LE-12` stays open — CI still does not run these passes, so the property is true today and unenforced tomorrow — but the backlog behind it is now zero.

## The finding: the timing gate does not pass on this host

Stated rather than omitted, because it looks like a regression and is not one. `check-timing-regression` failed on all five attempts this session — but on a **different set of metrics each time**, with the count climbing (2 → 4 → 6 statistics) as the session's own QEMU boots loaded the machine, and with `D05/dispatch_run_once_cooperative_round` measuring *below* its baseline in the same run where others measured 2× above theirs.

The measured binary never executes any code this Story touched: `fixture_measure` installs no IDT at all and calls only `remap_and_mask_pic`, so the GDT, TSS, `ltr`, IST gate and `#DF` stub are all unreachable from it. The metric that fails most often, `D07/pool_u64x64_alloc_free_round_trip`, is pure `kernel::mem::Pool` — untouched. A clean A/B against the pre-change tree could not be run, because `check-timing-regression` and the `measure`/`write_result` harness it depends on are themselves uncommitted work from `STORY-P1-01-02`.

The reading: the committed baselines were recorded on a quieter machine, and Tier 0 cycle counts on a loaded Windows host drift further than the gate's 60% tolerance allows. That is `LE-16` from the other side — ambient noise is now *larger* than the gate's tolerance, so the gate is not merely blunt, it is host-condition-sensitive. Filed as **`LE-18`** rather than fixed by loosening the tolerance or re-recording baselines from a noisy machine; either would make the gate quieter without making it better. CI runs the same gate on a GitHub runner, which this session has no evidence about.

## Verification

`cargo test --workspace --lib` 253 (exec 51, hal 13, hal-arm64 12, hal-x86_64 81, kernel 96) · `cargo test -p xtask` 79 · fmt clean · scoped clippy `-D warnings` clean · **target-profile clippy clean for every one of the eleven `kernel` fixture features and for `exec`** (see `LE-12`) · `check-assurance-spine` exit 0 · every Tier 0 fixture re-run and unchanged (real boot path, `fault`, `double-fault`, `context-switch`, `idt-apic-timer`, `pci-enumeration`, `pool-bench`, `address-space`, `win32-shim`, `shared-memory` all exit 0; `idt-apic-unrouted` exit 1, its own correct result) · **timing gate exit 1, see the finding above** · spine 14 Features / 36 Stories / 29 Tests / 35 Reports.

## Loose-ends register (canonical as of this handover)

Carried forward from [Handover 06](06-story-p1-02-01-fault-handling.md#loose-ends-register-canonical-as-of-this-handover); one new item, one closed, two narrowed.

| ID | Loose end | Origin | Owner / fix path | Status |
|---|---|---|---|---|
| LE-01 | Priority-inheritance behavioral half never verified | `STORY-P0-02-03` | `STORY-P1-04-01` criterion 2 | Open — owned |
| LE-02 | WCET enforcement has no timer and no watchdog behind it | `STORY-P0-02-04` | `STORY-P1-04-02` | Open — owned |
| LE-03 | No real `#PF`/`#GP`/`#UD` handling; every fault is terminal diverge-and-report | Handover 32 | `FEAT-P1-02` | **Narrowed further 2026-07-27** — `#UD`/`#GP`/`#PF` captured and contained (`STORY-P1-02-01`), `#DF` now captured and reported (`STORY-P1-02-02`). What remains: `#XF` (vector 19, sharpened by `STORY-P1-01-01`'s SSE work), `#MC` (18), and every other vector still reach the shared fail-closed default |
| LE-04 | No TSS/IST; a fault during fault handling triple-faults | Handover 32 | `STORY-P1-02-02` | **Closed 2026-07-27** — TSS installed on the real boot path, `#DF` on an IST stack, with the no-IST triple fault recorded as the contrast. `#MC` has no IST, which is `LE-03`'s remainder rather than this item's |
| LE-05 | `exec::AddressSpace` built but never installed; system runs all-RWX identity-mapped | `STORY-P0-05-02` | `FEAT-P1-03` | Open — owned. Both of the containment Stories `FEAT-P1-03` was waiting for now exist |
| LE-06 | `pool-bench` was a divergent sibling harness | Handover 35 | `STORY-P1-01-01` | **Closed 2026-07-27** |
| LE-07 | CI has never been observed running any of this work | Standing since Handover 07 | Phase-independent | **Closed 2026-07-27** |
| LE-08 | I/O APIC device-IRQ routing deferred (local APIC only) | `STORY-P0-04-02`/`-03` | First Story routing a device IRQ | Open — deferred with trigger |
| LE-09 | Pi 5 (ARM64) is the short-term hardware, but no ARM64 bring-up slice or deploy path exists | Handover 37 directive 1 | Option B with the carve-out ([Handover 03](03-le-09-arm64-pi5-slice-proposal.md)) | Open — pieces 3 and 4 delivered. `FEAT-P1-02`'s functional half is now complete, so pieces 1, 2 and 5 are no longer blocked on it — only on `LE-17` if the Feature must formally exit first |
| LE-10 | ECAM/MCFG config access and PCI bridge traversal deferred | `STORY-P0-04-03` | First Story needing extended config space | Open — deferred with trigger |
| LE-11 | `Context::new` seeds task `rflags` with `IF` set, so switching into a task enables interrupts even with no IDT installed | `STORY-P1-01-01` | `FEAT-P1-02` | Open — **mitigated further, still not fixed**: both fault fixtures load a real IDT, and a stray interrupt on one now has an IST-backed `#DF` behind it. The fail-open seam itself is untouched |
| LE-12 | CI's clippy never lints target-only fixture code | `STORY-P1-01-01` | Per-fixture target clippy in the CI lint job | Open — unowned, but **the backlog behind it is now zero**: verifying this Story meant building against the real target anyway, so clippy was run once per fixture feature. Three features failed `-D warnings` (`fixture-context-switch` 7, `fixture-fault` 7, `fixture-double-fault` 3), all the same missing `static_mut_refs` allow; all fixed. Every `kernel` fixture feature and `exec`'s fixture binaries now pass. Closing this is now a CI-wiring job with a clean starting point, not a cleanup of unknown size |
| LE-13 | Measurement ran dev-profile binaries | `STORY-P1-01-01` | `STORY-P1-01-02` | **Closed 2026-07-27** |
| LE-14 | `context::switch` saves no SSE/x87 state | `STORY-P1-01-01` / ADR 0003 | `FEAT-P1-04` | Open — owned |
| LE-15 | The AArch64 generic timer is a 54 MHz system counter (~18.5 ns/tick), so hardware metrics will be quantization-limited | `STORY-P1-01-03` | Decide when a board exists | Open — owned |
| LE-16 | The Tier 0 timing gate can only detect regressions of ~1.6x or worse | `STORY-P1-01-02` | Only a hardware tier fixes it (`LE-09`) | Open — owned, and now **sharpened by `LE-18`**: on a loaded host the noise exceeds the tolerance entirely, so the gate does not merely miss small regressions, it reports large false ones |
| LE-17 | The fault path has **no timing baseline**, which `FEAT-P1-02`'s own exit criteria require | `STORY-P1-02-01` | Add a fault-latency phase to `fixture_measure` and a baseline row. Now the **only** thing standing between `FEAT-P1-02` and exit | Open — owned, and next |
| **LE-18** | The timing gate is host-condition-sensitive: committed baselines were recorded on a quiet machine, and on a loaded one the gate reports 2–6 false regressions per run, varying by run | `STORY-P1-02-02` (this handover) | Not "loosen the tolerance" and not "re-record from a noisy host" — either makes the gate quieter without making it better. Needs a decision about what the baselines are *of* (a machine state, or a mechanism), which is really a hardware-tier question | Open — unowned, needs a Story |

## Next session — start here

1. **`LE-17` — the fault-latency baseline.** Small, mechanical, uses machinery that already exists, and it is now the single item between `FEAT-P1-02` and exit. One caution learned today: it puts a *fault* on the measured path, and the measure fixture currently installs no IDT at all deliberately — so this is not purely additive to `fixture_measure`, and the phase will need its own contained victim per sample.
2. **`FEAT-P1-03`** (per-task address spaces), which both of this Feature's Stories were the prerequisite for: a live `CR3` switch now has real containment *and* a surviving fault path behind it. This is the work Handovers 32/33/35 kept refusing to do, and the refusal no longer applies.
3. **`LE-18`**, if the timing gate starts failing CI. It has not been observed failing on a runner, only on this development host.

## What this handover does not do

No hardware ran anything, and `STORY-P1-02-01`'s finding applies here with full force — QEMU's escalation behavior matched the architecture this time, which is an observation, not a guarantee. `#MC` has no IST. Nothing survives a fault inside the `#DF` handler; "terminal but reporting" is the entire claim. `RSP0` is zero and unused, so there is still no privilege boundary anywhere. No `PERF-D02` guardrail closes, `STORY-P1-02-02` is `baseline-debt` rather than `verified`, and `FEAT-P1-02` does not exit.
