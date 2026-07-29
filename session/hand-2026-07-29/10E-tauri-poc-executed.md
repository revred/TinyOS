# Handover 10E — The Tauri Fork PoC, Executed: Stages 0–D, Every Kill Criterion Survived

**The session [`08C`](08C-tauri-poc-execution-cover-note.md) ordered.** The verdicts, the
measured diff, the findings and the non-claims are in
[`REPORT-2026-07-29-03`](../../goals/reports/REPORT-2026-07-29-03.md), which is the durable
record; this handover carries what the report pattern does not.

## 1. What happened, in order

1. **Landed sessions A and C first** (08C §0): the working tree held their entire uncommitted
   Tauri set — ADR 0007, the internals review, `EPIC-H2`, `LE-53`, handovers 03A/06A/07A/08C
   and both index edits. Every file read before staging (rule 3), the loose-ends register
   verified to carry only its one owed row, the live soak log line left with the soak session
   that owns it. `90e54fe`, spine green: 53 loose ends (31 open), 91 status headers.
2. **Executed the PoC** in a new fork repository `c:\Code\tinyos-tauri-fork` (outside the
   workspace, per ADR 0007 constraint 4), branch `tinyos-poc`, baselined at the release tag
   `tauri-runtime-wry-v2.11.4` = `ca90b46`. Five commits, one per stage:
   `65089e8` (0) → `7343dd5` (A) → `1a9b9e1` (B) → `8123124` (C) → `ff44d0c` (D).
   All PASS; Stage E (optional, cannot kill the fork) not run.
3. **Wrote the report and this handover**, cross-linked from ADR 0007 and review §7.4.

## 2. Decisions this session took that the next reader should know

- **The fork posture is one feature knob.** All TinyOS behaviour is behind
  `tauri-utils/tinyos-acl` (forwarded as `tauri/tinyos`), so the vendored tree with the knob
  off is behaviourally upstream and upstream's suite gates both configurations. This is what
  keeps the rebase obligation (ADR 0007 constraint 5) cheap.
- **An installed `AuthorityResolver` means authority is fully governed.** Upstream's
  no-app-manifest fast path would otherwise bypass an external engine for unlisted local
  commands — found by reading `on_message`, closed in the seam patch, argued in
  `UPSTREAM-PR-authority-resolver.md` (drafted, not yet submitted — an owner decision).
- **The invoke key was deliberately not removed** (review §7.1's stated contingency): until a
  transport carries kernel-derived identity, deleting the bearer secret removes a mitigation
  without supplying a boundary.
- **No Microsoft tooling, confirmed mid-session by the owner:** builds mirror Sharc.Blue's
  pattern (`rust-lld` + cargo-xwin splat); `vswhom-sys` — C code whose sole purpose is to
  locate a Visual Studio that deliberately does not exist — is stubbed in pure Rust. As a side
  effect the fork's *entire* unit suite now runs with no platform webview stack, which is worth
  upstreaming on its own.

## 3. Concurrency

Executed on branch `os.tauru.poc` at the owner's instruction. Mid-session, `main` advanced
with session D's `d6ab240`/`d077526`/`b9be8a1` (LE-23/LE-24, Handover 11D) — none touching
this session's files — and was **merged in** (rule 6, merge not rebase) before the report
landed. `REPORT-2026-07-29-02` was D's; this session took `-03`. The 10E slot itself was
claimed as an empty file at session start (rule 4). At one point another agent switched this
working tree's checked-out branch to `main` mid-session; caught by re-deriving git state on
the owner's warning, switched back, nothing lost — but it is a concurrency mode
`CONCURRENT_SESSIONS.md` does not currently name: **the working tree's HEAD is itself shared
mutable state.**

## 4. What is deliberately left open

- **Stage E** — the host-side operator console against TinyOS-under-QEMU over existing
  fixtures. Prototypes the `EPIC-H4` lane 03A §5 named; a future session's choice.
- **Submitting the upstream PR** — drafting was the deliverable; submitting shrinks the
  carried patch by its largest piece. Owner decision.
- **The advisory/rebase process** (ADR 0007 constraint 5) is now a live obligation against a
  real fork rather than prose. An unrebased fork with an open advisory is a loose-end row.
- **`EPIC-H3`** — untouched, deliberately, and still the largest unpriced item. The fork
  removed every objection it could reach; the engine is the one it cannot.
- This branch (`os.tauru.poc`) is unmerged and unpushed; merging to `main` is the owner's
  call.
