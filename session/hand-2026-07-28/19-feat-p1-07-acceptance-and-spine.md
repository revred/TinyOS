# Handover 19 — `FEAT-P1-07` Accepted: the §7 Decisions, the Assurance Spine, and What the First Hardware Session Starts With

Follows: [`17-raspberry-pi-5-bring-up-plan.md`](17-raspberry-pi-5-bring-up-plan.md). Written at the close of 2026-07-28.

**On the number.** This was written as Handover 18 and renumbered on landing: a concurrent session took slot 18 with [`18-story-p0-01-04-harness-assurance.md`](18-story-p0-01-04-harness-assurance.md) and its commits (`d0a2a60`…`75ac367`) arrived mid-session, sweeping this Feature's then-untracked documents into them. Nothing in this work was lost or altered by that; the only casualties were two numbers and this paragraph. It is the second numbering collision in this folder — Handover 17 caused the first, for the same reason — so treat concurrent sessions in one dated folder as a known hazard rather than a surprise.

Handover 17 was a decomposition proposal that deliberately stopped short of two things: the five decisions it reserved for the user (§7), and the assurance-spine artifacts it said must exist "before any code" (§9). **Both are now done.** This handover records the decisions, the paperwork, and the one honest gap — **no code was written, and none should have been**, because §9 is a precondition and §12's first step needs a serial adapter that has not been loopback-tested yet.

**On the folder date.** As Handover 16 records: this repository's document dates run one day ahead of the clock. Do not read a date here as evidence of when anything happened.

## The §7 decisions, as confirmed

Three were put to the user; two were mechanical consequences of the plan and are recorded rather than asked.

**§7.1 — ARM64 is the real-time tier. Confirmed.** Recorded as [`ADR 0004`](../../docs/adr/0004-arm64-is-the-real-time-tier.md). Worst-case latency bounds, WCET claims and jitter envelopes are stated and gated on ARM64 hardware; **x86_64 remains a full first-class target** for throughput, rich-workload, host-bridge and developer-experience claims, and remains the Tier 0 CI gate.

The argument is structural, not about speed: an SMI is entered by firmware above the kernel, cannot be masked, cannot be observed, and cannot be attributed. So an x86_64 worst-case bound is a claim about the firmware, not about TinyOS — and measurement does not rescue it, because an SMI that did not fire during a campaign is not an SMI that cannot fire. The ADR states the falsification condition too: a platform demonstrating bounded, attributable SMIs can be re-argued on its own evidence.

**Two consequences worth naming before someone trips on them.** `FEAT-P1-06` — this Epic's flagship, the deterministic-actuation `G-PA-1` path — states a *bound*, so under this ADR that bound is quotable from ARM64 hardware and a Tier 0 or x86_64 run of it is mechanism evidence rather than the bound. And **no existing number is retracted**: what the ADR forbids is *promoting* an x86_64 measurement into a worst-case bound. `LE-16`, `LE-18` and `LE-23` are untouched by it.

**§7.5 — `LE-15` resolved as the `PMCCNTR_EL0`/`CNTVCT_EL0` split. Confirmed.** The PMU cycle counter becomes the ARM64 `CycleSource` for microbenchmarks; `CNTVCT_EL0` stays the `Timebase`/wall-clock source. `TEST-P1-07-04-A` clause 3 carries the part that matters more than the decision: **if `PMCCNTR_EL0` traps or reads zero, the Story does not fail.** It falls back to `CNTVCT_EL0` with batched iteration and records a narrowed `LE-15` naming which register was unavailable and at what exception level — and the fallback path is exercised deliberately at least once, so it is tested rather than assumed. A bring-up Story that can only succeed is a bring-up Story that will be made to look successful.

**§7.4 — hardware evidence reaches CI by option (b). Confirmed.** Manual runs land in Reports; CI stays Tier 0. No infrastructure, nothing blocked, and the ratio baselines stay Tier 0 — so `LE-23` is exactly where Handover 16 left it.

**§7.2 — `README.md` reconciled.** Three edits: the claim-attribution paragraph from ADR 0004 in the hardware matrix preamble; the Pi 5 promoted from "a second, non-NVIDIA ARM64 board" to `EPIC-P1`'s first physical timing target; and line 147's "Raspberry Pi and real CAN/USB HIL rigs are added from Phase 3 onward" corrected in place with the reason the two were never one commitment — **the HIL rigs wait on the bus stack; the Pi 5 waits on nothing**, because the slice that reaches it needs no bus, no network and no drivers.

**§7.3 — the `EPIC-P1_5` transport decision is re-opened**, as `LE-26`. Not answered: `FEAT-P1-07` routes around it with SD swap plus the debug UART, and the question of what `EPIC-P1_5` actually deploys over on a Pi 5 is left standing where it can be seen.

## The loose ends renumbered, and why

Handover 17 §8 proposed `LE-25` and `LE-26`. **`LE-25` was already taken** — by the concurrent `STORY-P0-01-04` work (the `unhandled_interrupt_handler` cannot name the vector it caught), which was uncommitted when this session started and landed in `d0a2a60` while it ran. The register requires contiguous ids, so the plan's two became:

- **`LE-26`** — `EPIC-P1_5`'s recorded peer-to-peer Ethernet transport is not viable on a Pi 5 without PCIe and RP1 bring-up. Unowned, open. *(Handover 17 calls this `LE-25`.)*
- **`LE-27`** — the ARM64 `CycleSource`/`Timebase` shipped in `STORY-P1-01-03` have never executed on silicon, so host conformance is evidence about arithmetic, not about registers. Owned by `STORY-P1-07-04`. *(Handover 17 calls this `LE-26`.)*

