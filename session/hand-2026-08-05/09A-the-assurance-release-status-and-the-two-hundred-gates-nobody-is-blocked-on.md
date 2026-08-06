# 09A — The Assurance Release Status, and the Two Hundred Gates Nobody Is Blocked On

Session handover, written 2026-08-05. Follows [`08A`](08A-the-closing-pass-and-the-board-that-was-never-blocked.md).

**`08A` closed with a claim, and this document exists because the claim does not survive
being checked.** `08A` §8 said:

> *Assurance `verified` is still 0/97 and that is structural: zero qualified platforms.
> `06A` §4.3 is still the owner's undecided call and it is now the single largest thing
> standing between `EPIC-P1` and completion by its own definition.*

The first sentence is true. **The inference is half wrong, and the wrong half is the half that
was being used to explain why nothing moves.** This is a diagnosis document, like `06A`, because
the plan is wrong if the diagnosis is.

---

## 1. The register, decomposed. No interpretation yet

The dashboard shows two numbers — `0/97` Stories assurance-verified and `20/460` release gates
with dated evidence — and neither is decomposed anywhere.

**First, what "in play" means, because without it nobody can reproduce 460.** A domain is *in
play* when **at least one Story contract selects it**. Twenty of the catalogue's twenty-five are;
`D15`–`D19` are `design`-readiness and unselected by any Story, so their guardrails are not in
anyone's contract and are not counted. Twenty domains × 23 release guardrails (`G01`–`G23`;
`G24`/`G25` are *claim* gates, excluded by the catalogue's own `gate` column) = **460**.

Decomposed from the committed catalogue and contracts — **nested, not subtracted**, because the
buckets overlap and a flat column of subtractions does not reconcile:

```text
release gates in play (20 in-play domains × 23 release guardrails)   460
│
├─ in the 10 domains whose SUBSYSTEM DOES NOT EXIST                  230
│     D02 unbuilt · D10, D14 stand-in-only · D12, D13 specified
│     D20, D21, D22, D23, D25 design
│     ── this bucket already CONTAINS:
│          all 46 hardware-only (T1/T2) gates   — D02 23 + D25 23
│          10 of the 20 G04 bound-class gates
│
└─ in the 10 IMPLEMENTED in-play domains                             230
      D01, D03, D04, D05, D06, D07, D08, D09, D11, D24
   ├─ G04, barred by ADR 0005 while 0 platforms are qualified         10
   └─ not barred by G04, by the board, or by an absent subsystem     220
      ├─ carrying evidence                                            20
      └─ carrying nothing                                            200
```

**Of those 200, roughly 70 are not merely unmeasured.** `readiness` is tracked per *domain*, and a
`prototype` domain still contains guardrails whose mechanism does not exist: `G13` queue
residence, `G14` queue processing maximum, `G15` sustained throughput, `G16` burst and
backpressure, `G19` isolation under competing load, `G21` exhaustion and fault containment, `G22`
72-hour soak — seven load, queue and campaign shapes across ten implemented domains. `G19` and
`G21` are *precisely* `FEAT-P1-05`'s unbuilt mechanism (§9). So the register's `readiness` column
cannot express the distinction that matters here — **unevidenced because unmeasured** versus
**unevidenced because unbuilt** — and roughly 70 of the 200 are the second kind.

**The defensible headline, which loses nothing:** the 200 are **not blocked by the qualification
decision and not blocked by the board.** About 130 of them are measurement work available today;
about 70 need a mechanism built first, and that is a smaller and better-defined problem than "the
Epic cannot complete until a campaign starts". Nobody has run any of them.

## 2. The denominator is badly formed, and it has been flattering the problem

**Half the headline denominator cannot be closed and is already registered as debt.** 230 of the
460 belong to ten domains whose `readiness` is `design`, `specified`, `unbuilt` or
`stand-in-only`, and [`goals/assurance/README.md`](../../goals/assurance/README.md) is explicit
that *"the subsystem does not exist, and not one of those 25 can be closed."* They are recorded
as 37 open-debt selections precisely so this is visible.

