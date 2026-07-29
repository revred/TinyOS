# Handover 08C — Cover Note: Execute the Tauri Fork PoC

**The start-here document for the session that runs the PoC.** It authorises and orders the work;
the analysis behind it is [`docs/tauri-internals-review.md`](../../docs/tauri-internals-review.md),
the decision behind it is [`ADR 0007`](../../docs/adr/0007-modifying-tauri-is-in-scope-at-the-seams.md),
and the constraints every stage inherits are [`EPIC-H2`](../../goals/epics/EPIC-H2.md) §2. This note
does not amend [`05B`](05B-next-session-agenda.md)'s work order — `FEAT-P1-06` remains W1 on the
owner's instruction. The PoC is parallel-track work for a session that is not the W1 session.

## 0. Before anything else: the tree

Session C's edits are **uncommitted in the working tree**: `docs/adr/0007-modifying-tauri-is-in-scope-at-the-seams.md`
(new), plus reconciling edits to `docs/tauri-internals-review.md`, `goals/epics/EPIC-H2.md`,
`docs/whole-system-context.md`, and this cover note. Read them (rule 3 of
[`CONCURRENT_SESSIONS`](../../agent/CONCURRENT_SESSIONS.md) — read before you recover), stage them
**by path**, and land them before starting Stage 0. The PoC executes ADR 0007; the ADR must exist in
history before code that cites it does. `check-assurance-spine` was green over this tree at the time
of writing.

## 1. What the PoC is for, in one paragraph

ADR 0007 commits to *fork at the seams under a patch-size discipline* on the strength of a source
reading. Its riskiest claims are all falsifiable on a host today, with no TinyOS webview, no
`EPIC-H3` engine and no `EPIC-H1`: that the `tauri-runtime` trait seam is real, that the §7.1
"patch"-class fixes stay patch-sized, that authority resolution can be externalised to an ACI-shaped
engine, and that the manifest intersection (default inversion + `remote`-context stripping) is
enforceable. **The PoC's job is to try to break those claims cheaply, before a carried fork makes
them expensive to unlearn.** A failed stage is a finding, not a failure of the session — it triggers
a superseding ADR, which is exactly what ADR 0007 constraint 2 provides for.

## 2. The stages

Run in order; 0 and A retire the most risk soonest. Each stage's tests are written **Red first** —
rule 3 applies to PoC code the same as to kernel code.

| # | Stage | Proves | Kill criterion |
| --- | --- | --- | --- |
| 0 | **Vendor at the release tag** matching `tauri-runtime-wry` 2.11.4, in a repository **outside** the `os/` workspace. Re-run the review's measurements against the tag | The review is reproducible; drift from `dev@872428f` is caught | A review claim that does not survive the tag → amend the review before proceeding |
| A | **Headless `Runtime`** — start from upstream's own `tauri::test::MockRuntime`; boot core, register commands, drive IPC invokes end-to-end, exercise webview create/teardown and the `unstable` multiwebview gate | §7.3.1: the seam holds with **zero patches to `tao`/`wry`** | A trait method that leaks `tao`/`wry` types or platform assumptions through the seam |
| B | **The patch set, test-first**: `Capability.local` inversion; `remote`-context stripping; pre-parse size ceiling at the protocol layer. The failing tests are EPIC-H2 §2.2's boundary tests, prototyped early | §7.1's "patch" class is really patch-sized | `git diff --stat` vs the unmodified tag exceeds the low hundreds of lines |
| C | **Resolver seam**: extract a trait from `RuntimeAuthority`; implement it twice — wrapping upstream's `BTreeMap` (no regression), and deferring to a mock ACI that answers deny-by-default from an external table. Draft the upstream PR (ADR 0007 constraint 6) as a side effect | Authority can be externalised; upstream's IPC tests still pass | Authority decisions scattered beyond `resolve_access` (e.g. plugin scope checks bypassing the seam) — a real architectural finding the static review could not see |
| D | **Revocation on navigation**: slow handler + open `Channel`, flip origin to remote, assert cancel and close rather than drain | The `PD-13` obligation (review §2.3) is a clean patch | Cancellation requires restructuring dispatch rather than a token per invoke |
| E | *(optional)* **Host-side console**: a real Tauri app on Windows (WebView2) talking to TinyOS-under-QEMU over the existing serial path, driving fixtures `xtask list-fixtures` knows | The architecture end-to-end: typed commands ∩ signed manifest against a real kernel. Doubles as the `EPIC-H4` operator-console lane from [`03A`](03A-tauri-and-the-tab-host.md) §5 | None — E cannot kill the fork; it can only inform `EPIC-H4` |

