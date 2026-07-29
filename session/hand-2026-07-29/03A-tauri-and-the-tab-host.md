# Handover 03A — Tauri and the Tab Host: Where a Webview Belongs, and Where `EPIC-P2` §6 Already Excludes One

**A decision record, not work done.** No code, no contracts, no Story. One `LE-53` row and this
document. Raised by a direct question — *should TinyOS support Tauri for the multi-tab command prompt
and the UX aspiration?* — whose honest answer turned out to be three different answers to three
different questions that the wording bundles together.

## 1. The three questions, separated

| Question | Answer | Where it is already decided |
|---|---|---|
| Is Tauri a supported **application** lane on TinyOS? | **Yes, already.** Nothing to revisit | `SeedMVP.md` §3 `G-APP-2`; `application-platforms.tsv` `APP-05`; `EPIC-H2` |
| Should Tauri **implement** `EPIC-P2`'s multi-tab shell? | **No** | Nowhere — §2 is why, and that gap is `LE-53` |
| Should a Tauri **host-side** operator console exist? | **Plausibly yes**, and it is the cheapest route to the UX aspiration | `EPIC-H4` (WST) / `EPIC-H5` (lab) — unregistered as a concrete lane |

The middle row is the one worth writing down, because the first row makes the wrong answer to it
*attractive*: "Tauri is a first-class UX lane" is true, and it is one short inference away from "so
let's build the shell in it."

## 2. `EPIC-P2` §6 already excludes it — without saying so

This is the substance. [`EPIC-P2`](../../goals/epics/EPIC-P2.md) §6 states four requirements that
together rule out **any single-webview front end** — Tauri, Wails, Electron-shaped alike:

- **§6.3, the trusted path.** A reserved region of the window that **no tab content can ever paint**,
  "owned by the shell host, structurally unreachable by tab content", plus a secure-attention key
  handled before any tab sees input. Inside one webview the reserved region and the tab content are
  **the same DOM in the same renderer**. The requirement is not merely hard to meet there; it is not
  expressible.
- **§6.1, a tab is an authority boundary.** Tauri 2's per-webview capabilities look like a fit, but
  [`agent.md`](../../agent.md) rule 10 and [`SECURITY_CHARTER.md`](../../SECURITY_CHARTER.md) §"actual
  upstream execution models" both hold that runtime permissions never replace the Protection Domain.
  Enforcement is ACI capabilities per tab session either way — so the framework's ACL buys nothing on
  the hard part while adding a second permission model to keep in sync.
- **§6.6 and `agent.md` rule 9.** A JS engine JIT maps executable memory; *remote bytes are data,
  never code* runs through the `RCG-*` gates. A JIT below the real-time floor, inside the tool an
  operator uses to `admit` real-time tasks, is a trust inversion rather than a dependency.
- **§6.5's injection rule gets worse, not better.** The rule exists because filenames and labels are
  attacker-influenced strings the prompt renders. Terminal escape sequences are the stated threat;
  **HTML injection into a status bar is strictly more capable**, and it lands directly on §6.3's
  reserved region.

Two structural points beyond the security ones:

**Tauri ships no renderer.** It binds the *platform* webview — WebView2, WKWebView, WebKitGTK.
TinyOS has none, so "Tauri for the shell" reads "ship a Chrome-class browser engine first", which is
`EPIC-H3`. But `EPIC-P2`'s own header records that it **gates** `EPIC-P3`, `EPIC-H1` and `EPIC-H5`.
That inverts the critical path: the shell would wait on the browser that waits on the shell.

**The image budget already says no.** [`REPORT-2026-07-26-28`](../../goals/reports/REPORT-2026-07-26-28.md)
records Tauri and Wails as optional profiles, *"never additions to the 8 MiB core image by
definition."* The shell is core.

**§6.4 is one step from saying this and does not take it.** It works through Windows Terminal as
reference *and* counter-example, and lands on "take the interaction model, take the component
boundaries; do not take the authority model — there isn't one." That is the same reasoning applied to
a specific artefact. It never generalises to *which implementation families* §6.3 rules out, so a
reader can arrive at the opposite conclusion from the same document. **That is `LE-53`.**

## 3. The cheap thing that must not be missed

§6.4 calls Windows Terminal's **text-buffer / renderer separation** "worth copying as a *shape*". It
is more than that: **§6.6's obligation to drop frames rather than block depends on that seam
existing.** A renderer that cannot be starved independently of the buffer cannot drop a frame; it can
only block, which §6.6 forbids.

No requirement anywhere states it. It costs nothing before the `shell` crate exists and is expensive
to retrofit after — and it is also, incidentally, exactly what keeps a webview front end possible
*later* without taking the dependency *now*. Option preserved, dependency not taken. It is the second
half of `LE-53` for that reason.

## 4. The reality check that outranks all of this

`EPIC-P2` §1: **15 of its 22 verbs cannot be implemented at all**, because no filesystem crate exists
anywhere in the 18-crate map. That is `LE-48`, still open, still a decision the owner has not taken.

**The shell's blocker is storage, not the front-end framework.** Nothing in this document changes
that ordering, and it should not be read as making the front end the next question.

## 5. Where the "yes" actually is

[`whole-system-context.md`](../../docs/whole-system-context.md) §"Windows TinyOS Tools" and `EPIC-H4`
already carry WST as the Windows host-side companion; `EPIC-H5` carries the browser-hosted lab. A
Tauri operator console **running on a host** — Windows, macOS or Linux — talking to TinyOS over
serial/HBP/WCI has a real webview available, touches neither the 8 MiB image nor the real-time floor,
and could deliver the multi-tab UX aspiration long before the on-target shell exists.

That is a new decision, not a recorded one, and it is deliberately **not taken here** — this document
records that the lane exists and is cheap, so the next reader does not have to rediscover it.

## 6. What was registered

`LE-53` — the unrecorded exclusion (§2) and the unstated buffer/renderer seam (§3), together, because
they are one gap in one section and the remedy for both is an edit to `EPIC-P2` §6.

**Deliberately not registered:** the WST console (§5) is an *opportunity*, and `loose-ends.tsv` is a
defect register. It belongs in `EPIC-H4`'s decomposition whenever that happens, and it is recorded
here rather than forced into the wrong register.

## 7. Recommended next step, if this is worth more than a row

An ADR. `docs/adr/0003`–`0004` are 51 and 54 lines, so it is not a large document, and the ADR set is
where this project records *decisions with alternatives considered* rather than defects. §2's argument
is already the body of one. Not written yet, because `LE-53` is the cheaper artefact and it is the one
that will actually be read by someone editing `EPIC-P2`.
