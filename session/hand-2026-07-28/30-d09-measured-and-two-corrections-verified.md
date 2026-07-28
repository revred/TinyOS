# Handover 30 — D09 Measured, `LE-31` Evidenced for One Domain, and Two Corrections That Held

Filed to close a dangling reference: **`LE-42` names this document as its raising handover and this
document did not exist.** It covers two strands that ran concurrently at the close of 2026-07-28 —
`STORY-P0-01-06`'s D09 disposition (commit `b663376`) and the verification of Handover 29's two
audit corrections.

**On authorship.** The D09 narrative below is drawn from `b663376`'s own commit message and from the
artifacts it landed, not from having done that work. Where it states a number, the number is quoted
from the artifact and was re-checked against the register. The session that performed the
measurement is the authority on its own reasoning; this is a record, not a substitution for its
account.

## What landed

```text
main                    b663376
assurance spine         23 Features, 58 Stories, 45 Tests, 46 Reports
                        42 loose ends (31 open), 84 status headers
                        11 release gates with dated evidence, of 391
Stories verified        0 / 57
LE-09                   OPEN — and now known to be the wrong blocker for 24 of D09's 25 gates
```

## `LE-31`'s hypothesis, evidenced for one domain

Handover 29 named `STORY-P0-05-01` as the single candidate that could move `Stories verified` off
zero without buying hardware: it selects `D09` alone, and all 25 of `D09`'s release gates are tiered
`Host+T0`. **Nobody had checked whether that tier claim was true.**

`STORY-P0-01-06` dispositions all 25 with a named blocker each, and the result is the sharpest
confirmation `LE-31` has received:

**`LE-09` — no hardware tier — is the correct blocker for exactly one of the twenty-five.** That one
is `G08`, which wants retired-instruction, branch-miss and L1D-miss counts that QEMU/TCG does not
expose. The other twenty-four are blocked by tooling, environment, or a subsystem that does not
exist yet — **none of which a Raspberry Pi arriving in the post would change.**

Handover 28's audit predicted this shape from the tier table alone. `STORY-P0-01-06` is the first
time it has been established by working a domain rather than by reading a column, and it moves
`LE-31` from a well-argued hypothesis to an evidenced one for `D09`.

## The first gate closed from a measurement

**`PERF-D09-G20` closes on all three of its conditions**, and it is the first row in
`guardrail-evidence.tsv` recorded from a measurement rather than from a structural argument:

- **45.2 µs max** against a 125 µs budget, over 600 samples
- **state changes zero** — the fixture asserts an identical `PeError` across all 200 iterations, so
  the denial is *deterministic* rather than merely repeated
- **allocations zero**, by the no-heap property `G11` already established

The distinction in the second bullet is the one worth carrying. "Same error 200 times" and "no state
change" are different claims, and only the first is what a loop naturally demonstrates.

Eleven gates now carry dated evidence, from ten. Against 391, and against `0 / 57` Stories
assurance-verified — both of which are unchanged, correctly.

## `LE-42` — the finding the measurement produced

Measuring something for the first time is how you discover it was never measured. The **D09 accept
path runs 17.6–39.1× over every latency and cycle budget its own catalogue rows state**: p50
4,510,818 cycles (1952.7 µs) against `G06`'s 150,000 and `G01`'s 50 µs; p99 8,796,646 against
`G07`'s 500,000.

The row is careful about what this does and does not establish, and that care is the point:

- **Tier 0 QEMU/TCG timing is not proportional to hardware**, so the magnitude does not transfer.
- **The run-to-run p99 CV is 22–71%** where `G05` wants ≤ 5%, so no stable verdict exists in this
  environment at all.
- **And a 30× overshoot is far outside what measurement noise plausibly explains.**

Owner: re-measure on a stable environment (a CI run, per `LE-23`) before concluding anything about
the parser. If it survives that, the accept path needs either optimisation or a re-derived budget
with its reasoning recorded. **PE64 parse cost had never been measured before `STORY-P0-01-06`.**

## Two corrections verified, and one improved

Handover 29 downgraded two of six external audit findings. Both downgrades were independently
checked against the tree rather than inherited, and **both held.**

### `LE-37` — the correction is right, and stronger than the row records

