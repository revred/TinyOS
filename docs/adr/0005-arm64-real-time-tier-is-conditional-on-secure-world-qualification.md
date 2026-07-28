# ADR 0005 — The ARM64 Real-Time Tier Is Conditional on Per-Platform Secure-World Qualification

Status: **Accepted**
Date: 2026-07-28
Supersedes: [`ADR 0004`](0004-arm64-is-the-real-time-tier.md)
Introduced in: [`session/hand-2026-07-28/33-two-decisions-settled.md`](../../session/hand-2026-07-28/33-two-decisions-settled.md), settling `LE-39`
Raised by: an external expert audit recorded in [`29-next-session-mandate.md`](../../session/hand-2026-07-28/29-next-session-mandate.md) §1, answering the question [Handover 28](../../session/hand-2026-07-28/28-analysis-response-and-le-33.md) put to the reviewer

## Context

[`ADR 0004`](0004-arm64-is-the-real-time-tier.md) made ARM64 TinyOS's real-time tier of record. Its
argument is in two halves, and only the first half survives.

**The half that survives** is the disqualifying argument against x86_64. System Management Interrupts
are entered by firmware above the kernel, cannot be masked, observed, or attributed by the OS, and
are therefore outside its authority by design. Any worst-case bound TinyOS states on x86_64 is a
claim about the firmware, not about TinyOS. Better OS engineering cannot repair it, and measurement
cannot either — an SMI that did not fire during a campaign is not an SMI that cannot fire. Nothing
below weakens this. It is restated in full in the Decision section because `ADR 0004` is superseded
and this document must be readable alone.

**The half that does not survive** is the sentence the ADR leaned on to elect a replacement:

> Interrupt masking at `EL1` means what it says.

Against a GIC configured with secure interrupt groups, it does not. The mechanism, stated
architecturally rather than per-board:

- A GIC signals interrupts belonging to a secure group as FIQ. `SCR_EL3.FIQ` (and `SCR_EL3.IRQ`,
  `SCR_EL3.EA`) route those exceptions to `EL3` rather than to the currently executing exception
  level. When routing to `EL3` is configured, the exception is taken at `EL3` **irrespective of
  `PSTATE.I` and `PSTATE.F` at NS-EL1** — masking at `EL1` masks nothing that firmware has claimed.
- The secure handler runs for a duration NS-EL1 cannot observe, cannot bound, and cannot attribute.
  The only signal available to NS-EL1 is elapsed time it did not spend: an unexplained advance in
  `CNTPCT_EL0` relative to work completed.
- It also perturbs microarchitectural state NS-EL1 depends on — I-cache, D-cache, TLB, branch
  predictors — so the cost is not confined to the residency itself.
- NS-EL1 cannot even read the configuration. `SCR_EL3` is not accessible below `EL3`. A non-secure
  kernel therefore cannot enumerate, from its own vantage point, what has been routed away from it.

**That is structurally the same hole `ADR 0004` disqualifies x86_64 for**: invisible, unmaskable from
the OS's exception level, unattributable. Discovering it does not make `ADR 0004`'s conclusion wrong,
but it makes `ADR 0004`'s conclusion *unearned as stated*, and a real-time tier elected on an
unearned premise is the first thing an outside reviewer of a safety claim will find.

### The asymmetry that keeps ARM64, and why it is not special pleading

If the two architectures had the same defect, the honest conclusion would be that TinyOS has no
real-time tier at all. They do not, and the difference is specific:

| | x86_64 / SMM | AArch64 / secure interrupt groups |
|---|---|---|
| Entry mechanism | Undocumented per-vendor, not architecturally enumerable | Architecturally specified: GIC groups, `SCR_EL3` routing, `EL3` vectors |
| Sources | Not enumerable by the OS or, in general, by anyone outside the firmware vendor | A finite, per-platform set fixed by firmware and GIC configuration |
| Observability of residency | None the architecture guarantees | Elapsed-time divergence is measurable from NS-EL1 by construction |
| Can a platform be argued clean on evidence? | Only where server-class firmware happens to expose SMI counters and quiescence modes — the falsifiability carve-out `ADR 0004` already granted | Yes, per platform, from firmware configuration plus a residency campaign |
| Is the property a *platform* property or an *architecture* property? | Architecture-wide by design | **Platform-wide, not architecture-wide** |

