# Handover 11D — CI Fixed: the Baseline is Recorded on the Runner That Gates It

**Session `D`.** Follows [`09D`](09D-story-p1-06-01-actuation-path.md), which left W2 blocked. It is no
longer blocked. Sessions `A`/`C`/`E` were live throughout on their own branch (`os.tauru.poc`); nothing
of theirs was touched.

`main` is at **`d077526`**, pushed, **CI green** — including the timing gate, which now reports *"no
regression across 12 gated statistics"* with its reference landing on `observed=490` against
`baseline=490`.

## 1. What was red, and what fixed it

The only red run was the **manual baseline recorder** (`workflow_dispatch` 30483384040). `main`'s own
push CI was green. The recorder failed for a real reason and refused to write anything:

```text
xtask: refusing to write a baseline this parser rejects:
  baseline `D07/pool_u64x4_alloc_denied_exhausted` carries a zero ratio;
  nothing can be compared against it
```

That is `LE-24`, hit at its sharpest point. A denied `alloc` on a full `Pool<u64, 4>` costs about one
calibrated `RDTSC` read pair; `Stopwatch::stop` subtracts that pair and saturates at zero, so with
`overhead_cycles=26` against a 26-cycle operation the metric's minimum **was** zero. The writer was
right to refuse.

Two commits fixed it:

| Commit | What |
|---|---|
| `d6ab240` | `LE-24`: batch 64 denials per sample and divide — `LE-24`'s own recorded remedy, applied verbatim |
| `d077526` | `LE-23`: the baseline recorded on the runner (run 30487864315), taken from its artifact |

**Landed in one push on purpose.** Between those two commits the gate correctly says *"a metric that
vanished is a regression in evidence"* — the shape change renames the metric, so a baseline recorded
before it cannot match. The work was therefore done on `fix/le-24-batched-denial`, the recorder was
dispatched against **that branch**, and `main` was never pushed through the broken intermediate state.
The branch can be deleted; its only value now is as provenance for run 30487864315.

## 2. What the re-record actually changed

Every number in `goals/performance/baselines/tier0-x86_64.tsv` is now the CI runner's own. The
systematic offset `LE-23` confirmed — D04, `D05/dispatch_run_once` and D02 measuring 23–53% *below* a
Windows-recorded baseline, consistently signed, two of them reporting improved-is-the-baseline-stale —
is gone, and the reference metric now matches its baseline exactly.

**The line that looks alarming and is not:** `D07/pool_u64x64_alloc_free_round_trip` went from
`min 2 / p50 6` to `min 123 / p50 123`. **Nothing got slower.** The old figure was the residue left
after subtracting a calibration of the same magnitude as the operation — which is precisely `LE-24`, and
precisely why that metric sits in `gate.rs`'s `UNGATED_AT_TIER0`. On this runner the operation is
measurable, so the baseline now carries a real number for it. It **stays ungated**, because the reason it
was ungated is a property of where it can be honestly recorded, not of this file.

One piece of tidying that follows and was deliberately not bundled in: that entry's stated reason
("medians to 0 cycles…") is now **host-specific rather than universally true**, and CI prints it on every
run. Whoever picks up `LE-24` should reword it to say *on which host* it medians to zero — a one-line
change, not worth touching a Verified Story's gate for on its own.

## 3. `LE-24`'s other half is open, and I am not guessing at it

`LE-24` names two metrics. Only the denial one is fixed. `pool_u64x64_alloc_free_round_trip` was left
**exactly as it was**, and the reason is a measurement I could not explain:

| Shape | p50, this dev host |
|---|---|
| batch of 1 | 58 cycles/op |
| batch of 64 | **607 cycles/op** |

Ten times what linearity predicts, from a `Pool::alloc` that returns at the **first** free slot — and the
fixture's own self-consistency check confirms all 64 round trips succeeded, so the pool is not filling.
That is not a calibration artifact and it is not explained.

**So I did not re-baseline it.** Replacing a number that is honestly quantisation-limited with one that
merely looks plausible is strictly worse, because the second kind gets quoted. The factor of ten is
written down in `fixture_measure.rs` beside `D07_BATCH`, where the next person to touch this will read
it before repeating the experiment.

## 4. A correction to `d077526`'s own commit message

That message says **"LE-23 is closed"**. The register row is still `open`, deliberately, and the commit
message overstated it.

`LE-23` has two halves. The re-record is discharged, with evidence. Its second half — *"`D05`/
`dispatch_select` is separately unstable run-to-run on CI (+80% then +4.4% between two runs), which is
`LE-18`'s failure mode surviving in the ratio at smaller amplitude"* — is **not**, and this session
reproduced it: 52% run-to-run p99 CV on that metric locally. The row's own `owner_path` calls it "a
separate question [that] may need a second reference of memory-bound composition".

Closing the row would have dropped that finding on the floor, because **the follow-up row cannot be
filed**: id contiguity is enforced and another session holds the next id in an uncommitted append. An
open row carrying a precise note loses nothing; a closed row plus an unfileable finding loses the
finding. So the register is untouched and this document is the note.

**The register edits now owed** (all blocked on the same coupling, and this is the fourth arrival at it):

1. `LE-23` → closed, once its `dispatch_select` half has a row of its own to move to.
2. A new row for that `dispatch_select` instability.
3. A new row for `LE-24`'s unexplained 10× batching factor.
4. The three `09D` §5 / `05B` §4 already owed.

## 5. What is green, and what to check first if it is not

- `main` `d077526`: **CI green**, all three jobs, run 30488079669.
- The timing gate: `no regression across 12 gated statistics (2 reported without a verdict)`.
- Locally: `cargo test --workspace` 634 passed, `check-assurance-spine`, `check-spine-files`,
  `cargo fmt --all --check` all clean.
- The recorder job is skipped on `push` (0s) and only runs on `workflow_dispatch`, as designed.

**To re-record in future:** `gh workflow run CI --ref main`, then `gh run download <id> -n
tier0-x86_64-baseline`, then commit the file. Never `--update-baseline` on a dev host (`LE-28`) — that is
what produced the offset this work removed.
