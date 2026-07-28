# Handover 34 — Next-Session Mandate: Nothing Is Blocked on a Decision, and One Artifact Did Not Get the Memo

The start-here document. Supersedes [Handover 32](32-next-session-mandate.md), whose step 1 is now
done. **[Handover 33](33-two-decisions-settled.md) is the record of the decisions and is not
superseded** — this document grounds it, adds what it did not catch, and states the work order.

`main` is at `d89c00a` and is **one commit ahead of `origin`. It has not been pushed.**

## Handover 33, grounded

Every claim in it was checked against the tree rather than inherited. All of them hold:

| Claim | Verified |
| --- | --- |
| `ADR 0005` supersedes `0004`, with `Q1`–`Q4` and the positive-control rule | Present, §§"What a secure-world qualification record contains" and "The trap this ADR sets against itself" |
| `ADR 0004`'s body unedited, status header + forward pointer only | Diff is exactly one status line plus a blockquote — nothing else changed |
| `ADR 0006` confirms MIT with three rejected alternatives | Present, four options presented and the reasoning recorded |
| `LE-39` and `LE-41` closed with `closed_in` populated | Both `closed`, both cite `hand-2026-07-28/33` |
| `LE-33` grew rather than splitting | Second condition on the existing row, correctly |
| README, `EPIC-P1`, both dashboards reconciled | `README.md:115`, `EPIC-P1` as a dated amendment with the original paragraph intact, session index corrected to 42/29 |
| Spine green, 549 tests | 42 loose ends (29 open), 84 status headers; 549 passing |

The `EPIC-P1` amendment is worth singling out as the right pattern: the superseded paragraph is left
as written and an amendment note added beneath it, so a reader meets the old claim *and* the reason
it moved. That is `ADR 0004`'s treatment applied one level up.

## What Handover 33 did not catch — `LE-43`

**`ADR 0005` changed what closing `LE-09` means, and the two artifacts that carry `LE-09`'s closure
condition were not amended with it.**

- `LE-09`'s `owner_path` still reads *"a Pi 5 must produce a measurement."*
- `FEAT-P1-07`'s exit criteria still ask only for *"a Report stating board revision, firmware version,
  clock policy and thermal state."*

Neither mentions qualification. Neither links to `ADR 0005` — inside `goals/`, only `EPIC-P1.md` and
`index.html` do.

Handover 33 states the correct reading, and states it well: closing `LE-09` yields **mechanism
evidence and the first `Q3` campaign, not a bound.** But **a session working `FEAT-P1-07` reads the
Feature, not the handover.** That is the prose-versus-register failure in a new place, and it is the
fourth instance this week — `LE-28`, `LE-33`, `LE-36`, now `LE-43`. The narrative knows; the
machine-readable artifact does not.

Registered rather than described. The fix is a dated amendment to both, in `EPIC-P1`'s style,
separating the two claims that are currently fused: **a measurement establishes the tier; a bound
additionally needs a qualification record.** It pairs with `LE-33`'s second condition, which is
precisely the lint that would catch the error this row describes.

## What the two decisions changed, in one place

**`LE-39` → `ADR 0005`.** The real-time tier is now a property of a **qualified platform**, not of an
architecture. **Zero platforms are qualified, the Pi 5 included.** `ADR 0004`'s case against x86_64
survives untouched and is restated in full; what did not survive is its election of ARM64, because a
GIC's secure-group interrupts routed to `EL3` by `SCR_EL3.FIQ` are taken irrespective of `PSTATE.I`
at NS-EL1 — and NS-EL1 cannot read `SCR_EL3` to know. The distinction that makes the repair
proportionate: **on x86_64 the defect belongs to the architecture; on AArch64 it belongs to how one
platform's firmware configured one GIC.**

**`LE-41` → `ADR 0006`.** MIT confirmed, open-core dropped, fork-and-close accepted **in writing** as
a risk rather than left unstated. No manifest or build change follows — `LICENSE`, the workspace key
and all seven crates already agree. **Outside contributions now need no licensing gate**, which is
the operational consequence worth remembering.

## What to do

**If a serial adapter is in your hand, take the board.** Still the highest-value session available by
a wide margin, and Handover 26 §"If an adapter *is* in your hand" remains exactly right and
unsuperseded. One session closes both `STORY-P1-07-01`'s capture and `STORY-P1-07-02`'s clause 2.

**What `ADR 0005` adds to that session**: the `current_el=` line is the first fact anyone has about
this board's exception-level configuration, so the session also produces the first `Q1` and the
beginning of `Q2`. **It does not produce a bound.** Do not let a successful capture be written up as
one.

**If there is no board time**, in this order:

1. **`LE-43`** — amend `FEAT-P1-07` and `LE-09` to match `ADR 0005`. Small, and it changes what the
   next `FEAT-P1-07` session believes about its own exit criteria, so it compounds if left.
2. **The `-M virt` fixture.** [Handover 31](31-qemu-virt-fixture-scoping.md) is the scoping; §7 lists
   four decisions to settle before writing anything. `ADR 0005` does not touch it: `virt` produces no
   timing evidence by design, and a QEMU guest is not a qualifiable platform because its secure-world
   configuration is the emulator's rather than a product's.
3. **`LE-23`** — re-record the timing baseline from a CI run. `LE-24` may come free. One fix, two rows.
4. **`LE-30`** — generate the dashboard from `list-status`. Handover 33 hand-edited `goals/index.html`
   in six places, the fifth consecutive session to hand-edit it. That is the argument for the row.

## Traps

All six from [Handover 32](32-next-session-mandate.md) stand unchanged: a green ARM64 fixture is not
ARM64 coverage; do not patch `LE-37`/`LE-38` directly; an external reviewer's confidence is not
evidence and neither is your own; `LE-35` is load-bearing rather than theoretical; do not reach for
`--update-baseline` locally; validate a hand-edited machine-checked file before your next tool call.

**Handover 33 adds a seventh, and it is the sharpest one in the set:**

> **A qualification campaign is easy to fake by accident.** Not by dishonesty — by running an
> instrument that has never been shown to detect anything, getting a zero, and filing it. An
> excursion not observed is not an excursion that cannot occur.

`ADR 0005` therefore makes a `Q3` **inadmissible without a positive control in the same Report**. This
is the same discipline that padded the `.org` guard past 128 bytes before trusting it, and that
self-tested the SIMD detector on `v1.16b` and `fadd s0` before believing its zero. Three independent
arrivals at one rule is not a coincidence — it is this project's most reliable instinct, and it is
now written into an ADR.

**An eighth, from this document:** `main` is one commit ahead of `origin` and unpushed. Check before
you start, and say so in your handover if you push someone else's commit.

## State at the close

```text
main                    d89c00a — NOT PUSHED, one ahead of origin
assurance spine         23 Features, 58 Stories, 45 Tests, 46 Reports
                        43 loose ends (30 open), 84 status headers
                        11 release gates with dated evidence, of 391
host tests              549 across the workspace; hal-arm64 at 115
Stories verified        0 / 58
open decisions          none — LE-39 and LE-41 both closed in Handover 33
ADRs                    0005 accepted (supersedes 0004), 0006 accepted
platforms qualified     zero, the Pi 5 included
best available work     a board session, if an adapter exists
next best               LE-43, then the -M virt fixture
```

`goals/reports/_soak-p0-03-01.log` has been dirty in the working tree for several sessions. It belongs
to whoever is running that soak; leave it.
