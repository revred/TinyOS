# FEAT-P1-07 — Raspberry Pi 5 Hardware Tier: First Measured Evidence

Status: **Specified — six Stories, none started; this Feature exists to close `LE-09`, and `LE-09` closes on `STORY-P1-07-06`'s Report and on nothing earlier**
Epic: [`EPIC-P1`](../epics/EPIC-P1.md)
Introduced in: [`session/hand-2026-07-28/17-raspberry-pi-5-bring-up-plan.md`](../../session/hand-2026-07-28/17-raspberry-pi-5-bring-up-plan.md), accepted with its §7 decisions in [`session/hand-2026-07-28/19-feat-p1-07-acceptance-and-spine.md`](../../session/hand-2026-07-28/19-feat-p1-07-acceptance-and-spine.md)

## Description

Every Report `EPIC-P1` has filed carries the same named debt: **`LE-09`** — no hardware tier, every timing number Tier 0, every timing claim release-blocking. `STORY-P1-01-04` sharpened that problem rather than solving it: the ratio gate now keeps the runner's speed out of the verdict, which is what makes CI trustworthy, but the quantity it gates is still a QEMU number. Tier 0 tail variance of 39–61% is a statement about QEMU, not about TinyOS. **No evidence about this system's jitter exists yet**, and no Tier 0 Feature can produce any.

A Raspberry Pi 5 is now available. This Feature is the minimum slice that turns it into evidence.

It is deliberately *not* an ARM64 port. `fixture_measure` needs four things — a cycle counter, a serial port, a way to get the image onto the board, and a way for a fault to announce itself instead of hanging. It does not need per-task page tables, `EL0`, or a scheduler tick. The slice is **boot → fault reporting → flat cacheable MMU → timer → run path → measure**, and it stops precisely where evidence starts.

The one thing that looks like scope creep and is not: **`STORY-P1-07-03`'s MMU**. With `SCTLR_EL1.M` clear, AArch64 treats every data access as Device-nGnRnE regardless of what the memory actually is — caches are architecturally not consulted. Timing measured in that state is not slow-but-proportional, it is meaningless, and it would silently poison every number this Feature exists to produce. A flat identity map with Normal Write-Back Cacheable attributes is therefore a *prerequisite of measurement*. It is not the `FEAT-P1-03` port: no per-task `TTBR0`, no `EL0`, no W^X, no teardown.

## Crate(s) involved

`os/src/hal-arm64/` (boot stub, PL011, exception vectors, MMU, GIC, PMU/generic-timer counters), `os/src/kernel/` (the AArch64 side of the fixture entry points), `os/src/xtask/` (the `pi5` run path). `hal-arm64/src/lib.rs`'s standing constraint — *"this crate is not a HAL port and must not grow into one here"* — is lifted by this Feature and replaced by §6's boundary.

## Depends on

