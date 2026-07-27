# Handover 08 — Review of the Memory Confidentiality Proposal, and the Construction That Follows

Status: **Review + construction. No code changes. Answers the five decisions in [`07`](07-memory-confidentiality-proposal.md) §7, revises two of them, and specifies the key architecture `07` left unwritten.**
Reviews: [`07-memory-confidentiality-proposal.md`](07-memory-confidentiality-proposal.md).
Evidence base: `C:\Code\Sharc.Workspace\Sharc.Blue` — `Sharc.Bluekind/docs/ThePlan/EnvelopeSecurity_Plan.md`, `Sharc.Bluekind/Blue.Protocols/src/{key,oracle,sbox,crypto}.rs`, `Sharc.Bluekind/docs/ThePlan/CompressionContract.md`, and the frugality doctrine in `CLAUDE.md`.

Parts II and III of this document were developed in review with the operator. The derived-key construction in §5, the hibernate objection in §6, and the open-source requirement in §7 were raised by the operator against an earlier draft of this review that had concluded "the key lives in the CPU" — a conclusion §6 shows to be incomplete. They are recorded here as the way forward, not as commentary on `07`.

---

## Part I — Findings against `07`

### 1. Verdict

`07` is sound and its central correction — §3, that randomization does not defeat a dump — is right and worth the space it takes.

Three findings against it:

- It **has a hole exactly where Sharc.Blue's actual win was**: key provenance. `07` §4 specifies the AEAD construction with care and never says where the key lives. (§2 below.)
- It **mis-ranks the one item that is the same idea in hardware.** R4 is placed fourth and described in its own body as the best item in the list. (§3.)
- It **evaluates mechanisms without asking whether they survive publication.** The distinction between a mechanism that keeps its strength when the source is public and one that does not is the cleanest available test, and `07` does not apply it. (§7.)

### 2. The missing section: key provenance

Sharc.Blue's success was not the cipher and not the ten layers. It was one sentence in `Blue.Protocols/src/key.rs`:

> *"The account secret is the 'missing piece' — never in the binary, never on disk. Without the external service response, all data is locked."*

The construction is `session_key = HMAC(device_fp, account_secret)`, where `account_secret` arrives from a live external service per session and is never persisted. That single property is what makes both "binary RE" and "cold.start theft" dead ends in their own attack table — not the ten layers stacked above it.

`07` §4 and R3 specify nonce/AAD binding to `(address, generation)` correctly, and say **nothing about where the key lives**. Against A3 that is the whole question. An offline analyst holding a full memory dump also holds the `KeyRing` sitting in `.bss`. **R3 as written buys A3 approximately nothing, because the dump contains the key next to the ciphertext.**

So `07` §4's trap paragraph is correct but incomplete. There are two traps, and the second is the larger:

| # | Trap | Status in `07` |
| --- | --- | --- |
| 1 | Encryption without integrity → replay / relocation | **Stated**, and argued well via SEV → SEV-SNP |
| 2 | Encryption with the key co-resident in the thing being dumped | **Unstated** |

Part II is the section `07` needs in order to close trap 2.

### 3. R4 belongs first

R4 (AMD SME/SEV-SNP, Intel TME/MKTME/TDX, ARM CCA) is **the same doctrine as Sharc.Blue's session key, moved into silicon**: the key lives outside the artifact under analysis — in the memory controller rather than on a remote service. It is the only item in `07`'s list that answers "where does the key live" correctly for A3, and by `07` §2's own table it is also the only item that touches A4 and A5.

`07` already says this in the R4 body — *"best value in the list, and it is the honest answer to 'make a memory dump undecipherable'"* — and then ranks it fourth. That inconsistency is the first thing a reviewer will attack, and it undersells the strongest recommendation in the document.

**Reorder: R4 first, R2 second.** With the qualification in §6 — hardware memory encryption alone does not survive a power cycle, so it is necessary and not sufficient.

### 4. Copy Sharc.Blue's KeyRing. Do not copy Oracle Mono.

Worth being blunt, because the original request named `oracle.mono` specifically.

The 10-layer pipeline is **obfuscation stacked on AES-GCM**. L3 prime-composite residue, L5 device-FP-seeded Fisher-Yates row permutation, L7 S-box substitution, L8 XOR rolling mask — all key-derived, all reversible, none adding cryptographic strength beyond L2. In Sharc.Blue that was a defensible *commercial* choice: it protects licensing IP against reverse engineering and raises an analyst's cost. It is not what would protect a memory dump, and by §7's test it is the clearest example in either codebase of a mechanism whose value is destroyed by publication.