The last row is the decisive one. On x86_64 the defect is a property of the architecture and its
firmware model, so no amount of per-machine work retires it in general. On AArch64 the defect is a
property of *how a specific platform's firmware configured a specific GIC* — a bare-metal AArch64
board with no secure interrupts routed to `EL3` has no such hole at all, and one cannot tell the two
apart without looking.

So the repair is not to pick a different architecture. It is to stop treating the tier as a property
of the instruction set and start treating it as a property of a qualified platform.

## Decision

**1. x86_64 remains disqualified from carrying worst-case bounds**, on `ADR 0004`'s unmodified
argument, with `ADR 0004`'s unmodified falsifiability carve-out: a specific x86_64 platform that
demonstrates a bounded, attributable SMI mechanism may be re-argued on its evidence. x86_64 is
otherwise unchanged — a full first-class target for throughput, rich-workload, host-bridge and
developer-experience claims, and the Tier 0 CI gate. Nothing is de-scoped.

**2. ARM64 is no longer the real-time tier automatically. A *qualified ARM64 platform* is.**

A worst-case latency bound, WCET claim, jitter envelope, or any `G-RT-*` / `G-PA-*` guarantee may be
quoted only from an ARM64 platform holding a current **secure-world qualification record** (below).
An ARM64 platform without one produces **mechanism evidence**, exactly as Tier 0 and x86_64 runs do —
real, useful, retained, and not a bound.

**3. The default for an unqualified platform is "not qualified", never "presumed clean."** Silence is
not evidence. This includes every ARM64 platform in this repository today: as of this ADR, **the
count of qualified platforms is zero**, and the Raspberry Pi 5 is not one of them. It cannot be — at
the time of writing this repository has not yet read the board's `current_el=` line
([`STORY-P1-07-01`](../../goals/stories/STORY-P1-07-01.md)), so its entry exception level is still an
input rather than a fact, and its secure-world configuration is a fortiori unknown.

## What a secure-world qualification record contains

A qualification record is a dated Report under [`goals/reports/`](../../goals/reports/) naming one
platform at one firmware version. It carries four things. **Q1 and Q2 are declarations; Q3 is a
measurement campaign; Q4 is the statement of what the campaign does not prove.**

**Q1 — Platform identity, exactly.** SoC, board revision, firmware component and version (on the Pi 5:
the `start*.elf`/`bootcode` firmware revision and any `config.txt` settings affecting boot, including
`os_check`), the exception level TinyOS is entered at, and the GIC generation and configuration as
far as it is determinable. A qualification record is void for any other firmware version. Firmware
updates are not neutral to a real-time claim, and this is the row that makes that visible.

**Q2 — The secure-world configuration, and how it was determined.** What is routed to `EL3`, from
firmware documentation, firmware source, or vendor statement. **Where the firmware is closed and this
cannot be determined, the record says so in those words.** An undeterminable Q2 does not block
qualification — it bounds what qualification means, and Q4 must carry that limitation forward.

**Q3 — A residency campaign, from NS-EL1, with a stated duration.** The measurable signal is elapsed
time that TinyOS did not spend: divergence between the physical counter (`CNTPCT_EL0`) and work
NS-EL1 can account for, over a tight, cache-resident, interrupt-masked loop of known length.
`PMCCNTR_EL0`-versus-`CNTPCT_EL0` divergence is the sharper form of the same signal where the PMU is
accessible from NS-EL1 — which is the counter split
[`STORY-P1-07-04`](../../goals/stories/STORY-P1-07-04.md) already has to decide for `LE-15`, so this
is a second consumer of a decision the project is making anyway, not a new mechanism. The record
states: campaign duration, sample count, the largest excursion observed, the distribution's shape,
and the environmental conditions (thermal state, USB and network activity, display attached or not),
because a secure handler that fires on a thermal event is not exercised by an idle board.

