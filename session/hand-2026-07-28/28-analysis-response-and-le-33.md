# Handover 28 — Disposition of the External Comparative Analysis: One New Loose End, Two Sharpened, One Declined

Written against an external analysis of TinyOS covering RTOS/hardened-Linux/microVM/edge-inference
positioning plus a six-item risk taxonomy. This handover is the **disposition** of it, in the shape
[Handover 20](20-swot-response.md) established for the session SWOT: every item is fixed, registered,
or declined with a reason. An unactioned analysis is a document that made everyone feel reviewed.

**A session ran concurrently with this one.** `STORY-P1-07-02`'s host half was landing in
`os/src/hal-arm64/` (`esr.rs`, `fault.rs`, `vectors.rs`) and claimed slot 27 while this work was in
progress. Nothing here touches `hal-arm64`, `goals/features/FEAT-P1-07.md`,
`goals/stories/STORY-P1-07-02.md` or `goals/tests/TEST-P1-07-02-A.md`. Stage narrowly.

## The headline judgement

**The analysis names no risk this repository had not already registered.** All six of its risks map
onto existing artifacts with existing owners — `ADR 0004`, `D17`/`D25`/`G19`, `G-PC-2`, `G-AI-5`, the
Universal Driver Model, `D24`/`EPIC-P1_5`. That is a good sign about the analysis and a better one
about the spine.

What it *did* produce is more valuable than a new risk: it read our own documents back to us
slightly wrong, and each place it did so is a place where a correct decision has no machine behind
it.

## Corrections the analysis needed (recorded so they do not propagate)

1. **"14 Protection Domains (C0–C4)"** conflates two orthogonal structures — 14 `PD-*` invariants
   *and* 5 containment classes with a 25-pair matrix. Merging them makes the security model sound
   smaller than it is.
2. **"TCB fits within 8 MB"** is not measured and not claimed. 8192 KiB is `D25`'s *total image*
   budget at `design` readiness — a provisional engineering budget.
3. **"SMI latencies of 50–500 µs"** is a magnitude `ADR 0004` deliberately refused to assert. Its
   closing note: *"This ADR asserts nothing about SMI magnitudes on any specific machine."* The
   argument is structural — invisible, unmaskable, unattributable — precisely so it does not depend
   on a number anyone can dispute. Attaching a number **weakens** it.
4. **Risk 6 cites a transport already known stale.** `LE-26`: on a Pi 5, Ethernet/USB/GPIO sit behind
   RP1 over PCIe. And `EPIC-P1_5` is a *backlog row* — A/B partitions and watchdog rollback are
   planned, nothing implemented.
