# ADR 0006 — MIT Is Confirmed as TinyOS's Licence, and Open-Core Optionality Is Dropped

Status: **Accepted**
Date: 2026-07-28
Introduced in: [`session/hand-2026-07-28/33-two-decisions-settled.md`](../../session/hand-2026-07-28/33-two-decisions-settled.md), settling `LE-41`
Decided by: the project owner, on this session's presentation of the four options below

## Context

`os/Cargo.toml` has declared `license = "MIT"` since the workspace was created, inherited by all
seven crates through `license.workspace = true`. A commercial review recorded as `LE-41` found two
problems with that, one mechanical and one strategic.

**The mechanical problem is closed.** A declared-but-absent licence is a diligence finding on its own
— any acquirer, customer or downstream packager checks for the file, not the manifest key. The
`LICENSE` file landed in `b663376`. `LICENSE`, the workspace `license` key and the seven inheriting
crates now agree.

**The strategic problem is what this ADR settles.** MIT is permissive. Anything TinyOS publishes
under it may be taken by a silicon vendor, an RTOS incumbent or a competitor, closed, extended and
shipped, with no obligation to return anything and no obligation to say so. That is the precise
outcome an open-core or dual-licence model exists to prevent, and it is not hypothetical for an
operating system aimed at data centres, local AI, UAVs, medical and industrial edge, and consumer
deployment.

**The decision was time-sensitive in a way nothing else in the register is.** Relicensing is possible
only while authorship is single-source. The moment a first outside contribution lands, relicensing
requires every contributor's agreement, and — the part that made this urgent — **nothing announces
the transition**. Every other open row in [`loose-ends.tsv`](../../goals/assurance/loose-ends.tsv)
can be deferred at a known cost. This one could not, so it was put to the owner as a decision rather
than carried as debt.

## Decision

**TinyOS is MIT-licensed. This is confirmed as a deliberate choice, not an inherited default, and
open-core optionality is dropped.**

Concretely:

1. **`LICENSE` (MIT) and `license = "MIT"` stand.** No relicensing is planned. No dual-licence, no
   open-core split, no proprietary-exception scheme.
2. **No Contributor Licence Agreement is required for the purpose of preserving relicensing
   optionality**, because there is no longer optionality to preserve. If a CLA or DCO is adopted
   later it will be for provenance, not for relicensing rights, and that is a different decision.
3. **The window closing is an accepted, recorded consequence.** Once outside contributions land, this
   decision is effectively permanent. That is understood and is the point: the ambiguity was costing
   more than the option was worth.
4. **Fork-and-close by a vendor or a competitor is an accepted risk**, explicitly, in writing, here —
   so that if it happens it is a known outcome of a made decision and not a discovered surprise.
5. **The commercial position rests on qualification, evidence and the assurance spine, not on
   withholding source.** See the Rationale.

## Rationale

- **Adoption is the binding constraint at this stage, and permissive licensing is how an OS gets
  adopted.** TinyOS has zero Stories assurance-verified and no hardware tier. The failure mode that
  actually ends this project is nobody building on it — not a competitor forking it. A copyleft
  licence on a kernel excludes exactly the integrators, silicon partners and embedded product teams
  who would otherwise be first users, and it excludes them at the moment there is least to protect.
- **The thing worth selling is not the source, and this became clearer the same day.**
  [`ADR 0005`](0005-arm64-real-time-tier-is-conditional-on-secure-world-qualification.md) makes the
  real-time tier conditional on per-platform secure-world qualification, and names the cost as the
  moat: dated, firmware-versioned, campaign-backed qualification evidence is what commercial RTOS and
  safety-certification vendors actually charge for. **A fork gets the code and none of that.** It does
  not get the qualification records, the assurance spine's Reports, the ability to state a bound that
  survives a competent reviewer, or the obligation to keep any of it current. A copyleft licence
  would have been protecting the asset that is easiest to reproduce while leaving the defensible one
  untouched.
- **Open-core is a structural tax on a project with a single author and no revenue.** It requires
  drawing and defending a line between core and commercial in every design decision, a CLA on every
  contribution, and licence-provenance discipline in the build. That is real recurring cost paid now
  against a benefit that only materialises if the project succeeds — and if it succeeds, the moat
  above is the thing that will be earning, not the withheld half.
- **AGPL/GPL would not have prevented the feared outcome cleanly anyway.** The realistic
  fork-and-close scenario for an embedded OS is a vendor shipping a derived system inside a product,
  where copyleft's enforcement is slow, jurisdictional, and expensive for a single-author project to
  pursue. Choosing a licence whose protection depends on litigation the project cannot fund is
  choosing the appearance of protection.
- **A decision recorded beats an option preserved.** `LE-41`'s real cost was not MIT; it was that
  nobody had decided, so every downstream question — can this be quoted in a proposal, can a partner
  evaluate it, does a contribution need a CLA — had no answer. That cost was being paid continuously.

## Consequences

- **`LE-41` closes.** Both halves: the file exists, and the model is decided.
- **No code, manifest or build change is required.** `LICENSE`, `os/Cargo.toml`'s
  `[workspace.package] license = "MIT"`, and all seven crates' `license.workspace = true` already
  agree. This ADR records why they say what they say. Any future crate inherits by the same mechanism;
  a crate declaring its own `license` key differently is a defect.
- **Outside contributions may be accepted without a licensing gate.** The relicensing window is
  deliberately allowed to close. Nothing needs to be blocked, watched, or timed.
- **`publish = false` is unchanged and is not a licensing statement.** It reflects that these crates
  are not crates.io artifacts; it neither restricts nor extends the MIT grant.
- **Third-party dependency licensing is untouched by this ADR and is a separate obligation.** MIT on
  TinyOS's own source says nothing about what the workspace may depend on. That remains governed by
  [`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md) and by whatever dependency-licence
  gate the project adopts; none exists today, and this ADR does not create one.
- **If this is ever revisited, it is revisited as a new ADR superseding this one, with the
  authorship reality of that moment stated honestly** — which after the first outside contribution
  means naming whose agreement would be required. Reopening it is not forbidden; pretending it is
  still cheap would be.

## Alternatives considered and rejected

Presented to the owner as four options; the first three were rejected and the fourth was the choice.

- **Open-core: AGPL-3.0-or-later core plus a commercial exception, with a CLA.** The strongest
  protection against fork-and-close and the only option that keeps a proprietary-exception revenue
  line open. Rejected: it suppresses adoption of a kernel at precisely the stage where adoption is
  the binding constraint, and it taxes every future contribution and design decision to defend an
  asset less defensible than the qualification evidence.
- **Dual-licence: GPL-2.0 or MPL-2.0 plus commercial.** Weaker copyleft, more palatable to embedded
  integrators linking proprietary drivers, same dual-licence economics. Rejected for the same reasons
  in lesser degree, with less protection to show for the same recurring cost.
- **Defer, with the deadline mechanics written down** — a gate blocking outside contributions until
  the model is chosen. Rejected: it converts a decision into a tripwire and leaves every downstream
  question unanswered in the meantime, which is the cost `LE-41` was already imposing.
- **Confirm MIT and drop open-core optionality — chosen.** See Decision and Rationale.
