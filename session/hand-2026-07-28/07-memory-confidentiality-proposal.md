# Handover 07 — Proposal: Process Memory Confidentiality and Integrity Against a Memory Dump

Status: **Proposal — revised 2026-07-28 against [`08`](08-memory-confidentiality-review.md) §15.** Decomposed into [`EPIC-P9`](../../goals/epics/EPIC-P9.md).
Follows: [`06-next-session-mandate.md`](06-next-session-mandate.md). Same shape as [`02-first-real-task-integration-proposal.md`](02-first-real-task-integration-proposal.md) and [`03-story-p1-03-02-hardening-review.md`](03-story-p1-03-02-hardening-review.md) — a scoping document written to be argued with, not a plan to be approved.

> **Revision note.** The first draft of this document specified an AEAD construction with care and never said where the key lives. [`08`](08-memory-confidentiality-review.md) found that hole, and it is the largest of the three it found: against an offline analyst holding a memory dump, a construction whose key sits in `.bss` next to the ciphertext buys approximately nothing. This revision applies all eight changes `08` §15 requested — §4b (key lifecycle), §4c (power states), §4d (the publication test), the reordered recommendations, declared metrics in §11, the dump-scan audit as R1's acceptance test, R5's restated justification, and the local-only fallback tier — and adds one objection §11.4 that neither document had: **the fallback tier is a downgrade attack on the forged-kernel defence.**

## 1. The question as posed

> *Make it almost impossible to guess the process memory layout and consumption patterns — a driver or bridge that takes the process memory and decompresses/decodes it on the fly, the way Sharc.Blue does for `.rac` out-of-core and in-core — so each session of each process is not predictable by a third party.*

Narrowed by the requester, and this narrowing is what makes the question answerable:

> *Without complicating it beyond its usefulness, we just have to defeat spoofing and memory tracking and deciphering on the process memory dump.*

This proposal takes the narrowed goal as the requirement and the original text as the suggested mechanism, and argues that **the goal is right and the mechanism is aimed at a different adversary than the one named**.

## 2. Threat model — stated first, because nothing below means anything without it

| # | Adversary | Capability | In scope? |
|---|---|---|---|
| A1 | Another TinyOS process | Executes its own code; may contain a memory-safety defect | **Already handled** — page tables, W^X, capability mediation. Deterministic enforcement, `FEAT-P1-03`. |
| A2 | Remote attacker exploiting a C4 parser bug | Blind or semi-blind memory corruption **at runtime**; must predict addresses | **In scope** — this is the only adversary layout randomization actually degrades. |
| A3 | Offline analyst holding a **memory dump** | Reads the whole image at leisure; can scan, correlate, fingerprint | **Primary target of this proposal.** |
| A4 | DMA-capable malicious device | Reads/writes physical memory, bypassing page tables entirely | **In scope.** TinyOS enumerates PCI but has **no IOMMU support** — verified, zero `iommu`/`VT-d`/`DMAR`/`SMMU` references in the source tree. This adversary is currently unopposed. |
| A5 | Hypervisor, co-tenant, or physical attacker (cold boot, JTAG) | Reads physical memory outside the OS's control entirely | **In scope by deployment target** — the data-centre case makes this real, and no software the OS runs can defend against it. |
| A6 | **Attacker who builds their own TinyOS** | Compiles a variant with encryption stubbed out and a dumper added, and boots it on the target | **In scope, and created by §4d.** Only measured boot answers it. |

A1 is the case `FEAT-P1-03` closed this week and is *not* what this proposal is about. Conflating A1 with A3–A6 is the most common way this kind of work goes wrong: the mechanism that solves one does nothing for the others.

A6 did not appear in the first draft because the first draft did not apply §4d's test. It is not a new threat — it was always available — but a design that assumes source secrecy never has to confront it, and this one does not get to make that assumption.

## 3. The correction that drives everything: randomization does not defeat a dump

Layout unpredictability raises cost for **A2 only**, because A2 must commit to an address *before* observing memory. A3 does the opposite: it observes first and acts never. Against a dump, an analyst does not guess — they scan. PE headers, section tables, import name strings, page-table structure, entropy profiles and known constants all survive relocation, and the second pass finds whatever the first pass moved.