Two details from the source decide this:

- `Blue.Protocols/src/oracle.rs` opens with `#![allow(dead_code)]` and the comment *"not currently wired into a production caller; its tests exercise the full roundtrip locally."* **The ten layers never shipped into a production path.** What shipped, and what carried the value, was the KeyRing, the per-surface key separation, and device binding.
- `oracle_encrypt` calls `crypto::derive_nonce(cold_key, 0)` — a **fixed nonce per key**. Tolerable for a single whole-file seal; catastrophic across many independently-sealed blocks. This is exactly the failure `07` §4 already legislates against. Inherit `07` §4's rule, not this code.

The transferable pieces are cheap and high-value:

| Sharc.Blue mechanism | TinyOS analogue |
|---|---|
| `session_key = HMAC(device_fp, account_secret)` | `process_key = KDF(root_secret, process metadata)` — §5 |
| `stdio_key` / `cold_key` / `journal_key` | Per-surface subkeys: image-at-rest, spoor, future crash dump, IPC |
| `device_fp` binding — cross-machine copy fails | Measured-boot binding — an image replayed elsewhere is dead |
| `Chapter` dropped after a single read | Derive-use-wipe rather than hold a resident key — §5 |
| `hold_time` + heartbeat rotation | Rotation keyed to `TeardownGeneration` |

Per-surface separation earns its keep twice: it stops a compromise crossing surfaces, and it makes R1's invariant **auditable per surface** rather than as one global claim.

---

## Part II — The construction

### 5. Derive the key; do not store it

The design closing trap 2 is a two-input derivation. Everything about a process that is already public goes in as salt; exactly one input is secret and comes from hardware.

```
root_secret   ← unsealed from the TPM against boot-chain PCR measurements
                never in the source tree, never in the binary, never on disk in clear,
                different on every machine

process_key   = KDF(root_secret,
                    image_base ‖ image_size ‖ layout_hash ‖ process_id ‖ teardown_generation)

block_tweak   = (page_address, teardown_generation)   → AEAD nonce / AAD per `07` §4
```

Three properties follow, and each of them is load-bearing.

**The metadata is the right input, for binding rather than for secrecy.** Base address, image size, load layout, process number and generation bind ciphertext to *this process, at this address, in this generation*. Relocate a block and it fails; replay an old one and it fails. That is the anti-replay and anti-relocation property `07` §4 argues for, and it is the reason AMD had to add SEV-SNP's Reverse Map Table on top of encryption that was already sound.

**The metadata cannot be the secret, and must never be credited as such.** Every one of those values is present in the dump the adversary is holding. An analyst computes the same derivation, at the same speed. The entropy is not there either:

| Input | Entropy today | Entropy if fully randomized |
|---|---|---|
| Image base | **0 bits** — hardcoded `0x1_4000_0000` in 15 sites (`07` §3) | ~25 bits |
| Image size | ~0 bits — fixed by the build | ~0 bits |
| Process id | ~0 bits — sequential | ~16 bits |
| Layout | ~0 bits — deterministic | a few bits |

Zero today; brute-forceable even at the theoretical best. `root_secret` carries all of the secrecy and the salt carries none of it. A derivation over public inputs alone is a reversible transform whose parameters ship alongside the ciphertext — it is obfuscation, and §7 is the test that says so.

**Derive-use-wipe beats hold-resident.** An HMAC-SHA256 or AES-NI derivation costs on the order of hundreds of nanoseconds. That is cheap enough to re-derive at the point of use and wipe immediately, rather than keeping a `KeyRing` alive in `.bss` for the lifetime of the process. This is R2's own argument — shrink the puddle — applied to the key itself, and it is the direct analogue of Sharc.Blue dropping the `Chapter` handle after a single read.

**Honest limit.** Derive-use-wipe shrinks the key's residency; it does not eliminate it. During the operation the derived key exists in registers and possibly on the stack, and a dump captures both. This reduces exposure rather than closing it, which is precisely why the plaintext-byte-seconds metric in §11 must be applied to key material as well as to data, and why hardware memory encryption (R4) remains necessary — it is the only layer that protects the key at the moment of use.

### 6. Power states: why R4 alone is not sufficient

Hardware memory-encryption keys are ephemeral. SME/TDX generate a key at power-on and lose it at power-off. A hibernation image is written out from the OS's point of view as plaintext, and on resume the CPU holds a new key that decrypts nothing that came before.

