# 23A — The keyhole, the clamp, and the lie

Follows [`22A`](22A-the-usable-os-is-on-the-board.md), same date. Three fixes
taken at the owner's direction after a review of `22A`'s own SWOT: two the SWOT
identified and recorded rather than fixed, and one it correctly deferred.

**The one sentence, if only one survives:** *The command line the board accepts
went from 30 octets to 128 — the width its own shell has always accepted, and a
width the containment argument never forbade because that argument needed `>=`
and was written with `==`; the untested arch-only clamp became five arch-neutral
tests; and `VER` stopped naming the machine it was compiled on.*

## 1. The keyhole was self-imposed

`22A` filed the 30-octet command line as a documented decision:
`FIND /N "soul" README.TXT` does not fit, stated as a known bound. The bound was
real. Its justification was not.

`COMMAND_PAYLOAD_BYTES` was 46, chosen so `14 + 46` is **exactly**
`gem::MINIMUM_FRAME_LEN`, so that no sending NIC's padding could reach the
board's fixed-width classifier. That reasoning is sound and it is also **the
wrong quantifier**: Ethernet pads a short frame *up to* 60 octets and never pads
one already at or above it. Immunity needs `frame >= 60`, never `frame == 60`.
Every width from 46 upward carries the identical guarantee.

The tree already knew this and only the command envelope had forgotten:
`gem::text_frame` pads *up to* the minimum, and `gem::TEXT_FRAME_CAPACITY` is
`14 + 256`. The board has been transmitting 270-octet frames for days.

So the field is now **128 octets, and the number is derived rather than picked**:
`shell::capacities::MAX_LINE` is 128 and `shell::dos` refuses a longer line, so
128 is the widest line the thing on the far side can accept. A wider field could
only carry lines the shell rejects; a narrower one hands a 128-capable runner a
keyhole. `hal-arm64` cannot name `shell` without a cycle, so **the composition
root holds the two constants equal at compile time** — the same argument
`hal_arm64::wire_shell` already makes for passing arrays instead of pointers,
applied to the width of the line rather than the width of the buffer.

Cost, measured: `kernel8.img` 525,624 → 525,728 octets. **104 octets of image for
98 octets of command line.**

The refusal is untouched. `classify` still rejects any payload that is not this
width to the octet, and `ADMITTED_CAPACITY`'s sixteen spare still make
`Oversize` reachable *and* able to say how far over.

`LE-122`'s row said the width "should not move to 50 **to paper over this**" —
declining to disguise a defect. Moving it deliberately, for a reason stated where
the old reason lived, is the opposite act, and the old reason was corrected in
place rather than left standing beside the new one.

## 2. The clamp that no test could reach

`hal-arm64/src/wire_shell.rs` had **zero tests**. It is
`cfg(target_arch = "aarch64")`-only, so no host test could reach it, and it
contained three real arithmetic decisions: how much of a line is copied, what
happens to the rest of the field, and what a length returned across an
`extern "C"` boundary means. `22A`'s SWOT named this exactly — *"I wrote about
the hazard in the module header instead of extracting the clamp… writing a
comment about an untestable seam is not the same as removing it"* — and then did
not remove it.

It is removed. `stage_line` and `clamp_written` are arch-neutral, unconditional
and host-tested; the `cfg` block keeps only the `extern` call it cannot be
without. `stage_line` also **zeroes the field tail**, which the original did not:
a short line must not be able to show the far side whatever the previous command
left in the buffer.

## 3. `LE-124`: the constant that was true of every context

`VER` answered `TinyOS Version 0.2.0 (Tier 0, x86_64)` **on an AArch64 board**,
and `TEST-P2-07-01-A`'s byte-exact golden gate was the thing *requiring* the
false string.

`Platform { tier, arch }` is now injected into `World` exactly as `tasks` and
`spoors` already are, so the verb core carries no fact about its host.
`pi5-image` supplies `Tier 1 / aarch64` — **the board's own vocabulary**, the
same pair `hal_arm64::timer` announces in every `TOS64-MEAS/2` envelope, so a
reader holding a measurement capture beside a `SHELL VER` transcript sees one
machine described one way.

