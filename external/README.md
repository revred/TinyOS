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
| `npcap188/` | reference-only | **local only — git-ignored, never committed** | upstream `github.com/nmap/npcap`, v1.88 | See below. Not a submodule and not vendored |

### `npcap188/` — the one tree that must never enter this repository

Npcap is **source-available, not open source**. Its EULA (© 2013–2025 Nmap Software LLC)
grants no redistribution right, so committing the tree would be *unlicensed redistribution*
and would also falsely imply those files carry TinyOS's MIT licence. It is therefore listed
in [`.gitignore`](../.gitignore) rather than merely left untracked — untracked survives
exactly until someone types `git add external/`.

It is not a submodule either, unlike the two reference trees above: the local copy is
extracted files rather than a clone, so there is no commit to pin. The version above is the
pin; re-obtain it from [npcap.com](https://npcap.com) or `github.com/nmap/npcap` if needed.

**Nothing in this repository derives from it.** `work/tools/ti64dink` reaches Npcap the way
Wireshark and every other capture tool does — by calling the `wpcap.dll` that the *user*
installs, through P/Invoke declarations written against the public libpcap API. No Npcap
source is copied, linked, or shipped, which is what keeps Ti64Dink cleanly MIT and leaves
TinyOS itself untouched: the kernel never links it, and the two sides exchange only our own
`TOS64` frames over the wire. A licence governs copies of software, not packets that cross
its driver.

Redistribution is the line Npcap draws. If Ti64Dink is ever published it ships **Npcap-free**,
and each user installs Npcap themselves under their own free licence (which covers up to five
copies). Bundling the installer, the driver or the DLLs — or silent-installing any of them —
would require an OEM Redistribution licence from Insecure.Com and is out of scope.

Because the host tool is coded against the libpcap API rather than an Npcap-specific one, it
also stays portable to libpcap on Linux/macOS later with no Npcap-derived code anywhere.

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