So randomization is worth doing eventually and is worth almost nothing for the stated goal. One further cost makes it a poor *first* move here:

- **There is no entropy source in the kernel.** Verified: zero `RDRAND`/`RDSEED`/RNG/jitter-collector references outside `xtask`. Randomization needs that built and trusted first — and per §11.5 that source is a shared prerequisite for R3 and §4b as well, so it is its own Story unblocking three things rather than a cost chargeable to R5.

> **Withdrawn (`08` §13).** The first draft also argued randomization "fights reproducibility, which this project spends", and deferred R5 partly on that basis. That argument is void and is withdrawn. Reproducibility under encryption is a solved problem with a shipped precedent — Sharc.Blue's `envelope-debug`/`envelope-production` split keeps plaintext plus an HMAC tag so tests can still assert, with a deterministic `new_for_test(tag)` ring. §4d shows the split is *safe* under publication, too, because a debug build measures differently and therefore never receives the key. R5 stays deferred on its real merits — the missing entropy source and the priority of `root_secret` — and not on this one.

For calibration on how predictable things are today: the image base `0x1_4000_0000` is hardcoded in **15 places**, and `CAPABILITY_TRAP_VIRT` is the fixed constant `0xdead_0000`. A dump is currently fingerprintable as TinyOS in one pass. That is an argument for eventually randomizing — it is not an argument for randomizing *instead of* the measures below.

## 4. Spoofing and deciphering are two properties and need two primitives

**Deciphering** is confidentiality: the bytes in RAM must not be the plaintext. **Spoofing** is integrity: the bytes in RAM must not be forgeable.

Encryption alone does not give the second, and this is the trap worth spending a paragraph on. An attacker who can write memory does not need the key: they can **replay** a previously-valid ciphertext block, or **relocate** one to a different address, and a naive decryptor accepts both. This is not theoretical — it is why AMD had to add SEV-SNP's Reverse Map Table on top of SEV/SEV-ES, whose encryption was sound and whose integrity was not.

The fix is an AEAD whose nonce/AAD binds each block to **its address and its epoch**, so a block is valid only where and when it was written. TinyOS already has the epoch: `TeardownGeneration` landed this week for `PD-13`, and reusing it keeps the invariant in one place rather than inventing a second counter that can drift from the first.

**There are two traps here, and the first draft stated only one.**

| # | Trap | Consequence |
|---|---|---|
| 1 | Encryption without integrity → replay / relocation | Argued above, via SEV → SEV-SNP |
| 2 | Encryption with the key **co-resident in the thing being dumped** | The dump contains the key next to the ciphertext. R3 buys A3 nothing. |

§4b closes trap 2, and it is the larger of the two.

### 4b. Key lifecycle — provenance, residency, wipe, rotation

The construction is a two-input derivation. Everything about a process that is *already public* goes in as salt; exactly one input is secret, and it comes from hardware.

```
root_secret   ← unsealed from the TPM against boot-chain PCR measurements
                never in the source tree, never in the binary, never on disk in clear,
                different on every machine

process_key   = KDF(root_secret,
                    image_base ‖ image_size ‖ layout_hash ‖ process_id ‖ teardown_generation)

block_tweak   = (page_address, teardown_generation)   → AEAD nonce / AAD per §4
```

**The metadata is the right input, for binding rather than for secrecy.** Base address, image size, load layout, process number and generation bind ciphertext to *this process, at this address, in this generation*. Relocate a block and it fails; replay an old one and it fails. That is §4's anti-replay property, and it is why AMD had to add the Reverse Map Table on top of encryption that was already sound.

**The metadata cannot be the secret, and must never be credited as such.** Every one of those values is present in the dump the adversary is holding. An analyst computes the same derivation at the same speed. The entropy is not there either:

| Input | Entropy today | Entropy if fully randomized |
|---|---|---|
| Image base | **0 bits** — hardcoded `0x1_4000_0000` in 15 sites (§3) | ~25 bits |
| Image size | ~0 bits — fixed by the build | ~0 bits |
| Process id | ~0 bits — sequential | ~16 bits |
| Layout | ~0 bits — deterministic | a few bits |

