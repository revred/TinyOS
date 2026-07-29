# Handover 04B — Two Sessions Can Now Commit Independently, and the Letter Now Identifies the Session

**The first document filed under the amended naming rule**, and the reason for the `B` is in §3. No
code, no contracts. Two changes and one row that could not be written.

The observation this answers came from the owner: *two handover documents cannot be executed
concurrently.* That is correct, it is structural rather than accidental, and there are four coupling
points. Two are fixed here.

## 1. The diagnosis, in one place

Four things force concurrent sessions to serialise. **None of them is a mistake anyone made** — each is
a reasonable rule whose interaction with a second session was never considered.

| # | Coupling | Evidence from 2026-07-28/29 |
| --- | --- | --- |
| **1** | **The pre-commit gate validated the *working tree*, not the index** — so it checked *everyone's* uncommitted work | Two sessions each abandoned a complete, green Story rather than commit. A third needed a throwaway worktree **four times in one afternoon** |
| **2** | **`LE-NN` ids must be contiguous** (`assurance.rs:1294`), so **id allocation is inherently serial** | Demonstrated on this very document — see §4 |
| **3** | Three append-only shared registers (`loose-ends.tsv`, `story-contracts.tsv`, `open-debt.tsv`) | `LE-48` landed in another session's commit because path-level staging is file-level staging |
| **4** | `LE-30`'s dashboard gate made `goals/index.html` a **fourth** serialisation point | Every session adding a Story or row must now edit it, and only when the tree is otherwise clean |

**The sharpest evidence is what the sessions did right.** Both stranded sessions refused to sweep the
other's edits and left finished work uncommitted instead. That is the protocol working — and it means
the cost was being paid by discipline where a machine should have paid it.

## 2. Fix ① — the gate validates the index

[`.githooks/pre-commit`](../../.githooks/pre-commit) now materialises the **staged** content into a
throwaway directory with `git checkout-index -a --prefix=` and runs the three gates there.

**The index is what `git commit` is about to write, and what CI will see.** So a failure is now always
about *your* change, and another session's half-finished Story cannot block your commit. That single
change removes coupling **1** outright.

**It stayed fast, which was the design constraint.** `CARGO_TARGET_DIR` points back at the real target
directory, so the dependency build is reused: **3.4 seconds warm for all three checks**, measured
before the hook was written rather than hoped for afterwards. The hook's own comment already warned
that *"a host-sensitive pre-commit hook trains people to pass `--no-verify`"* — a hook that triggered a
cold rebuild would have been exactly that, so the cost was measured first and the approach chosen
second.

**It was shown to fail before it was believed.** A throwaway worktree, the new hook installed, an
`LE-99` row staged to leave a gap:

```text
pre-commit: checking the staged tree — assurance spine, performance catalogue, crate sizes...
xtask: assurance spine invalid: loose-ends line 54: `LE-99` is out of order or
leaves a gap; ids must run contiguously from `LE-01`
```

**Zero commits were created.** Per `ADR 0005`'s trap and `STORY-P0-01-07` clause 2 — an instrument never
shown to detect anything cannot be believed when it passes. A hook that silently stopped checking
would look exactly like a hook that always passes.

**What it deliberately does not change:** the working tree can still be broken while the index is
clean, so `check-assurance-spine` by hand before staging is still worth running. The hook now answers a
narrower and more useful question — *is what I am committing valid?* — instead of *is this machine's
entire tree valid?*

## 3. Fix ③ — the letter identifies the session, not the document

[`session/README.md`](../README.md) is amended. A session claims **one letter** when it starts and uses
it for every document it files that day: `03A`, `04A`, `05A` for one session while a concurrent one
writes `03B`, `04B`.

**The original rule failed twice on its first day**, which is why this is worth changing rather than
restating. It read *"the letter distinguishes multiple documents at the same number"* — and both live
sessions independently chose `A`, for slot `39A` and again for `41A`, because a per-*document* letter
gives two sessions no reason to differ. The second collision reached the machine-readable register,
where `LE-47` and `LE-48` came to cite `hand-2026-07-28/41A` meaning two different files. That is
`LE-51`.

