# Handover 12F — External Trees Restructured Under `external/`, and the Boundary Is Now a Gate

Follows: [`10E-tauri-poc-executed.md`](10E-tauri-poc-executed.md) (same branch, `os.tauru.poc`)
and [`11D-ci-fixed-le-23-recorded.md`](11D-ci-fixed-le-23-recorded.md). Executes "Part 1" of the
owner-approved plan: the repo restructure only; the per-unknown deliverables (upstream PR,
independent review, engine pricing, Stage E, …) are deliberately untouched.

## 1. What changed

1. **[`ADR 0008`](../../docs/adr/0008-external-trees-live-under-external.md)** — external trees
   live under `external/`, two tiers in one folder: *reference-only* (`MsDOS`,
   `WindowsTerminal`) and *fork-under-discipline* (`tauri`, governed by ADR 0007's six
   constraints).
2. **The submodules moved**: `git mv MsDOS external/MsDOS`, `git mv WindowsTerminal
   external/WindowsTerminal` — git rewrote `.gitmodules` itself; the submodule *section names*
   keep their old spelling, which is cosmetic and harmless.
3. **The fork became a submodule**: `external/tauri`, pinned at the PoC head `ff44d0c` on
   `tinyos-poc`, baseline tag `tauri-runtime-wry-v2.11.4`. The URL is the **local sibling path**
   `C:/Code/tinyos-tauri-fork` — the one compromise in this session, taken at the owner's
   option, recorded as **`LE-54`** with the exact repair (push fork to a remote, one-line
   `.gitmodules` edit, `git submodule sync`). Until then materialising it needs
   `git -c protocol.file.allow=always submodule update --init external/tauri`; git's default
   refuses the `file` transport.
4. **[`external/README.md`](../../external/README.md)** — the contract, stated where the trees
   live: never a workspace member, never a `path =` dependency, references never built upon, the
   fork carries the advisory/rebase duty, health metric is the diff against the baseline tag.
5. **The boundary is machine-enforced** — `check_external_isolation` in
   [`os/src/xtask/src/external_isolation.rs`](../../os/src/xtask/src/external_isolation.rs),
   folded into `check-assurance-spine` (it now prints `… 8 manifests isolated from external/`).
   Every `Cargo.toml` under `os/` is parsed; a `path =` dependency or workspace member resolving
   outside `os/` fails the spine. TDD held: the seven tests went in first against a stub and
   failed 5/7, then the implementation turned them green; 202 xtask tests pass. The 06A §4
   failure mode — "a reference silently becomes a fork" — is now a gate, not a rule.
6. **Living-document paths updated** (`README.md` layout + MsDOS link, `SeedMVP.md` §ref,
   `docs/mvp-delivery-strategy.md` layout, `docs/cli-compatibility-mvp.md`,
   `goals/epics/EPIC-P2.md` link, `work/case-motion-controller/README.md`,
   `.githooks/pre-commit` comment). `docs/tauri-internals-review.md`'s "Not vendored as a
   submodule" paragraph was false as of this session and now says so, pointing at ADR 0008.
   Dated `session/` documents untouched, per convention. `goals/index.html`'s spine-count
   sentence re-synced (54 loose ends, 32 open; 53 Reports).

## 2. Decisions a next reader should know

- **The isolation check is line-lexical, no TOML crate** — deliberate: a parser dependency is a
  supply-chain surface in the tool that gates the spine, and every other xtask validator already
  takes this shape. A false positive is a loud one-line fix.
- **No new subcommand.** The check runs only inside `check-assurance-spine`; a check nobody has
  to remember to run separately is the point.
- **The count prints as a floor-style evidence figure**, matching the house rule that a gate
  which examined things and a gate that never ran must not look alike.

## 3. Concurrency

`goals/reports/_soak-p0-03-01.log` sat modified in the tree the whole session — it belongs to
the live soak session and was left unstaged and uncommitted (rule 3). `LE-54` was appended with
a guarded write (last-id check before `Add-Content`) and validated with `check-spine-files`
before the next tool call (rule 8). No mid-session commits arrived on `main`.

## 4. Open, deliberately

- **`LE-54`** — the submodule URL swap, blocked on the owner pushing the fork to a remote.
- **Part 2 of the plan** (U1–U7): upstream PR submission, the 07A independent review, the
  EPIC-H3 pricing spike, Stage E, the H2-02 boundary-test rows, the advisory CI job (U6 — same
  shape as the LE-23 baseline job), and the H2-05 invoke-key criterion. None started here.
- This branch (`os.tauru.poc`) remains unmerged and unpushed; merging is the owner's call.
