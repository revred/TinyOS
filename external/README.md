# `external/` — External Trees, Under Contract

The trees here are governed by [`ADR 0008`](../docs/adr/0008-external-trees-live-under-external.md)
as amended by [`ADR 0009`](../docs/adr/0009-the-tauri-fork-is-vendored-in-tree.md): the two
*reference* trees are git submodules; the *fork* tree is vendored in-repo as plain files. The
contract, stated where the trees live:

- **Nothing here is ever a workspace member.** The only Cargo workspace TinyOS ships is `os/`
  (the vendored fork carries its own private workspaces, which `os/` never references).
- **Nothing here is ever a `path =` dependency of any `os/` crate.** Machine-enforced:
  `check-external-isolation` runs inside `cargo run -p xtask -- check-assurance-spine` and fails
  the spine on any violation.
- **Reference trees are never built upon and never modified.** They exist to be read.
- **A fork carries the advisory/rebase duty** of
  [`ADR 0007`](../docs/adr/0007-modifying-tauri-is-in-scope-at-the-seams.md) constraint 5: an
  unrebased fork with an open upstream advisory is a loose-end row in
  [`goals/assurance/loose-ends.tsv`](../goals/assurance/loose-ends.tsv). Mechanised by the
  `fork-advisories` CI workflow.

## The trees

| Tree | Tier | Form | Pin | Notes |
| --- | --- | --- | --- | --- |
| [`MsDOS/`](MsDOS/) | reference-only | submodule (`microsoft/MS-DOS`) | upstream `main` | Language forbidden by [`CODING_STANDARDS.md`](../agent/CODING_STANDARDS.md); self-enforcing |
| [`WindowsTerminal/`](WindowsTerminal/) | reference-only | submodule (`microsoft/terminal`) | upstream `main` | Same enforcement |
| [`tauri/`](tauri/) | fork-under-discipline | **vendored in-tree** (plain files) | baseline tag `tauri-runtime-wry-v2.11.4` (`ca90b46`) | ADR 0007's six constraints. History preserved at `github.com/revred/tauri` branch `tinyos-poc` (head `1bf5882` at vendoring) |

## The fork's health metric, post-vendoring

The upstream git history does not travel with the vendored files, so the ADR 0007 constraint 2
metric (`git diff --stat` vs the baseline tag) is carried two ways:

1. **The committed record**: [`tauri/TINYOS-PATCH-vs-tauri-runtime-wry-v2.11.4.diff`](tauri/TINYOS-PATCH-vs-tauri-runtime-wry-v2.11.4.diff)
   — the full divergence over `crates/` at vendoring time (16 files, +224/−19). Any change to
   `external/tauri/crates/` must regenerate this file in the same commit; a stale patch file is
   the drift the metric exists to catch.
2. **The re-derivable measurement**: diff `external/tauri/crates/` against the
   `tauri-runtime-wry-v2.11.4` tag of `github.com/revred/tauri` (or upstream
   `tauri-apps/tauri`), where the pre-vendoring commit history also lives.

`LE-54` (the unresolvable local submodule URL) closed with the vendoring: there is no
submodule to materialise, and plain `git clone` of TinyOS carries the fork tree.