So `20/460` invites a reading — *"4% done, 440 to go"* — that is wrong in both directions. It
overstates the work remaining, because 230 of it is not work; and it understates the
**indictment**, because against the denominator that can actually be closed the figure is
`20/220`, and none of those 200 is waiting on the qualification decision or on the board.

This matters beyond presentation. `06A` §6 chose this number as one of two measures of whether
the project is moving. **A measure nobody has decomposed is a measure nobody can act on**, which
is a fair description of what has happened to it: it sat at 11 for a week while three handovers
explained the 0/97 beside it.

## 3. The board unblocks nothing that is currently blocked

This is the finding that most contradicts the last four handovers, and it is checkable rather
than arguable:

**Five domains in the catalogue are entirely hardware-only** — every release cell `T1`/`T2`, no
`Host` or `T0` tier anywhere: `D02`, `D15`, `D16`, `D17`, `D25`. That is 115 cells. **Only two of
the five are in play** (`D15`–`D17` are unselected, per §1), so the number that enters the 460 is:

```text
hardware-only release gates, IN PLAY (T1/T2, no Host or T0 tier)     46
  in D02   (readiness = unbuilt,  in play)                           23
  in D25   (readiness = design,   in play)                           23
  in any IMPLEMENTED in-play domain                                   0

  (D15, D16, D17 are also wholly hardware-only — 69 further cells —
   but no Story selects them, so they are outside the 460 entirely.)
```

**Every hardware-only gate in play lives in a domain whose subsystem does not exist.** Over the
ten implemented in-play domains — `D01`, `D03`–`D09`, `D11`, `D24` — **all twenty-three release
guardrails are `Host`/`T0` tier.** Not one of them needs the Pi 5.

The board was never the constraint on this register, and the entire hardware-evidence sprint —
correct and valuable for `LE-09`, the tier, and functional `Verified` — moved the guardrail
count by **nine**, all of which could have been filed from a QEMU run. `08A` was right that the
board was never the blocker on the *Stories*; it did not notice that the same is true, far more
strongly, of the *gates*.

## 4. Eighteen of twenty-three release guardrails have never received a single row

Ever, in any domain, since the register was created:

| | guardrail | rows |
|---|---|---|
| ✓ | `G01` median latency | 3 |
| ✓ | `G02` p99 latency | 3 |
| ✓ | `G03` p99.9 tail latency | 3 |
| ✓ | `G11` steady-state allocation count | 12 |
| ✓ | `G20` security denial cost and safety | 1 |
| ✗ | `G05` jitter envelope · `G06` median cycles · `G07` p99 cycles · `G08` microarchitectural efficiency | 0 |
| ✗ | `G09` image and feature footprint · `G10` peak working memory · `G12` allocation/reclamation latency | 0 |
| ✗ | `G13` queue residence p99 · `G14` queue processing maximum · `G15` sustained throughput floor | 0 |
| ✗ | `G16` burst and backpressure safety · `G17` cold start · `G18` warm restart · `G19` isolation under load | 0 |
| ✗ | `G21` exhaustion and fault containment · `G22` 72-hour soak · `G23` spoor observability overhead | 0 |
| — | `G04` observed maximum and WCET bound | 0, and **may not** be filed (`ADR 0005`) |

`G11`'s twelve rows are one structural fact — the compiler-enforced absence of a heap — filed
once per domain. `G01`, `G02` and `G03` hold three rows each: `D04`, `D05` and `D07`, all from the
single envelope this bench measured on 2026-08-04. So the register's entire measured content is
**one afternoon's run**, and `G09` (image footprint, against a hard 8 MB ceiling that CI already
computes) and `G17` (cold start) are examples of gates whose evidence very likely *already exists
in CI output* and has simply never been written down.