Zero today; brute-forceable even at the theoretical best. `root_secret` carries all of the secrecy and the salt carries none of it. A derivation over public inputs alone is a reversible transform whose parameters ship alongside the ciphertext — it is obfuscation, and §4d is the test that says so.

**Derive-use-wipe beats hold-resident.** Re-derive at the point of use and wipe immediately, rather than keeping a key ring alive in `.bss` for the lifetime of the process. This is R2's own argument — shrink the puddle — applied to the key itself, and it is the direct analogue of Sharc.Blue dropping its `Chapter` handle after a single read. Per-surface subkeys (image-at-rest, spoor, future crash dump, IPC) earn their keep twice: a compromise does not cross surfaces, and R1's invariant becomes **auditable per surface** rather than as one global claim. Rotation keys to `TeardownGeneration`, for the same "one epoch counter, not two" reason as §4.

**Two honest limits.**

- **Derive-use-wipe shrinks residency; it does not eliminate it.** During the operation the derived key is in registers and possibly on the stack, and a dump captures both. This is why §11's plaintext-byte-seconds axis must be applied to key material as well as to data, and why R4 remains necessary — hardware memory encryption is the only layer protecting the key *at the moment of use*.
- **The derivation cost is unmeasured.** `08` §5 estimates "hundreds of nanoseconds" for an HMAC-SHA256 or AES-NI derivation, and §4c's hibernate argument leans on that being true. **It is an estimate, not a measurement, and it cannot be measured at Tier 0**: under TCG a figure like this is meaningless in the same way the `D04` cross-space number was ~27× off (`REPORT-2026-07-28-02`). It is carried here as a stated assumption with a named falsification in §11.5, not as a number.

### 4c. Power states — why hardware memory encryption alone is not sufficient

Hardware memory-encryption keys are ephemeral. SME/TDX generate a key at power-on and lose it at power-off. A hibernation image is written out from the OS's point of view as plaintext, and on resume the CPU holds a new key that decrypts nothing that came before.

So "the key lives in the CPU" is a true statement about the live system and an incomplete one about the system's lifetime. Any design that stops there fails at the first suspend-to-disk.

The derivation in §4b closes it, and TPM sealing is the mechanism built for exactly this case:

| Event | What happens |
|---|---|
| Boot | Boot chain extends PCRs; TPM unseals `root_secret` only if measurements match |
| Run | Per-process keys derived on demand from `root_secret` + metadata, used, wiped |
| Hibernate | Nothing key-shaped needs to survive. The image carries its own metadata |
| Resume | Same machine, same measurements → TPM unseals → every process key re-derives |
| Hibernation file stolen and resumed elsewhere | Different machine → no unseal → the file is inert |

**R4 is therefore necessary and not sufficient**, and must be stated that way wherever it is recommended.

Two dependencies this table quietly assumes, named rather than left implicit:

- **TinyOS has no suspend/resume, no storage and no filesystem.** Verified. This section is correct in principle and is reasoning about a subsystem that does not exist and is not scheduled. It is a constraint on the eventual design, not a requirement on current work.
- **"Boot chain extends PCRs" assumes a boot chain that measures.** TinyOS is PVH direct-kernel-loaded; under QEMU nothing measures anything. On real hardware that role belongs to firmware/shim/bootloader, and it is a deployment dependency of this design, not a property of it.

### 4d. The design must hold with the source published

This is the cheapest available test for telling a real mechanism from a decorative one, and the first draft did not apply it.

Kerckhoffs's principle: a system must remain secure when everything about it is public except the key. AES, TLS, LUKS, FileVault and the SEV specification are all fully published and their security is undiminished. §4b satisfies this by inspection:

| Component | Public? | Carries secrecy? |
|---|---|---|
| Cipher, KDF, AEAD construction | Publish it | No |
| The derivation formula | Publish it | No |
| Base, size, layout, pid, generation | Public **and already in the dump** | No — binding only |
| The binary, byte for byte | Publish it | No |
| `root_secret` | **Never** — per-machine, per-boot, TPM-sealed | **All of it** |

One row is secret, and it is not code. It cannot be committed, cannot be built into an artifact, and differs on every machine.

