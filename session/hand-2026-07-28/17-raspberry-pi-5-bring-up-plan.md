# Handover 17 — Raspberry Pi 5 Bring-Up: the Minimum Slice That Closes `LE-09`

Follows: [`16-next-session-mandate.md`](16-next-session-mandate.md). Written when a Raspberry Pi 5 became available, which changes what the highest-value work in this project is.

This is a **decomposition proposal**, not a record of work done. Nothing in it is implemented. It exists so that the board is used deliberately rather than opportunistically, and so the first hardware session does not rediscover the four things below at its own expense.

---

## 1. Why this is now the highest-value work

`EPIC-P1` has produced four complete Features and a great deal of Tier 0 evidence. Every one of those Reports carries the same named debt:

> **`LE-09`** — No hardware tier; every timing number is Tier 0 and every timing claim carries release-blocking hardware debt.

`STORY-P1-01-04` sharpened the problem rather than solving it. The ratio gate now keeps the *runner's* speed out of the verdict, which is what makes CI trustworthy — but the quantity it gates is still a QEMU number. Handover 16 §"The variance question" states the consequence plainly: Tier 0 tail variance is 39–61%, which is a statement about QEMU and not about TinyOS, so **no evidence about this system's jitter exists yet.**

That is the gap the board closes, and nothing else can. `FEAT-P1-05` and `FEAT-P1-06` are the Epic's remaining proof Features, and both would produce *more Tier 0 evidence* — the thing this project already has a surplus of.

**The gate that was blocking board work has cleared.** [Handover 03 of 27 July](../hand-2026-07-27/03-le-09-arm64-pi5-slice-proposal.md) recorded the user's decision — Option B with a carve-out — and deferred pieces 1, 2 and 5 until `FEAT-P1-02` exited, on the reasoning that a fault on a board with no exception handling is a silent hang with no output at all. `FEAT-P1-02` is complete. **Pieces 1, 2 and 5 are unblocked as of now.**

## 2. What exists today

`hal-arm64` is **470 lines: a timer and nothing else.**