**A per-session letter has no such failure mode**: the letter is chosen once, against what other
sessions hold, rather than per document against nothing.

**This document is the first application of it.** A concurrent session holds `03A` and is session `A`
today; this session is `B`. The folder's earlier letters predate the amendment — `01` carries none,
`02A` is this session's — and are left as they are, per rule 5.

## 4. The row that could not be written — coupling 2, demonstrated on itself

**This finding has no `LE-` row yet, and the reason is the finding.**

The next free id is `LE-54`. A concurrent session has `LE-53` written and **uncommitted** in
`loose-ends.tsv`. Contiguity is enforced, so `LE-54` cannot exist without `LE-53` — and `LE-53` is not
this session's to commit. The options were to take another session's row, or to wait.

**So the row is owed, and this paragraph is the placeholder.** It should say: *concurrent sessions
cannot commit independently, for four reasons, of which two remain* — coupling **2** (serial id
allocation) and coupling **4** (the dashboard as a serialisation point). Fix ② from the diagnosis is
its remedy for the first: an `xtask register-loose-end` that allocates the id and appends atomically,
so allocation stops being a manual race. Coupling **4** is `LE-30`'s second half — generate the whole
dashboard, not the tiles only.

I would rather record that here, visibly, than take a number that belongs to someone else. **The
inability to file this row is better evidence for the row than anything the row could say.**

## 5. The `EPIC-P2` review, acknowledged

A concurrent session reviewed `EPIC-P2` §6 and registered `LE-53` against it. **Both halves are
correct and neither is a quibble.**

- **§6.3 rules out every single-webview front end and never says so.** A reserved region no tab content
  may *ever* paint is not merely hard inside one webview — it is **inexpressible**, because the region
  and the tab content are the same DOM in the same renderer. §6.4 makes precisely that argument against
  one artefact (Windows Terminal: *take the interaction model, not the authority model*) and stops
  there, so a reader can reach the opposite conclusion from the same section. Given Tauri is a
  first-class lane by `G-APP-2`/`APP-05`/`EPIC-H2`, the conflation is likely rather than possible.
- **§6.4 undersells the buffer/renderer seam.** It calls it *"worth copying as a shape"*. §6.6's
  obligation to **drop frames rather than block** depends on that seam existing, because a renderer that
  cannot be starved independently of the buffer can only block. It is a requirement, not an aesthetic.

Their two structural exclusions are the part I would not have found: **Tauri ships no renderer**, so a
webview shell needs a browser engine first — inverting the critical path, since `EPIC-P2` gates
`EPIC-H1`/`H3`/`H5` — and `REPORT-2026-07-26-28` already rules webview runtimes optional profiles never
added to the 8 MiB core image, while the shell is core. Recording the security argument *and* two
independent structural ones is what makes the decision survive a reader who rejects the security half.

**The §6 edit is owed and is not done here**, deliberately: it is one edit carrying both halves, their
row scopes it, and their `03A` §2 is already most of an ADR body. Their ordering caveat travels with
it and matters — **§1 of the Epic says 15 of 22 verbs are blocked on a filesystem that does not exist
(`LE-48`). The shell's blocker is storage, not the framework**, and none of this makes the front end
the next work.

## State

```text
main                    ea15c61 + this commit; 2 ahead of origin (origin was pushed to mid-session)
changed                 .githooks/pre-commit, session/README.md, this document + index entry
gates                   spine, spine-files, catalogue, crate sizes green — and the new hook
                        proven to REJECT a broken index, 0 commits created
register                UNCHANGED by this session, deliberately (§4)
coupling points         1 and 3 addressed (the hook, and rule 1's shared-file clause);
                        2 and 4 remain, and are what the owed row should carry
```

`goals/index.html` and `goals/assurance/loose-ends.tsv` both carry a concurrent session's uncommitted
edits and were **not** staged by this session. The session index needed one entry, so it was
content-staged as `HEAD` plus that entry — their `03A` entry is theirs to land.
