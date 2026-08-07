# 04E — The report that cannot go stale, and what it shows

Follows [`01D`](01D-the-tiles-were-telling-the-truth-badly.md). Session letter
**`E`**: `A`–`D` are all claimed on this date, and `D` is now held by the
concurrent session that filed [`02D`](02D-le-98.md) and
[`03D`](03D-the-board-is-still-talking.md).

The owner asked for a live report on where TinyOS stands against its goals and
how close it is to being provably feasible. That report is
[`goals/feasibility.html`](../../goals/feasibility.html). This document records
what it is, the two findings that came out of building it, and — more usefully
— **what it says the gap actually is**, which is not what a reader would guess.

## 1. It is generated whole, and refused when stale

A hand-written status page is `LE-108`: the one number on `goals/index.html`
that no gate checked drifted by **33 Stories** while sitting two lines below
numbers that could not drift. So this page is not maintained.

```sh
cd os
cargo run -p xtask -- emit-feasibility > ../goals/feasibility.html   # refresh
cargo run -p xtask -- check-feasibility                              # refuse a stale one
```

`check-feasibility` byte-compares the committed page against a fresh render.
**Verified by mutating the real file** — changing one figure makes it fail with
the regeneration command in the error. It is wired into `ci.yml` *and* into
`ci_gates::CI_ENFORCED`, which now holds seven checks. That second half is not
belt-and-braces: `LE-106` records a gate this repository already owned, required
by nothing, which a session then re-derived by hand. **A mechanism nobody is
required to run is a mechanism that does not run.**

**Numbers derive; judgements are constants.** The verdict, the proven list and
the unproven list live in `os/src/xtask/src/feasibility.rs`, so changing a claim
is a reviewable source diff rather than an untracked edit to a page. That is
`dashboard`'s generated-tiles/gated-prose split with the boundary moved, because
this page is *all* claim and has no editorial half worth protecting.

**Built on the project's own terms, not an invented framework.** It leads with
`SeedMVP` §3's six founding goals, uses `SeedMVP`'s own bar — the MVP is *"the
smallest configuration that can **falsify** every in-scope goal"* — and carries
the nine landing zones with their `claim_gate` text verbatim, plus `G24`/`G25`,
the two marketing claims the project blocks on itself.

### The residues, named

- **`HOST_TESTS` is a literal** (1231). It is a property of a CI run rather than
  of this tree, and inventing a derivation would be worse than admitting the
  constant. It will go stale silently; nothing gates it.
- **The "distance" column on the six goals is a judgement**, mine, with no gate
  behind it. It is in source so it diffs, which is the most that is honest.
- The page reports **evidence**, and says so in those words: a gate carrying
  evidence is one somebody *measured*, never one that *passed*. Six rows are
  refusals.

## 2. `EPIC-P2` has been blocked on a closed row for nine days — `LE-112`

Deriving the Epic table surfaced this immediately:

```text
EPIC-P2 Status: "... Blocked on a storage decision (`LE-48`) ..."
LE-48         : closed, hand-2026-07-29/16G
```

The decision `LE-48` demanded — write down where a filesystem lives, or narrow
the file verbs to a RAM backing and annotate the verb table — **was taken**, by
the vertical slice that landed the `shell` crate and the `G-SEC-5` RAM volume.
The Epic never noticed.

The cost is already paid: every session triaging what to work on since
2026-07-29 has seen the operator command environment as gated on a storage
decision that had already been made, and the feasibility report reads that
state straight out of the header. Status headers are machine-checked for
grammar, for agreement with the rows beneath them, and for citations resolving
to real documents — but **nothing reads a loose-end id out of a header and asks
whether that row is still open.** `LE-107` is the same family one step along.

Filed rather than fixed. Correcting the clause is a judgement about whether the
RAM-volume answer is the one `EPIC-P2` *wants* or merely one it can live with,
and that belongs to whoever owns the Epic — a passing session rewriting it would
record a decision nobody took, which is the failure `LE-48` was raised about.

## 3. What the report actually shows, and it is not a measurement backlog

This is the part worth carrying forward. The gap splits in two and **only one
half is engineering.**

**The evidence gap.** The machinery exists and the numbers do not: 195 empty
closable gates, of which 69 are measurable today, 56 sit in domains with no
instrument, 70 need a mechanism built. Every one of these is ordinary work with
no external dependency.

**The capability gap.** Of six founding goals, **four have not started** —
host coexistence, install onto a laptop or Jetson-class device, hosting an
inference runtime, and taking orders from an LLM under audit. There is no code,
and there is no Epic document for Phases 3 through 8 at all.

Closing the *entire* evidence gap would move `25 / 220` to `220 / 220` and
would **not change the verdict**, because the verdict is about goals. Two owner
decisions gate that:

1. **Qualify one platform under ADR 0005.** Unlocks 10 `G04` gates and makes
   assurance `verified` reachable for the first time in the project's history.
   `Q2` is a laptop afternoon, `Q4` is already written, `Q1` needs one boot,
   `Q3` needs `LE-103`'s instrument corrected. Gated on `LE-95` — a £15 relay.
2. **Open an Epic for one not-started goal**, which the 2026-07-30
   hardware-evidence sprint rule currently forbids. Until that rule is lifted
   or an exception made, four of six goals cannot move by definition.

**Neither is a session's to take.** That is the honest headline of the report,
and [`05E`](05E-cover-note-for-the-next-session.md) is written against it.

---

**Written 2026-08-07.** Loose ends: `LE-112` raised. Gates: `cargo fmt --all
--check`, `cargo run -p xtask -- check-lints`, `cargo test --workspace`,
`check-assurance-spine`, `check-ci-gates` (7 enforced), `check-feasibility`. No
board crate touched. **One process failure to record against this session:**
`git add -A` swept a concurrent session's uncommitted work into two of my
commits, `231a6db` and `4716767`, under unrelated messages — the second time
after I had documented the trap in `01C`. `git status` before staging, and
stage paths rather than `-A` while another session is live.
