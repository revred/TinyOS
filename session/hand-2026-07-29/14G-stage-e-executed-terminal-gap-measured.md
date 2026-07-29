# Handover 14G — 13F Executed: The Console Runs, The Terminal Gap Is Measured, The Unknowns Are Dispositioned

Executes: [`13F`](13F-next-session-mandate-console-and-gap-analysis.md). The durable record is
[`REPORT-2026-07-29-04`](../../goals/reports/REPORT-2026-07-29-04.md); this handover carries
what the report pattern does not. Branch `os.tauru.poc`, still unmerged and unpushed.

## 1. What happened, in order

1. **Deliverable A — the console runs.** Fork commit `1bf5882`
   (`tinyos-poc/stage-e-console` + `stage-e-console-app`), TinyOS pin advanced in `4de93b3`.
   All four 13F acceptance criteria PASS: a real fixture's serial streamed live into a real
   WebView2 window with `xtask`'s own verdict; every console→target action resolved through a
   **signed manifest** (ed25519; verification is the only path to a resolver — fail-closed by
   type) over the Stage C `AuthorityResolver` seam; an unlisted verb (`format_disk`) denied
   visibly; upstream suite green both knob positions (57/57 off, 58/58 on); **patch metric
   unchanged** (16 files, +224/−19 — the console is entirely `tinyos-poc/`-side). 15 headless
   tests + 2 live QEMU tests + a page-driven windowed smoke run, evidence in the report.
2. **Deliverable B — the gap analysis.** [`docs/terminal-gap-analysis.md`](../../docs/terminal-gap-analysis.md)
   + [`goals/context/terminal-gap.tsv`](../../goals/context/terminal-gap.tsv): 33 rows
   (22 verbs / 10 window behaviours / 1 transport), DOS column read from the actual
   `external/MsDOS` v4.0 sources (exact message strings, switch tables, ERRORLEVEL quirks),
   Windows Terminal column read from source with the drop-frames-not-block seam evidenced at
   specific lines. 32 rows spec-level, 1 live-verified. No binary-compatibility claim; the
   header restates the prohibition.
3. **The unknowns**: U4 and U6 **closed**; U5 and U7 **pinned** to named artifacts
   (`EPIC-H2` §2.7: `H2-02-T1..T5` green-on-host, `H2-02-R1..R4` red-until-OS, `H2-05-AC1`);
   U2/U3 **pinned** to named ADR slots (**0010** review verdict, **0011** engine lane —
   renumbered by `15G` when ADR 0009 became the fork-vendoring decision);
   U1 **still owner-blocked** (`LE-54`) at the time of writing — resolved hours later, see `15G`.
   Commits `6ce8258` (TinyOS side) and the fork head.
4. **`LE-55` filed**: 13F's live-verification plan is not executable — no shell crate, no
   UART RX path anywhere, no interactive fixture, and no `boot-banner` fixture exists (the
   console smokes with `measure`). Repair path stated in the row.

## 2. Decisions this session took that the next reader should know

- **The console drives `xtask`, not QEMU directly.** Launch goes through
  `cargo run -p xtask -- qemu-x86_64 --fixture=… --serial-capture=…`, so fixture validation,
  the boot budget and the `isa-debug-exit` mapping are CI's own, incapable of drifting. The
  cost: `send_line` has no transport (serial is TX-only anyway — `LE-55`), and the harness
  must scrub `RUSTUP_TOOLCHAIN`/`CARGO`/`RUSTC`/rustflags from the child env or the pinned
  nightly build breaks (found live, fixed in `harness.rs`).
- **The manifest-signing key committed in the fork is a PoC dev key, not a custody model** —
  stated in the signer, the README, and the report's non-claims. Re-sign with
  `cargo run -p stage-e-console --bin sign-manifest` after editing the payload.
- **`containment-tests.tsv` stays closed.** 13F suggested `H2-02` rows there; the catalogue's
  `BND-01..20` grammar is machine-enforced and deliberately canonical, so the named-test
  register lives in `EPIC-H2` §2.7 and graduates into `story-contracts.tsv` at decomposition.
- **`terminal-gap.tsv` is not yet spine-gated**, deliberately: `spine_files.rs`'s cross-check
  test requires every fast-checked file to also be read by the full spine check, so gating
  needs a small `assurance.rs` validator first (status ∈ {spec-level, live-verified}, evidence
  non-empty). Mechanical next-session work; 13F said "can be gated later".
- **Two new build-tooling stubs in the fork** (the `vswhom-sys` pattern): `rc_shim.rs`
  (pure-Rust empty-`.res` emitter replacing the absent MS Resource Compiler) and
  `common-controls-v6` off (no embedded manifest ⇒ comctl32 v5 ⇒ `TaskDialogIndirect` is
  `STATUS_ENTRYPOINT_NOT_FOUND` at process init). Neither touches a vendored crate.

## 3. Owner actions, unchanged from 13F and still the head of the queue

1. **Push `C:\Code\tinyos-tauri-fork` to a remote** (GitHub fork of `tauri-apps/tauri`,
   branch `tinyos-poc`) → one-line `.gitmodules` URL swap + `git submodule sync external/tauri`
   → closes `LE-54`, unblocks U1 (the PR is drafted; submission is minutes).
2. **Choose a reviewer for `07A`** → the verdict lands as **ADR 0010** (U2), which feeds the
   **ADR 0011** engine-lane pricing spike (U3). Nothing else waits on these.
3. **The branch question**: `os.tauru.poc` → `main`. This session added its commits on top of
   the PoC/restructure set; spine green throughout; the evidence 13F asked to have in hand
   for the merge decision now exists.

## 4. Concurrency

The `14G` slot was claimed as an empty file at session start (rule 4). All spine TSV edits
used guarded appends validated with `check-spine-files` before the next tool call (rule 8);
one malformed first attempt at `LE-55` was caught by exactly that check and repaired in place.
`goals/reports/_soak-p0-03-01.log` sat modified in the tree all session — it belongs to the
live soak session and was left unstaged (rule 3). No mid-session commits arrived on `main`.

## 5. Deliberately left open

- **Gating `terminal-gap.tsv`** (validator in `assurance.rs` + `SPINE_FILES` entry + red-first
  tests) — see §2.
- **The first live gap rows** — blocked on `LE-55`'s repair (serial RX + interactive fixture),
  which belongs with `EPIC-P2`'s shell decomposition, not the PoC.
- **`EPIC-H3`** — still untouched, still the largest unpriced item; U2 → U3 is its path.
- The fork's `stage-e-console-app` window has no icon/version resources (rc shim) — cosmetic,
  revisit only if the console ever ships to anyone.
