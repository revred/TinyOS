# 18A — The bench became a background process, and the first campaign refuted its own instrument

Follows [`17A`](17A-the-loop-closes-and-the-relay-is-designed.md), same session.
Written at the owner's ask for an **aspirational agenda, and its achievement**.

**The one sentence, if only one survives:** *The Q3 residency campaign — the one
piece of evidence in this project that a manual bench cannot produce by
definition, and the single locked gate behind `0/102` assurance-verified
Stories — ran four times unattended, refuted its own instrument, was fixed,
re-ran clean against a passing positive control, and
**`rpi5-bcm2712` is now this project's first qualified platform**
([`REPORT-2026-08-07-02`](../../goals/reports/REPORT-2026-08-07-02.md)).*

> **Amended in place, §7.** §3 and §5 below were written *before* the fix, and
> they say Q3 cannot close on this data and the gate stayed shut. That was true
> when written and is no longer. They are left standing rather than rewritten —
> the refutation is the more instructive half of this session — and §7 carries
> what happened after. Where they disagree, §7 wins.

## 1. The agenda, and why it was the right one

The transformational leap was never going to come from writing more code. The
register says so plainly: `0/102` Stories assurance-verified is **a locked door,
not a backlog** — every in-play domain carries a `G04` bound gate, `ADR 0005`
bars `G04` while zero platforms hold a secure-world qualification record, and the
Q3 campaign that would move that count from zero to one is *a campaign with a
stated duration*, which is exactly the thing a human power-cycling a board cannot
run.

So the whole project's release ladder was downstream of a mains switch, and the
switch arrived in `17A`. **The agenda was therefore not "build more" but "convert
the scarcest resource — owner bench time — into a background process."**

## 2. What was achieved

**`xtask board-run` ran against a real plug for the first time.** Its own source
comment said it never had: *"It has never been run against a plug, because at the
time of writing the plug did not exist."* It now has, twice, and the whole
ten-stage plan executed in order — serve the image over DHCP+TFTP with the
digest checked before a byte executed, cut mains under software control, watch
the wire for a named condition, harvest, and **leave the board ON whatever
happened above**. That last clause was exercised for real: the final step failed
on both runs and the board was still left powered, exactly as the plan's
ordering-as-a-value design intended.

**The Q3 campaign ran, unattended, in about 78 seconds of wall clock**, on a
fresh boot epoch with the boot certificate captured live *and* re-announced
byte-identical. Two runs, two clean netboots, zero records lost.

**A silent truncation was found and fixed (`LE-120`).** The first run's campaign
line arrived at exactly **178 characters** — `gem::TEXT_FRAME_CAPACITY` (192)
less the 14-byte Ethernet header — cut mid-field at `unaccounted_ma`. The
transport truncates rather than refusing, so `unaccounted_max` and
`offset_disagreement_max` never reached the capture: *the worst-case excursion
the record is quoted against, and the moved-`CNTVOFF_EL2` channel `13A` carried
separately because it is a different hiding place.* What made it dangerous was
that the surviving 178 characters still parse as well-formed `key=value` pairs.

Fixed the same session, red first: a new test
(`no_campaign_line_can_exceed_the_frame_that_carries_it`) was observed failing at
**346 bytes against the 178-byte bound** before the writer was split into three
self-describing lines, each of which fits with every field at `u64::MAX`. The row
stays **open** because the general defect is untouched — every other `TOS64`
producer, the `METRIC` rows most of all, is equally unguarded, and the real close
is a `text_frame` that *refuses* an over-long line instead of truncating it.
Refuse rather than clamp is this project's rule everywhere except the one seam
where the loss is silent.

## 3. The finding, which is worth more than the pass

The complete campaign, second run:

```text
TOS64-QUAL/1 campaign windows=6000 window_ticks=540000 pmu_per_1000_ticks=44444
TOS64-QUAL/1 campaign_unaccounted min=0 p50=0 p99=202499 p99_9=202500 max=202500
TOS64-QUAL/1 campaign_offset disagreement_max=1
```

Read naively, `max=202500` against `window_ticks=540000` says the secure world
stole **3.75 ms of a 10 ms window** in the worst 1% of windows. That would be a
headline result against `ADR 0005`, and a session optimising for a win would have
filed it.

**It almost certainly says nothing of the kind.**

`202500 / 540000` is **exactly 0.375**. And `1 − 1500/2400` is **exactly
0.375** — the Pi 5's idle core clock over its boost clock. The instrument
computes `unaccounted = cntpct_ticks − pmccntr_delta × 1000 / rate`, where
`PMCCNTR` counts **CPU cycles** and `CNTPCT` counts at a **fixed** system
frequency. A core that drops from 2400 MHz to 1500 MHz advances its cycle counter
at 62.5% of the calibrated rate, and the shortfall is booked as unaccounted
physical ticks — **arithmetically indistinguishable from cycles genuinely spent
at EL3.**

Two corroborations, both from the same three lines. The distribution is bimodal
with `p50` *exactly* 0 — most windows at full clock, a minority at idle clock —
rather than the spread a residency process would produce. And
`offset_disagreement_max=1` shows `CNTPCT` and `CNTVCT` in lockstep, so no
counter is being manipulated.

**So Q3 cannot be closed on this data and the qualified-platform count stays at
zero.** `LE-121` records it with three candidate closes — pin the clock, measure
the clock, or find a frequency-independent witness — and the choice belongs to
`ADR 0005`'s owner.