**The objection that matters: the forged kernel (A6).** If the source is public, an attacker does not attack the cipher. They build their own TinyOS — identical but with encryption stubbed out and a dumper added — and boot it on the target. The defence is not secrecy; it is the sealing policy. `root_secret` is sealed against PCRs measuring the boot chain, so one byte different in the kernel image produces a different measurement and the TPM refuses to release the secret. The forged kernel boots perfectly and finds nothing but ciphertext it can never key. That property is supplied by hardware and is entirely unaffected by publication — the attacker already knew the design.

This also closes a hole that would otherwise be created by the debug/production split §3 withdraws its objection to: published, anyone could build the debug variant, but a debug build measures differently and so never receives the key. **The reproducibility carve-out and the security boundary end up enforced by the same mechanism** — a better outcome than defending them separately.

**What publication genuinely costs**, named rather than waved away:

- **Fixed constants become free.** `0x1_4000_0000` across 15 sites and `0xdead_0000` fingerprint a dump as TinyOS in one pass. Publication does not create this; it removes the last friction. R1's audit (§5) is what measures it.
- **"They would have to reverse-engineer the format" stops being chargeable.** Sharc.Blue explicitly banked on this. Under publication that credit is exactly zero, which is why §5 refuses to count compression as confidentiality.

The frequently claimed *benefit* — more eyes find more bugs — is real but conditional on review actually happening. Heartbleed sat in public OpenSSL for two years. Do not budget for it.

**Standing rules this implies.** Each is individually checkable in review:

1. **No secret ever enters the repository or a build artifact.** Not a key, not a seed, not a default, not a test fixture promoted to production. If any build can produce a working decryptor without talking to hardware, the design is broken.
2. **Every secret is per-machine and per-boot**, obtained from a hardware root at runtime.
3. **Kernel integrity is enforced by measurement, not by source secrecy.** This defeats A6 and is what makes rule 1 safe.
4. **Public process metadata is salt, never secret.** It binds; it does not conceal.
5. **Derive at point of use and wipe; do not hold resident.** Key material is under the same exposure budget as plaintext (§11).
6. **No fixed sentinels, magic numbers, or hardcoded addresses** in anything reaching memory or disk.
7. **Deployment configuration is separate from mechanism.** PCR policy, key hierarchy and attestation endpoints are per-site, and none are cryptographic secrets either.
8. **Assume the adversary holds the binary, the source, the build and the documentation.** Any conclusion that changes under that assumption was resting on obscurity and must be re-derived or discarded.

## 5. The constraint that decides the design: WCET

Decode-on-access makes memory latency **data-dependent and fault-driven**. For a kernel whose entire thesis is determinism — declared WCET budgets, a CI timing gate, committed `D04`/`D05` baselines — that is close to disqualifying as a blanket policy.

It also has a specific, documented collision: `kernel::fault` deliberately has **no `Resume` arm**, and says so in its own module doc ("no demand paging, no copy-on-write, no guard-page stack growth… the day a genuine recoverable case exists, it arrives with its own Story, its own enumeration and its own test"). Decode-on-fault *is* that case. It is admissible, but it must be paid for explicitly and not smuggled in.

**Conclusion: encode at rest; never on the live RT working set.**

Sharc.Blue is direct precedent rather than a first-principles argument. Its ten-layer pipeline is applied to `cold.start` and `journal` — cold surfaces. The hot surface (`stdio`) gets only AES-GCM plus padding, explicitly documented *"hot path — zero alloc."* That system had **no WCET budget at all** and still kept the expensive encoding off its hot path.

**On compression.** If the encoding is compression, memory consumption becomes content-dependent, and an observer watching consumption learns about the plaintext — the CRIME/BREACH shape applied to memory rather than TLS. Since "memory tracking" was named as part of the threat, this matters directly: **compress-then-encrypt, and pad to fixed-size buckets.** Sharc.Blue ships exactly that shape — `[seq:8][nonce:12][compressed+AEAD:N][pad_to_512]`, uniform blobs plus ghost traffic — so this is a solved problem with a reference implementation. **Credit compression with zero confidentiality** (rule 8): `EnvelopeSecurity_Plan.md` claims its RJL rotation and B2 column schema are *"implicit encryption layers"*; in this threat model that is worth nothing.