5. **"137 tests passing"** is the `xtask` crate alone. Workspace total measured this session:
   **549 passing** (498 at Handover 26's close; the delta is the concurrent `hal-arm64` work).
6. **Jetson Orin Nano is not in `FEAT-P1-07`.** Its §6 boundary excludes PCIe, RP1, Ethernet, USB,
   GPIO and multi-core. Listing Jetson bring-up as current debt implies scope deliberately excluded.

## Registered

### `LE-33` — `ADR 0004` has no machine behind it (new)

`ADR 0004` decides that x86_64 cannot carry a worst-case latency bound. Nothing enforces that
decision. A Report can quote an x86_64 or Tier 0 QEMU number as a `G04`-class observed-maximum or
WCET bound and **every gate in this repository stays green.**

The ADR states the prohibition in prose; prose is weaker than a gate. This is `LE-28`'s failure mode
in a second place — a correct decision with nothing mechanical behind it. Owner shape named in the
row: carry the measuring architecture and tier in the `TINYOS-MEAS/1` envelope, and have
`check-performance-catalogue` or a Report lint refuse a `G04` bound sourced from x86_64 or Tier 0.

### `LE-29` — the worked instance it was missing

`LE-29` argued in the abstract that declared-but-never-exercised has never been asked of the 625
performance cells. The analysis's Risk 2 supplies the concrete case, and **the numbers are worse
than the analysis or the reviewing session first stated**:

- `D25` is selected by **eleven** Stories. `PERF-D25-G19` — isolation under 90% competing inference,
  network, driver and memory load — is therefore a contracted obligation eleven times over with **no
  evidence behind any of them**.
- `D17` (GPU UMM and admission) is selected by **zero** Stories. The CPU/GPU memory-bus contention
  risk has a home in the catalogue that nothing points at.

Those are two *different* failure modes — a declared cell nobody measured, and a domain nobody
selected — and the bidirectional guard `LE-29` calls for must catch both. The row now says so.

### `LE-31` — first pass done, and it changes the conclusion

The audit was run mechanically over `story-contracts.tsv` against the domain tier table. It confirms
Handover 22's nine, and adds the finding that matters:

| Story | Domains | Tier | Blocked by |
| --- | --- | --- | --- |
| `P0-02-03` | D06 | Host+T0+HIL | HIL |
| `P0-03-01` | D07 | Host+T0+HIL | HIL |
| `P0-03-03` | D07 | Host+T0+HIL | HIL |
| **`P0-05-01`** | **D09** | **Host+T0** | **nothing** |
| `P0-06-01` | D11 | Host+T0+HIL | HIL |
| `P0-06-02` | D11 | Host+T0+HIL | HIL |
| `P0-06-03` | D06, D11 | Host+T0+HIL | HIL |
| `P0-07-01` | D12 | Host+T0+HIL | HIL |
| `P0-07-02` | D13 | Host+T0+HIL | HIL |

**HIL is not the Pi 5.** The HIL rigs are CAN/USB hardware-in-the-loop, deferred to Phase 3 once the
bus stack exists (`README.md`'s test matrix). So **closing `LE-09` will not move eight of these
nine.** The attribution is not merely wrong today — it stays wrong after `LE-09` closes. That is a
stronger statement than Handover 22 was able to make, and it is now in the row.

`STORY-P0-05-01` remains the sole candidate needing no hardware purchase: `D09` alone, tier
`Host+T0`, no HIL row at all.

## Declined, with a reason

**Re-opening `LE-26` was proposed and is a no-op.** The proposal was to record that Pi 5
deploy-over-Ethernet is blocked on RP1/PCIe and that `FEAT-P1-07` routes around it with SD-card image
swap plus debug UART, leaving the `EPIC-P1_5` transport decision unowned. `LE-26` already says
exactly that, in those terms, with `ownership=unowned`. Rewriting a correct row to match a proposal
it already satisfies adds churn and costs the row its history.

## Also landed

`docs/competitive-position.md` — the analysis's sections 1–4 captured with every comparative
assertion tagged against `G24`/`G25` as claim-gated and unearned. Sections 1–4 as originally written
would have breached those gates outright ("first-of-its-kind"; a latency claim about Qubes/Xen with
no same-hardware data). Its `C0`–`C4` block now **quotes `containment-classes.tsv` verbatim** rather
than paraphrasing, because a paraphrase that drifts from an exactly-asserted catalogue is
indistinguishable from a charter amendment.

## Verification

```text
check-assurance-spine        green — 33 loose ends (22 open), was 32 (21)
check-performance-catalogue  green — 625 cells (25 x 25)
cargo test -p xtask          137 passed
cargo test --workspace       549 passed
```

## What the next session should take from this

The two new registrations point the same way, and it is the same direction `LE-28` pointed:
**this project's decisions are sound and its enforcement is uneven.** `ADR 0004` is a good decision
with no gate. `G19` is a good guardrail selected eleven times and measured zero. `LE-29` is the
generalisation of both, and it remains the highest-value non-hardware work in the register —
now with two worked instances instead of one hypothetical.

`STORY-P0-05-01` is still the single cheapest thing on the board: one Story, one domain, no hardware,
and it would move `Stories verified` off zero for the first time.
