# Handover 06A — Tauri Reviewed From Source, and What It Changes in the Context Model

**Reference analysis and context elaboration. No code, no contracts, no Story.** Three documents:
[`docs/tauri-internals-review.md`](../../docs/tauri-internals-review.md) (new),
[`goals/epics/EPIC-H2.md`](../../goals/epics/EPIC-H2.md) (new), and an expanded Tauri section in
[`docs/whole-system-context.md`](../../docs/whole-system-context.md).

Follows [`03A`](03A-tauri-and-the-tab-host.md), which settled the *shell* question (`LE-53`). This is
the other half: the **application lane**, where Tauri is already first-class by founding intent and
where the repository's position was one paragraph of assertion.

## 1. Why a review rather than another opinion

[`SECURITY_CHARTER.md`](../../SECURITY_CHARTER.md) §"Application-runtime policy" sets the standard
itself: policy is *"informed by the actual upstream execution models rather than framework names."*
The Tauri paragraph in `whole-system-context.md` did not meet it — it asserted alignment and cited a
security page. So the review is from source.

**Pinned**: `tauri-apps/tauri` `dev` at `872428f`, `tauri-runtime-wry` 2.11.4, `wry` 0.55, `tao` 0.35.
Dual Apache-2.0/MIT, compatible with `ADR 0006`.

## 2. What the source says

**The good part, and it deserves credit.** Tauri resolves the calling window and webview from its
own Rust-side objects, never from the message payload:

```rust
// crates/tauri/src/webview/mod.rs:1518
.resolve_access(&cmd_name, self.window().label(), self.label(), &origin);
```

The IPC handler is bound per-webview at creation, so *which* webview called is known from where the
message arrived. **That is `PD-02` — "never from caller-supplied identifiers" — reached
independently by a userspace framework.**

**The exception that must not travel with it.** `__TAURI_INVOKE_KEY__` is a bearer secret separating
the app's own frame from other frames that can also reach the custom protocol. That one *is*
caller-supplied, and an XSS in the local page yields it. Sensible mitigation, not a boundary — and a
reader who takes the paragraph above as the whole story would over-trust it.

**Origin is genuinely dynamic.** Computed from the webview's current URL at each call, so navigating
to a remote origin changes authority on the next call with no teardown. The Charter already requires
this; it turns out to be **implemented upstream, not a requirement TinyOS would have to add.**

**Authority is resolved at build time.** Plugin permission manifests × app capability files, resolved
by `tauri-codegen` and baked into the binary. This is the strongest point of alignment in the whole
review: a *signed manifest* is exactly a build-time-fixed authority set. **But the default inverts** —
`Capability.local` defaults to `true`, where `PD-03` requires empty authority first. That is the most
likely place for a silent authority leak in any future port, and it is specific enough to be a
boundary test.

**The OS binding is already a trait seam.** `tauri-runtime` abstracts windowing and webview;
`tauri-runtime-wry` is the sole implementation. A TinyOS port is `impl Runtime for TinyOsRuntime`
rather than a fork — **well-shaped, and not small**: Tauri ships no renderer, so the port still needs
the window/input service *and* a browser engine (`EPIC-H3`).

**The finding that governs the rest: it is one process.** Core, `wry` and `tao` share a process, and
every command handler runs there with the app's full OS authority. The ACL decides whether a *string*
reaches a handler; it does not constrain what the handler then does. **Tauri's ACL is an in-process
input filter, not an OS boundary** — which is what the Charter already said, now evidenced rather
than inferred, and which makes `agent.md` rule 10 load-bearing here rather than boilerplate.

The full `PD-01`…`PD-14` map is in the review. Its shape is the conclusion: **everything Tauri does
well is interface shape; everything it does not do is isolation, accounting and time.** That is
exactly the division of labour `APP-05` already assumes.

## 3. `EPIC-H2` now has a document

It had a backlog row. It now has an Epic document on `EPIC-P2`'s pattern — **Specified, not
decomposed** — carrying three things the row could not:

- **The five lanes do not cost the same.** The row flattens Wails, Tauri, .NET AOT, Node and Bun into
  one cell. `.NET` Native AOT needs no webview and no JIT; Bun needs a JIT, transpiler, FFI and
  lifecycle scripts. **`.NET` is the cheapest lane and Tauri is not** — worth stating because
  `G-APP-2` names Tauri first and the ordering in a goal statement is not a schedule. §3 therefore
  recommends `H2-03` (.NET) before `H2-05` (Tauri runtime): it proves the conformance harness and the
  manifest intersection — the two pieces every other lane reuses — without waiting on `EPIC-H3`.