**The golden transcript did not change.** The row's disposition assumed it would
— *"regenerate `golden/parity-smoke.golden.txt` DELIBERATELY and review it as a
diff"* — but the Tier 0 fixture supplies the string it always produced, so
`p1_transcript_matches_golden` passes untouched. The cost the row budgeted for
was avoidable, and avoiding it is strictly safer than paying it.

Row closed. **The silicon re-confirmation is owed**: `REPORT-2026-08-08-02`
captured the defect on the wire before the fix existed, and one boot turns that
record from *confirmed defect* into *confirmed, fixed, re-confirmed*.

## 4. Every guard was mutation-verified, and one of them fails at compile time

`22A`'s SWOT recorded that its TDD was uneven — two of four areas
implementation-first. Rather than re-litigate that, every guard added here was
**broken on purpose and observed red**:

| Mutation | What went red |
|---|---|
| `stage_line` drops `.min(field.len())` | the truncation test, by **panic** in `copy_from_slice` |
| `stage_line` drops the tail zeroing | two tests, on residue from the previous command |
| `VER` reverted to the literal | the `shell` test **and** the board test |
| `ARGUMENT_BYTES` 128 → 30 | **compile error** — `pi5-image`'s const assertion |
| `ARGUMENT_BYTES` → 10 (frame under 60) | the padding test, naming the reason |
| host `PayloadBytes` 144 → 46 | four C# tests, including the source-parity gate |

The fourth is the one worth keeping: a width disagreement between the wire and
the shell is **not a test failure, it is a build failure**. Nothing can be run
in that state, so nothing can be observed passing in it.

The source-parity gate needed teaching rather than reverting. It parses
constants out of the Rust with `pub const (\w+): usize = (\d+);`, and
`COMMAND_PAYLOAD_BYTES` is now arithmetic (`HEADER_BYTES + ARGUMENT_BYTES`). The
gate resolves derived constants in two passes instead — a width stated as
arithmetic over its own field layout is the form least likely to drift when a
field moves, and the gate exists to check the **value** both ends agree on, not
the syntax the board writes it in.

## 5. What this did not touch, and it is now the binding constraint

**The answer is narrower than the question.** Today's work widened the *input*;
the *output* keyhole is now the one that binds. A two-file `DIR` already
truncates with `more=16`, because the answer rides a text frame bounded by
`transcript::MAX_LINE_BYTES` (256). Unlike the command envelope, **this bound is
real** — it is `LE-120`'s derivation, not an inherited constant — so widening it
is a design step (a continuation protocol, or a larger frame within the ~1486
octets an MTU allows) and not a constant bump. It wants its own change and it
did not get one here.

Beside it, and larger: the board answers **one line per park beat**, and the beat
is ~1 Hz. That rate bound *is* `SEC-20`'s amplification containment, so it is a
security decision rather than a tuning knob. A terminal a human would call
responsive needs sub-100 ms, and nothing in this session moved it.

## 6. What the next session does, in order

1. **The re-confirmation boot.** `SHELL VER` must answer `(Tier 1, aarch64)` on
   silicon, which closes the loop `REPORT-2026-08-08-02` opened.
2. **`21A` §5 item 3** — make `emit-dashboard` and `emit-feasibility` *write*.
   Deferred by `22A` and again by this session; the evidence for it is now
   overwhelming and it is still the cheapest item in the mandate.
3. **The answer width**, per §5 — the binding constraint on anything that could
   be called a UX.
4. `20A` §7 and `22A` §8's remaining items, unchanged and still not cancelled.

## 7. Standing instruction earned

**A constraint's quantifier is part of the constraint.** `frame == minimum` and
`frame >= minimum` protect against exactly the same attack, and one of them cost
this project 98 octets of command line and a documented usability limit that was
never required. The argument was correct, carefully written, and load-bearing —
which is precisely why nobody re-derived it for four days. When a bound is
inherited rather than re-measured, check the comparison operator before checking
the number.
