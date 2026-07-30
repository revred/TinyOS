# Handover 02A — Cover Note: Request for a Deep Review of TinyOS at the `main` Merge

Same session as [`01A`](01A-multitab-ux-visually-confirmed.md), ordered by the owner:
*merge with `main`, then deliver a cover note requesting a deep review* — capabilities,
things yet to be done, known issues, and the areas where real expertise and extreme
intelligence are needed. The owner also named the unwritten goal every OS carries:
**zero chance of zero-day exploits**. That is not a claim anyone can make honestly; this
note states it as the *asymptote* the architecture is aimed at, and asks reviewers to
attack the distance between the two.

This note follows the `08C`/`13F` cover-note pattern: it is an instruction set for
reviewers (human or agent), not a summary that replaces the documents it points to.

## 0. How to review this repository

1. Read [`agent.md`](../../agent.md) → [`SECURITY_CHARTER.md`](../../SECURITY_CHARTER.md)
   → [`docs/whole-system-context.md`](../../docs/whole-system-context.md) in that order.
2. The machine-readable state is authoritative over prose:
   `cargo run -p xtask -- check-assurance-spine` (from `os/`) must pass; open defects are
   `LE-*` rows in [`goals/assurance/loose-ends.tsv`](../../goals/assurance/loose-ends.tsv)
   (**31 open at this merge**), not adjectives in reports.
3. Verify, don't trust: every claim below names its gate. Run the gates.
4. File findings as new `LE-*` rows (the grammar is in
   [`goals/assurance/README.md`](../../goals/assurance/README.md)) or as a dated review
   document in this folder. Do not soften a status to make a finding fit.

## 1. Capabilities at this merge (each with its gate)

- **Tier 0 x86_64 kernel under QEMU, 30 fixtures, two-signal discipline** (in-guest
  `isa-debug-exit` AND serial-transcript checks): memory pools that fail closed, address
  spaces + W^X sealing, context switching, preemptive priority scheduling with WCET
  budgets, degrade/restart, priority inheritance (see the soak log's regression sweep
  list). Gate: `cargo run -p xtask -- qemu-x86_64 --fixture=<name>`; catalogue via
  `list-fixtures`.
- **Executable admission**: PE64 → TXE container packing, probe generation, the
  code-admission-gate catalogue (14 gates), "remote bytes are data, never code" (`RCG-*`).
  Gate: `goals/security/code-admission-gates.tsv` + exec fixtures (`wx-seal`,
  `pe-measure`, `win32-shim`, `blue-sharc`).
- **TINYCMD** (`os/src/shell`, `#![forbid(unsafe_code)]`, `no_std`, heap-free): 22 typed
  verbs behind one deny-by-default `VerbPolicy` seam, audited denials carrying session
  identity, `G-SEC-5`-labelled RAM volume (quarantine survives transform chains),
  DOS front-end total over adversarial input, `.TCB` batch runner with 4.0 echo
  discipline, MS-DOS parity gate green (`check-shell-parity`: fixture verdict AND golden
  byte-comparison). 22/22 host tests.
- **The host-side multi-tab operator console** (17G, `external/tauri/tinyos-poc/`): one
  window, sibling webviews, host-owned reserved region holding zero verbs, ed25519-signed
  per-identity grant tables, per-tab in-process TINYCMD sessions, the parity suite
  runnable from a tab. Evidence: [`REPORT-2026-07-30-01`](../../goals/reports/REPORT-2026-07-30-01.md)
  + committed screenshots. **Host-side interaction model only** — `LE-53` stands.
- **A vendored Tauri fork with an `AuthorityResolver` seam** (+224/−19 lines over the
  tag, knob-gated, upstream suite green both positions) and a weekly OSV advisory sweep
  (`.github/workflows/fork-advisories.yml`).
- **Assurance spine**: 27 Features / 67 Stories / 52 Tests / 55 Reports, contracts
  machine-checked, dashboards gated against the register (`LE-30` closed).

## 2. Not yet done (the honest queue)

- **Flavour equivalence**: POSIX (`FEAT-P2-05`) and RT (`FEAT-P2-06`) front-ends; batch
  control flow (`IF`/`GOTO`/`FOR`/`CALL`); pipes/redirection in the verb core.
- **Serial RX** (`LE-55`): the target's UART is TX-only; no interactive path to the real
  kernel exists yet. The tab host's `send_line` says so rather than pretending.
- **The on-target tab host and §6.3's kernel-enforced trusted path** — `EPIC-H3` remains
  the largest unpriced dependency; ADR slots 0010/0011 wait on the named reviews.
- **Performance evidence**: the 625-test catalogue is mostly unmeasured; every `TG-P*`
  row is red; `D23`/`D14` shell numbers are open debt. UX speed is to be *tested, never
  asserted* (owner directive).
