# EPIC-P9 — Memory Confidentiality and Integrity Against a Dump

Status: **Specified — no Story started. Every Feature except `FEAT-P9-01` is gated on a hardware precondition this project does not yet meet (`LE-09`).**
Roadmap phase: cross-cutting. Deliberately **not** numbered into the `README.md` phase sequence — see [Why this Epic is out of the phase sequence](#why-this-epic-is-out-of-the-phase-sequence).
Introduced in: [`session/hand-2026-07-28/07-memory-confidentiality-proposal.md`](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md), reviewed and corrected in [`08-memory-confidentiality-review.md`](../../session/hand-2026-07-28/08-memory-confidentiality-review.md)
Depends on: [`EPIC-P1`](EPIC-P1.md) for `TeardownGeneration` and the W^X/sealing substrate (`FEAT-P1-03`, complete) — and on hardware that does not exist in this project's test estate.

## Goal

Make a process memory dump undecipherable and unforgeable, against an adversary who holds the dump, the binary, the source, the build and this documentation.

The design is one sentence: **the secrecy lives in a per-machine, per-boot root that never existed in the source tree**, every process key is derived from it on demand using public process metadata as salt (so nothing is stored or carried across a power cycle), and the thing stopping an attacker booting their own modified TinyOS is the TPM refusing to unseal for a boot that measures differently. That is a property hardware supplies. Obscurity never did, and this Epic is not permitted to rely on it (`07` §4d rule 8).

## The precondition, stated first

**None of `FEAT-P9-02` through `FEAT-P9-08` can reach `baseline-debt`, let alone `verified`, on the evidence base this project currently has.**

`07` §12 states it plainly: no hardware root, no defence. Every acceptance test for the central construction requires a TPM 2.0 and — for `FEAT-P9-07` — a memory-encryption engine. TinyOS's only test tier is Tier 0 (QEMU `q35`), `LE-09` is open, and no board has ever produced a measurement. Three consequences follow and none of them may be quietly dropped:

1. **`LE-09` is a hard gate on this Epic**, not a caveat on its Reports. A Story here whose acceptance criteria need a TPM cannot be Verified by running it under QEMU, and a swTPM emulator would verify the emulator, exactly as `STORY-P1-03-03`'s `D04` figure measured TCG's TLB model rather than a `mov cr3`.
2. **Timing claims in this Epic are unmeasurable today.** `07` §4b's derivation cost carries §4c's whole hibernate argument, and a Tier 0 figure for it would be actively misleading. It is recorded as a stated assumption with a named falsification (`07` §11.5), never as a number.
3. **`FEAT-P9-01` is exempt** — it needs no cryptography, no entropy, no TPM and no hardware. It is the only part of this Epic that can land inside the current phase, and it is scoped precisely so that it can.

## Why this Epic is out of the phase sequence

`EPIC-P1` is the determinism proof and confidentiality is not in its decomposition; forcing this work into `FEAT-P1-07` would corrupt that Epic's thesis and its exit criteria. Equally, this is not a self-contained phase in `README.md`'s roadmap sense — it is a cross-cutting property whose prerequisites (entropy, crypto primitives, a hardware root of trust) are infrastructure other phases will also want. `SEC-01` (hardware-rooted verified boot) and `SEC-15` (secret and credential isolation) have been in the Security Charter since the beginning with nothing behind them; this Epic is what would put something behind them.

It is therefore a **capability Epic** that runs alongside the phase sequence rather than inside it, and its one phase-landable Feature says so explicitly.

**On the number.** `P9` is an identifier, not a position. The assurance spine requires every Feature and Story id to carry a `Pn` phase token (`xtask`'s `is_phase_id`), so a capability Epic still needs one; `P0` through `P8` are reserved by [`backlog.md`](backlog.md) for the roadmap phases — `P2` in particular is Shell & UX — and `P9` is the first free number. **Nothing about this Epic runs after Phase 8.** `FEAT-P9-01` is workable today, alongside `EPIC-P1`; the rest waits on hardware, not on the phases in between.

## Goals verified (from `SeedMVP.md` §3)

- **G-SEC-2** — process memory private by construction, extended from "private from other processes" (which `FEAT-P1-03` delivered) to "private from an offline analyst holding the RAM".
- **G-SEC-13 through G-SEC-15** — containment-class integrity, unified policy/spoor provenance, and **secret isolation**, which is the one this Epic is really about: `SEC-15` currently has no implementation anywhere in the tree.
- **G-SEC-8** *(partial)* — an image that cannot be read or forged off-box is a materially harder target for durable ambient reach.

## Threat model

Inherited unchanged from [`07` §2](../../session/hand-2026-07-28/07-memory-confidentiality-proposal.md). A1 (another TinyOS process) is **already handled** by `FEAT-P1-03` and is not this Epic's subject. A2 (blind runtime corruption) is degraded only by `FEAT-P9-08`. **A3 (offline analyst with a dump) is the primary target.** A4 (DMA) is real and currently unopposed — TinyOS has no IOMMU — but is `SEC-18`'s own Feature and does not gate this work. A5 (hypervisor / co-tenant / physical) is accepted as un-defendable in software, which is exactly why `FEAT-P9-07` exists. **A6 (an attacker who builds their own TinyOS) is created by this Epic's own publication rule** and is answered only by `FEAT-P9-04`.

## Features and the dependency chain

The chain is the point of this decomposition. Read top to bottom, nothing below can start before everything it names above it exists.

```
FEAT-P9-01  Plaintext residency + dump-scan invariant   ── no dependencies, no hardware ──► CAN LAND NOW
                                                                                            (R1, R2's cheap half)
FEAT-P9-02  Entropy source ──────────────┐
                                         ├──► FEAT-P9-05  Key lifecycle (derive-use-wipe)
FEAT-P9-03  Bounded crypto primitives ───┤         │
                                         │         ├──► FEAT-P9-06  At-rest AEAD, (address, generation)-bound
FEAT-P9-04  Hardware root + measured ────┘         │
            boot  [HARDWARE]                       └──► FEAT-P9-07  HW memory encryption + attestation  [HARDWARE]

FEAT-P9-08  Layout randomization ── needs FEAT-P9-02 only; last on merit, not on dependency
```

| Feature | Summary | Depends on | Status |
|---|---|---|---|
| [`FEAT-P9-01`](../features/FEAT-P9-01.md) | Plaintext residency reduction & the dump-scan invariant (R1, R2's cheap half) | nothing | **Specified — landable in the current phase** |
| [`FEAT-P9-02`](../features/FEAT-P9-02.md) | Kernel entropy source with health tests & a Tier 0 determinism carve-out | nothing | Specified |
| [`FEAT-P9-03`](../features/FEAT-P9-03.md) | Bounded `no_std` AEAD & KDF primitives | `FEAT-P9-02` | Specified |
| [`FEAT-P9-04`](../features/FEAT-P9-04.md) | Hardware root of trust: TPM 2.0, measured boot, sealing policy | **hardware** | Specified — gated on `LE-09` |
| [`FEAT-P9-05`](../features/FEAT-P9-05.md) | Key lifecycle: derive-use-wipe, per-surface subkeys, generation-keyed rotation | `-02`, `-03`, `-04` | Specified — gated on `LE-09` |
| [`FEAT-P9-06`](../features/FEAT-P9-06.md) | At-rest AEAD bound to (address, generation) (R3) | `-05` | Specified — gated on `LE-09` |
| [`FEAT-P9-07`](../features/FEAT-P9-07.md) | Hardware memory encryption, attestation & the fallback tier (R4) | `-04`, `-05` | Specified — gated on `LE-09` |
| [`FEAT-P9-08`](../features/FEAT-P9-08.md) | Layout randomization (R5) | `-02` | Specified |

**Ordering rationale.** `FEAT-P9-01` first because it is free, because it is the only part that can be worked now, and because its dump-scan audit is the *instrument* every later Feature's evidence is read with — `07` §11.1's falsification of the whole randomization argument is a measurement on that instrument. `FEAT-P9-02` next because it is a shared prerequisite for three separate things (`-03`'s nonces, `-05`'s salt diversity, `-08`), which is why `07` §7.5 promotes it to its own Story rather than charging it to randomization. `FEAT-P9-04` before `-05` because a key lifecycle whose root is not hardware-sealed is `07` §4's trap 2 — the key co-resident in the thing being dumped — and buys A3 approximately nothing. `FEAT-P9-07` is recommended **first on value and built late on dependency**, which is not a contradiction: it is low-code and high-ops, and it is useless against a power cycle without `-05`'s sealed root behind it (`07` §4c). `FEAT-P9-08` is last on merit — its entropy is defence in depth and never a substitute for `root_secret`.

## Exit criteria for this Epic

- All eight Features **Verified**, each with valid contract rows, Tests and dated Reports.
- **`SEC-01` and `SEC-15` hold evidence-backed state on at least one Story each.** Both have been declared in the Security Charter since the beginning with nothing behind them, and this Epic is the first thing that could change that.
- The dump-scan audit runs in CI and reports **zero hits** for PE headers, section names, import strings, `0x1_4000_0000` and `0xdead_0000` in a post-teardown dump — the measured form of R1's invariant.
- **`07` §8.1's downgrade attack is resolved, not deferred.** A fallback tier an attacker can force is a strictly easier path than forging PCR measurements, and until the resolution is designed and tested this Epic's central claim has a hole in it.
- **At least one Story is Verified on real hardware.** This Epic cannot honestly exit at Tier 0, and saying so here is what stops that happening by momentum.
- Every mechanism is marked as surviving publication or not (`07` §4d), and any that does not is either re-derived or discarded.

## Explicitly out of scope

- **IOMMU / DMA isolation (A4, `SEC-18`).** In scope as a threat, its own Feature as work. `07` §7.1: do not gate this on it.
- **A general decode-on-access bridge for all process memory.** `07` §5: it buys least against A3, costs most against the RT thesis, and forces a `kernel::fault` `Resume` arm that module's own doc comment says must arrive with its own Story. R2's sliding-window half pays that price explicitly and separately; R2's cheap half (`STORY-P9-01-02`) does not get to ride in on it.
- **Licensing / IP enforcement.** Publication genuinely breaks client-side obfuscation as a licensing mechanism. That needs server-side entitlement and is a different design (`07` §12).
- **Copying Sharc.Blue's Oracle Mono pipeline.** `07` §6: ten layers of key-derived reversible transforms over AES-GCM, never wired into a production caller, using a fixed nonce per key. Copy the KeyRing and the per-surface separation; leave the rest.
