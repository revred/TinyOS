# ADR 0009 — The Tauri Fork Is Vendored In-Tree; The Sibling Repository Is Retired

Status: **Accepted**
Date: 2026-07-30
Follows: [`ADR 0007`](0007-modifying-tauri-is-in-scope-at-the-seams.md) (the fork posture —
unchanged) and [`ADR 0008`](0008-external-trees-live-under-external.md) (amended: the fork
tier's *form* changes from submodule to vendored files)
Decided by: the project owner — "this folder must be removed and all the code must be in one
repo" (2026-07-30, executing session `15G`)

## Context

ADR 0008 held the fork as a submodule at `external/tauri`, whose URL was the local sibling
path `C:/Code/tinyos-tauri-fork` — recorded at the time as `LE-54`, because every clone that
was not that one machine could not materialise it. The owner's decision resolves it the other
way from LE-54's original repair: rather than keeping a second repository and repointing the
URL, **all the code lives in the one TinyOS repository**. A second repo that must be pushed,
pinned and synced separately is a coordination surface; the fork's working tree is ~40 MB of
source, which the repository can simply carry.

## Decision

1. **`external/tauri` is plain files in this repository** — the fork's full working tree at
   what was `1bf5882` on `tinyos-poc` (baseline tag `tauri-runtime-wry-v2.11.4` = `ca90b46`),
   including `tinyos-poc/` (stages A–E and the operator console). The submodule entry is gone.
2. **The pre-vendoring history is preserved, not lost**: pushed to
   `github.com/revred/tauri`, branch `tinyos-poc` — a GitHub fork of `tauri-apps/tauri`, which
   also unblocks the ADR 0007 constraint 6 upstream-PR submission (13F's U1). The commit
   hashes cited in `REPORT-2026-07-29-03/-04` (`65089e8…1bf5882`) resolve there.
3. **The health metric survives the loss of in-repo upstream history** two ways, per
   [`external/README.md`](../../external/README.md): the committed divergence record
   `external/tauri/TINYOS-PATCH-vs-tauri-runtime-wry-v2.11.4.diff` (16 files, +224/−19 —
   regenerated in the same commit as any `crates/` change, a stale patch file being exactly
   the drift the metric catches), and re-derivation against the baseline tag on the GitHub
   fork or upstream.
4. **Everything else ADR 0007/0008 decided stands.** The exclusion rule is untouched and
   still machine-enforced: nothing under `external/` is ever a member of the `os/` workspace
   or a `path =` dependency of an `os/` crate (`check-external-isolation`); the vendored
   tree's own private Cargo workspaces (`external/tauri/`, `external/tauri/tinyos-poc/`) are
   not TinyOS workspaces and the 20,000-line crate ceiling does not apply to them. The
   advisory/rebase duty (constraint 5) continues via the `fork-advisories` CI workflow, whose
   pins' source of truth is now the in-tree `external/tauri/crates/*/Cargo.toml`. The
   reference tier (`MsDOS`, `WindowsTerminal`) remains submodules — their upstreams are
   public GitHub repositories any clone can resolve, and vendoring trees we never modify
   buys nothing.
5. **`LE-54` is closed** by construction: there is no fork submodule to materialise; a plain
   `git clone` carries the fork tree.

## Consequences

- A rebase of the fork onto a newer upstream tag happens on the GitHub fork (where the
  upstream history lives), and lands here as a refreshed vendored tree plus a regenerated
  patch file in one commit.
- Build caches under `external/tauri/**/target/` are gitignored by the fork's own
  `.gitignore`, which now applies in-repo.
- The repository grows by the fork's source (~200k lines of vendored upstream code). That
  code was already governed, measured and carried; it is now also *cloned*, which is the
  point.
