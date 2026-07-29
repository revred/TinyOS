# Handover 13F — Mandate: Finish the Unknowns, Run the Console, Measure the Terminal Gap

Follows: [`12F-external-trees-restructure.md`](12F-external-trees-restructure.md) (same session).
This is a cover note in the `08C` pattern: the start-here document for the sessions that execute
it. It orders three things — close or pin every remaining unknown from the Part 2 plan, produce a
**runnable Tauri operator console**, and produce a **gap analysis of terminal-window and command
behaviour against MS-DOS and Linux**.

## 0. One correction before anything runs

The owner's phrase "run the Tauri OS Shell" must not be read against the decided record.
**Tauri is disqualified as the on-target shell/tab host** (`LE-53`: the §6.3 reserved region and
secure-attention key, the missing renderer, the 8 MiB core image) and nothing here reopens that.
What *is* runnable — and what this mandate orders — is the **host-side operator console**: a Tauri
application on the host, holding a signed manifest, driving TinyOS-under-QEMU over the existing
serial fixtures. That is Stage E of the PoC (`08C` §Stage E), the `EPIC-H4` lane `03A` §5 named,
and `U4` of the Part 2 plan. It is the closest thing to "the Tauri OS shell" that the
architecture permits, and it is genuinely close: every piece it composes already exists and
passed its stage.

The on-target shell remains native Rust `TINYCMD`, and its blocker remains storage (`LE-48`),
not any frontend framework.

## 1. Deliverable A — the console runs (U4 / Stage E / EPIC-H4)

**What:** `tinyos-poc/stage-e-console/` in the fork repository (`external/tauri` submodule →
`C:\Code\tinyos-tauri-fork`), a new member of the `tinyos-poc/` workspace beside stages A–D,
path-depending on the vendored crates in the one permitted direction. **Never** in the `os/`
workspace — the exclusion rule travels, and `check-external-isolation` now fails the spine if
this is gotten wrong.

**Shape:** a Tauri app whose window hosts the console UI and whose Rust side owns the QEMU
child. Launch fixtures through the same command surface `cargo run -p xtask -- qemu-x86_64
--fixture=<name>` uses (see `list-fixtures` for the catalogue; `boot-banner` is the natural
smoke fixture). Serial I/O is the transport: QEMU serial ↔ console pane. Authority goes through
the Stage C seam — an `AuthorityResolver` backed by a **signed manifest** enumerating exactly
the console's verbs (launch fixture, send line, read stream, terminate), deny-by-default, so the
console *demonstrates* the manifest-resolved-authority lane rather than merely rendering text.

**Acceptance (all must hold):**

1. A fixture boots, its serial output renders live in the console, and a clean
   `isa-debug-exit` is reported as PASS/FAIL — the same verdict `xtask` computes.
2. Every console→target action is resolved through the installed `AuthorityResolver` against
   the signed manifest; an unlisted verb is denied and the denial is visible in the UI.
3. The upstream suite still passes both knob positions (the standing Stage B/C invariant), and
   the cumulative patch metric is re-measured and stated — the console must add ~zero to it,
   since it is `tinyos-poc/`-side, not vendored-crate-side.
4. The fork's working tree is committed and the `external/tauri` submodule pin in TinyOS is
   advanced to the new head in the same session that lands the console.

**Non-claims (state them in the report):** this proves the `EPIC-H4` lane's shape end to end —
Tauri UI ∩ signed manifest ∩ real kernel under QEMU. It proves nothing about on-target
isolation, accounting or time (`PD-01/07/08/12` still wait on the OS, `U5`), and it does not
advance `EPIC-H3`.

**Kill criterion:** none — `08C` already ruled Stage E cannot kill the fork. If it fails, the
failure is a finding about the console lane, filed as a loose end, not a verdict on ADR 0007.

## 2. Deliverable B — the terminal gap analysis

**What:** a measured comparison of TinyOS's *decided* terminal behaviour against the two
reference trees that now live where the contract says (`external/MsDOS`, `external/WindowsTerminal`)
and against Linux/POSIX behaviour, producing `docs/terminal-gap-analysis.md` plus a
machine-checkable `goals/context/terminal-gap.tsv` (one row per behaviour, columns:
`behaviour`, `msdos`, `linux`, `windows_terminal`, `tinyos_decision`, `status`, `evidence`).
A TSV because prose gap analyses rot; a register can be gated later the way everything else is.

**Three axes, honestly separated:**

1. **Command surface** — [`docs/cli-compatibility-mvp.md`](../../docs/cli-compatibility-mvp.md)
   already maps the canonical verb core to both bindings (`DIR`/`ls`, `COPY`/`cp`, …). The gap
   work here is verification of the *DOS half* against source rather than memory: for each verb,
   read the actual `external/MsDOS` `v4.0/src/CMD` implementation for observable behaviour the
   spec table doesn't state (switch semantics, error text shape, exit behaviour, prompt
   interaction) and record where TinyOS deliberately diverges (e.g. `DEL /S` still requiring
   confirmation policy). The Linux half compares against POSIX/coreutils *documented* behaviour.