| Piece (Handover 03's numbering) | State |
|---|---|
| 1 — AArch64 boot + target spec | **Does not exist** |
| 2 — PL011 UART driver | **Does not exist** |
| 3 — `CNTVCT_EL0`/`CNTFRQ_EL0` cycle source and timebase | Shipped as `STORY-P1-01-03` (host-tested, never run on silicon) |
| 4 — UART-borne pass/fail | Shipped inside `STORY-P1-01-02` |
| 5 — Host-side SD/serial run path | **Does not exist** |

`hal-arm64/src/lib.rs` states its own constraint and it still holds: *"this crate is not a HAL port and must not grow into one here."* This plan is where it becomes one, deliberately, with a scope boundary.

---

## 3. The thesis: what "minimum effort, maximum impact" actually means here

**You do not need to port the MMU's address spaces, W^X, teardown, preemption or the WCET watchdog to close `LE-09`.**

`fixture_measure` needs four things: a cycle counter, a serial port, a way to get the image onto the board, and a way for a fault to announce itself instead of hanging. It does not need per-task page tables, `EL0`, or a scheduler tick. That is the whole reason this slice is small.

**But there is one exception that must not be discovered late** — see §4.1. Running with the MMU *disabled* on AArch64 forces all memory to be treated as Device-nGnRnE: uncached, unbuffered, no speculation. Timing measured in that state is not slow-but-proportional, it is **meaningless**, and it would silently poison every number this slice exists to produce.

So the minimum slice is: **boot → fault reporting → flat cacheable MMU → timer → run path → measure.** Six Stories. It stops precisely where evidence starts, and the follow-on Feature (address spaces, preemption, WCET on silicon) is deliberately *not* in it.

---

## 4. Four hardware realities that shape the plan

Every one of these should be verified against the current BCM2712 documentation and Raspberry Pi firmware notes before implementation. **Pi 4 tutorials are actively misleading for the Pi 5** — it is a larger departure than the version number suggests.

### 4.1 MMU off means uncached memory, and therefore meaningless timing

On AArch64, with `SCTLR_EL1.M == 0`, all data accesses behave as Device-nGnRnE regardless of what the memory actually is. Caches are architecturally not consulted. Measuring a dispatch path in that state produces a number dominated by DRAM round-trips.

**Consequence:** a minimal identity-mapped MMU with Normal Write-Back Cacheable attributes is a *prerequisite of measurement*, not a follow-on nicety. It is one Story (§5, `-03`) and it is not the `FEAT-P1-03` port — no per-task `TTBR0`, no `EL0`, no W^X, no teardown. One flat map, caches on, done.

This is the single most likely way for this slice to produce confidently wrong numbers.

### 4.2 `CNTVCT_EL0` is too coarse to measure what this project measures — and that is `LE-15`

`LE-15` is recorded as "the AArch64 generic timer is a 54 MHz system counter; decide when a board exists." A board exists, so here is the decision this plan recommends.

At 54 MHz, one tick is ~18.5 ns. A Cortex-A76 at ~2.4 GHz executes roughly **44 cycles per tick**. `D05/dispatch_select` currently measures a p50 of ~168 cycles at Tier 0. Measured with `CNTVCT_EL0`, that entire operation is **under four ticks** — quantisation noise, not a measurement. It is exactly the failure mode `LE-24` already documents for `D07`, arriving on a second axis.

**Recommendation: use the PMU cycle counter (`PMCCNTR_EL0`) as the ARM64 `CycleSource` for microbenchmarks, and keep `CNTVCT_EL0` as the `Timebase`/wall-clock source.** The two roles are genuinely different and the existing `hal::time` seam already separates them. Verification required before committing to this: `PMCCNTR_EL0` must be enabled and made readable (`PMCR_EL0`, `PMCNTENSET_EL0`, `PMUSERENR_EL0`), and if the firmware leaves us at `EL2` the trap configuration in `MDCR_EL2` matters.

**Second recommendation, independent of the above: adopt batched-iteration measurement.** Measure N iterations and divide, rather than one operation per sample. This is required for any coarse counter and it **also closes `LE-24`** — `D07/pool_u64x64_alloc_free_round_trip` medians to 0 cycles at Tier 0 precisely because a single operation costs less than the calibrated subtraction. One change, two loose ends.

### 4.3 RP1 breaks the recorded `EPIC-P1_5` deploy decision

On Pi 5, USB, Ethernet and GPIO sit behind the **RP1 southbridge, reached over PCIe.** There is no poking a NIC or a GPIO at a fixed physical address the way Pi 4 code does.

[Handover 08 of 27 July](../hand-2026-07-27/08-epic-p1_5-deploy-loop-transport-decision.md) recorded peer-to-peer Ethernet as the near-term dev-loop transport. **On Pi 5 that decision now implies PCIe bring-up plus an RP1 driver plus a NIC driver before a single byte can be deployed.** That is not a near-term transport; it is a Feature of its own.

**This plan does not use Ethernet.** The deploy path is SD-card image swap plus the debug UART, which needs no drivers at all. The `EPIC-P1_5` transport decision should be explicitly re-opened rather than left to collide with reality later. Recorded here as `LE-25` (§8).

### 4.4 The debug UART is a separate connector, and it is the only output that exists

The Pi 5 exposes a dedicated 3-pin debug UART distinct from the GPIO header. For Stories `-01` through `-03` it is the **only** channel by which the board can say anything at all.

Practical notes: buy two USB-serial adapters, because a suspected-dead board is usually a dead adapter. Confirm the expected baud from firmware documentation rather than assuming. Get the loopback test working against the adapter *before* trusting any silence from the board.

---

## 5. Proposed decomposition — `FEAT-P1-07`

One Feature under `EPIC-P1`, six Stories. Each Story is independently `Verified`-able and each leaves the board in a more useful state than it found it.

**`FEAT-P1-07` — Raspberry Pi 5 hardware tier: first measured evidence**

Exit criterion: **`fixture_measure` runs on a Raspberry Pi 5 and produces a `TINYOS-MEAS/1` envelope that `xtask` parses**, with the resulting numbers recorded in a Report that states the board revision, firmware version, clock policy and thermal state per the measurement protocol. `LE-09` closes on that Report and on nothing earlier.

| Story | Deliverable | Milestone |
|---|---|---|
| `STORY-P1-07-01` | AArch64 target spec, boot stub that takes `EL2 → EL1`, PL011 UART, and one character over serial | M0 |
| `STORY-P1-07-02` | AArch64 exception vectors; a synchronous fault prints a decoded `ESR_EL1` rather than hanging | M2 |
| `STORY-P1-07-03` | Minimal identity MMU with Normal cacheable attributes and caches enabled — explicitly *not* address spaces | M3-min |
| `STORY-P1-07-04` | GIC + generic timer periodic tick; `LE-15` resolved by choosing the `PMCCNTR_EL0`/`CNTVCT_EL0` split | M1 |
| `STORY-P1-07-05` | Host-side run path: SD image build, serial capture, and the existing UART pass/fail protocol driving exit codes | piece 5 |
| `STORY-P1-07-06` | `fixture_measure` on the board, batched-iteration measurement, first hardware Report | **M5 — closes `LE-09`** |

**Order matters and is not negotiable.** `-02` before `-03` and `-04`: this is the same reasoning that produced Option B's carve-out. Debugging an MMU configuration on a board that cannot report a translation fault is the failure shape Handover 03 spent two paragraphs arguing against.

### Suggested acceptance criteria, per Story

- **`-01`** — A named target spec builds `hal-arm64` for the board. The stub establishes a stack, zeroes `.bss`, drops to `EL1` if entered at `EL2`, and writes a known byte sequence to PL011. Evidence is a serial capture, quoted in the Test document.
- **`-02`** — A deliberately-triggered synchronous exception prints exception class, fault address and a decoded `ESR_EL1`. **A deliberate fault fixture is mandatory** — this Story's whole value is that failure becomes visible, which is unprovable without inducing one.
- **`-03`** — Identity map covering RAM and the UART MMIO with correct attributes (Normal WB Cacheable for RAM, Device-nGnRnE for MMIO), `SCTLR_EL1.M/C/I` set. **Acceptance requires evidence that caches are actually on**: the same measured loop before and after, showing the expected order-of-magnitude difference. Without that, this Story cannot distinguish success from a silently-ignored write.
- **`-04`** — Periodic tick at a declared interval, verified by ratio between consecutive intervals the way `kernel::fixture_idt_apic_timer` already does — not by absolute value. `LE-15` closed with a recorded decision and a conformance run of `hal::time::conformance` against the real registers.
- **`-05`** — `cargo run -p xtask -- pi5 --fixture=...` builds an image, reports how to place it, captures serial, and exits with the same code scheme as `qemu-x86_64`. Manual SD swap is acceptable; **automating the physical swap is out of scope.**
- **`-06`** — A `TINYOS-MEAS/1` envelope parsed by the existing `xtask` parser with no changes to the parser. That last clause is the point: it is the final test of the arch-neutrality claim `STORY-P1-01-03` made and never got to check on silicon.

---

## 6. Explicit non-goals

"No half measures" means the slice is complete and honest, not that it is large. The following are **out of scope for `FEAT-P1-07`** and must not be pulled in:

- **RP1, PCIe, Ethernet, USB, GPIO.** §4.3.
- **Per-task address spaces, W^X, `EL0`, teardown.** The `FEAT-P1-03` port is a follow-on Feature. `-03` here is a flat map and stops there.
- **Preemption and WCET enforcement on the board.** The `FEAT-P1-04` port, follow-on.
- **An SD-card driver.** The firmware loads the image; TinyOS never touches the SD controller in this slice.
- **Multi-core.** Cores 1–3 stay parked. A single core is sufficient for every number this Feature produces, and SMP changes what the measurements mean.
- **A device-tree parser.** Read the DTB pointer if convenient, but hardcode-and-verify is acceptable here. A real DT parser is a hostile-input parser and belongs behind the `C4` discipline in the Security Charter, not in a bring-up Story.
- **CI running on hardware.** See §7.

---

## 7. Decisions required before implementation starts

These are for the user, not for the implementing session to assume.

1. **Confirm ARM64 as the real-time tier**, with x86_64 retained for throughput and rich-workload claims. The technical argument is that x86 System Management Interrupts are invisible to the OS and unbounded, so no x86 worst-case bound is provable — a firmware-dependent claim, not an OS claim. ARM has no SMM equivalent. If this is confirmed it should be an ADR, because it reverses a positioning the README currently implies.
2. **Reconcile [`README.md`](../../README.md) line 147**, which still places Raspberry Pi at "Phase 3 onward." The `EPIC-P1` Hardware & test tier section already supersedes it for planning; the README has not caught up. The epic backlog flagged this needed reconciling and a board in hand forces it.
3. **Re-open the `EPIC-P1_5` transport decision** given §4.3. Ethernet is not a near-term Pi 5 transport.
4. **Decide how hardware evidence reaches CI.** Three honest options: (a) a self-hosted runner with the board attached; (b) hardware runs stay manual and land in Reports, with CI remaining Tier 0 only; (c) both, with hardware gated on a schedule rather than per-PR. **This plan assumes (b)** because it needs no infrastructure and blocks nothing, but it means the ratio baselines stay Tier 0 for now and `LE-23` is unaffected either way.
5. **Confirm `LE-15`'s resolution** — the `PMCCNTR_EL0` / `CNTVCT_EL0` split in §4.2 — or reject it with a reason.

---

## 8. Loose ends this plan creates and closes

**Closed by this Feature, if it completes:**

- **`LE-09`** — closes on `STORY-P1-07-06`'s Report and not before. A decision is not evidence; a plan is not evidence.
- **`LE-15`** — closes on `STORY-P1-07-04` with a recorded counter decision.
- **`LE-24`** — closes if batched-iteration measurement is adopted per §4.2, because the D07 quantisation problem and the coarse-counter problem have the same fix.

**New, to be added to [`goals/assurance/loose-ends.tsv`](../../goals/assurance/loose-ends.tsv) when this plan is accepted:**

- **`LE-25`** — `EPIC-P1_5`'s recorded peer-to-peer Ethernet deploy transport is not viable on Pi 5 without PCIe and RP1 bring-up, so the transport decision is stale against the actual first board. Unowned; needs the §7.3 decision.
- **`LE-26`** — the ARM64 `CycleSource` shipped in `STORY-P1-01-03` has never executed on silicon, so `hal::time::conformance` passing on the host is evidence about arithmetic, not about the registers. Closes with `STORY-P1-07-04`.

**Unaffected and still open:** `LE-03`, `LE-08`, `LE-10`, `LE-11`, `LE-12`, `LE-16`, `LE-18`, `LE-19(b)`, `LE-21`, `LE-22`, `LE-23`.

## 9. Assurance-spine obligations

This Feature does not get an exemption for being hardware work. Before any code:

- A row in [`feature-contracts.tsv`](../../goals/assurance/feature-contracts.tsv) for `FEAT-P1-07` — implementation class, subject classes, authority posture, hostile inputs, `BND-*` selection.
- Six rows in [`story-contracts.tsv`](../../goals/assurance/story-contracts.tsv). Likely domains: `D01` (boot), `D02` (faults), `D04`/`D05`/`D07` (measurement), `D08` (paging). Likely controls: `SEC-01`, `SEC-03`, `SEC-19`.
- A `TEST-P1-07-0n-A` document per Story, written before implementation, quoting the serial capture that constitutes evidence.
- `cargo run -p xtask -- check-assurance-spine` green before the first line of implementation.

**A note the implementing session should not talk itself out of:** hardware bring-up is exactly the work where "just get it booting first, contracts after" is most tempting and most damaging, because the resulting numbers are the ones every later claim will rest on.

## 10. Risks, and how each announces itself

| Risk | Detection | Response |
|---|---|---|
| Silent hang, no output ever | Nothing on serial after `-01` | Loopback-test the adapter first; bisect with a byte written from the very first instruction |
| Entered at `EL2`, code assumes `EL1` | Faults or silence at `-01` | Read `CurrentEL` and print it before anything else |
| MMU misconfigured; measurements uncached | `-03`'s before/after cache evidence shows no difference | The acceptance criterion exists precisely to catch this |
| `PMCCNTR_EL0` traps or reads zero | `-04` conformance run | Fall back to `CNTVCT_EL0` with batched iteration; record as a narrowed `LE-15` |
| Numbers arrive but are wrong | No local detection | Report states board revision, firmware, clock and thermal state so a third party can reproduce |
| Scope creep into RP1/SMP/address spaces | Story count grows past six | §6 is the boundary; a seventh Story means re-decomposing, not extending |

## 11. Definition of done for `FEAT-P1-07`

- Six Stories `Verified`, each with a Test document and a serial capture.
- One Report carrying the first hardware measurement in this project's history, with full protocol metadata.
- `LE-09`, `LE-15`, `LE-24`, `LE-26` closed in the register with `closed_in` populated.
- `README.md` reconciled; an ADR recorded for the ARM64-as-RT-tier decision.
- Tier 0 remains green and unchanged. **This Feature adds a tier; it does not replace one.**

## 12. Where to start

1. Get a serial adapter working against a known-good source before the board is ever blamed.
2. Write `FEAT-P1-07`'s contract rows and `TEST-P1-07-01-A` first.
3. Target spec, boot stub, `CurrentEL` printed, one byte out of PL011.
4. Then exception vectors — before the MMU, always.

The first character on that serial line is worth more than every remaining Tier 0 Feature in `EPIC-P1`, because it is the first thing this project has ever done that QEMU was not doing for it.
