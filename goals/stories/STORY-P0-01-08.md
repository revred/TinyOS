# STORY-P0-01-08 — The Dashboard Stops Being Hand-Maintained

Status: **Functionally Verified (Host), 2026-07-28** — assurance state `baseline-debt` (no guardrail closed, per the Report itself); delivered by [`REPORT-2026-07-28-11`](../reports/REPORT-2026-07-28-11.md), Pass on all five clauses. *Header corrected 2026-08-01, four days stale*: it still read `Specified` while the Story's own filed Report recorded the pass, and every gate stayed green because the badge gate compares the dashboard to this header, not this header to the Report — the gap is registered as `LE-65`. Corrected only after re-verifying the evidence against the current tree (241 `xtask` host tests, `emit-dashboard`'s byte-compared region, the badge and count gates), per Handover 35's verify-don't-inherit rule
Feature: [`FEAT-P0-01`](../features/FEAT-P0-01.md)
Introduced in: [`session/hand-2026-07-28/41A-the-dashboard-as-a-work-order.md`](../../session/hand-2026-07-28/41A-the-dashboard-as-a-work-order.md) §1 and §3 (`L3`); registered as `LE-30`

## Description

`LE-30`, and the row's own evidence has been accumulating for nine sessions.

[`goals/index.html`](../index.html) is the page a reader meets first. Every number on it was copied
there by hand from `check-assurance-spine`'s output, it drifted three times in one day when the row
was raised, and **nine consecutive sessions have now paid the hand-sync price** rather than the
one-Story price. [Handover 41A](../../session/hand-2026-07-28/41A-the-dashboard-as-a-work-order.md)
re-synced four stale tiles and then watched two of its own figures go stale *while the sync was being
written* — Reports 46 → 47 and loose ends 44 → 46. That is the row proving itself inside the document
that was fixing it.

**The page has two kinds of content and they need different treatment, which is why this is not one
generator.** The stat tiles are pure spine arithmetic with no editorial content: they are generated.
The prose is an argument written by people and must stay that way: it is *gated*, by extracting only
the claims it makes — the spine-count sentence, and every Story's status badge — and refusing the
ones that disagree with the spine.

**The badge half is `LE-44`'s rule one document along, and it found the same defect class on first
contact.** Seven badges read `VERIFIED` for Stories whose own headers say `Functionally Verified` — a
weaker state carrying assurance debt a reader of the stronger word would not go looking for. **One of
the seven was written by the session that built the `LE-44` gate**, three days after it corrected
seven Feature-table cells for exactly this. That is the argument for the machine rather than against
it: the same person, holding the same rule, made the same mistake one document along within the week.

It also derives a number `41A` computed by hand and asked to have verified rather than trusted:
**345 of 391 in-play release gates are reachable with no board.** A ratio that argues about where
effort should go is exactly the kind of figure that must not be an assertion in a document nobody
re-checks.

## Depends on

[`STORY-P0-01-07`](STORY-P0-01-07.md) — the same pattern (a gate over a hand-maintained
cross-reference) and the same vocabulary of `Status:` states.

## Acceptance criteria

1. **The stat tiles are generated, and the generator is the fix.** `cargo run -p xtask -- emit-dashboard`
   prints the tile block from live spine data. `check-assurance-spine` byte-compares the committed
   page's marked region against it and, on a mismatch, prints the expected block so the fix is in the
   error rather than in someone's head. `emit-dashboard` **must not** run the dashboard check itself,
   because the command that prints the fix would otherwise refuse to run exactly when it is needed.
2. **The prose is gated, not generated.** The spine-count sentence and the loose-end count are
   extracted and compared; the paragraphs around them are untouched. This Story does not acquire the
   power to rewrite the page's argument.
3. **Every Story badge agrees with that Story's own `Status:` header.** The badge may append a tier —
   `FUNCTIONALLY VERIFIED (Tier 0 + Host)` — because that is genuinely extra information; it may not
   name a different state. A Story linked in prose without a badge is making no claim and is not
   checked. Applied to the committed tree: the seven overstatements are corrected, not grandfathered.
4. **`41A`'s reachability count is derived.** `391` in play, `345` reachable at Host or Tier 0, `46`
   needing a board, computed from the catalogue's own `tier` and `gate` columns and asserted against
   the hand count. `G24`/`G25` are excluded as `claim` gates. The `345 / 391` ratio becomes a tile.
5. **Every refusal is demonstrated.** As in `STORY-P0-01-07` clause 2: the committed tree satisfies
   all of the above by construction once it is fixed, so a green run is not evidence that any of it
   works. Host tests drive each refusal — a stale tile, a missing region, an unclosed region, a stale
   count sentence, a stale loose-end count, an overstated badge, a badge for a nonexistent Story —
   each with an acceptance case beside it.

## Named debt this Story leaves open

- **Only the tiles are generated.** The per-Story tables, their prose and their Report links stay
  hand-written. What is now impossible is for them to *contradict* the spine; what is still possible
  is for them to be incomplete. Generating them wholesale would destroy the editorial content that
  makes the page worth reading, which is a trade this Story declines.
- **`LE-34` is untouched.** `README.md`'s v1 supported-set list is the same failure mode in a third
  document and needs a state column of its own. This Story's shape transfers; the work does not.
- **`reachable` is not `easy`.** The `345 / 391` tile says no board is required. Many of those gates
  need real fixtures and some will fail their thresholds when finally measured — `LE-42` is what that
  looks like. The tile is a denominator correction, not a promise.
- **The badge vocabulary is a hand-maintained mapping.** Adding a `Status:` state requires deciding
  how the dashboard spells it, deliberately: the alternative is uppercasing the state string, which
  would let a new state onto the page without anyone choosing its wording.
- **No performance guardrail closes and no Story's assurance state moves.**

## Tests

[`TEST-P0-01-08-A`](../tests/TEST-P0-01-08-A.md) — written before implementation, per the TDD mandate.

## Reports

- [`REPORT-2026-07-28-11`](../reports/REPORT-2026-07-28-11.md)