So "the key lives in the CPU" is a true statement about the live system and an incomplete statement about the system's lifetime. Any design that stops there fails at the first suspend-to-disk.

The derivation in §5 is what closes it, and TPM sealing is the mechanism built for exactly this case:

| Event | What happens |
|---|---|
| Boot | Boot chain extends PCRs; TPM unseals `root_secret` only if measurements match |
| Run | Per-process keys derived on demand from `root_secret` + metadata, used, wiped |
| Hibernate | Nothing key-shaped needs to survive. The image carries its own metadata |
| Resume | Same machine, same measurements → TPM unseals → every process key re-derives in sub-microsecond |
| Hibernation file stolen and resumed elsewhere | Different machine → no unseal → the file is inert |

The sub-microsecond derivation cost is what makes this practical: nothing has to be carried across the power cycle except one sealed root.

**Consequence for `07`:** R4 must be stated as necessary-and-not-sufficient, paired with a sealed root and a derivation. `07` as written has no position on power states at all, and neither did the first draft of this review.

### 7. The design must hold with the source published

This is the test `07` does not apply, and it is the cheapest way to tell a real mechanism from a decorative one.

Kerckhoffs's principle: a system must remain secure when everything about it is public except the key. AES, TLS, LUKS, FileVault and the SEV specification are all fully published, and their security is undiminished. The construction in §5 satisfies this by inspection:

| Component | Public? | Carries secrecy? |
|---|---|---|
| Cipher, KDF, AEAD construction | Publish it | No |
| The derivation formula | Publish it | No |
| Base, size, layout, pid, generation | Public **and already in the dump** | No — binding only |
| The binary, byte for byte | Publish it | No |
| `root_secret` | **Never** — per-machine, per-boot, TPM-sealed | **All of it** |

One row is secret, and it is not code. It cannot be committed, cannot be built into an artifact, and differs on every machine.

### The objection that matters: a forged kernel

If the source is public, an attacker does not attack the cipher. They build their **own** TinyOS — identical but with the encryption stubbed out and a memory dumper added — and boot it on the target.

The defence is not secrecy; it is the sealing policy. `root_secret` is sealed against PCRs measuring the boot chain. One byte different in the kernel image produces a different measurement, and the TPM refuses to release the secret. The forged kernel boots perfectly and finds nothing but ciphertext it can never key.

That property is supplied by hardware and is entirely unaffected by publication — the attacker already knew the design. What they cannot do is produce a different kernel that the TPM will still unseal for.

This also closes a hole that would otherwise be created by §13's debug/production split. Sharc.Blue's `envelope-debug` feature keeps plaintext so tests can assert against it; published, anyone could build that variant. But a debug build measures differently, so it never receives the key. **The reproducibility carve-out and the security boundary end up enforced by the same mechanism** — which is a considerably better outcome than defending them separately.

### What publication genuinely costs

Two real costs, named rather than waved away.

- **Fixed constants become free.** `0x1_4000_0000` across 15 sites and `CAPABILITY_TRAP_VIRT` = `0xdead_0000` fingerprint a dump as TinyOS in one pass. Publication does not create this, but it removes the last friction. See §12.
- **"They would have to reverse-engineer the format" stops being chargeable.** Sharc.Blue explicitly banked on this, counting proprietary RJL rotation and B2 column schema as security layers. Under publication that credit is exactly zero — which is why §10 refuses to count it. If any part of the design quietly relies on an attacker not knowing the layout, this test is what surfaces it.

The frequently claimed *benefit* — more eyes find more bugs — is real but conditional on review actually happening. Heartbleed sat in public OpenSSL for two years. Do not budget for it.

### 8. Rules

These are the standing rules the construction implies. They are stated as rules because each one is individually checkable in review.

1. **No secret ever enters the repository or a build artifact.** Not a key, not a seed, not a default, not a test fixture promoted to production. If any build can produce a working decryptor without talking to hardware, the design is broken.
2. **Every secret is per-machine and per-boot**, obtained from a hardware root at runtime.
3. **Kernel integrity is enforced by measurement, not by source secrecy.** This is what defeats the forged-kernel attack, and it is what makes rule 1 safe.
4. **Public process metadata is salt, never secret.** It binds; it does not conceal. Never credit it with entropy it does not have.
5. **Derive at point of use and wipe; do not hold resident.** Key material is subject to the same exposure budget as plaintext (§11).
6. **No fixed sentinels, magic numbers, or hardcoded addresses** in anything reaching memory or disk.
7. **Deployment configuration is separate from mechanism.** PCR policy, key hierarchy, attestation endpoints are per-site — and none of them are cryptographic secrets either.
8. **Assume the adversary holds the binary, the source, the build and the documentation.** Any conclusion that changes under that assumption was resting on obscurity and must be re-derived or discarded.