**`G20`'s single row is not the same kind of gap, and §8 step 1 must not treat it as one** —
`STORY-P1-06-01` measured `PERF-D03-G20` and **deliberately declined to file it**. See §11.

## 5. `G23` — measured in the wrong units, and it is this project's own substrate

Worth its own section because it is the sharpest instance of the pattern, and because `08A`
got it wrong in the other direction.

`08A` filed the three spoor costs against nothing, on the reasoning that `D07` is *static pool
allocation* and the spoor stamp is not that. Correct as far as it went — and it missed that
**`G23` is `spoor observability overhead`, and it exists in every domain.** So there *was* a gate
for those numbers.

But it still cannot be closed by them, for a reason worth recording rather than discovering
twice. `G23`'s target is a **ratio**:

> `spoor enabled adds <= 2% p99 and <= 2% CPU cycles; allocations = 0; torn records = 0`

The board measured **absolute** costs — stamp 136 cycles, announce 3099, drain 122005. There is
no spoor-**disabled** arm, so no percentage can be computed, and the measurement as taken cannot
close the gate it was made for. Two of the four clauses *are* met and could be filed today:
`allocations = 0` is compiler-enforced, and `torn records = 0` was observed across 400+ decoded
records at 0 refused and 0 lost. The other two need one more fixture arm.

**The general shape, which is the reusable finding:** this project measures things and then
discovers the guardrail wanted them expressed differently. A measurement taken without reading
the gate's `target` column is a measurement that will need retaking.

## 6. So what is actually blocking `EPIC-P1`

Three separate walls, which four handovers have discussed as one:

1. **Assurance `verified` per Story (`0/97`) is gated on qualification.** True, structural,
   unchanged, and **the owner's decision** — `06A` §4.3 stands exactly as written. Nothing in
   this document touches it.
2. **`G04`, the bound class, is barred by `ADR 0005`** — 20 gates, also waiting on the same
   decision. Correctly barred: a bound quoted from an unqualified platform is the failure the
   ADR exists to prevent.
3. **The other 200 gates are waiting on nobody.** Not the owner, not the board, not a campaign.

Wall 3 is roughly **half of everything the assurance register can currently express**, and it is
the half that has been invisible because it was never separated from walls 1 and 2. `08A` calling
the qualification decision "the single largest thing standing between `EPIC-P1` and completion"
was the conflation in its most compact form.

**What this does not say.** Closing 200 guardrail rows does not make one Story assurance
`verified` — that conversion is all-or-nothing and still requires *every* applicable gate,
including the `G04` the ADR bars. The register is "a count of evidence, never a score", and
`06A` §2's Ceiling B is real. **The claim here is narrower and harder to dismiss:** the project
has been explaining a `20/460` with a constraint that accounts for at most 66 of the 440 empty
gates.

## 7. What this session changed

- **`LE-83`, found by causing it.** `08A` reported the evidence figure moving `11 → 22`. It moved
  `11 → 20`: the tile publishes *gates* over a denominator of *gates*, and the code counted
  *rows*. Filing `PERF-D07-G11` for two more Stories — legitimately, the register's unit is the
  `(guardrail, story)` pair — inflated the published number by two while covering nothing new.
  The field's own doc comment had said "a count of gates that have evidence" all along, so the
  code disagreed with its documentation and the documentation was right. Fixed, with a test
  asserting the published figure equals the gate count *and* differs from the row count, plus a
  guard test that fails if the committed register ever stops containing a duplicated gate — so
  the first test cannot start passing for the wrong reason. **A ratio whose numerator and
  denominator count different things reads as progress and is not**, and every other `x/y` tile
  on that page deserves the same question.
- **`08A` corrected in place** rather than quietly, because the number is one of `06A` §6's two.

## 8. The next session — and the order matters