## 3. What the PoC cannot prove — write this into the report before the results

`PD-01`, `PD-07`, the `PD-08`/`PD-09` charging, and `PD-12` all need the OS underneath. A host PoC
proves the **interface shape composes**; it never proves isolation. Nothing here touches the
renderer (`EPIC-H3`), which remains the largest unresolved dependency. **Green stages must not read
as "the Tauri lane works"** — that is the overclaim `EPIC-H2` §4 forbids, and the report should
state the non-claims in its header, not its footnotes.

## 4. Governance constraints, restated as a checklist

- **All PoC code lives in the fork repository, outside the workspace.** Never a workspace member,
  never a `path =` dependency of a workspace crate (ADR 0007 constraint 4; `agent.md` rules 4 and 7
  are not amended). Stage E's TinyOS side uses existing fixtures only.
- **TDD, no exceptions** — including in the fork repo. Stage B's tests graduate into `H2-02`'s real
  boundary-test set when `EPIC-H2` decomposes; write them to survive that move.
- **What lands in this repository:** a dated report on the `goals/reports/` pattern recording each
  stage's pass/fail, the measured diff size, and the non-claims of §3 — cross-linked from ADR 0007
  and review §7. No Feature, no Story, no `application-platforms.tsv` change: the horizon is
  unchanged and nothing here makes a support claim.
- **A failed stage is registered**, either as the superseding-ADR trigger (stages A–C) or as a
  loose-end row if it is a defect in our own documents rather than in the posture.

## 5. Traps

1. **Do not let the vendored tree become a dependency.** The moment a workspace crate names it by
   `path =`, a reference has silently become a fork inside the workspace — the exact failure
   [`06A`](06A-tauri-internals-reviewed.md) §4 warned about. The exclusion rule goes in the fork
   repo's README *and* wherever the submodule is declared.
2. **Start Stage A from `MockRuntime`, not from scratch.** Upstream already runs its core headless
   in its own test suite; reimplementing that is a week spent proving what is already proven. The
   PoC's value is in the paths MockRuntime stubs, not the ones it covers.
3. **The diff metric is cumulative across stages B–D.** Measure `diff --stat` against the unmodified
   tag after every stage, not per-stage — three "small" patches can compose into a rewrite.
4. **A green Stage B on a hand-written capability proves nothing about ported manifests.** The
   boundary tests must feed hostile inputs: a capability that *carries* a `remote` grant, not one
   that omits it (EPIC-H2 §2.2 — a carried grant looks intentional; the test must not assume it was).
5. **Concurrent sessions.** This folder's `index.html` is a serialisation point and the pre-commit
   gate validates the index. Claim your session letter, stage by path, `git diff` before staging —
   the clobber [`06A`](06A-tauri-internals-reviewed.md) §5 caught is the standing reason.

## 6. Definition of done

Stages 0–D executed with recorded verdicts; the dated report landed and cross-linked; the upstream
PR for the resolver trait drafted (submitted is better, but drafting is the deliverable); and either
every kill criterion survived — in which case ADR 0007's posture stands with evidence behind it —
or the superseding ADR is written naming which claim fell and what replaces it. Both outcomes are
success; the failure mode is a PoC that ends without a verdict either way.