- **Platform qualification**: 5 platforms, 0 qualified; ARM64 RT tier is conditional on
  secure-world qualification (ADR 0005). No real-hardware bring-up yet — everything is
  QEMU Tier 0.
- **Isolation/accounting/time on target** (`PD-01/07/08/12`, `H2-02-R1..R4`): red rows.
- Upstream PR (U1) submission; the invoke-key removal (`H2-05-AC1`) once a
  kernel-identity transport exists.

## 3. Known issues a reviewer should press on

- **The soak found a recurring intermittent**: `priority-inversion` failed its sweep
  twice (elapsed 35.85h and 71.88h), second time with a diagnostic —
  `released=false, ok=false` while every other field matches a passing run — plus one
  `context-switch` exit-2 harness error. Neither reproduces on demand. This is exactly
  the class of bug that deserves extreme intelligence: a rare scheduling/timing window
  in the priority-inheritance release path, or a QEMU/host artifact. The captures are
  retained (`LE-46` machinery); the soak log names every occurrence.
- **31 open loose ends** — notably `LE-09` (no qualified platform ⇒ no quotable WCET),
  `LE-53` (Tauri disqualified on-target), `LE-55` (no serial RX).
- Kernel/exec/shell **fixture binaries don't type-check on Windows hosts** (`not(windows)`
  HAL gates) — CI's ubuntu covers them; a Windows-only contributor can be surprised.
- The parity-suite tab **parses `cargo`'s human-readable output**; a toolchain format
  change surfaces as a red row (safe direction, but a fragility to know about).
- The console's manifest signing key is a **committed dev key** — deliberately not a
  custody model; any real deployment needs signing infrastructure (see §4).

## 4. Where expertise and extreme intelligence are needed — the zero-day asymptote

The architecture's bet is that zero-days are made *unrewarding* by construction:
memory-safe Rust with `unsafe` confined to reviewed HAL boundaries, deny-by-default
authority everywhere (no privileged caller, including the local shell and any LLM agent),
**remote bytes are data, never code** with C4 inspection destroyed before admission,
labels that survive transform chains, W^X sealed, fail-safe over keep-trying. A reviewer
who wants to matter should attack precisely these seams:

1. **The `unsafe` perimeter**: enumerate every `unsafe` block in `os/src/` (HAL, context
   switch, page tables, PE mapping) and try to prove — or better, formally verify — that
   safety invariants hold across interrupt and preemption windows. The
   priority-inversion intermittent (§3) may live here. Tools: Kani/Prusti/Verus-class
   proof, or a hostile review that produces a reproducer.
2. **The code-admission chain**: the 14 gates claim no path from external bytes to
   executable memory. Attack it end to end — TXE packing, W^X seal timing (TOCTOU
   between inspect and map), the win32-shim, DMA once real devices exist (IOMMU policy
   is unwritten). A single counterexample here is *the* finding.
3. **Parser totality under fuzzing**: the DOS front-end, `.TCB` runner, TXE/PE parsers
   and the serial protocols are tested total, but have never met a coverage-guided
   fuzzer. Wire `cargo-fuzz`/AFL at the `no_std` seams; the heap-free fixed-capacity
   design should make crashes impossible — demonstrate it, don't assume it.
4. **Timing and side channels** (`PD-12`): nothing yet bounds what one domain learns
   from another through time, cache, or the shared scheduler. This needs someone who has
   actually built constant-time/partitioned systems.
5. **Supply chain**: the vendored fork (+224/−19, reviewable by hand), the crates.io
   graph of the host tooling, and the toolchain itself. The OSV sweep covers advisories;
   it does not cover a hostile maintainer. Reproducible builds and a vendored-audit
   policy are unwritten.
6. **Key custody and attestation**: replace the dev signing key with a real model
   (measured boot → per-device keys → manifest signing), and design the update path so
   a compromised signer still cannot cross the admission gates.
7. **The concurrency of the host console**: multiple webviews, a reconciler thread, and
   process-spawning verbs — a classic confused-deputy hunting ground. The grant tables
   are signed and disjoint; try to make one identity spend another's authority.

The instruction to reviewers is Charter-shaped: do not review for style; review to
**break a stated guarantee**, and file the break as an `LE-*` with a reproducer. Where a
guarantee cannot be broken but also cannot be *proven*, file that gap — an unprovable
guarantee is standing debt, and pretending otherwise is how zero-days are born.

## 5. Merge record

`os.tauru.poc` merged to `main` at this note's commit (fast-forward; `main` was at 11D,
13+ commits behind). Spine green at the merge; all suites as recorded in
[`REPORT-2026-07-30-01`](../../goals/reports/REPORT-2026-07-30-01.md).