**Q4 — The refusal to over-claim, in the record's own words.** An excursion not observed is not an
excursion that cannot occur. This is `ADR 0004`'s own sentence against x86_64 measurement, applied
here to ourselves; a qualification record that omits it is incomplete. What a qualified platform
licenses is quoting a bound *at the confidence the campaign supports and no further* — with the
campaign, its duration, and its conditions cited alongside the number, never the number alone.

**A platform qualifies when Q1–Q4 exist and the largest observed excursion is inside the bound being
claimed.** It does not qualify by having a campaign that found nothing: see the trap below.

## Rationale

- **Safety before security before correctness before performance.** Unchanged, and the reason this
  ADR exists. A real-time guarantee whose worst case is set by a third party's firmware is not a
  guarantee — and that sentence indicts a Pi 5 with unqualified secure interrupts exactly as it
  indicts an x86_64 machine with SMIs. Applying it to only one of them was the defect.
- **It applies `ADR 0004`'s own falsifiability test to `ADR 0004`'s own choice.** `ADR 0004` granted
  x86_64 a route back on evidence. It gave ARM64 the conclusion for free. A test worth stating is
  worth passing.
- **The cost is also the moat, and this should not be read as a retreat.** Per-platform qualification
  evidence — dated, versioned against firmware, campaign-backed — is precisely what commercial RTOS
  and safety-certification vendors charge for, and it is what a customer deploying into a data
  centre, a UAV, a medical device, or an industrial cell actually needs. TinyOS holding a
  qualification record per platform is a stronger commercial position than TinyOS holding an
  architecture-wide assertion, because the assertion is the thing a competent buyer's reviewer would
  puncture in the first meeting. The work this ADR adds is work that was always owed; what changes is
  that it is now named and scheduled instead of assumed.
- **It is falsifiable in both directions.** A platform that qualifies can be dequalified by a
  firmware update or a longer campaign; a platform that fails can qualify later on a different
  firmware version or a different configuration. Neither direction requires amending this ADR.
- **The alternative that fails silently is worse than the cost.** Without this, the first ARM64
  measurement campaign produces a number, the number becomes a bound, and the bound is wrong in a way
  no gate in this repository can detect — `LE-33`'s failure mode, on the tier that was supposed to be
  the safe one.

## Consequences

- **No existing measurement is invalidated, retracted, or reinterpreted.** Like `ADR 0004`, this ADR
  constrains what may be **promoted** into a bound; it does not touch the measurements themselves.
  Every Tier 0 number stays Tier 0 and stays valid. `LE-23`, `LE-18` and `LE-16` are unaffected.
  **If applying this ADR leads you to retract a Tier 0 number, you have misread it.**
- **`ADR 0004` becomes `Superseded`, and is not edited otherwise.** `README.md`, `EPIC-P1`, and the
  Handover series cite it; rewriting its body would rewrite what those documents were pointing at.
  It keeps its argument, its date, and its status header gains a forward pointer here.
- **`LE-33`'s enforcement lint gains a second condition.** Its first condition refuses a `G04`-class
  bound sourced from x86_64 or Tier 0. Its second refuses a `G04`-class bound sourced from an ARM64
  platform with no qualification record — which means the `TINYOS-MEAS/1` envelope must carry a
  platform identity and a qualification-record reference, not only an architecture and a tier. That
  is an addition to `LE-33`'s scope, recorded on its row, not a new loose end.
- **`FEAT-P1-07` gains an obligation and loses none.** Its six Stories are unchanged. What changes is
  that [`STORY-P1-07-06`](../../goals/stories/STORY-P1-07-06.md), the measuring Story, produces
  mechanism evidence and the *first* Q3 campaign rather than a bound, and closing `LE-09` gives the
  project a hardware tier — not, by itself, a quotable worst-case bound. Whether the qualification
  record is `-06`'s scope or a seventh Story is a decomposition decision for `FEAT-P1-07` §6 and is
  deliberately **not** settled here; this ADR states the requirement, not the work breakdown.
