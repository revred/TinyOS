# 01A — The Decomposition Is Code Now, and the First Instruction It Refutes

Session handover, written 2026-08-06. Follows
[`09A`](../hand-2026-08-05/09A-the-assurance-release-status-and-the-two-hundred-gates-nobody-is-blocked-on.md).

**`09A` closed with a five-step mandate and an instruction for whoever wrote next: *do not report
`x/460` without decomposing it.* This session did the decomposition in code, which is `LE-84`, and
the code's first act was to refute step 1 of the mandate that asked for it.** That is the shape of
this document: most of `09A` executed, one of its instructions withdrawn on evidence, and the
withdrawal is the part worth reading.

---

## 1. `LE-84` is closed: `cargo run -p xtask -- assurance-status`

`09A` §10 argued the release-status analysis *"should not be ad hoc at all"* and filed the row
rather than promising it in prose. The subcommand now exists, reads only committed TSVs, and
prints §1's ledger derived rather than quoted:

```text
release gates in play (20 in-play domains x 23 release guardrails)   460
|
+- in the 10 domains whose SUBSYSTEM DOES NOT EXIST                  230
|     D02 unbuilt | D10 stand-in-only | D12 specified | D13 specified | D14 stand-in-only
|     D20 design | D21 design | D22 design | D23 design | D25 design
|     -- this bucket already CONTAINS:
|          all 46 hardware-only (T1/T2) gates in play
|          10 of the 20 G04 bound-class gates
|
+- in the 10 IMPLEMENTED in-play domains                             230
      D01 prototype | D03 partial | D04 prototype | D05 prototype-cooperative | D06 prototype
      D07 prototype | D08 prototype-inactive | D09 prototype | D11 prototype | D24 partial
   +- G04, barred by ADR 0005 while 0 platforms are qualified         10
   +- needing a board (T1/T2 with no Host or T0 tier)                  0
   +- not barred by G04, by the board, or by an absent subsystem      220
      +- carrying evidence                                            21
      +- carrying nothing                                            199
         +- mechanism not built (declared, see source)                70
         +- unmeasured, and MEASURABLE TODAY                         129
         (of the empty, 1 carries a reasoned REFUSAL rather than silence)
```

**Nested, never subtracted.** `09A` §10 recorded that its own first printing produced 164 where
the answer is 220, because three buckets overlapped and a column of subtractions cannot say so.
Here every parent is the sum of its children and a test asserts it at each level, so that class of
error is unavailable rather than merely unlikely.

Four things it derives that were previously assertions:

- **`09A` §3's headline is now a test.** `no_implemented_in_play_domain_has_a_gate_only_a_board_
  could_move` asserts `implemented.hardware_only == 0`. If it ever becomes non-zero the board is
  genuinely blocking a closable gate and §3 stops being true — which is the point of asserting a
  claim rather than repeating it.
- **The decomposition is pinned to the dashboard.** It parses `story-contracts.tsv` and
  `guardrail-evidence.tsv` itself, so a test holds its totals to the authoritative spine walk.
  Two readers of one file that merely resemble each other are how `LE-83` happened.
- **The shape is pinned; the evidence figures are not.** Pinning numbers that are *supposed* to
  move is how a pin stops being read and starts being updated reflexively. What is pinned instead
  is a ratchet: evidence may not go backwards.
- **The output states its own two caveats** — that the mechanism-not-built split is a *declared*
  list of guardrails rather than a derived fact, and that a gate carrying evidence is one someone
  measured, never one that passed.

**What it cannot do, and this is the honest limit.** `readiness` is per *domain*; the register has
no per-guardrail readiness column. So the ~70 gates that are *unevidenced because unbuilt* are
identified from a hand-declared list of seven guardrails (`G13`–`G16`, `G19`, `G21`, `G22`) in the
module source. That is a judgement in one reviewable place instead of re-made by each session, and
the output says so — but the real fix is a catalogue column, and nobody should read the 70 as
derived.

## 2. The instruction that did not survive being checked

`09A` §8 step 1 said, of things whose evidence already exists:

> *`G09` against the 8 MB image ceiling CI already computes […] This is registration, not
> measurement.*

**It is not.** `G09`'s `target` column reads *"feature code plus read-only data delta <= 96 KiB"*
for `D01`, `<= 16 KiB` for `D03`, `<= 24 KiB` for `D05`, and so on per domain. `check-image-size`
measures the **whole `os` image** against an 8 MiB ceiling and names a different gate entirely —
`G-DX-8`. A per-feature code-and-rodata delta and a whole-image ceiling are not the same
measurement, are not in the same units, and differ by three orders of magnitude.

So `G09` is **not** registration work. It is measurement work nobody has done, and doing it needs
per-feature section attribution that does not exist yet.

**Which is `09A` §5's own finding, committed by `09A` itself, in the instruction telling the next
session to avoid it.** §5's rule is *a measurement taken without first reading the gate's `target`
column is a measurement that will need retaking*; step 1 proposed filing a measurement against a
gate whose target it had not read. Filed as `LE-86`, because a defect found in a mandate is worth
more than the mandate.

The `G23` half of step 1 was sound and is done — see §3.

## 3. The register can now say "measured, read against the target, and refused"

`09A` §11 action 2 and `LE-85`. The `evidence_kind` column accepted **any string** until this
session; it is now a closed vocabulary of `structural`, `measured` and `refused`, with a test
proving a typo is rejected rather than silently counted as a novel kind.

`refused` is the new state and it is **excluded from the published count by construction**.
`STORY-P1-06-01` measured `PERF-D03-G20`, found 55% run-to-run p99 CV, and declined the filing in
Report prose — where no gate could see it, and where `09A` §8 step 1 executed mechanically would
have re-filed it or re-derived the refusal from scratch. That refusal is now a row.

Two rows were filed. The published figure moved **20 → 21, not 22**:

| Row | Kind | Counts? |
|---|---|---|
| `PERF-D07-G23` / `STORY-P1-10-02` | `measured` | yes — two of the gate's four clauses |
| `PERF-D03-G20` / `STORY-P1-06-01` | `refused` | **no** |

The `G23` row is deliberately partial and says so in its note: `allocations = 0` is
compiler-enforced and `torn records = 0` was observed on silicon (160 records, 0 refused, 0 lost,
sequence unbroken), while **both ratio clauses were not computable from any measurement this
project had taken** — every spoor cost on record is absolute and `G23`'s target is a percentage.

**Filing a partial row moves a headline number for a gate that cannot pass, and that deserves the
objection it invites.** It is not `LE-83`'s defect: that was a numerator inflating while covering
nothing new. This covers a gate nothing covered before, with real evidence, and the register's
unit has always been *a gate carrying evidence*, never *a gate that passed*. But a reader is owed
the distinction rather than left to find it.

## 4. `G23`'s missing arm now exists

`09A` §8 step 2, done. `kernel::measure_phases::phase_pool_alloc_free_batched_spoored` is
`phase_pool_alloc_free_batched` with one spoor stamp per round trip inside the timed region and
nothing else changed, registered in both the x86_64 and AArch64 measure fixtures as a new metric.
A reader computes `G23`'s ratio from the two rows; the fixture emits measurements and never
verdicts.

Three decisions worth recording rather than rediscovering:

- **The disabled arm is the existing function, not a copy.** Two arms sharing a loop *shape* but
  not a loop cannot be differenced honestly, because an edit to one silently changes the ratio.
- **A source-level test holds the pair to that claim.** It extracts each arm's timed region — from
  `Stopwatch::start` to `watch.stop` — and asserts the enabled one equals the disabled one plus
  exactly one stamp line. Behaviour cannot show this: two arms that had quietly diverged would
  both still run, both still fill their samples, and report the divergence as spoor overhead.
- **Both arms run in the same boot.** `BOARD VERDICT 9` measured ~3% build-to-build movement on
  untouched code paths, which is *larger than the 2% the gate allows* — so two arms from two runs
  could not answer the question at all.

**Not yet measured.** The arm exists, compiles for AArch64, and is host-tested; no board has run
it. `G23` stays two-of-four until one does.

## 5. The actuation path reaches the real-time architecture

`09A` §11 action 3 — *"the finding that most changes the priority"*. `FEAT-P1-06` is the `G-PA-1`
flagship path and every line of its evidence was Tier 0 QEMU `x86_64`; `ADR 0004` designates ARM64
the real-time tier; `hal-arm64` contained no `actuation` module and no `OutputLine` implementation.
The arch-neutral trait was built so a Pi 5 backend could slot in, and for a week nothing had.

`hal_arm64::actuation::Rp1CommandLines` is that backend: eight RP1 bank-0 GPIOs (20..27) driven as
one byte-wide command bus. Filed as **`STORY-P1-06-02`**, with its own contract row.

- **GPIO 20..27, not 0..7**, because GPIO 0 and 1 are the HAT ID EEPROM's `ID_SD`/`ID_SC`. A
  stand-in that quietly drives a bus the board uses for something else is not a stand-in — the
  same argument `hal_x86_64::actuation` makes for port `0x80` over the DMA page registers.
- **The XOR alias, not SET-then-CLEAR.** A byte written as two stores presents a transient in
  which some bits have changed and others have not, and on a command bus that transient *is* an
  intermediate command. XOR against a shadow of the last value updates exactly the changed bits in
  one store and touches no other pin in the bank.
- **No branch on `command == shadow`.** A conditional store would make the RT path's cost depend
  on the data flowing through it, which is the thing this Feature exists to measure the absence of.
- **One named limitation, stated rather than discovered:** two identical consecutive commands
  produce two stores and no pin change, so a downstream device cannot distinguish them without a
  strobe or an edge-encoded protocol. A real actuator will need one; inventing it now with no
  implementor to honour it is the speculative-consumer trap.

**The ordering test was seen to fail before it was trusted.** `ADR 0005`'s rule — a clean run
proves nothing until the detector has been seen to fire — applied by hand: the claim sequence was
temporarily inverted to funcsel-first, the glitch-ordering test failed as it must, and the correct
order was restored. Seven host tests, and `check-boot-images` compiles and lints it for AArch64.

**Criterion 4 is open and the Story says so.** Nothing has run on silicon. Per `06A` §4.1 the
header reads `In progress` and names the one missing thing.

## 6. `assurance.rs` split into a module directory

Owner instruction, mid-session. One 4,482-line file became nine cohesive modules under
`os/src/xtask/src/assurance/`, plus `release_status.rs` from §1 and one integration-test file.
`mod.rs` now holds only the register shapes, the shared data model, and `walk_spine` — whose
ordering is load-bearing and is documented as such, because a later check reads what an earlier one
returned.

Move-only, verified against a green baseline before and after. **The split immediately weakened a
real guarantee and the fix is the interesting part.** `spine_files.rs` had a test asserting every
fast-checked TSV is named in the *source text* of the full check, via `include_str!("assurance.rs")`
— a file that no longer exists. A hand-kept list of eleven sources replaces it, and because
`include_str!` needs literal paths, that list is exactly the `LE-80` shape: a hand-kept mirror of a
real set with nothing checking the mirror. So a guard now reads the directory the compiler read and
fails if the two disagree. **It fired twice on its first two runs** — once for `spine_tests.rs`,
once for `release_status.rs` — which is the only reason either is classified rather than silently
absent.

## 7. Two scope decisions, settled before the work rather than during it

`09A` §9 step 1 and §11 action 7, both costed at about an hour and both done.

**`STORY-P1-05-01` gained `D11`.** The Story names spoor-journal saturation and its contract did
not select the journal's own domain. `D11` is `prototype` readiness so it costs no open-debt row.
`D12`/`D13` — IPC channels and grants — are `specified` and are deliberately *not* selected: they
would force debt rows that can never be closed. They split into a second Story when those
subsystems exist. The Story now also carries the four blockers in cost order, so the next reader
learns that starting it is a build job before spending a day discovering it.

**Both Features' exit criteria re-worded.** `FEAT-P1-05` asked for `SEC-20` converting *"for the
Tier 0 scope"* — **a state the spine cannot represent.** A Story's assurance state is
all-or-nothing; there is no per-tier value. A criterion that cannot be represented is one that
will be declared met by prose. It now names what the spine *can* record: functional `Verified`,
guardrail rows for gates read against their targets first, `SEC-20` staying `baseline-debt`, and no
`G04` row from any of it. `FEAT-P1-06`'s three halves are now a table, each naming its own gate and
its own blocker.

## 7a. The board ran, and `G23` has a number at last — it is 26%, not 2%

Added at session close, after the owner powered the board. **`09A` §8 step 2 is discharged: the
ratio the gate asks for is computable, and the first thing it says is that the gate does not
pass.** Both arms, one boot, `kernel8.img` `05f3495c…`, envelope harvested off the wire and parsed
by the **unchanged** `xtask` parser at `metrics=12`:

```text
D07/pool_u64x64_alloc_free_round_trip_per_op_of_8          p99 = 479 cycles/op   spoor DISABLED
D07/pool_u64x64_alloc_free_round_trip_per_op_of_8_spoored  p99 = 604 cycles/op   spoor ENABLED
                                                           delta = 125 = +26.1%