## 6. Recommendation, in priority order

Reordered per `08` §3. The first draft ranked R4 fourth while its own body called it the best item in the list — the first thing a reviewer would attack, and it undersold the strongest recommendation in the document.

**R4 — Hardware memory encryption for the live set. (Cost: low in code, real in ops. Buys: A3, A4, A5 — the only item that does.) Necessary, not sufficient.**
AMD SME/SEV-SNP, Intel TME/MKTME/TDX, ARM CCA. The key never leaves the memory controller, so a live dump is ciphertext at approximately zero per-access latency — the one thing no software design achieves. It is the same doctrine as §4b's derivation moved into silicon: the key lives outside the artifact under analysis. Mostly enablement, configuration and attestation rather than kernel code. **Pair it with §4b's sealed root** — per §4c, R4 alone fails at the first hibernate — and with the fallback tier in §11.4.

**R2 — Shrink the staging arena. (Cost: moderate. Buys: A3, A4, A5.)**
The concrete finding from code written this week. The entire loaded image sits fully decoded in a static `.bss` arena, at a link-time-fixed address, for the whole process lifetime — a complete plaintext PE with headers intact, and the single most dump-friendly artifact in the system. Decoding into a small sliding window instead of one large puddle is where `.rac`-style on-the-fly decode genuinely earns its cost, and it shortens the exposure window for every adversary at once. **This is the one place the requester's original mechanism is exactly right.**

R2 has more backing than the first draft claimed. It is Sharc.Blue's frugality doctrine applied to a different substrate — structural reduction → graph skeleton → lazy materialisation, ~0.01% resident at query time — and that is the mechanism that produced their headline numbers, not a speculative idea.

**Note the split.** R2's *cheap half* — wipe the arena once the image is mapped and sealed, so the plaintext PE does not outlive its own use — needs nothing that does not already exist and is [`STORY-P9-01-02`](../../goals/stories/STORY-P9-01-02.md). R2's *sliding-window* half needs decode-on-fault and therefore pays §5's price in full. Do not let the second ride in on the first's justification.

**R1 — Keep and harden what already exists. (Cost: none. Buys: A3.)**
Teardown already wipes staged frames and advances the generation before reuse, and `PD-12` keeps addresses, error codes and register content out of spoor. The consequence is real anti-forensics: a dump taken after a task dies contains nothing of it. The recommendation is to treat "no plaintext survives teardown" as a stated invariant with its own audit, rather than a property holding by accident of implementation.

**Acceptance test, per `08` §10.** Sharc.Blue's own verification steps transfer directly and give R1 something runnable:

> *"Binary inspection: `grep -ao "pattern" target/release/blue-sharc.exe` finds no recognizable envelope/key/crypto strings."*
> *"File inspection: `xxd .sharc/cold.start | head` shows no magic bytes, no JSON, no recognizable structure."*

For TinyOS: dump the image and scan for PE headers, section names, import strings, `0x1_4000_0000` and `0xdead_0000`. One command, and it either finds them or it does not. That is [`STORY-P9-01-01`](../../goals/stories/STORY-P9-01-01.md), it needs no cryptography, and it converts R1 from an assertion into a measured invariant.

**R3 — Keyed AEAD for anything genuinely at rest. (Cost: moderate. Buys: A3.)**
Image bytes before and after use, plus any future crash-dump or journal export (none exists today — verified). Nonce/AAD binds (address, generation) per §4; the key comes from §4b and never from `.bss`. Deliberately excludes the live working set.

**R5 — Layout randomization. (Cost: high relative to benefit here. Buys: A2, plus salt diversity, plus constant removal.)**
Last, and only after an entropy source exists. Its justification is now threefold rather than single (`08` §12): under §4b a randomized image base and process id are no longer only address obfuscation — **they are entropy inputs to the key derivation**, and today they contribute zero bits. It also removes the fingerprint constants §4d flags. This does *not* promote R5: `root_secret` carries the secrecy and must continue to do so alone, and salt entropy is defence in depth, never a substitute. But it is a materially better case than the first draft recorded, and the reproducibility objection against it is withdrawn (§3).

