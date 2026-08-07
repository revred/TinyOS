# 06D — Coherence review: the ceiling nobody is warned about, and the directory no gate reads

Requested review of `04E` and `05E`, plus a coherence pass over goals, technical
debt, folder structure and the 20,000-line crate rule. Session letter **`D`**,
continuing from [`02D`](02D-le-98.md) and [`03D`](03D-the-board-is-still-talking.md).

**The one sentence, if only one survives:** *`xtask` is at 14,422 of 20,000 and
is the crate every new gate lands in, while the standard's own trigger for
splitting — 18,000, by its worked example — has no machine behind it; and
`work/tools/`'s 6,079 lines of bench instruments, including two real test
suites, are read by no gate at all.*

---

## 1. `04E` and `05E` — verified, and they hold

Every checkable claim in `04E` was checked at source rather than read:

| claim | verified |
|---|---|
| `check-feasibility` in `ci.yml` **and** `CI_ENFORCED` (7 checks) | both, and the count is 7 |
| the gate passes on the committed tree | `feasibility.html agrees with the live spine` |
| `EPIC-P2` blocked on a closed row | header says `Blocked on a storage decision (LE-48)`; `LE-48` closed `hand-2026-07-29/16G` — **nine days** |
| `LE-112` raised | open |

**`04E`'s central judgement is right and is the most valuable thing either
document says:** closing the entire evidence gap moves `25 / 220` to
`220 / 220` and does not change the verdict, because the verdict is about
*goals* and four of six have no code. A session that spends itself on
measurement does honest work that leaves the headline where it found it. That
deserved to be stated first and it was.

**`05E` is correctly built against it** and its ordering is right: `Q2` first
because it is the largest gap and the cheapest, then `LE-103`, then the
read-and-re-harvest rows. One thing I would strengthen — `05E` §"Do not start"
carries `LE-112` as *"correcting that clause belongs to whoever owns the
Epic"*, which is the right call, but the reason deserves to be louder: every
triage since 2026-07-29 has read `EPIC-P2` as gated on a decision that was
already taken, so the cost of leaving it is another session mis-triaging, not
merely a stale sentence.

**Both documents are honest about their own residues** — `HOST_TESTS` as an
ungated literal, the distance column as a judgement in source, `git add -A`
recorded against the session twice. That is the standard this tree holds and
they meet it.

## 2. The 20,000-line rule warns 2,000 lines after the standard says to act

This is the finding the review was asked for and it is measurable.

`agent.md` rule 4: *"If you're approaching that limit, split the crate — do not
ask for an exception; there isn't one."* `CODING_STANDARDS.md` §81 repeats it
and its **worked example uses 18,000** as the number that counts as
approaching.

`check-crate-sizes` implements none of that. It is a **binary refusal at the
ceiling** — passes at 19,999, fails at 20,001 — so the only machine signal
arrives roughly 2,000 lines *after* the documented trigger. A breach is
therefore discovered by a red build, and splitting a crate under a red build is
an architectural decision taken under time pressure, which is the exact
circumstance `CODING_STANDARDS` exists to prevent.

**Measured across the last six commits**, raw `.rs` lines under `os/src/xtask/`:

```text
2026-08-06  fb3f36c  16,326
2026-08-06  e273931  18,991      (LE-91)
2026-08-06  dff7b1d  19,434      (LE-100)
2026-08-07  da77cef  19,934      (LE-109/110)
2026-08-07  4b58393  20,636      (feasibility)
```

Counted against the ceiling — tests excluded, as rule 4 specifies — `xtask` is
**14,422**, up from 12,926 on 2026-08-06. Roughly **1,500 lines in a day and a
half of sessions**, which reaches the 18,000 worked example in about two and a
half more comparable sessions.

**The growth is not incidental, it is this project's governance practice.** A
loose end is closed by adding a gate, and every gate lands in `xtask`:
`metric_labels.rs` 1,193 · `dashboard.rs` 1,195 · `spine_tests.rs` 1,122 ·
`release_status.rs` 785 · `ci_gates.rs` · `feasibility.rs`. **Closing loose ends
has a monotonically increasing cost in exactly one crate, and nothing reports
the trend.**

