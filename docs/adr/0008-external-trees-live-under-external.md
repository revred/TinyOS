# ADR 0008 — External Trees Live Under `external/`, Under a Stated Contract

Status: **Accepted**
Date: 2026-07-29
Follows: [`ADR 0007`](0007-modifying-tauri-is-in-scope-at-the-seams.md), which decided the fork
posture; this ADR decides where forks and references live and how the boundary is enforced
Decided by: the project owner

## Context

The repository root holds two reference submodules, `MsDOS/` and `WindowsTerminal/`, loose beside
the documents and the single `os/` workspace. ADR 0007 added a third external tree — the Tauri
fork, executed as a PoC in a sibling repository — with a stricter contract than the other two:
constraint 4 keeps it outside the workspace, constraint 5 gives it a live advisory/rebase duty.
Three trees, two different contracts, no stated place for either. The internals review
([`docs/tauri-internals-review.md`](../tauri-internals-review.md)) named the failure mode this
invites: a reference tree in a language we build in is one `path =` dependency away from silently
becoming a fork. A change to the root layout is a posture change, so it is recorded here rather
than done by drift.

## Decision

**All external trees live under `external/`, as submodules, under the contract stated in
[`external/README.md`](../../external/README.md). Two tiers share the one folder:**

1. **Reference-only** — `external/MsDOS`, `external/WindowsTerminal`. Read, never built upon,
   never modified. Self-enforcing today because both are in languages
   [`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md) forbids.
2. **Fork-under-discipline** — `external/tauri`, pinned at the PoC head on branch `tinyos-poc`,
   baselined at the release tag `tauri-runtime-wry-v2.11.4`. Governed by ADR 0007's six
   constraints; the health metric is the diff against that tag.

**The boundary is machine-enforced, not advisory:** `check-external-isolation`, folded into
`cargo run -p xtask -- check-assurance-spine`, fails if any `Cargo.toml` under `os/` declares a
`path =` dependency resolving outside `os/`, or if any workspace member resolves outside `os/`.
Rule 7 of [`agent.md`](../../agent.md) — nothing compiled lives outside `os/src/` — is thereby a
gate rather than a rule.

## Consequences

- The root directory states its own structure: one workspace, the documents, and one folder where
  submodules are *expected*. A new external tree goes in `external/` with a README entry and a
  tier, or it does not come in.
- The fork gains a real pin: the submodule records the exact PoC head (`ff44d0c`), where before
  the PoC repository existed only on one disk. The submodule URL is temporarily the local sibling
  path until the owner pushes the fork to a remote; the swap is a one-line `.gitmodules` edit.
- Living documents that referenced `MsDOS/` and `WindowsTerminal/` at the root are updated; dated
  `session/` documents are immutable and keep their old paths.

## Alternatives considered and rejected

- **Leave the trees at the root.** Rejected: the root then has no stated rule for what may appear
  there, and each new tree re-litigates the question ADR 0007 already had to settle.
- **A `vendor/` folder implying build integration.** Rejected: nothing in `external/` is compiled
  by the workspace, and the name should not suggest otherwise.
- **Enforce by review alone.** Rejected: the review found the silent-fork failure mode precisely
  because nothing mechanical watched for it. A ~40-line xtask check removes the doubt permanently.