**Not recommended as a first move:** a general decode-on-access bridge for all process memory. It buys least against A3 (the working set is plaintext at the moment of the dump either way), costs most against the RT thesis, and forces the fault-policy change in §5.

**Do not copy Oracle Mono.** The original request named `oracle.mono` specifically, so this is worth being blunt about. Its 10-layer pipeline is obfuscation stacked on AES-GCM: L3 prime-composite residue, L5 device-FP-seeded row permutation, L7 S-box substitution, L8 XOR rolling mask — all key-derived, all reversible, none adding cryptographic strength beyond L2. Two details from the source decide it: `oracle.rs` opens with `#![allow(dead_code)]` and *"not currently wired into a production caller"* — **the ten layers never shipped** — and `oracle_encrypt` uses a **fixed nonce per key**, which is exactly the failure §4 legislates against. What carried the value there was the KeyRing, the per-surface separation and the device binding. Copy those.

## 7. Decisions requested — answered

`08` §14 answered all five. Recorded here so this document stands alone.

1. **Is A4 (DMA) in scope?** **Yes, but do not gate this work on it.** IOMMU is containment, not confidentiality, and R4 covers A4 anyway. File IOMMU as its own Feature on its own merits (`SEC-18`).
2. **Is A5 accepted as un-defendable in software?** **Yes — explicitly.** This is the decision that promotes R4. Where no in-artifact defence exists, put the root of trust outside the artifact: Sharc.Blue's outside was a remote service, ours is the TPM and the memory controller.
3. **Compression, encryption, or both?** **Both, in this order:** structural reduction → compress → encrypt → **pad to fixed buckets**. The padding is not optional; it is what closes the size channel. Compression gets zero confidentiality credit.
4. **Does the RT carve-out hold?** **Yes** (§5). If the requirement is genuinely "the live set is also unreadable", that is R4 and only R4.
5. **Does randomization stay deferred?** **Yes on ordering, on revised grounds** (§3, §6). The entropy source is a shared prerequisite for R3 and §4b as well as R5 — its own Story unblocking three things, not a cost chargeable to R5.

## 8. Graceful degradation, and the objection it creates

`08` §13 correctly observes that **an OS cannot refuse to boot because attestation is unreachable**, and points at Sharc.Blue's `KeyRing::from_local_only()`: derive a device-only secret from the fingerprint, keep the data encrypted, accept that it is not service-bound. R3/R4 need this path designed up front and named as a deliberate tier rather than discovered as a failure.

### 8.1 The downgrade attack — an objection neither document raised

This tier is in direct tension with §4d, and the tension has to be resolved before either is treated as settled.

The forged-kernel defence (A6) rests entirely on the TPM refusing to unseal for a boot that measures differently. A fallback tier that activates when the TPM is unreachable hands the attacker a strictly easier path than forging measurements: **make the TPM unreachable.** Cut the bus, disable it in firmware, boot on a board without one. Under rule 8 the adversary holds the design and will read §8 before attacking §4d.

So a fallback that silently degrades is not a resilience feature, it is the answer to the question the whole design was built to make unanswerable. Whatever is chosen, it must be chosen deliberately:

- **The tier must be distinguishable and attributable.** A boot that did not unseal is a different security state, and must be a spoor and a reported system property, never a silent substitution.
- **It must not decrypt what the sealed tier sealed.** A local-only key must derive a *different* key space, so degraded mode can protect new data and cannot retroactively open old data. If the fallback can read what the sealed tier wrote, the sealed tier's guarantee was never real.
- **Availability and confidentiality genuinely conflict here**, and the resolution is per-deployment, not per-mechanism (rule 7). A data-centre node may reasonably refuse to run; a UAV in flight may not. That is a policy input, and it must be one — not a default buried in a `Result` arm.

This is `STORY-P9-04-02`'s subject, and it is the reason that Story exists separately from the TPM driver Story.

## 9. Declared metrics

`08` §11's sharpest structural criticism: R1–R5 had no declared axes, and this project uses a scoreboard everywhere else. Sharc.Blue kills any lane that does not measurably move a declared axis, with parity as a hard correctness gate rather than a win axis. Same discipline here.