2. **Terminal-window behaviour** — scrollback, wrapping, paging, VT/ANSI handling, resize, the
   buffer/renderer seam. `external/WindowsTerminal` is the reference for the one property
   `LE-53`(b) already promotes to a requirement: **a renderer that can be starved independently
   of the buffer**, because §6.6's drop-frames-not-block obligation depends on it. The gap
   analysis states, per behaviour, what MS-DOS did (single synchronous buffer), what Windows
   Terminal does (separated buffer/render with its own pacing), what a Linux pty/VT does, and
   what TinyOS commits to.
3. **Execution-level verification — scoped by `LE-48`.** 15 of the 22 MVP verbs are blocked on
   a filesystem that does not exist (`EPIC-P2` §1). For those, the gap analysis is *spec-level*
   (source-vs-spec), and each row says so in `status`. The handful of verbs that need no
   storage (`echo`, `env-get`/`env-set`, `task-list`, `clear-screen`, prompt behaviour) can be
   verified *live* the moment Deliverable A exists — the console is the harness: type the DOS
   form and the POSIX form at a QEMU fixture and record what happens. That is the first real
   execution-level DOS/Linux parity evidence the project will have, and it is why B follows A
   in sequence rather than running independently.

**Non-goal:** no binary compatibility claims, ever (`EPIC-P2` §2 makes this an exit criterion);
the analysis compares *behaviour*, and its header must restate the prohibition.

## 3. The unknowns, finished or pinned — each with its definition of done

| # | Unknown | What "finished" means | Blocked on |
|---|---|---|---|
| U1 | Upstream PR fate | `UPSTREAM-PR-authority-resolver.md` **submitted** to `tauri-apps/tauri` and the PR URL recorded in ADR 0007's evidence section. Accepted → the largest patch piece vanishes; rejected → known carried cost. Either outcome closes the unknown | **`LE-54`** — the owner pushing the fork to a remote; submission is trivial after |
| U2 | Trusted-path circularity (07A §7 Q1–Q2) | The independent review **commissioned** (07A is written to hand over verbatim) and its verdict recorded as an ADR | Owner choosing a reviewer |
| U3 | The engine (`EPIC-H3`) | A **pricing spike, not a build**: one document sizing port / restricted renderer / no-on-target-webview against the 8 MiB rule and the real-time floor → an ADR choosing a lane | U2's verdict feeds it; critical path U2 → U3 |
| U4 | Stage E composition | **Deliverable A of this mandate** | Nothing — executable now |
| U5 | Isolation/accounting/time (PD-01/07/08/12) | Cannot be shortcut. Finished-for-now = **named red tests**: graduate the stage-b boundary probes into `H2-02` rows in `containment-tests.tsv`/story contracts so "unproven" is a test id, not vague debt | The OS itself, by design |
| U6 | Advisory rot on the fork | ADR 0007 c5 **mechanised**: a CI job diffing RUSTSEC/GHSA against the pinned `tauri`/`wry`/`tao` versions; a hit files a loose-end row. Same shape as the LE-23 baseline job | Nothing — executable now |
| U7 | Invoke-key removal | Already pinned by design: record as an `H2-05` acceptance criterion so it cannot be forgotten or done early | A kernel-identity transport |

U1, U4, U6 and the U5/U7 pinnings can all proceed in parallel; none deepens the engine
dependency. U2 → U3 is the only ordering that matters for the architecture question.

## 4. Sequencing for the executing session(s)

1. **Owner action first, five minutes:** push `C:\Code\tinyos-tauri-fork` to a remote
   (a GitHub fork of `tauri-apps/tauri`, branch `tinyos-poc`), then the one-line `.gitmodules`
   swap + `git submodule sync external/tauri` — closes `LE-54`, unblocks U1.
2. **Deliverable A** (one session): the console. Advance the submodule pin.
3. **Deliverable B** (one session, after A): spec-level rows for all 22 verbs; live rows for the
   storage-free verbs through the console; the two registers committed.
4. **U1 + U6** any time after step 1; **U5/U7 pinning** any time at all.
5. **U2/U3** on the owner's clock — nothing above waits on them, and they wait on nothing above.

## 5. Definition of done for this mandate

- The console launches a fixture, streams its serial, and enforces its signed manifest through
  the resolver seam — demonstrated, reported (`REPORT-*` pattern), non-claims stated.
- `docs/terminal-gap-analysis.md` + `goals/context/terminal-gap.tsv` exist, cover the 22-verb
  surface and the window-behaviour axis, and every row is marked spec-level or live-verified.
- Every row of the §3 table is either **closed** (U1, U4, U6) or **pinned to a named artifact**
  (U2's ADR slot, U3's ADR slot, U5's test rows, U7's acceptance criterion).
- `LE-54` closed; the spine green throughout; the branch question (`os.tauru.poc` → `main`)
  put to the owner with the evidence in hand.