Handover 17 is not edited — the session convention holds that dated folders are an immutable record — so **read its §8 numbering against this paragraph.**

## What now exists

| Artifact | Content |
|---|---|
| [`FEAT-P1-07`](../../goals/features/FEAT-P1-07.md) | The Feature, its containment contract, exit criteria and §6 non-goals |
| `STORY-P1-07-01`…`-06` | Six Story documents, `Specified`, in the plan's non-negotiable order |
| `TEST-P1-07-01-A`…`-06-A` | Six Test documents, **written before any implementation**, per the TDD mandate |
| `feature-contracts.tsv` | One row: impl **C0/C1**, subject **C0/C1**, **BND-01/-02/-03/-17**, **PD-07/-12/-14**, **RCG-01/-13/-14** |
| `story-contracts.tsv` | Six rows: `D01`, `D02`, `D08`, `D02+D03`, `D01`, `D02+D04+D05+D07`; all `specified` |
| [`ADR 0004`](../../docs/adr/0004-arm64-is-the-real-time-tier.md) | ARM64 as the real-time tier |
| `loose-ends.tsv` | `LE-26`, `LE-27` added |
| `README.md`, `EPIC-P1.md` | Reconciled as above |

`cargo run -p xtask -- check-assurance-spine` is **green**: 23 Features, 56 Stories, 43 Tests, 27 loose ends (17 open), 82 status headers.

Three choices in the contract are worth defending, because each is a place where a looser selection would have been easier and dishonest:

- **Subject classes stop at C1.** This slice runs no tasks, loads no image and creates no C2/C3/C4 domain. Claiming otherwise would import evidence obligations no Story here can discharge.
- **`SEC-01` is selected by `STORY-P1-07-01` and cannot be closed.** The Pi 5 firmware chain gives TinyOS no measured-boot evidence, so `BND-01` is stated debt for the whole Feature. It is named rather than omitted so no reader infers the question was considered and answered.
- **`BND-03` is the reason the device-tree non-goal exists.** "C1 contains no complex hostile-format parser" is not a nicety here — a real DT parser is a hostile-input parser and belongs behind the Charter's `C4` discipline, so the Feature hardcodes-and-verifies.

## One repair made in passing

`check-assurance-spine` was **red** when this session started, on uncommitted work: `goals/tests/TEST-P0-01-04-A.md` declared `BND-01/-02/-03`, `PD-01/-02` and `RCG-01/-02`, none of which matched `FEAT-P0-01`'s contract row. Corrected here to the Feature's actual selections (`BND-01/-02/-03/-17/-18`, `PD-02/-12/-13/-14`, `RCG-05/-06/-07/-12/-14`), and that correction went on to land inside the concurrent session's own commit (`49acf55`) — the tidiest outcome available, and recorded so nobody looks for it under this Feature. It is metadata drift in an unfinished document, not a finding about `STORY-P0-01-04` — but §9 requires the gate green before the first line of implementation, and it was not.

## What the first hardware session starts with

`FEAT-P1-07`'s §12 order stands, with step 2 already done. So:

1. **Loopback-test a serial adapter against a known-good source, before the board is ever blamed.** Buy two. A suspected-dead board is usually a dead adapter, and this is the only clause in the Feature that can be run before anything else exists.
2. **Verify every BCM2712 address and the expected baud against current documentation.** Pi 4 material is actively misleading for the Pi 5 — a larger departure than the version number suggests — and the debug UART is a dedicated 3-pin connector distinct from the GPIO header. `TEST-P1-07-01-A` clause 7 asks for divergences from Pi 4 sources to be recorded; that is the most reusable output the first session produces.
3. **Then `STORY-P1-07-01`**: target spec, boot stub, `CurrentEL` printed *before anything else*, one byte out of PL011.
4. **Then `-02`, before the MMU, always.**

**Three traps, named up front.**

**The ordering is not stylistic.** `-02` before `-03` and `-04` because on this board a fault with no vector table is a silent hang with no output whatsoever — indistinguishable from a dead adapter, a rejected image, or a board that never started. `-03` and `-04` are the two easiest things in the Feature to get subtly wrong and the first symptom of either is an exception.

**`STORY-P1-07-03` is the one that produces confidently wrong numbers if it is skipped or faked.** With `SCTLR_EL1.M` clear, every access is Device-nGnRnE and timing is not slow-but-proportional, it is meaningless. And a silently-ignored `SCTLR_EL1` write is indistinguishable from success in every respect except one — that the cached case is dramatically faster. `TEST-P1-07-03-A` clause 4's before-and-after paired capture **is** that Story; everything else in it is scaffolding around the one measurement that can tell the difference.

**The temptation this Feature is most exposed to is "just get it booting first, contracts after."** That is why §9 was done before any code, and it is worth restating why hardware is the *worst* place to yield to it: the numbers this Feature produces are the ones every later timing claim in the project will rest on.

## What has not changed

`LE-09` is **open**. Nothing in this handover is evidence — a decision is not evidence and a plan is not evidence, and neither is a contract row. `LE-09` closes on `STORY-P1-07-06`'s Report and nothing earlier.

Tier 0 is unchanged and green. `EPIC-P1` remains four Features of seven complete. No code was written this session.