```

Against `G23`'s `<= 2%`. **Recorded as the fail it is**, not softened.

**Read the density before quoting the 26%.** The enabled arm stamps once per pool operation — the
deliberate worst case, not a realistic instrumentation density — so the figure over-states
overhead rather than flattering it. The useful derived result is what the gate actually
constrains: 2% of 479 cycles is **9.6 cycles**, and a stamp costs **137** (p50, same boot), so the
budget admits **one stamp per ~14 pool operations**. `G23` is a constraint on instrumentation
*density*, not on stamp cost — and nothing in this project had said so, because until today no
percentage could be computed at all. That is `09A` §5's finding reaching its conclusion.

The two structural clauses still hold: `allocations = 0` compiler-enforced, `torn records = 0`
observed. So `PERF-D07-G23` now carries three of four clauses answered and one of them answered
*negatively*, which is worth more than the silence it replaces.

### Three bench defects, and the board was blameless in all three

Four power cycles were spent, and **not one of them failed in the kernel.**

- **`LE-87` — two `tos64-netboot` instances bound UDP 69 at once and the stale one won.** An
  instance left running from the previous session (started 05/08 22:00) received every TFTP
  request while the new one answered DHCP and logged a clean `OFFER`/`ACK`. The board booted a
  **stale image** and emitted a complete, plausible, entirely wrong envelope — `metrics=11` where
  the tree said 12. It was caught only because a metric was missing *by name* rather than
  different *by value*. `SO_REUSEADDR` is the mechanism; **open**, because the fix is to refuse to
  start when the port is held, and I killed the process by hand instead.
- **`LE-88` — the server refused a legitimate request as a path traversal, and it presented as a
  kernel fault.** The firmware fetches `config.txt` bare at one stage and **`/config.txt`** at a
  later one; `Path.Combine(root, "/config.txt")` discards the root, so the traversal guard
  correctly refused a path that should never have reached it. `pciex4_reset=0` therefore never
  applied, the firmware reset the RP1 PCIe link, and TinyOS reported **confession code 2**
  (`LinkAbsent::PhyDown`, detail `0xE080` — `PORT_IS_RC` set, `PHY_LINK_UP` clear). **The kernel
  diagnosed the host tool's bug precisely and a human went looking at the kernel.** Fixed, and
  verified in both directions before being trusted: `/config.txt` REFUSED → served,
  `../../../secrets.txt` still REFUSED.
- **`LE-89` — the board measured `G23` correctly and the transport dropped the answer.**
  `TRANSCRIPT_CAPACITY` was a hand-picked 2048 under a doc comment saying *"~11 lines of ≤ 140
  bytes"*; the twelfth metric, with the longest name in the set, overran it. The arm's line reached
  the wire carrying **its name and none of its numbers**, and `END metrics=12` never arrived.
  Fixed: the capacity is now *derived* as `MAX_LINES * MAX_LINE_BYTES`, with `MAX_LINE_BYTES` 256
  rather than a snug fit over the observed 169 — the thing that grows is the metric name, and a
  bound with 20 bytes of headroom is one rename from the failure it prevents.

**The recurring shape, and it is now three-for-three:** `LE-80` reported a live event as an
absence, `LE-81` died on the first packet it answered, and `LE-87`/`LE-88` reported success for
the half they did while being silent about the half that mattered. **Every visible signal said the
run was good.** The instruments are now more likely to be wrong than the board.

**`LE-89` is the one to keep, though, and it is a compliment to the design.** The transcript's doc
promises *"overflow is dropped, never wrapped — a truncated transcript reads as truncated, a
wrapped one lies"*, and that is exactly why this cost one capture instead of producing a
believable envelope with a quietly wrong tail.

**Still not done on the board:** `STORY-P1-06-02` criterion 4 — `Rp1CommandLines` compiles for
AArch64 and nothing calls it, so there is no fixture driving the actuation path and no external
probe on GPIO 20..27. `LE-82` likewise needs `FaultTaken` stamped before a fault boot proves
anything. Neither is blocked; both need a build, not a decision.

## 8. The next session

1. **Two board items remain, and both need a build before a boot.** The `G23` pair is done (§7a).
   Still open: `STORY-P1-06-02` criterion 4 needs a fixture that *calls* `Rp1CommandLines` —
   nothing does — and, to be observed, either an external probe on GPIO 20..27 or a `sys_rio0`
   `OUT` readback stamped into a spoor, which proves the store reached the pins from the SoC's
   side and is honestly weaker than a probe. `LE-82` needs `Rung::FaultTaken` stamped at the
   `hal-arm64` fault-report site before any mmu-fault boot proves anything.
2. **Fix `LE-87` before the next board session, not during it.** A bench server that silently
   shares its port cost three cycles and produced one entirely plausible wrong envelope. Refusing
   to start when the port is held is a ten-line change; logging the served file's sha256 on every
   transfer would have diagnosed it in one line.
3. **`G09` needs a mechanism, not a filing** (§2, `LE-86`). Per-feature code-and-rodata attribution
   does not exist. Decide whether it is worth building before anyone else reads step 1 and files
   an 8 MiB number against a 16 KiB target.
4. **Then measure toward the 129**, choosing gates from the `target` column first.
   `assurance-status` prints which they are; there is now no excuse for choosing them any other way.
5. **`06A` §4.3 remains the owner's**, correctly sized: `0/98` and 20 `G04` gates. Not the 199.

**And the instruction `09A` left, restated because it held:** do not report `x/460` without
decomposing it — and now there is a command that does, so quoting it undecomposed is a choice.

## 9. State at close

- **Gates:** `check-assurance-spine`, `check-spine-files`, `check-citations`, `check-lints`,
  `check-crate-sizes`, `cargo fmt --all --check` green. `check-boot-images` green — **required
  this session**, `kernel` and `hal-arm64` both changed. Host tests: xtask 324, `hal-arm64` 300,
  `kernel` 212 with `fixture-measure`.
- **Spine:** 31 Features / **98 Stories** / 81 Tests / 62 Reports, **89 loose ends (45 open)**,
  73/98 Stories functionally verified, **21/460 release gates with dated evidence** (24 rows, one
  of them a refusal that does not count).
- **Board:** four power cycles, **none of which failed in the kernel** (§7a). `PERF-D07-G23`'s
  ratio measured at last — **+26.1% against a 2% target**, both arms in one boot, recorded as the
  fail it is. Raw evidence:
  [`goals/reports/wire-meas-envelope-2026-08-06.txt`](../../goals/reports/wire-meas-envelope-2026-08-06.txt),
  image `05f3495c…`, parsed by the unchanged `xtask` parser at `metrics=12`.
- **Unchanged and still true:** 5 platforms, **0 qualified**; `0/98` Stories assurance-verified;
  `06A` §4.3 undecided.
- **Uncommitted.** Nothing committed, `git add -A` never used (`CONCURRENT_SESSIONS` rule 1).
- **Bench:** board powered and cycling its transcript at close, netbooted with no SD card;
  `tos64-netboot` serving `os/target/pi5` — **stop it before the next session starts one**
  (`LE-87`).