1. **File what already exists.** `G09` against the 8 MB image ceiling CI already computes, and
   `G23`'s `allocations = 0` and `torn records = 0` clauses. This is registration, not
   measurement, and it is `07A` §0's own diagnosis — *evidence was captured and not registered* —
   one layer out.
   **Do not read this as "and `G20` beyond its single row".** `STORY-P1-06-01` measured
   `PERF-D03-G20`, found **55% run-to-run p99 CV**, and refused the filing in writing: *"No
   `guardrail-evidence.tsv` row was filed — not even a `G20`. The reason is evidence, not
   caution."* Executed mechanically this step would either re-file it — resurrecting the exact
   numerator inflation `LE-83` was raised about — or re-derive a refusal that is already
   reasoned. §11 has the detail and the register-shape gap underneath it.
2. **Add the spoor-disabled fixture arm** so `G23`'s two ratio clauses become computable. One
   arm, and the substrate this project measures everything else with stops being unquantified in
   the units its own gate asks for.
3. **Then measure toward the reachable gates deliberately**, choosing them from the `target`
   column rather than measuring first and looking for a home afterward (§5).
4. **`06A` §4.3 remains the owner's, and is now correctly sized.** It gates `0/97` and 20 `G04`
   gates. It does not gate the 200, and it should stop being cited as though it did.

5. **`FEAT-P1-05`'s scope decision — one hour, and worth doing now** (§9). Not the build work:
   that is long, is two Features rather than one Story, and competes directly with the board
   sprint. Only the scope question, while the reasoning is fresh.

**And one instruction for whoever writes the next handover:** do not report `x/460` without
decomposing it. The number has been quoted in four consecutive documents and none of them said
that half of it is undeliverable by construction.

## 9. `FEAT-P1-05` — verifying it is a build job, not a test-writing job

Recorded here because it is the same disease this document diagnoses, in a Feature nobody has
started: **three of the four things its exit criteria assert must be true have no mechanism in
the kernel to be true of.** You cannot flood a budget that does not exist, and you cannot
attribute a denial to an offender the spoor vocabulary cannot name. Every claim below was
checked against the tree rather than inferred from the Feature document.

### What already exists

Both declared dependencies are satisfied — [`FEAT-P1-04`](../../goals/features/FEAT-P1-04.md)
has real timer-driven preemption and WCET enforcement on the shipping image, and
[`FEAT-P1-01`](../../goals/features/FEAT-P1-01.md) gives cycle-calibrated Tier 0 measurement with
committed baselines. The contract rows exist and are legal **today**: `STORY-P1-05-01` selects
`D05,D07`, both implemented-readiness, so no `open-debt.tsv` row is required as written. And
`fixture_preempt.rs`, `fixture_pool_bench.rs` and the `TOS64-MEAS/2` parser are a working
template for a campaign fixture.

### The four blockers, in cost order

1. **There is no RT reserve and no per-class budget.** `BND-15` reads *"exhaustion cannot consume
   another class budget or RT reserve"*. `Tcb` carries `base_priority`, `inherited_priority`,
   `wcet_budget`, `overrun_policy`, `entry`, `ticks_consumed` and `state` — **and no containment
   class**. The pool is one flat capacity with no class tag and no reservation floor. Grepping
   the kernel for `reserve` returns an `rflags` comment, some reserved stack bytes, a reserved
   MSR and a reserved sentinel — **no scheduling or allocation reserve exists anywhere**. So
   `RCG-08`'s *"refuse admission that would endanger existing RT reservations"* has nothing
   behind it. This is Feature-sized design work — class on the TCB, per-class partitioning or a
   reserved-floor allocator, and the same for task slots and ready-queue admission — and it must
   land before the campaign can claim anything.