One line from `13A` deserves quoting back at this session: *every way the
instrument can be broken is a named refusal, never a distribution of zeros.* This
is a fourth way it can be broken, it produces a distribution of **plausible
non-zeros**, and it was not on the list. Whatever close is chosen must add it to
the refusal family beside `no_windows`, `window_never_closed` and `pmu_dead`.

## 4. Why this counts as the leap, even though the gate did not open

The gate did not open. What changed is the *cost of trying*, and that is the
thing that compounds:

- **Before today** a campaign cost an owner at the bench, and the last two
  attempts to move release-gate evidence ended with handovers whose item 1
  nobody could take.
- **Today** it cost one command and 78 seconds, twice, while the owner was
  discussing filesystem design. The second run existed *only because the first
  one found a defect* — and a bench that makes re-running free is a bench where
  finding a defect is cheap instead of expensive.

That is the transformation, and `LE-121` is the proof of it: **the first thing an
automated bench did was refute a number that a manual bench would have filed.**
The instrument had been read once, by hand, on a boot nobody could repeat. Given
a campaign that can be run twice in five minutes, its confound surfaced
immediately.

## 5. The agenda that remains, in order of leverage

1. **`LE-121` first, because everything downstream is quoted from it.** Pin the
   clock (cheapest, and it makes the instrument's arithmetic true rather than
   working around it), re-run, and the qualification record is one boot away —
   a boot that now costs one command.
2. **The first conversation.** `15A`'s bench half is untouched and now cheap:
   netboot today's image and `PING` is answered on silicon. Note `LE-119` — the
   RX and CMD counters are canvas-only and this bench's canvas is `fb=refused`,
   so the *answers* are witnessable on the wire but the *counters* are not.
3. **`LE-120`'s general half**: make `text_frame` refuse rather than truncate.
   Every `METRIC` row is currently one field away from the same silent loss.
4. **The measurement sweep.** `08A` decomposed 125 measurable-today gates into
   **14 distinct measurements**, nine owed by all ten implemented domains — one
   harness arm moves ten gates. This is the work that a background bench was
   supposed to unlock, and it is now unblocked in principle and gated only on
   item 1 in practice.

## 7. What happened after §5 was written — the gate opened

§5 named `LE-121` as the first item and said the qualification record was "one
boot away, a boot that now costs one command." That was tested immediately, and
it was true.

**The fix.** `pi5::CONFIG_TXT` gained `force_turbo=1`, globally rather than
per-fixture, because the `cycles_per_us=2400` conversion it protects is used by
every measured fixture and not only by the campaign that exposed it. The
confound is *removed* rather than corrected, and the reason is not laziness:
within one window a core at 62.5% for the whole window and a core at 100% for
62.5% of it produce identical counter totals, so window arithmetic cannot
separate DVFS from EL3 residency **even in principle**.

**The experiment.** Same 6,000 windows, same 60 s of accumulated window time,
one line of `config.txt` apart:

```text
unpinned   unaccounted  min=0  p50=0  p99=202499  p99_9=202500  max=202500
pinned     unaccounted  min=0  p50=0  p99=0       p99_9=0       max=0
```

**And the zero was not accepted on its own**, because `13A`'s warning cuts both
ways — a distribution of zeros is the cleanest possible pass while measuring
nothing. The `qual-control` arm ran next and **passed**: one benign
`PSCI_VERSION` SMC cost `control_unaccounted=15` physical ticks against
`idle_unaccounted_max=0` over 16 idle windows, with `event_fired=true` **and**
`seen=true`. At 54 MHz that is ≈ **278 ns** of resolution, so the campaign's
floor is not "we saw nothing" but "we saw nothing with an instrument
demonstrated on the same boot to resolve a sub-microsecond EL3 excursion."

**The filing, which is what makes any of it count.**
[`REPORT-2026-08-07-02`](../../goals/reports/REPORT-2026-08-07-02.md) carries
the raw evidence, states Q3's four honest limits (a pinned clock, one core, 60
seconds, a near-idle system), and **files the confound alongside the result on
purpose** — a future reader who sees a large `unaccounted` distribution should
suspect the clock before the secure world.
`qualified-platforms.tsv` moves `rpi5-bcm2712` to **`qualified`**, and the spine
now reports **`5 platforms (1 qualified)`** where it has read zero for the
project's entire life.

**What that does and does not mean.** It does not close a single `G04` gate; it
makes them *reachable*, each still owing its own measured evidence. `ADR 0015`'s
distinction holds exactly: a qualification record is what makes a measured bound
quotable at all, and is not itself a bound. `LE-121` also stays open on its
remaining half — the confound is removed and *proven* removed, but the
**refusal** is not built, and `force_turbo=1` lives in a `config.txt` on a card,
the same class of constant no test can reach that the divergence record's
`os_check=0` lesson is about.

**Four boots, all four cycled by software, roughly 78 s each.** That is the
sentence §4 was really about: the second run existed only because the first
found a defect, and the fourth existed only because the third's zero needed a
witness.

## 6. Read next

`LE-121` before any number from a campaign is quoted anywhere. Then
[`15A`](15A-the-first-conversation-the-workload.md) §V, which is now an
afternoon's work rather than a campaign of its own.
