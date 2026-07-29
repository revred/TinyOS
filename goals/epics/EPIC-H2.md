# EPIC-H2 — Application Runtime Profiles: Wails, Tauri, .NET AOT, Node and Bun

Status: **Specified — no Feature document written and no Story started, deliberately.** [`README.md`](../../README.md)'s horizon rule holds: destination horizons "remain undecomposed until their prerequisites are real", and this Epic's are not. `EPIC-H1` (application ABI, graphics, audio, input) and `EPIC-P5` both precede it, and the Tauri and Wails lanes additionally need a webview engine that belongs to `EPIC-H3`.
Roadmap phase: **Destination horizon H2**, per [`backlog.md`](backlog.md). Not on the numbered critical path.
Introduced in: [`session/hand-2026-07-29/06A-tauri-internals-reviewed.md`](../../session/hand-2026-07-29/06A-tauri-internals-reviewed.md), which reviewed Tauri from source and found the existing one-paragraph position under-evidenced rather than wrong.
Depends on: `EPIC-H1`, `EPIC-P5` per `backlog.md`. **The Tauri and Wails lanes also depend on `EPIC-H3`** for a rendering engine — recorded here because `backlog.md`'s row does not say so and the omission reads as though a webview lane were reachable at H2.

## Goals verified (from `SeedMVP.md` §3)

**`G-APP-2`** (Wails and Tauri are first-class UX lanes), **`G-APP-3`**, **`G-APP-4`**.

## Why this document exists now, ahead of decomposition

Not to schedule the work. To stop a specific class of mistake.

`G-APP-2` says Tauri is a *first-class UX lane*, which is true and is founding intent. It is one short
inference from there to *"so build the shell in it"* — and
[`03A`](../../session/hand-2026-07-29/03A-tauri-and-the-tab-host.md) records why that inference is
wrong and `LE-53` carries the fix. **A first-class application lane is not a shell technology**, and
the two live in different Epics for reasons that are architectural rather than clerical.

The second reason: [`tauri-internals-review.md`](../../docs/tauri-internals-review.md) now establishes
from source what these runtimes actually do. The constraints in §2 are derived from that review rather
than from framework marketing, which is the standard [`SECURITY_CHARTER.md`](../../SECURITY_CHARTER.md)
§"Application-runtime policy" already sets: *"informed by the actual upstream execution models rather
than framework names."*

## 1. The five lanes do not have the same cost

`backlog.md` lists them in one row, which flattens a real difference. Porting cost, highest first:

| Lane | What it needs beyond `EPIC-H1` | Blocked by |
|---|---|---|
| **Tauri** | `impl Runtime for TinyOsRuntime` (a defined trait set) **plus a rendering engine** | `EPIC-H3` |
| **Wails** | The same webview requirement, **plus a Go toolchain targeting TinyOS** and GC/scheduler characterisation as C3 workload | `EPIC-H3` + a Go target |
| **Node** | A V8 JIT — `PD-04` says executable memory is sealed. Node's own permission model explicitly does not claim to contain malicious code | An `RCG-*` decision on JIT |
| **.NET AOT** | No runtime JIT by construction; needs generated capability bindings and signed, hash-pinned native deps | The least blocked of the five |
| **Bun** | JavaScriptCore JIT, transpiler, FFI, native addons, lifecycle scripts | Research only |

**`.NET` Native AOT is the cheapest lane and Tauri is not the cheapest lane**, which is worth stating
because `G-APP-2` names Tauri first and the ordering in a goal statement is not a schedule.

## 2. Constraints every lane inherits, derived from the review

Each is written so it can become a boundary test rather than a paragraph.

### 2.1 The framework's permission model is metadata, never the boundary

`agent.md` rule 10. The review's governing finding is that Tauri's core, `wry` and `tao` are **one
process**, and every command handler runs there with the app's full OS authority: the ACL decides
whether a string reaches a handler, not what the handler may then do. The same is true of Wails, and
Node's permission model says so itself.

**TinyOS intersects the framework's declared capabilities with the signed manifest and local policy.
It never treats them as the outer sandbox.** A conformance test that passes because the framework's
own ACL denied something has tested the framework, not the OS.

