# ADR 0007 — Modifying Tauri Is In Scope, at the Seams, Under a Patch-Size Discipline

Status: **Accepted**
Date: 2026-07-29
Introduced in: [`docs/tauri-internals-review.md`](../tauri-internals-review.md) §7, which first
recorded the owner's intent inside a document whose own header says "no commitment"; this ADR is the
decision's proper record, and the review now cites it instead of carrying it
Decided by: the project owner

## Context

Tauri is a first-class application lane by founding intent (`G-APP-2`, `APP-05`,
[`EPIC-H2`](../../goals/epics/EPIC-H2.md)). The source review found that most of what it faults
Tauri for — the `Capability.local` default, the `__TAURI_INVOKE_KEY__` bearer secret, the
string-keyed in-process ACL, the unbounded IPC — are *policy and interface* choices, reasonable for
a desktop framework and editable in source. Apache-2.0/MIT permits modification outright
([`ADR 0006`](0006-mit-licence-confirmed-and-open-core-optionality-dropped.md)).

The reference-only treatment `MsDOS/` and `WindowsTerminal/` receive enforces itself, because both
are in languages [`agent/CODING_STANDARDS.md`](../../agent/CODING_STANDARDS.md) forbids. Tauri is
Rust: a vendored copy is one `path =` dependency away from being built upon, so the posture toward
it has to be decided, not assumed. The owner stated that modifying Tauri, `tao` and the IPC
internals to fit the security framework is in scope. A posture change of that size belongs in the
ADR set, where decisions with alternatives are recorded.

## Decision

**Modifying Tauri and its IPC internals is in scope, under six binding constraints:**

1. **Prefer the seams over the patch.** Windowing and webview binding are already behind
   `tauri-runtime`'s traits; TinyOS implements `Runtime`/`WebviewDispatch`/`WindowDispatch` and
   patches neither `tao` nor `wry`. Replacing `tao` wholesale is cheaper than modifying it.
2. **The patch set against unmodified upstream is the health metric.** Review §7.1 classifies each
   candidate modification as *patch*, *seam*, or *rewrite-risk*. If the carried patch grows past a
   few hundred reviewable lines, the fork has become a rewrite and is re-decided as one — by a new
   ADR, not by drift.
3. **The fork baseline is a signed release tag, never `dev`.** The review's pin at `dev` `872428f`
   was for analysis. The baseline (and the vendoring referent of review §7.4) is the release tag
   corresponding to `tauri-runtime-wry` 2.11.4, so that upstream advisories — which reference
   releases — map onto the patched surface.
4. **The fork lives outside the `os/` workspace**, as a pinned external dependency: never a
   workspace member, never a `path =` dependency of a workspace crate. `tauri` measures 32,457
   lines against the 20,000-line crate ceiling, and [`agent.md`](../../agent.md) rules 4 and 7 are
   not amended by this ADR.
5. **Security maintenance transfers with the patch, so the transfer has a process:** subscribe to
   the RUSTSEC/GHSA advisory streams for `tauri`, `wry` and `tao`; rebase onto upstream patch
   releases as they land; and re-run the review's §6 `PD-*` mapping over the patched surface at
   every rebase. An unrebased fork with an open upstream advisory is a defect, registered in
   [`loose-ends.tsv`](../../goals/assurance/loose-ends.tsv) like any other.
6. **Upstream-first for mechanical seams.** The `RuntimeAuthority` resolver trait (review §7.3) is
   attempted as an upstream contribution before being carried as a patch.

## Consequences

- Review §7 cites this ADR rather than being the decision's only record, and its "reference
  analysis, no commitment" header is again true of the document that carries it.
- Vendoring happens at the release tag of constraint 3; `872428f` remains the referent for the
  review's line numbers only.
- Nothing about the horizon changes: `APP-05` stays `later`, `EPIC-H2` stays undecomposed, and
  `EPIC-H3` still gates any webview lane. This ADR settles posture, not schedule.

## Alternatives considered and rejected

- **Reference-only, like `MsDOS/` and `WindowsTerminal/`.** Rejected: it forgoes exactly the fixes
  the review shows are cheap — every policy misfit becomes permanent TinyOS-side wrapper code
  around behaviour the licence permits changing at the source.
- **Unmodified upstream plus TinyOS-side wrappers only.** Rejected: wrappers cannot reach
  `Capability.local`'s default or remove the bearer-token path; the defect ships and the wrapper
  masks it, which is worse than either fixing or documenting it.
- **Hard fork / rewrite.** Rejected now: it transfers the whole maintenance burden immediately,
  when the review shows the valuable properties are reachable with a small patch set. Becomes the
  standing question again only if constraint 2's metric trips.
- **Fork at the seams under patch discipline — chosen.** See Decision.
