# Handover 31 — Scoping the `qemu-system-aarch64 -M virt` Fixture

A scoping document, not a Story and not an implementation. It exists because the question *"why
can't we simulate a Pi 5?"* has three different answers, and the third one found a real gap:
**there are zero ARM64 fixtures in a repository whose next four Stories are all AArch64.**

Nothing here is decided. §7 is the decision list.

## 1. What this is, and the one sentence that governs it

A Tier 0 fixture that runs TinyOS AArch64 code under `qemu-system-aarch64 -M virt`, so that
**mechanism** defects are found on a host instead of on a board.

> **It produces no timing evidence, closes no release gate, and does not touch `LE-09`.**

That sentence is the whole boundary. `LE-09` closes on `STORY-P1-07-06`'s Report and nothing
earlier, and this Story must be written so that no future reader can mistake a green `virt` run for
hardware evidence. QEMU TCG models no pipeline, no cache and no DRAM latency; a cycle count from it
is a statement about the translator and the host. `LE-42` demonstrated the consequence days ago —
the D09 accept path at 17.6–39.1× over budget with a p99 CV of 22–71% against `G05`'s ≤5%, meaning
**no stable verdict exists in that environment at all.**

## 2. Why it is worth doing anyway

Handover 26 states the governing economics: **the scarce resource is board time, not host time.**

The defect class this catches is the one that costs a board session and produces no diagnostic. The
canonical example is already written into `TEST-P1-07-02-A` clause 1: **a misaligned `VBAR_EL1` write
is architecturally ignored** — no fault, no error, the handler simply never runs. On silicon that is
indistinguishable from a dead adapter, a wrong `config.txt`, or a UART at the wrong address. Under
`virt` it is a fixture that fails in seconds with a reason.

Everything `STORY-P1-07-03` (MMU) and `-04` (timer) will write is in this class. Handover 26 trap 4
says those two are "the two easiest things in this Feature to get subtly wrong, and the first symptom
of either is an exception."

**A second, unplanned benefit.** The Pi 5 firmware enters at `EL2`, so the board exercises exactly
one entry path. `-M virt` can be started to enter at either `EL1` or `EL2`, which means
`boot::entry`'s `CurrentEL` decode and the `needs_drop_to_el1` branch — currently 64 host tests
against a *decoder*, never against a real `CurrentEL` — can both be executed. The board can never
test the `EL1`-entry branch at all.

## 3. The dependency nobody has written down

**Nothing in this workspace produces an AArch64 executable.**

- `hal-arm64` is `[lib]` only — no `[[bin]]`.
- `kernel`'s sole binary is `src/main.rs`, which is x86_64.
- `STORY-P1-07-01`'s linker script was validated by linking a throwaway binary **outside the
  workspace**, and `TEST-P1-07-01-A` records that as a layout check rather than as evidence.
- Handover 26 trap 7: *"There is no `kernel8.img` yet, and building one is not your Story."*

So this fixture needs an AArch64 binary crate, and that is currently inside `STORY-P1-07-05`'s scope
(target spec → binary → SD image). `-M virt` takes an ELF via `-kernel` and needs **no SD image**, so
the two pieces separate cleanly:

| Piece | Needed by `virt` | Needed by the board |
| --- | --- | --- |
| AArch64 binary crate + entry | yes | yes |
| SD-card image packaging, `config.txt` | **no** | yes |

**This is a sequencing argument, not an obstacle.** Building the binary crate for `virt` first
de-risks `-05` rather than duplicating it: the half that both need gets exercised on a host before
the half only the board needs is attempted. Whoever writes this must decide explicitly whether the
binary crate belongs to this Story or to `-05`, and record it — silently absorbing `-05`'s scope is
the failure mode.

## 4. Placement: not a seventh `FEAT-P1-07` Story

`FEAT-P1-07` has six Stories, and Handover 26 trap 6 is explicit: **a seventh means re-decomposing,
not extending.** That boundary was written to stop `hal-arm64` growing into a HAL port, and it should
hold here even though this work is sympathetic to the Feature's goals.

The Feature's *Explicit non-goals* list does not forbid a QEMU fixture — it forbids RP1/PCIe, address
spaces, preemption, an SD driver, multi-core, a device-tree parser, and CI on hardware. So this is
not a violation of the letter. It is a violation of the shape.

**Recommendation: `FEAT-P0-01`, as `STORY-P0-01-07`.** The precedent is direct and recent —
`STORY-P0-01-04` (nine fixtures with no CI step), `-05` (the guardrail-evidence register), `-06` (the
D09 disposition) are all harness-and-discipline Stories added to `FEAT-P0-01` long after it was
Verified. **This is a fixture-harness Story**: it extends where the harness can run, not what the
kernel can do.

The counter-argument, stated fairly: it touches `hal-arm64` for the board definition, and that crate
is `FEAT-P1-07`'s. Whoever takes the decision should weigh that rather than treat §4 as settled.

## 5. The board-definition problem