- **`LE-09` is unchanged in shape and is now visibly insufficient in a second way.** It already was
  not sufficient for 24 of `D09`'s 25 gates. It is now also not sufficient for a worst-case bound on
  the very board that closes it.
- **The `-M virt` fixture is unaffected and stays unaffected.** It produces no timing evidence by
  design ([Handover 31](../../session/hand-2026-07-28/31-qemu-virt-fixture-scoping.md)), so it cannot
  produce a bound and there is nothing here for it to satisfy. A QEMU `-M virt` guest is also not a
  qualifiable platform: its secure-world configuration is the emulator's, not a product's.
- **The Jetson Orin Nano's role is unchanged**, and it becomes the natural second qualification
  record. Two qualified platforms is the point at which this ADR's per-platform framing pays for
  itself, because a second board is where an architecture-wide assertion would first have been
  silently wrong.
- **`SeedMVP.md`'s founding intent is untouched**, and neither this ADR nor `README.md` may weaken
  [`SECURITY_CHARTER.md`](../../SECURITY_CHARTER.md). Nothing here does.
- **This ADR asserts nothing about secure-interrupt residency magnitudes on any specific board**, and
  in particular asserts nothing about the Raspberry Pi 5. It rests on the structural property, which
  holds regardless of what any campaign measures. No measurement in this repository is offered in
  support of it and none is needed for the argument as stated. (This is the same discipline
  `ADR 0004` applied to SMI magnitudes, and the same one `LE-33`'s originating session had to
  re-assert when an external reviewer quoted a magnitude back at us that the ADR had deliberately
  refused to assert.)

## Alternatives considered and rejected

- **Mandate bare-metal `EL3` — run TinyOS as the secure monitor.** This closes the hole completely and
  is the technically strongest answer. It is rejected because **the Raspberry Pi 5's firmware owns
  `EL3` and cannot be displaced**, so mandating it makes the real-time claim unreachable on the only
  board in hand and on every consumer-class board like it. A rule that cannot be satisfied on the
  target hardware is not a safety rule; it is a way of not having a real-time tier while appearing to
  have a strict one. It remains available as a *platform-specific* route to a very strong Q2 on
  hardware where `EL3` is ours, and a platform qualified that way should say so.
- **Silently amend `ADR 0004`.** Rejected: `README.md`, `EPIC-P1` and several Handovers already cite
  it, and editing a cited document changes what those citations mean without their authors' knowledge.
  Supersession is the mechanism that leaves the record intact.
- **Drop ARM64 as the real-time tier.** Rejected: it treats a platform-level defect as an
  architecture-level one, which the asymmetry table above shows it is not, and it would leave TinyOS
  with no real-time tier while the actual repair is available.
- **Accept the risk and keep `ADR 0004` as written.** Rejected on the priority ordering. This is the
  option under which the first ARM64 bound ships wrong and nothing in the repository notices.
- **Treat `PSTATE.I` masking as sufficient because no secure interrupts are believed to be
  configured.** Rejected: this is the unsound premise itself, restated as a belief about a platform
  nobody has looked at.

## The trap this ADR sets, named up front

**A residency campaign that observes nothing is the most dangerous possible result**, because it
reads as qualification and is the cheapest thing to obtain. A detector is exercised before its
nothing is believed — the discipline stated in
[`32-next-session-mandate.md`](../../session/hand-2026-07-28/32-next-session-mandate.md) §"Traps"
trap 3, arrived at by the session that verified two external findings and found neither survived
contact with the file it described.

Concretely: a Q3 campaign is not admissible unless the same instrumentation has been shown to
*detect* a known perturbation — an injected interrupt, a deliberate cache flush, a synthetic stall of
known length. A zero from an instrument never shown to produce a non-zero is not a measurement of
zero. It is an absence of measurement, and the two are indistinguishable in the Report unless the
positive control is recorded alongside.