2. **Denial is not attributable to an offender.** `Actor` is a three-value nibble — `Kernel`,
   `Exec`, `Session`. Acceptance criterion 2 wants every denial *"charged to the offender, and
   spoor-attributed"*, and `BND-17` wants *"class identity actor action target result and
   order"*. Today a spoor can say the kernel denied something, **not which task, of which class,
   caused it**. Extending the encoding is a wire-format change touching `spoor_stream.rs`,
   `spoor_wire.rs` and the ARM64 parity vocabulary — and it interacts with **`LE-82`, which is
   open right now** (`Rung::FaultTaken` declared and stamped by nothing). Do these together:
   both are "the spoor vocabulary cannot say the thing the contract requires".
3. **No property-testing infrastructure.** Criterion 4 requires the interleaving invariant in CI
   with a recorded seed policy. **There is no `proptest`, `quickcheck` or `arbitrary` dependency
   anywhere under `os/`** — zero hits across every `Cargo.toml` — and `kernel` is `no_std`. This
   needs a host-tier dev-dependency, a decision recorded against `RCG-07`'s
   minimal-dependency-surface stance, and the seed policy written down *before* the first test.
4. **No campaign harness.** `xtask` parses single measurement envelopes; this Feature needs
   concurrent flood-plus-RT-workload orchestration, idle-versus-flood distributions side by side,
   and a measured recovery-time bound after the flood stops. A new subcommand and a new Report
   shape — the Feature says so itself: the first Report whose primary content is adversarial-load
   data.

### Two spine problems to settle before the Story starts

- **The contract row under-selects.** `STORY-P1-05-01` names saturation of the spoor journal, IPC
  channels and grants, and task slots — but the row selects only `D05,D07`. **One correction to
  the obvious reading:** `D11` (the journal) is `prototype` readiness, so it can be *added for
  free*; it is `D12` and `D13` that are `specified` and would force `open-debt.tsv` rows that can
  never be closed, in both directions, enforced by `check-assurance-spine`. **Recommendation:**
  first Story covers `D05`/`D07`/`D11` — pools, task slots, ready queue and the journal — and
  IPC/grant saturation splits into a second Story that lands when those domains are built.
- **`verified` is all-or-nothing.** The exit criteria say `SEC-20` converts *"for the Tier 0
  scope"*, but a Story's assurance state has no partial value. Tier 0 x86_64 is fine for
  `D05`/`D07`/`D11` evidence rows — **but file no `G04` bound row from it**: those are refused at
  `T0`, on `x86_64`, and from any platform absent from `qualified-platforms.tsv`, where the count
  is zero, the Pi 5 included. So the honest outcome of a full campaign is **functionally
  `Verified` with `baseline-debt` retained**, unless `SEC-20`'s hardware-tier obligations are
  discharged separately. Re-word the exit criteria so "Tier 0 verified" means something the spine
  can actually represent.

### Realistic sequence

1. Resolve the domain-scope question and re-word the exit criteria.
2. Design and build class-tagged budgets and an RT reserve floor, test-first. **The long pole,
   and probably its own Feature rather than a Story of this one.**
3. Extend spoor attribution to carry task/class identity; coordinate with `LE-82`.
4. Add host property tests with a recorded seed policy.
5. Build the campaign fixture and `xtask` harness; capture idle/flood/recovery raw data.
6. Write `TEST-P1-05-01-A`, run the campaign, file the Report and the guardrail-evidence rows.

**Steps 2 and 3 are the honest answer to "what will it take". Steps 4–6 are the part the Feature
document describes, and they are the smaller half.**

### Sequencing against the board

This is all x86_64 Tier 0 work and it competes directly with the Pi 5 hardware-evidence sprint
this session is in the middle of. **Do not start step 2 until the board work reaches a natural
stopping point.** Step 1 costs an hour and should be done now, while the reasoning is fresh.