- **A missing dependency.** `backlog.md` lists `EPIC-H2` as depending on `EPIC-H1` and `EPIC-P5`. The
  Tauri and Wails lanes also need **`EPIC-H3`** for a rendering engine, and the row's silence reads as
  though a webview lane were reachable at H2. The row now says so.
- **Six constraints derived from the review** (§2 of the Epic), each written to become a boundary test
  rather than a paragraph.

## 4. What was deliberately not done

- **No Feature or Story documents.** [`README.md`](../../README.md)'s horizon rule holds — destination
  horizons "remain undecomposed until their prerequisites are real" — and `agent.md` says decompose
  just-in-time. `EPIC-H1` and `EPIC-P5` both precede this Epic and the webview lanes need `EPIC-H3`.
  **`EPIC-H2` §3 proposes six Features with the reasoning behind their ordering**, so decomposition
  starts from the review rather than from scratch; creating the documents now would pre-build a tree
  for work that cannot start.
- **No loose-end row, and the reason is mechanical.** Contiguity is enforced, `LE-53` is still
  uncommitted, and [`05B`](05B-next-session-agenda.md) records that the concurrent session already
  owes **two** rows it could not write for exactly that reason. Adding a third uncommitted claim on
  `LE-54` would deepen the jam and risk the `LE-43` double-write. The one candidate — that this review
  is pinned to a commit nothing in the repo vendors, so the next reader cannot reproduce it — is
  recorded in the review's §7 instead, with the argument both ways.
- **Tauri was not vendored as a submodule.** Recommended in review §7, not taken. `MsDOS/` and
  `WindowsTerminal/` are both in languages `CODING_STANDARDS.md` forbids, so "reference only" enforces
  itself. **Tauri is Rust**, and a vendored Rust workspace is one `path =` dependency away from being
  built upon — a `G-APP-2` lane silently becoming a fork. Worth doing *with the rule written down*;
  an owner decision either way.
- **No `application-platforms.tsv` change.** `APP-05`'s row already selects the right classes,
  controls and evidence, including renderer-compromise. The review found nothing that contradicts it.

## 5. Concurrency — a clobber caught, and someone else's red gate

Two commits landed mid-session (`280fe6e`, `3dfd1eb`), and the tree holds a concurrent session's
in-progress `FEAT-P1-06` actuation work.

- **`check-assurance-spine` is red in the working tree, and it is not this session's break.**
  `TEST-P1-06-01-A` has an assurance-state mismatch; that file is untracked and belongs to the other
  session. Per [`agent/CONCURRENT_SESSIONS.md`](../../agent/CONCURRENT_SESSIONS.md): not repaired, no
  `--no-verify`, **this session's subset verified over clean `HEAD` in a throwaway worktree instead.**
- **A real clobber was caught before it happened.** This session edited
  `session/hand-2026-07-29/index.html` *before* `3e624bc`/`3dfd1eb` landed. By the time the diff was
  read, the working copy would have **deleted the other session's `04B` and `05B` entries** — 9
  deletions against 6 additions. Restored from `HEAD` and the additions re-applied on top.
  **`git diff` before staging is what caught it**, which is the habit rule 1's second half asks for
  and the reason it is worth the tool call.
- **`LE-53` is still uncommitted and another session is blocked behind it.** `05B` says so explicitly:
  contiguity is enforced, so `LE-54` cannot exist while `LE-53` sits unstaged. **Landing this session
  is what unblocks them**, and it is the strongest argument for staging it promptly.

## 6. State at the close

```text
new documents      docs/tauri-internals-review.md, goals/epics/EPIC-H2.md
edited             docs/whole-system-context.md (Tauri section), goals/epics/backlog.md (H2 row)
registers          none touched beyond LE-53, already pending from 03A
spine              green for this session's subset over clean HEAD (worktree);
                   red in the working tree on another session's TEST-P1-06-01-A
code               none
```

## 7. Next

The work order is [`05B`](05B-next-session-agenda.md)'s and this document does not amend it —
`FEAT-P1-06` is W1 on the owner's instruction, and nothing here is on the critical path.

Two things this session leaves on the table, both cheap and both decisions rather than work:

1. **Vendor Tauri as a reference submodule**, with the "never a workspace member, never a `path`
   dependency" rule stated in `.gitmodules` and `EPIC-H2` (review §7).
2. **The `EPIC-P2` §6 edit** that `LE-53` scopes — still the cheapest open row, and now with `03A`
   and this review behind it.