`hal-arm64/src/board.rs` is Pi 5 constants, hardcoded and documented as such, with `BND-03` and
`FEAT-P1-07` §6 forbidding a device-tree parser in this slice. `virt` has a different PL011 base and
a different reference clock, so a second board definition is required.

**Options, with a recommendation:**

| Option | Assessment |
| --- | --- |
| Cargo feature selecting a `board` module | **Recommended.** Compile-time, zero runtime cost, matches Non-Negotiable #12 (absence unless opted in), and a `virt` build cannot accidentally ship Pi 5 constants or vice versa |
| Runtime detection | Rejected — needs a device-tree parser, which `BND-03` forbids in this slice |
| A `Board` trait with two impls | Defensible, but adds a generic parameter to a crate whose whole discipline is that it stays small; revisit if a third board appears |

**Whatever the mechanism, the constants themselves must follow `board.rs`'s existing discipline:**
every value carries its source and revision, and is marked as a transcription until something
executes. `virt`'s PL011 base and clock should be taken from the QEMU source or from
`-machine virt,dumpdtb=` output and cited — **not from memory and not from this document.**
Handover 23 exists because a transcription that is wrong fails as silence.

## 6. The Red

`TEST-P0-01-07-A`, written before implementation. Draft clauses, for the Story author to sharpen:

1. **The fixture runs and exits cleanly.** An AArch64 binary boots under `-M virt`, writes a known
   byte sequence over `virt`'s PL011, and exits with a success code the harness recognises — the
   envelope discipline the x86_64 fixtures already use.
2. **`CurrentEL` is read from the register, not decoded from a constant.** Run at both entry levels;
   the `EL2` run must report the drop to `EL1` and the `EL1` run must report `already-el1`. This is
   the branch the board can never exercise.
3. **A deliberately-triggered fault reaches the handler and reports a decoded `ESR_EL1`.** This is
   the clause that pays for the Story, and it must **prove it can fail**: a run with a deliberately
   misaligned `VBAR_EL1` must produce silence-then-timeout rather than a pass, exactly as the `.org`
   guard was padded past 128 bytes before it was trusted.
4. **The fixture is registered and CI runs it.** `list-fixtures` names it with its owning `TEST-*`,
   and the bidirectional drift guard `STORY-P0-01-04` built covers it — because that Story found
   nine of twenty-three fixtures with no CI step, and adding a tenth silently would be the same
   defect.
5. **Nothing here is timing evidence.** No `PERF-*` gate is filed from a `virt` run, and the Story's
   Report states this in its own words rather than by omission.

Clause 3 is the one to resist softening. A fault path tested only against code that does not fault is
not a test — `TEST-P1-07-02-A` says so in bold, and it is the sharpest sentence in that document.

## 7. Decisions this scoping does not take

1. **Placement** — `FEAT-P0-01`/`STORY-P0-01-07` (recommended) versus re-decomposing `FEAT-P1-07`.
2. **Who owns the AArch64 binary crate** — this Story, or `STORY-P1-07-05` with this Story depending
   on it. §3 argues for the former on de-risking grounds; it is still a choice.
3. **Whether a `virt` fault run can satisfy `TEST-P1-07-02-A` clause 2.** **My reading is that it
   cannot** — clause 2 is a *board* clause and Handover 26 trap 2 is emphatic — but it is not mine to
   decide, and if the Story author reads it differently that reading goes in the Test document under
   its own heading with the reason, the way `-01` handled its unsatisfiable constraint. Do not
   quietly widen it.
4. **Contract selections.** A row is needed in `story-contracts.tsv`. `D01` (boot and topology
   discovery) is the obvious performance domain, but note `LE-35`: selecting a domain pulls all 25
   guardrails in, and this Story closes none of them. Whatever is selected must be initialised as
   **stated open debt**, which is exactly the rule `LE-35` says has never been written down. **This
   Story is a good forcing function for that rule.**

## 8. Risk, stated plainly

- **The largest risk is misinterpretation, not implementation.** A green ARM64 fixture in CI will
  look, to anyone not reading carefully, like the ARM64 coverage the README falsely claimed until two
  commits ago. §1's sentence needs to appear in the Story, the Test, the Report and the fixture's own
  description — four places, deliberately.
- **Scope creep toward a HAL port.** `virt` has a GIC, a timer and virtio devices, all of which are
  easy to reach for and none of which this Story needs.
- **Effort:** small-to-moderate. The binary crate and the board-selection mechanism are the real
  work; the fixture harness, the envelope and the CI step all follow patterns that already exist and
  have been exercised twenty-three times.

## 9. One piece of drift found while scoping

`hal-arm64/Cargo.toml`'s `description` still reads *"generic-timer cycle source and timebase only —
no boot path, no UART, no MMU."* The crate now has a boot path, a PL011 driver, exception vectors,
`ESR` decoding and a fault path. Not registered as a loose end — it is a one-line fix for whoever
next touches that manifest, and registering a row for it would be heavier than the repair.