Note how this Feature relates to §1–§4 above: `D05` and `D07` are implemented in-play domains, so
every one of their 23 release guardrails is `Host`/`T0` and **none of them needs the board** —
`FEAT-P1-05` is squarely inside the 200. But it is also the clearest case in the register of a
gate that cannot be closed by *measuring harder*, because the mechanism the guardrail describes
has not been built. That distinction — *unevidenced because unmeasured* versus *unevidenced
because unbuilt* — is the one the register's `readiness` column tracks per domain and cannot
track per guardrail, and `BND-15`/`RCG-08` fall in the gap.

## 10. Method note, recorded because it was corrected mid-session

The decomposition in §1, §3 and §4 was produced with throwaway Python scripts. **That violates
the standing rule that host tooling is C# under `work/tools/`, with PowerShell only as thin
invocation**, and the owner said so. The scripts are deleted and the file edits were redone
through the ordinary editing path.

The durable fix is not "use a different scripting language for one-off analysis" — it is that
**this analysis should not be ad hoc at all.** Every number in this document is derived from
committed TSVs by rules `xtask` already implements pieces of (`release_gate_reach` computes the
reachability split; `validate_open_debt` knows which domains are unbuilt). `xtask assurance-status`
belongs in the repository, in the repository's own language, so the next session reads the
decomposition instead of rediscovering it — and so a claim like §3's is re-derivable rather than
quoted from a handover. That is the same argument `LE-30` made for the dashboard and
`STORY-P0-01-05` made for the register itself: **a number that argues about where effort should
go must not be an assertion in a document nobody re-checks.**

**Filed as `LE-84` before this handover closed, rather than promised in prose.** A recommendation
that lives only in a handover is the exact failure mode this session has now found three times —
`LE-83`'s miscounted ratio, §11's actuation row promised on 2026-07-29 and never filed, and this
one, which would have been the third. The row also carries the evidence for its own necessity:
**§1's ledger did not reconcile on first printing.** It subtracted three overlapping buckets and
produced 164 where the answer is 220 — the 46 hardware-only gates and 10 of the 20 `G04` are
*inside* the 230, not beside them. Corrected above. A decomposition a reader has to repair is the
argument for deriving it in code rather than writing it down, made against this document.

## 11. `FEAT-P1-06` — verdict, and the actions that complete it

Asked at the close of this session: *is it technically complete with evidence?* **The claimed half
is, and I re-derived it rather than reading the Report. The Feature is not, and will not be this
sprint.** Recorded here alongside §9 because the two Features are each other's blockers in one
direction only — `FEAT-P1-05` gates `FEAT-P1-06`'s exit, and nothing gates `FEAT-P1-05`.

### The evidence reproduces, seven days and one wire-rename later

[`REPORT-2026-07-29-02`](../../goals/reports/REPORT-2026-07-29-02.md) was written against `3dfd1eb`,
**before** the `TINYOS-*` → `TOS64-*` rename and a week of ARM64 work. Re-run on the tree as it
stands, 2026-08-05:

```text
TOS64-MEAS/2 METRIC domain=D03 metric=decision_to_actuation_emit n=1000 dropped=0 warmup=100
             min=9090 p50=9550 p99=24594 p99_9=184681 max=1306218 unit=cycles
fixture-actuation: iterations=1100 line_writes=1100 emitted=1100 refused=1100 emit_failures=0 denial_failures=0
fixture-actuation: denial_phase_writes before=1100 after=1100 (equal in a passing run)
fixture-actuation: ticks=41 unknown=0 preemptions=0 enforcements=0 deadline=2 deadline_misses=0
TOS64-RESULT/1 fixture=actuation ok=true                          [exit 0]

fixture-actuation-overrun: TRIP task=0 attributed_ticks=13 budget=12 (bound 13) finished=true within_bound=true
fixture-actuation-overrun: line_writes=0 emitted=0 refused=1 deadline_misses=1 deadline=2
             last_emit=Some((Err(NotAuthorized), 0)) late_probe=Some(Err(DeadlineMissed)) probe_writes=0 probe_ran=true
TOS64-RESULT/1 fixture=actuation-overrun ok=true                  [exit 1 — the declared pass condition]
```