---

## Part III — Items from `07` that survive or strengthen

### 9. Sharc.Blue confirms the RT carve-out (`07` §5)

Note where the layers actually live. The ten-layer pipeline is applied to `cold.start` and `journal` — cold surfaces. The hot surface (`stdio`) gets only AES-GCM plus padding, explicitly documented *"hot path — zero alloc."*

Sharc.Blue had **no WCET budget at all** and still kept the expensive encoding off its hot path. That is direct precedent for `07`'s "encode at rest; never on the live RT working set", and `07` §5 can cite it rather than arguing the point from first principles.

### 10. The size channel is already solved, with a shipped implementation

`07` §8 identifies the CRIME/BREACH shape applied to memory and asks for fixed-size buckets. Sharc.Blue ships precisely that:

```
[seq:8][nonce:12][compressed + AES-GCM ciphertext:N][pad_to_512]   — uniform blobs, plus ghost traffic
```

Their verification steps transfer directly, and give R1 a runnable acceptance test:

> *"Binary inspection: `grep -ao "pattern" target/release/blue-sharc.exe` finds no recognizable envelope/key/crypto strings."*
> *"File inspection: `xxd .sharc/cold.start | head` shows no magic bytes, no JSON, no recognizable structure."*

For TinyOS: dump the image and scan for PE headers, section names, import strings, `0x1_4000_0000` and `0xdead_0000`. One command, and it either finds them or it does not.

**Caution.** `EnvelopeSecurity_Plan.md` claims RJL and B2 are *"implicit encryption layers"* and that an attacker who breaks AES-GCM *"still faces B2 decompression."* Count that as **zero** in TinyOS's threat model, per §7 and rule 8.

### 11. R2 has more backing than `07` claims for it

`07` calls R2 "highest value per unit of work" and argues it from code written this week. The Sharc.Blue record puts it on much stronger footing: R2 is their frugality doctrine applied to a different substrate. From `CompressionContract.md`:

```
Source (N bytes)
  → structural reduction: skip bodies/whitespace/comments  (~8% survives)
    → graph skeleton: iblock nodes + B2 edges              (~0.1% of source)
      → lazy materialisation: only hot blocks resident      (~0.01% at query time)
```

Shrinking the resident plaintext arena to a sliding window is not speculative. **It is the mechanism that produced Sharc.Blue's headline numbers.**

It also arrives with a discipline `07` lacks. Sharc.Blue kills any lane that does not measurably move a declared axis — `B/node`, `spans-touched`, `ns/node`, `ns/edge`, `allocs/request`, `freshness-sync-cost` — with parity as a hard correctness gate rather than a win axis. R1–R5 have no declared axes. They should:

| Axis | What it measures | Which items move it |
|---|---|---|
| Resident plaintext bytes per task | Size of the puddle | R2 |
| **Plaintext-byte-seconds** | Exposure-window integral — what R1 and R2 jointly reduce and neither measures today. **Applies to key material as well as data** (§5) | R1, R2, §5 |
| ns/access delta | Cost of any decode on a live path | R2; the rejected decode-on-access bridge |
| Derivation ns, p99 | Whether derive-use-wipe is affordable at the call site | §5 |
| WCET delta, `D04`/`D05` parity as a hard gate | The RT thesis | all |

That turns `07` §10 from a list of ways to be wrong into a scoreboard — the form this project already uses everywhere else.

### 12. Randomization has a second motive `07` did not consider

`07` §3 argues correctly that randomization does not defeat A3: an analyst holding a dump scans rather than guesses, and the second pass finds whatever the first pass moved. That argument stands.