- `FEAT-P1-01` — the measurement harness, the `hal::time` seam and the `TINYOS-MEAS/1` envelope this Feature carries onto silicon. `STORY-P1-01-03` already supplied the AArch64 `CycleSource`/`Timebase`; it has never executed on a register (`LE-27`).
- `FEAT-P1-02` — the recorded gate. [Handover 03 of 27 July](../../session/hand-2026-07-27/03-le-09-arm64-pi5-slice-proposal.md) deferred every board-dependent piece until fault handling existed, because a fault on a board with no exception handling is a silent hang with no output at all. `FEAT-P1-02` is complete, so pieces 1, 2 and 5 are unblocked.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P1-07-01`](../stories/STORY-P1-07-01.md) | AArch64 target spec, boot stub, `EL2 → EL1`, PL011 UART, first byte on the wire | In progress — host half Green, criteria 3 and 4 need a board |
| [`STORY-P1-07-02`](../stories/STORY-P1-07-02.md) | Exception vectors; a synchronous fault prints a decoded `ESR_EL1` instead of hanging | In progress — host half Green, criterion 2 needs a board |
| [`STORY-P1-07-03`](../stories/STORY-P1-07-03.md) | Flat identity MMU, Normal cacheable RAM, Device MMIO, caches on — explicitly *not* address spaces | Specified |
| [`STORY-P1-07-04`](../stories/STORY-P1-07-04.md) | GIC + generic-timer periodic tick; `LE-15` resolved by the `PMCCNTR_EL0`/`CNTVCT_EL0` split | Specified |
| [`STORY-P1-07-05`](../stories/STORY-P1-07-05.md) | Host-side run path: SD image build, serial capture, UART pass/fail driving exit codes | Specified |
| [`STORY-P1-07-06`](../stories/STORY-P1-07-06.md) | `fixture_measure` on the board, batched-iteration measurement, the first hardware Report | Specified |

**Order matters and is not negotiable.** `-02` before `-03` and `-04`, for the same reason Handover 03 spent two paragraphs on: debugging an MMU configuration on a board that cannot report a translation fault is the failure shape the carve-out existed to avoid.

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0/C1** · subject **C0/C1** · boundary tests **BND-01, -02, -03, -17** · **PD-07, PD-12, PD-14** · **RCG-01, RCG-13, RCG-14**.

Subject classes stop at C1 deliberately: this slice runs no tasks, loads no images and creates no C2/C3/C4 domain. Claiming otherwise would import evidence obligations no Story here can discharge.

Three things in this Feature are hostile input, and each is named because a bring-up session's instinct is to trust all three:

- **The firmware handoff.** Entry exception level, register state and the device-tree blob pointer come from the Raspberry Pi firmware. They are read and *reported* (`CurrentEL` is printed before anything else, per §10's second risk row); they never become authority (`PD-14`, `BND-02`).
- **The device-tree blob.** A real DT parser is a hostile-format parser and belongs behind the Security Charter's `C4` discipline, not in a bring-up Story. This Feature hardcodes-and-verifies. That is exactly `BND-03` — C1 contains no complex hostile-format parser — and it is the reason the non-goal exists (`PD-12`).
- **Register values.** `CNTFRQ_EL0` may be unprogrammed (zero) and `PMCCNTR_EL0` may trap or read zero. `STORY-P1-01-03` already made honest-absence the host-side behaviour; `STORY-P1-07-04` is where that survives contact with real silicon.

`SEC-01` (hardware-rooted verified boot) is selected by `STORY-P1-07-01` **so that its absence is named rather than omitted**: the Pi 5 firmware chain gives TinyOS no measured-boot evidence, and `BND-01` cannot be closed by this Feature. That is stated debt, not a silent gap.

## Exit criteria

**`fixture_measure` runs on a Raspberry Pi 5 and produces a `TINYOS-MEAS/1` envelope that `xtask` parses**, with the resulting numbers recorded in a Report stating board revision, firmware version, clock policy and thermal state per the measurement protocol.

- Six Stories `Verified`, each with a Test document written first and a quoted serial capture as evidence.
- One Report carrying the first hardware measurement in this project's history.
- `LE-09`, `LE-15`, `LE-24` and `LE-27` closed in the register with `closed_in` populated.
- Tier 0 remains green and unchanged. **This Feature adds a tier; it does not replace one.**

`LE-09` does not close on this Feature being decomposed, contracted or half-implemented. A decision is not evidence; a plan is not evidence.

### Amended 2026-07-28 by [`ADR 0005`](../../docs/adr/0005-arm64-real-time-tier-is-conditional-on-secure-world-qualification.md) — a measurement establishes the *tier*, not a *bound*

Registered as `LE-43`. The criteria above fused two claims that `ADR 0005` separates, and this section separates them. **Nothing above is withdrawn** — every criterion still holds exactly as written, and this Feature is still the only one that can close `LE-09`.

- **What this Feature establishes on completion:** a hardware tier exists. `LE-09` closes. Every number in its Report is real hardware evidence and is quotable as such.
- **What it does not establish:** a worst-case latency bound, WCET claim, jitter envelope, or any `G-RT-*` / `G-PA-*` guarantee. Under `ADR 0005` those are quotable only from a platform holding a **secure-world qualification record**, and **no platform holds one — the Pi 5 included.** A GIC's secure interrupt groups routed to `EL3` by `SCR_EL3.FIQ` preempt NS-EL1 irrespective of `PSTATE.I`, unattributably, and no one has yet looked at how this board's firmware configured that.
- **What this Feature therefore also produces**, as evidence toward a future qualification record rather than as a new exit criterion: **`Q1`** — the Report's board revision, firmware version, clock policy and thermal state already *are* `Q1`, plus the entry exception level the first serial capture reveals; and the beginning of **`Q2`** — what can and cannot be determined about secure-world configuration on closed Pi 5 firmware, stated in those words where it cannot.
- **Whether the qualification record itself is [`STORY-P1-07-06`](../stories/STORY-P1-07-06.md)'s scope or a seventh Story is not settled here**, and `ADR 0005` deliberately declines to settle it. §6 governs: a seventh Story means re-decomposing this Feature, which is a scope decision rather than a diff.
- **If a `Q3` residency campaign is attempted, its positive control is not optional.** A campaign that observes nothing reads as qualification and is the cheapest result to obtain, so an instrument that has never been shown to detect a known perturbation cannot be believed when it reports zero. `ADR 0005` §"The trap this ADR sets, named up front" is binding on any Report claiming `Q3`.

A Report from this Feature that quotes one of its numbers as a `G04`-class bound is wrong under `ADR 0005`, and **nothing in this repository would currently catch it** — that gap is `LE-33`'s second condition, which is open.

## Explicit non-goals

Out of scope, and pulling any of them in means re-decomposing rather than extending:

- **RP1, PCIe, Ethernet, USB, GPIO.** On a Pi 5 these all sit behind the RP1 southbridge over PCIe — see `LE-26`. The deploy path here is SD-card image swap plus the debug UART, which needs no drivers at all.
- **Per-task address spaces, W^X, `EL0`, teardown.** The `FEAT-P1-03` port is a follow-on Feature.
- **Preemption and WCET enforcement on the board.** The `FEAT-P1-04` port, follow-on.
- **An SD-card driver.** The firmware loads the image; TinyOS never touches the SD controller.
- **Multi-core.** Cores 1–3 stay parked. SMP changes what every measurement means.
- **A device-tree parser.** See the containment contract above.
- **CI running on hardware.** Decision (b) of the plan's §7.4, recorded in Handover 18: hardware runs stay manual and land in Reports; CI stays Tier 0. The ratio baselines therefore stay Tier 0 and `LE-23` is unaffected either way.

## Named debt this Feature does not touch

`LE-03`, `LE-08`, `LE-10`, `LE-11`, `LE-12`, `LE-16`, `LE-18`, `LE-19`, `LE-21`, `LE-22`, `LE-23`, `LE-25`.

`LE-26` is created by this Feature's §4.3 finding and routed around rather than closed by it — the `EPIC-P1_5` transport decision is re-opened, not answered here.
