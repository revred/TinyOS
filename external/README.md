# `external/` — External Trees, Under Contract

Every tree in this folder is a git submodule and is governed by
[`ADR 0008`](../docs/adr/0008-external-trees-live-under-external.md). The contract, stated where
the trees live:

- **Nothing here is ever a workspace member.** The only Cargo workspace is `os/`.
- **Nothing here is ever a `path =` dependency of any `os/` crate.** Machine-enforced:
  `check-external-isolation` runs inside `cargo run -p xtask -- check-assurance-spine` and fails
  the spine on any violation.
- **Reference trees are never built upon and never modified.** They exist to be read.
- **A fork carries the advisory/rebase duty** of
  [`ADR 0007`](../docs/adr/0007-modifying-tauri-is-in-scope-at-the-seams.md) constraint 5: an
  unrebased fork with an open upstream advisory is a loose-end row in
  [`goals/assurance/loose-ends.tsv`](../goals/assurance/loose-ends.tsv).

## The trees

| Tree | Tier | Pin | Notes |
| --- | --- | --- | --- |
| [`MsDOS/`](MsDOS/) | reference-only | upstream `main` | Language forbidden by [`CODING_STANDARDS.md`](../agent/CODING_STANDARDS.md); self-enforcing |
| [`WindowsTerminal/`](WindowsTerminal/) | reference-only | upstream `main` | Same enforcement |
| [`tauri/`](tauri/) | fork-under-discipline | `ff44d0c` on `tinyos-poc`, baseline tag `tauri-runtime-wry-v2.11.4` | ADR 0007's six constraints; health metric: `git diff --stat tauri-runtime-wry-v2.11.4` |

## The fork's URL is a temporary local pin

`external/tauri` currently points at the sibling path `C:/Code/tinyos-tauri-fork`, because the
fork has no remote yet. This breaks `git submodule update --init` for any other clone. The owed
follow-up is a one-line `.gitmodules` edit once the owner pushes the fork to a remote they
control (a GitHub fork of `tauri-apps/tauri`, branch `tinyos-poc`, is the natural choice — it
also makes ADR 0007 constraint 6's upstream PR submission trivial), then
`git submodule sync external/tauri`. Tracked as `LE-54`.

Until then, materialising this submodule needs git's `file` transport, which is disabled by
default: `git -c protocol.file.allow=always submodule update --init external/tauri`. The two
reference submodules are unaffected.