But under §5, a randomized image base and process id are no longer only address obfuscation — **they are entropy inputs to the key derivation**, and today they contribute zero bits (§5's table). That is a different argument for the same work, and neither `07` nor the first draft of this review made it.

It does not promote R5 to the front. `root_secret` carries the secrecy and must continue to do so alone; salt entropy is defence in depth, not a substitute. But it does change R5's justification from "buys A2 only" to "buys A2, plus salt diversity in the derivation, plus removal of the fingerprint constants §7 flags." That is a materially better case than `07` records.

### 13. Two smaller gaps in `07`

**Debug/production duality.** `07` §3 objects that randomization "fights reproducibility, which this project spends", and defers R5 partly on that basis. Sharc.Blue has the standard answer already built: `envelope-debug` / `envelope-production` cargo features, where debug keeps plaintext plus an HMAC tag so tests can still assert, plus `KeyRing::new_for_test(tag)` to derive a deterministic ring from a string tag. Reproducibility under encryption is a solved problem, not a reason to defer — and §7 shows the split is safe under publication because a debug build measures differently and never receives the key. **Defer R5 on its real merits and drop the reproducibility argument**, or it will be answered in one line at review.

**Graceful degradation.** `KeyRing::from_local_only()` handles the Guard service being unreachable by deriving a device-only secret from the fingerprint: data stays encrypted, it is simply not service-bound, and the comment insists *"Still not degraded mode — the resulting ring just isn't Guard-bound."* An OS cannot refuse to boot because attestation is unreachable. R3/R4 need this path designed up front and named as a deliberate tier rather than discovered as a failure.

---

## Part IV — Decisions, changes, residual risk

### 14. Answers to the five decisions in `07` §7

1. **Is A4 (DMA) in scope?** **Yes, but do not gate this work on it.** IOMMU is containment, not confidentiality, and R4 covers A4 anyway. File IOMMU as its own Feature on its own merits.
2. **Is A5 accepted as un-defendable in software?** **Yes — accept it explicitly.** This is the decision that promotes R4. Sharc.Blue's precedent is exactly this move: where no in-artifact defence exists, put the root of trust outside the artifact. Their outside was a remote service; ours is the TPM and the memory controller.
3. **Compression, encryption, or both?** **Both, in Sharc.Blue's order:** structural reduction → compress → encrypt → **pad to fixed buckets**. The padding is not optional; it is what closes `07` §8. Credit the compression with zero confidentiality (§7).
4. **Does the RT carve-out hold?** **Yes.** See §9 — Sharc.Blue kept its expensive path off a hot loop that carried no timing budget at all.
5. **Does randomization stay deferred?** **Yes on ordering, but on revised grounds.** It stays behind the root-secret work, because salt entropy is defence in depth and not a substitute for `root_secret`. But per §12 its justification is now threefold, not single, and per §13 the reproducibility objection is void. Note also that an entropy source is a shared prerequisite for R3 and §5 as well as R5 — it is its own Story unblocking all three, not a cost chargeable to R5.

### 15. Requested changes to `07`

1. Add **§4b — key lifecycle**: provenance, residency, wipe, rotation, per Part II §5. Without it, R3 does not buy what it claims.
2. Add **§4c — power states**, per §6. `07` currently has no position on hibernate, and R4 alone fails there.
3. Add **§4d — the publication test**, per §7 and the rules in §8. Every mechanism in `07` should be marked as surviving publication or not.
4. **Reorder**: R4 first — necessary and not sufficient — R2 second.
5. Add the **declared metrics** in §11, and restate `07` §10's falsification tests against them.
6. Add the **dump-scan audit** from §10 as R1's acceptance test — it is one command, and it makes "no plaintext survives teardown" a measured invariant rather than an asserted one.
7. Restate **R5's justification** per §12, and drop the reproducibility argument per §13.
8. Note the **local-only fallback tier** for R3/R4 per §13.

### 16. Residual risk — what none of this protects

Stated plainly so it is not assumed away:

- **No hardware root, no defence.** On a machine with no TPM and no memory-encryption engine there is no secret to hold, and publication changes nothing because there was nothing to protect. This is decision 2's accepted-risk case and must be documented as such, per `07` §10's own warning about R4's premise.
- **Physical attackers** — cold boot, bus interposers, TPM reset attacks — remain outside software's reach. Hardware memory encryption is the answer, and it is a hardware answer.
- **Legitimate access to a running system** sees plaintext by definition. That is an authorization problem, not a confidentiality one.
- **Key material at the moment of use** is in registers and on the stack (§5). Reduced, not eliminated.
- **Licensing and IP enforcement** is the one thing publication genuinely breaks. Client-side obfuscation was never a solution to it; it needs server-side entitlement, which is a different design and out of scope here.

### 17. The one-line summary

Publish everything: the secrecy lives in a per-machine, per-boot root that never existed in the source tree, every process key is derived from it in sub-microsecond using public process metadata as salt so nothing needs to be stored or carried across hibernate, and the thing stopping a forged kernel is the TPM refusing to unseal for a boot that measures differently — a property hardware supplies and obscurity never did.
