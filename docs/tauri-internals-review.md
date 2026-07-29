# Tauri Internals — A Source-Grounded Review Against the TinyOS Protection Domain

**Status: reference analysis. No commitment, no schedule.** This document reviews what Tauri
*actually does*, from its source, and maps each mechanism onto the [`SECURITY_CHARTER.md`](../SECURITY_CHARTER.md)
`PD-*` contracts. It exists because [`whole-system-context.md`](whole-system-context.md)'s Tauri
section was one paragraph asserting alignment, and an assertion is not a review.

**Read alongside** [`session/hand-2026-07-29/03A-tauri-and-the-tab-host.md`](../session/hand-2026-07-29/03A-tauri-and-the-tab-host.md),
which settles the separate question of whether Tauri should build `EPIC-P2`'s shell (it should not).
This document is about the **application lane** — `G-APP-2`, `APP-05`, `EPIC-H2` — where Tauri is
already first-class by founding intent.

## Provenance

Reviewed against `tauri-apps/tauri` `dev` at **`872428f`**, `tauri-runtime-wry` **2.11.4**, over
`wry` **0.55** and `tao` **0.35**. Line references are to that commit. Tauri is dual Apache-2.0/MIT,
compatible with TinyOS's MIT ([`ADR 0006`](adr/0006-mit-licence-confirmed-and-open-core-optionality-dropped.md)).

**Now vendored as a submodule at `external/tauri`** (the PoC fork, pinned per
[`ADR 0008`](adr/0008-external-trees-live-under-external.md)) — this review predates that and was
written when it was not. `external/MsDOS` and `external/WindowsTerminal` are held on the "reference
only, never built upon" terms [`EPIC-P2`](../goals/epics/EPIC-P2.md) §6.4 sets out. Tauri was a
stronger candidate for a *stricter* treatment than either, because unlike them it is Rust and the
temptation to build on it is real — which is why its tier carries ADR 0007's constraints and the
isolation gate; §7 records the argument both ways.

## 1. The workspace, and where the seams are

| Crate | Role | Why it matters here |
|---|---|---|
| `tauri` | The app-facing core: IPC, ACL enforcement, plugins, windows, webviews | The part with the security model |
| `tauri-runtime` | **Trait-level abstraction** of windowing and webview | **The porting seam** — §4 |
| `tauri-runtime-wry` | The only shipped implementation, over `wry` + `tao` | What a TinyOS port would replace |
| `tauri-utils` | ACL data model, isolation pattern, config | Where the manifest grammar lives |
| `tauri-build` / `tauri-codegen` / `tauri-macros` / `tauri-plugin` | Build-time permission resolution and codegen | **Authority is computed at build time** — §3 |

The single most useful structural fact: **the OS binding is already behind a trait**, and the
security model is in a different crate from it.

## 2. IPC — how a call actually crosses the boundary

### 2.1 Transport

A frontend call becomes an HTTP-shaped request to a **custom protocol**, `ipc://localhost/{command}`
(`crates/tauri/src/ipc/protocol.rs`), with a `postMessage` fallback on platforms where the custom
protocol is unavailable. The payload is `InvokeBody::Json(JsonValue)` or `InvokeBody::Raw(Vec<u8>)`,
plus two callback function ids and a `Tauri-Invoke-Key` header.

### 2.2 Caller identity — and this is the good part

```rust
// crates/tauri/src/webview/mod.rs:1518
.resolve_access(&cmd_name, self.window().label(), self.label(), &origin);
```

`window` and `webview` are **`&str` labels read from the Rust-side objects**, not from the payload.
The IPC handler is bound per-webview at creation, so which webview sent a message is known from
*where the message arrived*, never from what it claims.

**That is `PD-02` — "resolve actor identity from the running TCB; never from caller-supplied
identifiers" — reached independently by a userspace framework.** It is worth crediting precisely,
because it is the mechanism TinyOS's own ACI needs and Tauri gets it right.

### 2.3 Origin, and why navigation is the interesting event

```rust
// crates/tauri/src/webview/mod.rs:~1505
let current_url = self.url()?;
let is_local = self.is_local_url(&current_url);
let origin = if is_local { Origin::Local } else { Origin::Remote { url: current_url } };
```

Origin is derived from the webview's **current URL at call time**. Navigate the webview to a remote
origin and its authority changes on the next call, with no teardown required.