Also green on the current tree: `cargo test -p kernel` 207 passed, including the 12
`kernel::actuation` host tests the Report claims; both fixtures still registered to
`TEST-P1-06-01-A` in `list-fixtures`; `check-assurance-spine` clean.

The strongest line is `late_probe=Some(Err(DeadlineMissed))` with `probe_writes=0`. That is
**prevention, not unreachability**: the overrunning task presented a command with its own declared
identity, while still running, after its deadline closed — and the port refused it. Without that
probe the claim would rest on the offender never reaching an emit, which is a different and much
weaker claim. `08A`'s and this document's recurring lesson — *a clean run proves nothing until the
detector has been seen to fire* — was applied here two days before `ADR 0005` wrote it down.

### Verdict

**Technically complete as claimed. Not complete as a Feature, and its own Status header says so
correctly** — one of three halves closed, the other two gated by `FEAT-P1-05` and by qualification.
Two things the Feature document does *not* say, both found by checking rather than reading:

- **Zero release gates.** `STORY-P1-06-01` has **no row in `guardrail-evidence.tsv`** — not one of
  the 460, not even the `G20` it measured. In the register this Feature is indistinguishable from
  one nobody started. That is honest (the Report declined `G20` at **55% run-to-run p99 CV**) and it
  is also the §1 pattern from the other direction: here the evidence *was* read against the gate's
  `target` column, and correctly refused.
- **A promised loose end was never filed.** The Report's *"What is not claimed"* says the
  fixture-only path — the `LE-20` shape, proven in a fixture rather than on the shipping `os` image
  — *"is an unregistered loose end … owed the moment theirs lands"*, blocked at the time by id
  contiguity against a concurrent session. It had been owed for seven days. **Filed as `LE-85`
  before this handover closed**, carrying all three findings in this section: the fixture-only
  path, the zero guardrail rows with the reasoned `G20` refusal underneath them, and the absence
  of any ARM64 backend.

### Actions to complete it, in the order they become possible

| # | Action | Blocked by | Cost |
|---|---|---|---|
| 1 | ~~**File the owed loose end**~~ — **DONE: `LE-85`**, filed before this handover closed | — | done |
| 2 | **Record the `G20` refusal where the register can see it**, not only in Report prose | nothing | see below |
| 3 | **An ARM64 `OutputLine` backend** over RP1 GPIO | nothing | small, and it belongs to *this* sprint |
| 4 | **Move the path onto the shipping `os` image** | nothing structural | a Story |
| 5 | The **under-hostile-load** half | `FEAT-P1-05` — §9 steps 2–3, Feature-sized | long |
| 6 | The **bound** half (`PERF-D03-G04`/`PERF-D05-G04`) | `06A` §4.3 + qualification | owner's |
| 7 | **Re-word the exit criteria** so each half names its own gate | nothing | one hour, do it with §9's |

**On action 2, and it is a warning about §8 step 1.** That step says *"`G20` beyond its single
row"*. `STORY-P1-06-01` **deliberately declined** a `G20` filing, with a measured reason. A session
executing §8 step 1 mechanically will either re-file it — resurrecting exactly the
`20/460`-inflation `LE-83` was raised about — or re-derive the refusal from scratch. The register
records only that something *was* measured; it has no way to say *"measured, read against the
target, and refused."* That absence is a register-shape question worth one paragraph in
`xtask assurance-status` (§10), not a measurement.