| Axis | What it measures | Which items move it |
|---|---|---|
| Resident plaintext bytes per task | Size of the puddle | R2 |
| **Plaintext-byte-seconds** | Exposure-window integral — what R1 and R2 jointly reduce and neither measures today. **Applies to key material as well as data** (§4b) | R1, R2, §4b |
| Dump-scan hits | Recognizable structures found in a dump: PE headers, section names, import strings, fixed constants | R1, R2, R5 |
| ns/access delta | Cost of any decode on a live path | R2's sliding-window half; the rejected decode-on-access bridge |
| Derivation ns, p99 | Whether derive-use-wipe is affordable at the call site | §4b |
| WCET delta, `D04`/`D05` parity as a hard gate | The RT thesis | all |

## 10. Note on `.rac`'s recorded intent

The project's recorded intent for `.rac`-style mmap/pointer-access loading is the **Phase 6 inference runtime** — model weights, out-of-core, where decode-on-access is a *performance* mechanism for data too large to hold resident. That is exactly the right home for it: a non-RT path, large data, and confidentiality as a welcome side effect rather than the justification. Generalizing the same mechanism to all process memory imports its latency into the RT path, where it works against the project's core claim. R2 is the narrow case where it crosses over on merit.

## 11. How to falsify this proposal

Restated against §9's axes, per `08` §15.5.

1. **§3 is wrong** if an analyst with a full dump genuinely cannot locate a relocated image cheaply. Test: take a dump from the `os` image today, relocate the image base, and measure the **dump-scan hits** axis on both. If randomization meaningfully reduces it, R5 moves up the list. `STORY-P9-01-01` builds the instrument this needs.
2. **§5 is wrong** if decode-on-access holds a bounded, measurable WCET on the `D04`/`D05` harness. Measured on the **ns/access delta** and **WCET delta** axes. The measurement path exists as of this week; this is a runnable experiment, not a matter of opinion.
3. **R4's premise is wrong** if the deployment target does not expose SME/TME, in which case A5 has no answer at all and must be documented as accepted risk rather than quietly assumed covered.
4. **§8.1 is wrong** if a fallback tier can be constructed that an attacker cannot force. If so, availability and confidentiality do not conflict here and the policy input is unnecessary.
5. **§4b's derivation cost is wrong** if it does not hold sub-microsecond p99 on real hardware — and §4c's hibernate argument depends on it. **Not measurable at Tier 0**: a TCG figure would be as misleading as the `D04` cross-space number was. This falsification is blocked on `LE-09` and must be stated as blocked, not deferred silently.

## 12. Residual risk — what none of this protects

Stated plainly so it is not assumed away.

- **No hardware root, no defence.** On a machine with no TPM and no memory-encryption engine there is no secret to hold, and publication changes nothing because there was nothing to protect. Decision 2's accepted-risk case, and it must be documented as such.
- **Physical attackers** — cold boot, bus interposers, TPM reset — remain outside software's reach. Hardware memory encryption is the answer, and it is a hardware answer.
- **Legitimate access to a running system** sees plaintext by definition. That is an authorization problem, not a confidentiality one.
- **Key material at the moment of use** is in registers and on the stack (§4b). Reduced, not eliminated.
- **Licensing and IP enforcement** is the one thing publication genuinely breaks. Client-side obfuscation was never a solution to it; it needs server-side entitlement, which is a different design and out of scope.

## 13. Decomposition

This proposal is decomposed into [`EPIC-P9`](../../goals/epics/EPIC-P9.md) — 8 Features, 10 Stories, with the dependency chain and the hardware precondition stated explicitly. Only `FEAT-P9-01`'s two Stories can be worked before a hardware tier exists; everything else is gated on `LE-09`.

## 14. The one-line summary

Publish everything: the secrecy lives in a per-machine, per-boot root that never existed in the source tree, every process key is derived from it using public process metadata as salt so nothing needs to be stored or carried across hibernate, and the thing stopping a forged kernel is the TPM refusing to unseal for a boot that measures differently — a property hardware supplies and obscurity never did.