Filed as **`LE-113`**, in two parts deliberately. Part one is cheap and is not
new policy: make `check-crate-sizes` print headroom on every run and refuse at
the standard's own 18,000 trigger — that is the existing rule made executable,
and printing always matters because a number nobody sees until it fails is what
`LE-108` was filed about. **Part two, the split itself, is explicitly not that
row**: the seams are visible in the module list (assurance, reporting, board
orchestration, timing) but which become crates is a judgement about what `xtask`
*is*, and taking it reactively at 20,000 is the failure being prevented.

## 3. Folder structure: one top-level code directory is outside every gate

`work/tools/` holds **6,079 lines of C# across 27 files** — `tos64-netboot`,
`tos64-power`, `ti64dink`, `sdprep`, `cardswap`, `linkwatch`, `imgwrite`,
`serialwatch` — plus two real test projects, `netboot.tests` and `power.tests`,
recorded in the handovers at 54 and 99 passing tests.

**Nothing runs them.** `ci.yml` contains **zero** occurrences of `dotnet`;
`ci_gates::CI_ENFORCED` names none of them; the pre-commit hook runs none of
them. `check-crate-sizes` measures Rust crates and does not see the directory,
so those 6,079 lines are outside the 20,000-line discipline as well.

**This is `LE-100` for the other language**, filed nine days after `LE-100`
closed on *a gate is only as strong as the weakest place it is actually
executed*. And it is worse in one specific way: these are the **bench
instruments**, and the two most expensive instrument defects this project has
recorded live in them — `LE-80` (`ti64dink` decoding a live rung as an absence
and exiting 1 as a timeout) and `LE-87` (two `tos64-netboot` instances on UDP 69,
the stale one silently winning and serving a wrong image that produced a
complete, plausible, entirely wrong envelope). **The tools whose failures cost
the most bench time are the tools whose tests gate nothing.**

Filed as **`LE-114`**, with the two cautions that stop it being a one-liner: the
runner has no dotnet SDK installed today, and the first Linux run may surface
tests that pass on this Windows bench because the tools do NIC enumeration and
raw capture — `LE-64`'s family, and the reason to land it alone.

### What is coherent, and worth not disturbing

- **`agent.md` rule 7** (*"All code lives under `os/src/`"*) reads as absolute
  but qualifies itself with *"Every Rust crate, every workspace member"*, so
  `work/tools/` is not a violation. The wording could invite one; the rule is
  sound.
- **`goals/` is the strongest part of this tree.** Registers are machine-checked
  for header, field count, key uniqueness and id contiguity; the dashboard and
  now the feasibility report are generated and byte-compared. `04E` extended
  that correctly rather than adding a second hand-maintained page.
- **`docs/` has no index**, so a note is discoverable only by listing the
  directory. Minor, and not filed — `SeedMVP.md` §12 is the cross-reference map
  and mostly serves this purpose.

## 4. A trap firing on the session reviewing it

Recorded because it is the fourth instance and it cost me a wrong belief for one
tool call.

I checked the spine with `... | grep -o "Expected \`[^\`]*\`" || echo "spine
green"` and read **"spine green"**. The gate had failed; my `grep` did not match
its error shape, so the fallback printed success. The real failure was that
`LE-113` cited `hand-2026-08-07/06D` — this document — before it existed.

**A pipeline whose fallback prints a reassuring word is an instrument that
cannot return both answers**, which is `01A`'s standing rule and `01B`'s
sharpening of it, applied to a shell one-liner rather than to an API. `05E` §"The
standing instruction that earned its place" already says this; it earns another
line. Read the gate's own output.

## 5. The board earned its keep, by refusing to answer

`05E` asks what makes a session worthwhile. `LE-110` says the Pi 5 is readable
without a relay. So this session tried to get **live** evidence out of it —
and the attempt is the result.