| Claim | Evidence |
| --- | --- |
| `"abi": "softfloat"`, `"features": "…,-neon"` | Confirmed in `os/targets/aarch64-tinyos.json` |
| No SIMD is emitted | **Zero** SIMD/FP operands and mnemonics across **218,004** AArch64 instructions in `hal-arm64` + `core` + `compiler_builtins`, by two independent detectors |
| `CPACR_EL1` never initialised | Only occurrence in `hal-arm64` is a doc comment in `esr.rs` |
| Nothing enforces the flag | Confirmed — no test, no gate, no `xtask` reference |

**The detector was self-tested before its negative was believed**: it fires on `v1.16b`, `q0`, `d0`
and `fadd s0`, and correctly ignores `add x1`. *A zero from an unexercised detector would have been
the same class of error as the finding being corrected* — which is the same discipline that padded
the `.org` guard past 128 bytes before trusting it, and that made `TEST-P0-01-06-A` file one gate
rather than three.

**One improvement on the row as written.** `LE-37` says the defect is that a JSON build flag stands
in for a hardware initialisation with nothing enforcing the flag. The verification found a *better*
enforcement than pinning the JSON: **a check over the built AArch64 artifact catches SIMD arriving
from `core` or a dependency**, not merely from an edited flag. That is strictly stronger, and it is
the shape `LE-37`'s fix should take.

### `LE-40` — the correction is right on all three points

- `owner_space: &AddressSpace<'_, OWNER_FRAMES>` — a **shared** borrow ([`shared_memory.rs:201`](../../os/src/exec/src/shared_memory.rs#L201)). No `&mut` can alias it, so the "time" in time-of-check-to-time-of-use does not exist between the validation loop and the re-read.
- **No payload header anywhere.** The input is a `GrantRequest` of plain integers; `translate()` walks page tables. There is no C4 byte parsing in this function.
- The `.expect()` is real, and is **the only one on a non-test path in the file** ([`shared_memory.rs:242`](../../os/src/exec/src/shared_memory.rs#L242)). On a function already returning `Result<_, SharedMemoryError>`, failing closed costs one line.

Offered at its true weight rather than inflated: the comment *"validated present in the loop above"*
rests on a second unstated assumption beyond SMP — that the two address spaces share no page-table
structure covering `owner_virt`, which `attach_shared_pd` makes possible in principle. **No reachable
case was constructed**, and Rust's borrow rules already prevent the obvious one. It does not add a
finding; it slightly strengthens the fix `LE-40` already names.

## The process note, narrowed

Handover 29's trap 2 said an external reviewer's confidence is not evidence. The verification pass
tested that claim against itself and it held — but **the durable version is narrower than "reviewers
are unreliable"**:

> A claim about code is checked against the file. A detector that returns nothing is exercised before
> its nothing is believed.

The second sentence is the one that generalises, and it applies to this project's own audit output,
not only to outside reviewers. Both downgrades were accurate; the security-vulnerability report was
correctly demoted; and the one thing that *changed* under verification was a claim of this
repository's own making — that pinning the JSON was the available enforcement.

## What is still owed

**Two decisions, unchanged from [Handover 29](29-next-session-mandate.md) and still the first thing
to settle:**

1. **`LE-39`** — `ADR 0004`'s premise does not hold against Secure Group 1 interrupts. Recommended:
   `ADR 0005` superseding `0004`, making the real-time tier conditional on per-platform secure-world
   qualification. Blocks every future ARM64 WCET claim.
2. **`LE-41`** — declared MIT, no `LICENSE` file. The only deadline in the register; the relicensing
   window closes on the first outside contribution and announces nothing when it does.

**And one piece of work now well-specified:** `LE-37`'s SIMD-absence check as a real `xtask` gate
over the built AArch64 artifact. It is better understood than when the row was written, and **trap 1
still applies** — it needs a Red first, and it is in no clause of `TEST-P1-07-02-A`.

## For the next session

`STORY-P0-01-06` is the template. It took a domain nobody had worked, asked what actually blocks each
of its 25 gates, and produced three things a reading could not: one gate closed from real
measurement, `LE-31` evidenced rather than argued, and `LE-42` — a 30× budget overshoot in a path
that had never been timed.

**Twenty-four of twenty-five blockers were not the hardware everyone assumed.** The other
twenty-four domains have not been asked the same question.
