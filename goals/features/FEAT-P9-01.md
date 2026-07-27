# FEAT-P9-01 — Plaintext Residency Reduction & the Dump-Scan Invariant

Status: **Specified — landable in the current phase**
Epic: [`EPIC-P9`](../epics/EPIC-P9.md)
Introduced in: [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md) (R1 and R2's cheap half)

## Description

The one part of `EPIC-P9` that needs **no cryptography, no entropy source, no TPM and no hardware** — and the reason it is first.

Two things. First, `07` R1: teardown already wipes staged frames and advances the generation before reuse (`PD-13`), and `PD-12` keeps addresses, error codes and register content out of spoor, so a dump taken after a task dies genuinely contains nothing of it. That is real anti-forensics **holding today by accident of implementation rather than as a stated invariant with a test**. This Feature makes it measured: one command that dumps guest memory and scans for PE headers, section names, import strings, `0x1_4000_0000` and `0xdead_0000`, and either finds them or does not.

Second, `07` R2's cheap half: the entire loaded image currently sits fully decoded in a static `.bss` staging arena, at a link-time-fixed address, for the whole process lifetime — a complete plaintext PE with headers intact, and the single most dump-friendly artifact in the system. It is *already dead* by the time the task runs, because `AddressSpace::create` has copied it into the task's own mapped frames. Wiping it costs one memset and closes the largest plaintext puddle in the image.

**What this Feature deliberately does not do** is R2's sliding-window half. Decoding into a small window instead of one large puddle needs decode-on-fault, which collides directly with `kernel::fault` having no `Resume` arm — a change that module's own doc comment says must arrive with its own Story, its own enumeration and its own test. The cheap half must not be allowed to carry the expensive half in on its justification.

## Crate(s) involved

`os/src/xtask/` (the dump-scan audit and its CI step), `os/src/os/` and `os/src/exec/` (the staging arena's lifetime)

## Depends on

Nothing. This is the only Feature in `EPIC-P9` with no prerequisite — no entropy source, no crypto primitives, no hardware root — which is what makes it workable alongside `EPIC-P1` without blocking or being blocked by it.

## Stories

| Story | Summary | Status |
|---|---|---|
| [`STORY-P9-01-01`](../stories/STORY-P9-01-01.md) | The dump-scan audit: R1's invariant becomes a measured, CI-gated command | Specified |
| [`STORY-P9-01-02`](../stories/STORY-P9-01-02.md) | Wipe the staging arena once the image is mapped and sealed | Specified |

## Containment contract

Canonical row: [`assurance/feature-contracts.tsv`](../assurance/feature-contracts.tsv) · implementation **C0** · subject **C0/C1** · boundary tests **BND-02, -17, -20**.

Residency reduction removes data, never authority: nothing here grants a capability, changes a containment class, or alters what any task may do. The audit is read-only by construction — it inspects a dump taken from outside the guest and cannot influence the run it measures (`PD-12` fault containment, `PD-13` revoke-wipe-advance, `RCG-13` blast radius, `RCG-14` durable non-persistence).

## Exit criteria

Both Stories **Verified** at Tier 0: the dump-scan audit runs in CI and reports zero hits for every fingerprint it scans for, on a dump taken after the workload has terminated; and the staging arena is provably zero after the image is mapped and sealed, with the audit as the check rather than an assertion in the code that wrote it.

**This Feature can exit at Tier 0 honestly**, unlike every other Feature in this Epic — because what it claims (bytes are absent from a dump) is exactly what Tier 0 can observe. It carries no hardware debt of its own. `LE-09` still bounds what the *Epic* can claim; it does not bound this Feature.