**What is actually live on the wire.** `03D` established that the
`TOS64-MEAS/2` envelope is a *replay* of the boot transcript, byte-identical to
the committed capture, so re-harvesting it yields nothing new. But
`TOS64-PRESENT/1` carries an **incrementing `seq` at the 1 Hz park beat**, and
that is not a replay — it is generated now. `seq` against wall clock measures
the board's beat **with interrupts live**, which is exactly the condition
`ADR 0015` added its `irq_state` column for and which every existing timing
number lacks: the spine prints *25 release gates with evidence (**0** measured
under real-time conditions)*.

**Measured, over a 176.392 s baseline:**

```text
seq 56550 @ 11:12:27.542Z   ->   seq 56725 @ 11:15:23.934Z
Δseq = 175   Δt = 176.392 s   period = 1.00795 s
```

**And it establishes nothing, which is the honest reading.** `ti64dink` prints
frame text with **no arrival timestamp**, so the best anchor available is *the
last `seq` seen before a capture ended* — uncertain by up to a whole beat at
each end. That puts the true period in `[0.99653, 1.01938] s`, an interval that
**contains the declared 1.000 s**. The measurement is consistent with 1 Hz and
cannot resolve better than ~1%.

**~1% is not good enough to be worth filing**, and the tree already says why:
it is larger than the 2% allowance `PERF-D05-G23` is stated against, and the
same order as the 0.7% for which `PERF-D11-G02` was **refused** on the reasoning
that a verdict which flips on a recompile is not a verdict. Filing this number
would be `LE-104`'s defect committed deliberately.

Filed as **`LE-115`**: the fix is a monotonic arrival timestamp per frame in
live mode — `Stopwatch` ticks from capture start, since only differences
matter. That collapses the anchor error from one beat to host jitter, and a
three-minute capture would then resolve the beat to better than 0.1%. The row
carries two cautions, because the easy mistake is worse than the gap: host
timestamps include Windows scheduling, Npcap buffering and NIC coalescing, so
per-frame **jitter** measured this way is the host's and must never be filed as
the board's — only the **mean rate** over a long window survives, because host
delay averages out and the board's counter does not.

**The board did not produce a number. It produced the reason there is no
number**, which is `LE-95`'s scope correction applied one level in: the relay
blocks booting a new image, and what blocks *this* measurement is a host tool
that discards a field.

## 5a. `LE-107` built, and mutation showed it covers less than I claimed

The Epic-enumerates-its-Features check is in, wired into
`check-assurance-spine`, and it passes because `02D` had already repaired
`EPIC-P1`.

**Mutating the real file rather than a fixture found the limit immediately.**
Deleting only the Features **table row** left the gate green — the `Status:`
header still names `FEAT-P1-11`, and one mention satisfies the check. Deleting
**every** mention fires it, naming the Epic, the Feature and the fix.

So the check catches the stronger half of the 2026-08-06 drift (`FEAT-P1-11`,
absent from both places) and **not** the weaker half (`FEAT-P1-12`, in the
table and missing from the header). Asserting per-location would mean deciding
which sentences are meant to enumerate Features — a judgement, not a scan, and
most Epics' headers legitimately stop. The scope is therefore *mentioned at
all*, which is where the four-handover drift lived; the limit is recorded in
the function's own doc comment, and the error message names the header half
explicitly so a reader fixing one does not leave the other.

## 6. For the next session

1. **`LE-113` part one** — headroom in `check-crate-sizes`, refusing at 18,000.
   Small, and it buys the time in which part two can be decided calmly.
2. **`LE-114`** — a `dotnet test` job. Land it alone.
3. **`05E`'s list stands unchanged** and is the substantive work: `Q2` first, it
   is a laptop afternoon and takes the qualification record from zero parts held
   to two of four.
4. **The two owner decisions in `04E` §3 gate everything else** — qualify a
   platform (blocked on a £15 relay), and lift or except the sprint rule for one
   not-started goal. Neither is a session's to take.

**Do not start:** `FEAT-P1-12`, `G09`/`LE-86`, `06A` §4.3, the DT parser
(`LE-98`), and the filesystem-on-a-database question — which now has a reference
note that deliberately stops short of decomposition, and should stay that way.