### 2.2 Empty authority first — and this one inverts a real default

`PD-03`. Tauri's `Capability.local` defaults to **`true`**: the local frontend gets what the
capability file lists. TinyOS requires the opposite default. **A ported manifest must not be trusted
to have made the safe choice**, because upstream's safe choice is a different one. This is the single
most likely place for a silent authority leak in a port, and it is a specific, testable claim.

The same boundary test carries a second, sharper assertion: a `Capability` may also bind commands to
**remote** URL-pattern contexts (the `remote` field's domain globs), and **no `remote` context
survives the manifest intersection** — the Charter's webview rule already forbids remote content
holding application IPC authority. A defaulted `local: true` looks like an omission; a carried
`remote` grant looks intentional, which is exactly why the test must not assume it was.

### 2.3 Caller identity is derived, never asserted

`PD-02`. Tauri already resolves the calling window/webview from Rust-side objects rather than the
payload, and that shape is worth preserving. **The exception must not travel:** the
`__TAURI_INVOKE_KEY__` bearer secret separates the app's frame from other frames and is
caller-supplied. Under TinyOS the frame's authority derives from the domain, not from possession of a
secret.

### 2.4 Remote content is C4, and the transition is dynamic

`PD-11`. Tauri computes origin from the webview's **current URL at each call**, so navigation changes
authority without teardown. The Charter already requires this; the review confirms it is implemented
upstream. What TinyOS adds is that the C4 renderer is a *separate domain*, not a separate `enum`
variant — and that `G-SEC-5` labels must survive the transition, which nothing upstream does.

### 2.5 Bounded, accounted, and below the real-time floor

`PD-05`, `PD-07`, `PD-08`, `PD-09`. Typed and mediated IPC is upstream; **bounded** is not — there is
no backpressure or size ceiling in Tauri's model, and no accounting of command or renderer cost.
[`FEAT-P0-07`](../features/FEAT-P0-07.md)'s `kernel::ipc::Channel` is the contrast worth keeping in
view: fixed-capacity, no-heap, fails closed on a full buffer. **Same word, different contract.** The
size ceiling must be enforced **before** deserialisation, at the transport layer — a bounded channel
after an unbounded parse has bounded nothing. No runtime profile runs at or above the real-time
floor.

### 2.6 The parser and the JIT are the admission question

`PD-04`, `PD-12`. A webview is a very large parser running in-process, and a JS engine maps
executable memory. Every lane except .NET AOT brings one or both. **This is an `RCG-*`
code-admission decision, not a runtime configuration choice**, and it is the gate that actually
governs whether Node and Bun are ever admitted.

### 2.7 The `H2-02`/`H2-05` named-test register (added 2026-07-29, 13F mandate U5/U7)

The PoC (`REPORT-2026-07-29-03`, Stage E in `REPORT-2026-07-29-04`) proved what a host can
prove and no more. This register turns both halves into **test ids**, so "unproven" is a
named red row rather than vague debt. The ids are stable now; the rows graduate verbatim
into `goals/assurance/story-contracts.tsv` when this Epic decomposes. (They cannot live in
`goals/security/containment-tests.tsv`: that catalogue's id grammar is the closed canonical
set `BND-01..BND-20`, machine-enforced — a deliberate property, not an oversight. The 13F
mandate's suggestion of `H2-02` rows there is corrected here.)

**Green on host today** — the Stage B probes, graduating into `H2-02`'s boundary-test set:

| id | Asserts | Today's evidence |
|---|---|---|
| `H2-02-T1` | `PD-03`: a capability that *omits* `local` resolves to no local authority (§2.2's default inversion) | fork `stage-b::b1_local_defaults_to_deny` |
| `H2-02-T2` | The inversion is of the default, not a ban: an explicit `local: true` grant is honoured | fork `stage-b::b2_explicit_local_grant_is_honoured` |
| `H2-02-T3` | A **carried** `remote` grant is stripped by the manifest intersection (the hostile input, 08C trap 4) | fork `stage-b::b3_carried_remote_grant_is_stripped` |
| `H2-02-T4` | A remote-only capability resolves to nothing at all | fork `stage-b::b4_remote_only_capability_resolves_to_nothing` |
| `H2-02-T5` | `PD-05`: the pre-parse size ceiling refuses oversized payloads on raw bytes, before any deserialisation (§2.5) | the knob-on `+1` test in the fork's `tauri` suite (58th) |

**Red until the OS exists** — `PD-01`/`PD-07`/`PD-08`/`PD-12` cannot be host-proven
(REPORT-2026-07-29-03 non-claims). Each is a named row that stays red by design:

| id | Asserts | Blocked on |
|---|---|---|
| `H2-02-R1` | `PD-01`: a command handler holds only granted authority — handler compromise cannot reach ungranted memory, devices or files (§2.1: the framework ACL is metadata, never the boundary) | real domains under the runtime profile (`U5`) |
| `H2-02-R2` | `PD-07`: every command's cost is bounded and accounted against its caller | OS accounting |
| `H2-02-R3` | `PD-08`/`PD-09`: renderer and IPC work is charged to the requesting domain, and backpressure fails closed | OS charging |
| `H2-02-R4` | `PD-12`: no runtime-profile work runs at or above the real-time floor, under load, measured | OS scheduling + `EPIC-H3` |

**`H2-05-AC1` (U7 — the invoke-key criterion, pinned so it cannot be forgotten or done
early):** the `__TAURI_INVOKE_KEY__` bearer secret (§2.3) is removed **only** in the same
change that makes the IPC transport carry kernel-derived caller identity, and its removal
test asserts both directions: the key is gone, *and* a forged/absent identity is refused by
the kernel-side check rather than by possession of a secret. Removing the key before that
transport exists deletes a mitigation without supplying a boundary
(REPORT-2026-07-29-03 finding 3); adding the transport without removing the key leaves a
caller-supplied credential load-bearing. Acceptance is the pair, atomically.

## 3. Proposed Features — for whenever this Epic is decomposed

**Not created as documents.** Recorded so decomposition starts from the review rather than from
scratch. Naming and numbering are provisional.

| # | Proposed Feature | Note |
|---|---|---|
| `H2-01` | The runtime-profile conformance harness | Shared by all five lanes; the thing that makes "supported" mean something. Should come first regardless of which lane leads |
| `H2-02` | Capability-binding generator: framework manifest → signed TinyOS manifest | §2.1/§2.2 live here. The intersection, and the default inversion |
| `H2-03` | `.NET` Native AOT profile | Cheapest lane; proves `H2-01`/`H2-02` without a webview |
| `H2-04` | Window/input/display service (`app-webview` profile's non-webview half) | `SeedMVP.md` §"`app-webview`" already names it; shared with `EPIC-P2`'s tab host |
| `H2-05` | `tauri-runtime` implementation for TinyOS | The trait seam. Blocked on `EPIC-H3` for an engine |
| `H2-06` | Wails/Go target and GC characterisation | Blocked additionally on a Go toolchain |

**`H2-03` before `H2-05` is the recommendation**, and it is the reverse of the order `G-APP-2` implies.
A lane with no webview and no JIT proves the harness and the manifest intersection — the two pieces
every other lane then reuses — without waiting on `EPIC-H3`.

## 4. Non-goals

- **Not a shell technology.** See [`03A`](../../session/hand-2026-07-29/03A-tauri-and-the-tab-host.md)
  and `LE-53`. `EPIC-P2`'s tab host is not built on any of these.
- **Not a compatibility claim.** A runtime profile runs applications *written for* the framework
  against TinyOS's ABI. It is not the upstream framework's own binary, and no Report may imply it is.
- **Never additions to the 8 MiB core image.** [`REPORT-2026-07-26-28`](../reports/REPORT-2026-07-26-28.md)
  already fixes these as optional profiles by definition.

## 5. Exit criteria

- Every lane that claims support has a passing `H2-01` conformance run, and a capability binding
  produced by `H2-02` rather than a hand-written manifest.
- No lane's framework-declared permissions are load-bearing in any test (§2.1).
- `PD-03`'s default inversion **and remote-context stripping** (§2.2) have a boundary test per lane,
  not a note in a document.
- Any JIT-bearing lane has an explicit `RCG-*` admission decision recorded before it is called
  supported (§2.6).