[`SECURITY_CHARTER.md`](../SECURITY_CHARTER.md) — the Wails/Tauri bullet under §"JavaScript,
webview, and generated-code paths" — already requires exactly this — *"a local webview that
navigates to a remote origin loses local application IPC authority."* This review confirms it is
**an implemented mechanism upstream, not an aspiration TinyOS would have to add.** The Charter's
requirement and Tauri's behaviour agree.

**One edge the per-call check does not close.** A handler already executing when the navigation
happens retains its authority for the rest of its run, and a `Channel` (§2.6) opened before
navigation keeps streaming after it. Under TinyOS's target model the question dissolves — the C4
renderer is a separate domain, not the same webview with a different origin — but on the fork path
(§7) revocation-on-navigation is a real requirement, and it belongs to `PD-13`'s row in §6, not to
this section's success story.

### 2.4 Authorisation

`RuntimeAuthority` (`crates/tauri/src/ipc/authority.rs:28`) holds two
`BTreeMap<String, Vec<ResolvedCommand>>` — `allowed_commands` and `denied_commands`. `resolve_access`
checks deny first, then allow, then filters by window/webview glob and origin match. Beyond the
allow/deny verdict, commands carry **scopes**: typed allow/deny object lists (`ScopeObject`,
`ResolvedScope`) that the *plugin* interprets — filesystem path globs, URL patterns.

**The shape to note: authorisation is a lookup on a string command key.** `plugin:{name}|{command}`.
It is a well-built input filter over a flat namespace.

### 2.5 The invoke key — the one caller-supplied credential

`__TAURI_INVOKE_KEY__` / the `Tauri-Invoke-Key` header is a secret injected into the trusted local
page so the core can distinguish its own frontend from other frames that can also reach the custom
protocol.

**This is a bearer token supplied by the caller**, and it is the exception to §2.2's otherwise clean
property. Its security rests on the secret not leaking; an XSS in the local page yields it. It is a
sensible mitigation for a real problem and it is *not* a boundary — the contrast with `PD-02` is
sharp and worth stating, because a reader who takes §2.2 as the whole story would over-trust it.

### 2.6 Channels

`ipc/channel.rs` provides `Channel` / `JavaScriptChannelId` for streaming Rust→JS. Conceptually
adjacent to [`FEAT-P0-07`](../goals/features/FEAT-P0-07.md)'s `kernel::ipc::Channel`, and worth
naming the difference: TinyOS's channel is **bounded, fixed-capacity, no-heap and fails closed on a
full buffer** (`ChannelError::Full`). Tauri's is a `serde`-serialising, heap-allocating stream. Same
word, different contract; do not let the name imply portability.

## 3. Plugin architecture — build-time authority

```rust
// crates/tauri/src/plugin.rs:37
pub trait Plugin<R: Runtime>: Send {
  fn initialize(&mut self, app: &AppHandle<R>, config: JsonValue) -> Result<()>;
  fn on_page_load(&mut self, webview: &Webview<R>, payload: &PageLoadPayload<'_>);
  fn on_event(&mut self, app: &AppHandle<R>, event: &RunEvent);
  fn extend_api(&mut self, invoke: Invoke<R>) -> bool;   // returns: did I claim this command?
}
```

The pipeline that matters is not the trait, it is **where authority is decided**:

1. A plugin ships a `PermissionFile` (TOML/JSON) declaring permissions and permission sets.
2. `tauri-build` / `tauri-plugin` compile those into a `Manifest` at **build time**.
3. The app ships `Capability` files selecting permissions and binding them to windows/webviews and
   to local/remote contexts.
4. `tauri-codegen` resolves manifests × capabilities into a `Resolved` ACL **baked into the binary**.

**The authority set is fixed at compile time.** (`dynamic-acl` is an opt-in feature for runtime
capability construction; the default path is static.) That is a genuinely strong property and it is
the single best fit with TinyOS's model: a *signed manifest* is exactly a build-time-fixed authority
set, and `PD-03`'s "empty authority first" wants precisely this kind of enumerable, pre-resolved
grant rather than a runtime request-and-prompt flow.

**Where it stops fitting:** `Capability` defaults `local: true`. The default posture is *the local
frontend gets what the capability lists*, not *nothing until granted*. Under `PD-03` the default
must invert.

