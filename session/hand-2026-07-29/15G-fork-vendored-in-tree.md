# Handover 15G — One Repo: The Fork Vendored In-Tree, the Sibling Repository Retired

Same session as [`14G`](14G-stage-e-executed-terminal-gap-measured.md), hours later (dated
2026-07-30). Executes the owner's decision, given verbatim: **"This folder must be removed and
all the code must be in one repo"** — the folder being `C:\Code\tinyos-tauri-fork`. Recorded
as [`ADR 0009`](../../docs/adr/0009-the-tauri-fork-is-vendored-in-tree.md), which amends
[`ADR 0008`](../../docs/adr/0008-external-trees-live-under-external.md)'s fork tier.

## 1. What happened, in order — safety first, deletion last

1. **Nothing could be lost before anything was removed.** `os.tauru.poc` pushed to
   `origin` (`revred/TinyOS`). The fork's full history — every stage commit the reports cite
   (`65089e8 … 1bf5882`) — pushed to a fresh GitHub fork of `tauri-apps/tauri`:
   **`github.com/revred/tauri`, branch `tinyos-poc`**. That push is, verbatim, `LE-54`'s
   stated repair — done as the loss-proofing step of the decision that made the submodule
   itself unnecessary.
2. **The submodule became plain files.** `external/tauri` deinit'd and removed; the fork's
   working tree (minus `.git`, with build caches kept locally and gitignored by the fork's
   own `.gitignore`) copied in as regular files — 1,098 files, ~208k lines, the tree at what
   was `1bf5882`. `.gitmodules` now holds only the two GitHub-resolvable references
   (`MsDOS`, `WindowsTerminal`), which stay submodules: their upstreams resolve for any
   clone, and vendoring trees we never modify buys nothing.
3. **The health metric survived the history loss** (ADR 0007 constraint 2): the divergence
   over `crates/` vs the baseline tag is committed as
   `external/tauri/TINYOS-PATCH-vs-tauri-runtime-wry-v2.11.4.diff` (16 files, +224/−19), to
   be regenerated in the same commit as any `crates/` change; and it remains re-derivable
   against the tag on the GitHub fork. Both stated in
   [`external/README.md`](../../external/README.md).
4. **Everything re-verified from the new location.** The full `tinyos-poc` workspace suite
   green (15 headless stage-e tests + stages A–D), both live QEMU tests green, and the
   windowed console smoke PASS — including the one code change the move required:
   `stage-e-console-app`'s `os_dir()` now walks ancestors for `os/targets/x86_64-tinyos.json`
   instead of assuming the sibling-checkout layout.
5. **`LE-54` closed** (55 loose ends, 32 open; dashboard re-synced). ADR slots for U2/U3
   renumbered **0010/0011** in `REPORT-2026-07-29-04` and `14G`, since 0009 is now this
   decision. Living docs updated: `external/README.md`, ADR 0008 status note,
   `tauri-internals-review.md` vendoring note, root `README.md` layout line,
   `fork-advisories.yml` source-of-truth comment.
6. **`C:\Code\tinyos-tauri-fork` deleted** — only after the GitHub push was verified and the
   in-repo suites passed.

## 2. What the next reader must know

- **The exclusion rule did not move an inch.** Nothing under `external/` is ever an `os/`
  workspace member or `path =` dependency; `check-external-isolation` still gates the spine.
  The vendored tree's own Cargo workspaces (`external/tauri/`, `external/tauri/tinyos-poc/`)
  are private to it, and the 20,000-line ceiling does not apply there (ADR 0009 §4).
- **Rebasing the fork now happens on the GitHub fork** (where upstream history lives) and
  lands here as a refreshed vendored tree + regenerated patch file in one commit.
- **U1 is unblocked**: `UPSTREAM-PR-authority-resolver.md` can be submitted from
  `revred/tauri` — minutes of work, owner's call on timing.
- Build caches under `external/tauri/**/target/` are local-only (gitignored); a fresh clone
  rebuilds them.

## 3. Remaining queue (unchanged otherwise from 14G)

1. Submit the upstream PR (U1) from `revred/tauri`.
2. The `07A` reviewer → **ADR 0010** (U2) → **ADR 0011** (U3).
3. Gate `terminal-gap.tsv` into the spine (validator in `assurance.rs` first — the
   two-sided invariant in `spine_files.rs`).
4. The `os.tauru.poc` → `main` merge decision — the branch is pushed and current.