**On action 3, and it is the finding that most changes the priority.** All of this Feature's
evidence is Tier 0 QEMU **`x86_64`**. `ADR 0004` makes ARM64 the real-time tier, and
[`os/src/hal-arm64/src/`](../../os/src/hal-arm64/src/) contains **no `actuation` module and no
`OutputLine` implementation** — the arch-neutral trait was built so a Pi 5 backend could slot in,
and nothing has. The Report is careful that *the boards carry the product's numbers*; what is
written nowhere is that **the mechanism itself has never run on the architecture this project
designates as real-time.** Against §3's finding this is the interesting inversion: the board
unblocks no *gate*, and it is nonetheless the only thing that can carry `G-PA-1`'s mechanism onto
the RT tier. Actions 1, 3 and 4 are the whole of what is achievable here without the owner or an
unbuilt Feature, and 3 is the one that fits the sprint already running.

## 12. State at close

- **Gates:** `check-assurance-spine`, `check-spine-files`, `check-citations`, `check-lints` green;
  host tests pass. `check-boot-images` not required — no board-crate source changed.
- **Spine:** 31 Features / 97 Stories / 81 Tests / 62 Reports, **85 loose ends (44 open)**,
  73/97 Stories functionally verified, **20/460 release gates with dated evidence** (22 rows) —
  and see §1 for why that denominator should never again be quoted undecomposed.
- **Unchanged and still true:** 5 platforms, **0 qualified**; `0/97` Stories assurance-verified;
  `06A` §4.3 undecided.
- **Uncommitted.** Nothing committed, `git add -A` never used (`CONCURRENT_SESSIONS` rule 1).

### Bench, as of the last check — and one thing that will bite

- **The board is alive, on its THIRD netboot of the session, and still dispatching.** Epoch is
  now `0x04B1DF41`, not `BOARD VERDICT 14`'s `0x04B1CA6E`; `MmuEnabled cost=183677` against that
  verdict's `183624`. `ti64dink --until rung=DispatchRound` still exits 0, so `STORY-P1-11-01`'s
  criterion-7 behaviour has survived hours and two further boots — durability nobody set out to
  test.
- **`tos64-netboot` IS NO LONGER RUNNING.** It was terminated by the harness after the session's
  work finished. **No SD card is in the Pi, so if the board is power-cycled now it will not
  boot** — there is nothing to serve it. Restart the server *before* touching power:
  `work/tools/netboot/bin/Debug/net10.0/tos64-netboot.exe --mac 88:a2:9e:11:4e:cc --root
  C:/tmp/tftproot --server 169.254.113.248`. The explicit `--server` is not optional cargo-cult:
  it is `LE-81`'s workaround for the startup-time address guess, and the fix only *re-resolves*
  the address, it does not make a wrong one impossible.
- **`LE-81`'s fix held across the whole session, and the log says so quantitatively**: 242 lines,
  **6 DHCP OFFER/ACKs, 3 complete 578-block `kernel8.img` transfers, zero unhandled exceptions
  and zero failed sends.** Before the fix the server died on the *first* reply it ever attempted.
  The row nonetheless stays **open** as written, because the path it names as untested — a send
  that fails and a retry that then succeeds — still never executed: zero failed sends means zero
  recoveries. A clean run is not the same as an exercised recovery, which is this session's own
  recurring lesson pointed at its own fix.
- **The second power cycle §5 asked for probably did happen — after the capture window closed.**
  Three complete transfers means three boots; only the first was inside a listening window. So
  `LE-76`'s frame-0-plus-`TOS64-RESULT/1` capture was missed on *timing*, not for want of a power
  cycle. Next session: arm the capture first, then ask.
- **A third `LE-74` data point, and it is the strongest yet.** `0x04B1CA6E` → `0x04B1DF41` is
  **0x14D3 = 5331 counter ticks ≈ 99 µs** at 54 MHz — between boots separated by *hours*, not by
  a quick double power-cycle. `LE-74`'s existing measurements were 151 ticks (2.8 µs) and ~1.28 M
  ticks (~23 ms) from back-to-back cycles. This says the epoch's entropy **is not a function of
  how long the board was off**, which is a stronger and more damaging statement than the row
  currently makes: it removes the intuition that a long gap makes collision unlikely. The epoch
  remains sound as a change detector and unusable as an identifier.