**And the second place, which is the sharper one:** a `Capability` may also bind its commands to
**remote** contexts — URL-pattern grants (the `remote` field's domain globs) that hand typed-command
authority to remote origins, exactly the thing §2.3's origin tracking otherwise takes away. The
Charter's webview rule forbids this outright — *"remote web content is a separate C4 renderer, not a
trusted continuation of the local frontend"* — so the manifest intersection must **strip every
`remote` context**, not merely invert the `local` default. It is the sharper leak surface of the
two because a ported manifest carrying a `remote` grant looks intentional, where a defaulted
`local: true` at least looks like an omission.

## 4. OS specifics and the "driver" seam

`tauri-runtime` defines `Runtime<T>`, `RuntimeHandle<T>`, `WebviewDispatch<T>`, `WindowDispatch<T>`,
`EventLoopProxy<T>`, `WindowBuilder`. `tauri-runtime-wry` is the sole implementation, over `wry`
(webview binding) and `tao` (windowing, a `winit` fork).

`wry` binds the **platform** webview: WebView2 (Windows), WKWebView (macOS/iOS), WebKitGTK (Linux),
Android WebView. One capability worth naming while here: **multiple webviews per window exist
upstream but only behind the `unstable` feature gate** — relevant to TinyOS because one-webview-per-
domain is the natural shape for per-origin C4 renderers, and §7.1 lists the gate among the things a
fork can simply turn on.

**So a TinyOS port is, structurally, `impl Runtime<T> for TinyOsRuntime`** — a well-defined trait set
rather than a fork. That is the strongest synergy in this review, and it is real.

**And it is also where the cost sits, undiminished.** Implementing `Runtime` requires something to
dispatch *to*. Tauri ships no renderer; it has never had one. A TinyOS `Runtime` implementation is a
window/input/compositor service (which `SeedMVP.md`'s `app-webview` profile already names) **plus a
browser engine** (`EPIC-H3`). The trait seam makes the port *tractable and well-shaped*; it does not
make it small, and nothing about it removes the engine.

## 5. Process model — the finding that governs everything else

**Tauri's core, `wry` and `tao` are one process.** Webview *content* may be multi-process, but that
is WebView2's or WebKitGTK's doing, not Tauri's — and Tauri does not broker it.

Consequences, stated plainly:

- **Every command handler runs in the app process with the app's full OS authority.** The ACL decides
  *whether the string reaches the handler*. It does not constrain what the handler may then do.
- **Tauri's ACL is an in-process input filter, not an OS boundary.** This is not a criticism; Tauri
  does not claim otherwise, and [`SECURITY_CHARTER.md`](../SECURITY_CHARTER.md) already states the
  position. What this review adds is that the position is now **evidenced from source** rather than
  inferred from a security page.
- **`agent.md` rule 10 is therefore load-bearing, not boilerplate.** "Runtime permissions never
  replace the OS Protection Domain." Tauri's capabilities are *useful app metadata* that TinyOS
  intersects with the signed manifest — which is what `whole-system-context.md` already says, and now
  there is a reason behind it.

### The Isolation Pattern

Optional (`tauri-utils/src/pattern/isolation.rs`): injects a sandboxed `<iframe>` that **AES-256-GCM**
encrypts IPC payloads, so a developer-controlled script mediates messages from the main frame before
Rust sees them. Real defence in depth against compromised *frontend* code.

It is enforced by JavaScript in the same renderer, keyed by a secret in that renderer. Against a
renderer compromise it is not a boundary. Useful; not a substitute for `PD-01`.

## 6. The map: Tauri against `PD-01`…`PD-14`

| Contract | Verdict | Note |
|---|---|---|
| `PD-01` Private active address spaces | **Not provided** | One process; the OS supplies this or nobody does |
| `PD-02` Kernel-derived caller identity | **Shape matches** (§2.2) | Except the invoke key (§2.5) |
| `PD-03` Empty authority first | **Inverted default** | `Capability.local` defaults `true`; `remote` URL-pattern contexts must not survive intersection (§3) |
| `PD-04` Executable memory is sealed | **Violated by construction** | A JS engine JITs; `RCG-*` gates apply |
| `PD-05` Typed bounded mediated IPC | **Typed and mediated; not bounded** | No backpressure, no size ceiling — and the ceiling must sit before the `serde` parse, not merely on the channel (§7.1) |
| `PD-06` Generation-safe shared memory | **Not applicable** | No shared-memory primitive |
| `PD-07` Temporal isolation | **Not provided** | No scheduling guarantees; below the RT floor |
| `PD-08` Finite charged resources | **Not provided** | No accounting of command or renderer cost |
| `PD-09` Caller-funded broker work | **Not provided** | Commands run on the app's own runtime |
| `PD-10` Device-bound DMA/IRQ/MMIO | **Not applicable** | |
| `PD-11` Non-increasing provenance | **Partial** (§2.3) | Origin tracked; no `G-SEC-5` label propagation, and `remote` capability contexts can grant against the gradient (§3) |
| `PD-12` Fault containment, parser exclusion | **Violated by construction** | The webview is a very large parser, in-process |
| `PD-13` Revoke, wipe, advance generation | **Not provided** | In-flight handlers and open channels survive navigation (§2.3); revocation-on-navigation is the fork's obligation |
| `PD-14` No ambient namespace authority | **Shape matches** | Commands are explicitly enumerated, not ambient |

**The pattern in that table is the conclusion.** Everything Tauri does well is *interface shape* —
identity derivation, typed commands, origin tracking, build-time authority, explicit enumeration.
Everything it does not do is *isolation, accounting and time*. That is exactly the division of labour
`APP-05` already assumes: TinyOS supplies the domain, Tauri supplies the ABI ergonomics.

## 7. Forking Tauri — which is the owner's stated intent

**Modifying Tauri, `tao` and the IPC internals to fit the security framework is in scope — decided
2026-07-29 in [`ADR 0007`](adr/0007-modifying-tauri-is-in-scope-at-the-seams.md), which records the
alternatives considered; this section is the analysis behind that decision, not its record.** It is
a legitimate and materially different posture from the reference-only treatment `MsDOS/` and
`WindowsTerminal/` get, and it changes this review's conclusions. Apache-2.0/MIT permits it outright.

### 7.1 What a fork fixes — and it is most of §6's middle column

Every finding in this review that is a *policy* or *interface* defect becomes editable. The third
column classifies each against §7.3's health metric — **patch** (small, rebaseable diff), **seam**
(TinyOS-side code behind an existing trait, zero patch), or **rewrite-risk** (can silently blow the
metric):

| Finding | Fixed by a fork? | Cost class |
|---|---|---|
| `Capability.local` defaults `true`, `remote` contexts stripped (`PD-03`, §3) | **Yes, trivially.** A default and a schema | **Patch** |
| `__TAURI_INVOKE_KEY__` bearer secret (§2.5) | **Yes.** Replace with kernel-derived domain identity — the mechanism `PD-02` already wants | **Patch**, contingent on the transport below |
| ACL is a string-keyed in-process filter (§5) | **Yes, in part.** Authority resolution can defer to the real ACI engine instead of a `BTreeMap` | **Patch** if the §7.3 resolver trait lands upstream; a carried patch otherwise |
| IPC unbounded, no backpressure (`PD-05`) | **Yes.** Replace the transport with a bounded, fails-closed channel of `kernel::ipc::Channel`'s shape | **Rewrite-risk** — see below |
| Revocation on navigation for in-flight work (`PD-13`, §2.3) | **Yes.** Cancel handlers and close channels when origin drops to remote | **Patch** |
| multiwebview behind `unstable` (§4) | **Yes.** A feature gate | **Patch** |
| No resource accounting (`PD-08`, `PD-09`) | **Partly** — the hooks can be added; the charging needs the OS underneath | **Seam** |

Two of those rows deserve their caveats stated:

- **The transport row is the one that can convert the fork into a rewrite on its own.** Swapping the
  custom-protocol/`postMessage` path for a bounded channel touches serialisation, the JS glue and
  every command's dispatch path. The *patch-sized* part of `PD-05` is different and cheaper: a size
  ceiling enforced **before** the `serde_json` parse, at the protocol layer — a bounded channel after
  an unbounded parse has bounded nothing. Take the ceiling as a patch; treat the full transport
  replacement as the thing §7.3's metric exists to catch.
- **The invoke-key row depends on what identity the transport can carry.** Until a TinyOS transport
  exists that delivers kernel-derived identity, deleting the bearer token removes a mitigation
  without supplying the boundary.

**This is a real strengthening of the case**, and it should be said plainly: most of what this review
faults Tauri for is *upstream's reasonable choice for a desktop app framework* and not a constraint
on TinyOS.

### 7.2 What a fork does not fix

- **Tauri ships no renderer, and forking it does not produce one.** `wry` binds the platform webview.
  The engine question (`EPIC-H3`), the JIT question (`PD-04`, `RCG-*`), and the real-time question
  (`PD-07`, §6.6 of `EPIC-P2`) all live in the *engine*, which is not in this repository and is not
  Tauri's. **The fork removes the objections it can reach and leaves the largest one untouched.**
- **Two binding repository rules collide immediately.** `agent.md` rule 7: *all code lives under
  `os/src/`*. Rule 4: *no crate exceeds 20,000 lines, excluding tests* — and
  [`CODING_STANDARDS.md`](../agent/CODING_STANDARDS.md) says it is enforced "the same way for every
  crate in the workspace", with *"do not ask for an exception; there isn't one."*

  Measured at `872428f`: **`tauri` is 32,457 lines** (1.6× the ceiling), `tauri-utils` 15,452,
  `tauri-runtime-wry` 6,719, `tauri-runtime` 2,683. A vendored fork inside the workspace is
  non-compliant on day one. It must therefore live **outside** the workspace as a pinned external
  dependency, or be split, or the rule must be amended deliberately — a decision, not an oversight.
- **Security maintenance transfers**, and a transfer needs a process, not an acknowledgment.
  [`ADR 0007`](adr/0007-modifying-tauri-is-in-scope-at-the-seams.md) fixes it as three conditions of
  the fork: subscribe to the RUSTSEC/GHSA advisory streams for `tauri`, `wry` and `tao`; rebase onto
  upstream patch releases as they land; and re-run this review's §6 mapping over the patched surface
  at every rebase. An unrebased fork with an open upstream advisory is a defect for
  [`loose-ends.tsv`](../goals/assurance/loose-ends.tsv), not a backlog preference.

### 7.3 The recommendation: minimise fork surface by preferring the seams

Not all modifications cost the same, and the difference is architectural rather than clerical:

1. **Windowing and webview binding — no fork needed.** `tauri-runtime` is already a trait. Implement
   `Runtime`/`WebviewDispatch`/`WindowDispatch` for TinyOS and neither `tao` nor `wry` is patched at
   all. **Replacing `tao` wholesale is cheaper than modifying it**, and this is the single most
   valuable structural fact in the review.
2. **Authority resolution — this is where the patch set belongs.** `RuntimeAuthority` is a concrete
   struct, not a trait, so deferring authorisation to the ACI engine means patching it or wrapping
   it. Introducing a resolver trait there is a small, mechanical diff that plausibly benefits
   upstream too — **worth attempting as a contribution before carrying it as a patch.**
3. **IPC transport — a patch, bounded in size**, replacing the custom-protocol/`postMessage` path
   with a typed bounded channel.

**Keep the diff small, mechanical and rebaseable, and track upstream rather than hard-forking.** The
measurable health metric is the size of the patch set against unmodified upstream; if it grows past
"a few hundred reviewable lines", the fork has become a rewrite and should be recognised as one.

### 7.4 Vendoring, for reproducibility

Independent of the fork question, this review is pinned to a commit nothing in the repository
contains, so it cannot currently be reproduced or re-checked. Vendor the upstream so the analysis
has a referent — but **at the release tag corresponding to `tauri-runtime-wry` 2.11.4, not at the
`dev` commit** ([`ADR 0007`](adr/0007-modifying-tauri-is-in-scope-at-the-seams.md) constraint 3).
Advisories reference releases; a dev-commit baseline never maps onto them. `872428f` remains the
referent for this document's line numbers only. Wherever the vendored tree lands, state the
workspace-exclusion rule from §7.2 explicitly alongside it.

### 7.5 Executed

The PoC [`08C`](../session/hand-2026-07-29/08C-tauri-poc-execution-cover-note.md) ordered ran
on 2026-07-29: this review was reproduced at the release tag (every claim survives; §7.2's
size figures measure slightly smaller there, as the Provenance section predicts), and §7.1's
patch/seam classification held under test —
[`REPORT-2026-07-29-03`](../goals/reports/REPORT-2026-07-29-03.md).

## 8. What this changes, and what it does not

**Changes:** `whole-system-context.md`'s Tauri paragraph is now backed by mechanism. Three claims in
it are confirmed from source (the C3 core, typed-command frontends, remote→C4). One is sharpened:
Tauri's capabilities are not merely "useful app metadata" — they are *build-time-resolved* metadata,
which is a better fit for signed manifests than a runtime permission model would be.

**Does not change:** the horizon. `APP-05` stays `later`, `EPIC-H2` stays undecomposed, and the
dependency chain is unchanged — `EPIC-H1` (application ABI, graphics, input) and a webview engine
both precede any of this. Nothing here is a prerequisite of Phase 0–2 work, and §4 is explicit that
the trait seam makes the port well-shaped rather than cheap.
