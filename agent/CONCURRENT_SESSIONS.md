# Concurrent Sessions — the Protocol for Two Agents in One Working Tree

Binding, like [`CODING_STANDARDS.md`](CODING_STANDARDS.md). Short on purpose.

## Why this exists

On 2026-07-28 two agents worked this repository at the same time with no coordination at all. In one day that produced, in order:

- a `git add -A` that swept a **half-finished Feature from the other agent's tree** into a commit — six contract rows and one of six Story documents — and pushed a broken assurance spine to `main` (`585a027`, CI red);
- a transient non-compiling `main.rs` on `main`;
- **two** handover-number collisions in one dated folder (17, then 18/19);
- a commit landing under one agent's authorship containing **an entire Feature, six Stories and an ADR that agent had not read**;
- five successive re-syncs of the same hard-coded spine counts, each treated as a symptom.

None of that was a mistake of reasoning. All of it was the absence of a protocol. This is the protocol.

## The rules

**1. Stage narrowly. `git add -A` is banned when another session may be live.**

Stage the paths you actually changed. This one rule prevents the worst outcome above — committing another agent's unfinished work under your name — and it costs nothing when you are alone in the tree.

**2. Re-run the gates *after* staging, never only before.**

`.githooks/pre-commit` now does this for you; install it with `git config core.hooksPath .githooks`. The gate you ran before `git add` describes a tree that no longer exists. This is exactly how the broken spine shipped.

**3. Never commit a file you have not read.**

Disclosing it in the commit message is not review, and authorship is not a technicality: `Co-Authored-By` and your name on the commit assert that the change was reviewed. If a file appears in your `git status` that you did not write, leave it — it belongs to the other session.

**4. Claim your handover number by creating the file first.**

Write the empty (or one-line) file the moment you know you will need the slot, before writing its contents. A slot taken is a collision avoided; a slot claimed late is a rename, a set of dangling links, and a paragraph of explanation in someone else's document.

**5. Never rewrite another session's dated documents.**

`session/` folders are an immutable record. When a renumber or a supersession happens, record it in *your* document and point back — do not repair theirs. Handover 19 §"On the number" is the worked example.

**6. Pull before you branch off `main`, and merge rather than rebase across sessions.**

Another agent's commits may already be on `main` that were not there when you started. Rebasing rewrites shared history you cannot see the other side of.

**7. Say so in the handover.**

If a session ran concurrently, its handover states which commits arrived mid-session and what they touched. Handovers 01 and 19 both do this; the reader's alternative is reconstructing a race from a commit graph.

## The rule for counts and totals

The five spine-count re-syncs have one cause worth naming separately, because it recurs anywhere two sessions share a repository:

**A count of how much work exists is a floor, never a total.**

`os/src/xtask/src/assurance.rs` now splits the two: closed catalogues (5 classes, 20 controls, 14 gates, …) are asserted exactly, because changing one is a deliberate charter amendment that *should* break a test; population counts (Features, Stories, Tests, Reports, loose ends) are asserted as floors, because they grow with every Story — including one landing in a tree you cannot see. A floor still catches the failure that matters: documents are added, never deleted, so a shrinking count means something was lost.

## What this does not solve

Nothing here prevents two agents editing the same file. It makes the *cheap* failures — bad staging, unread commits, number collisions, brittle totals — stop happening, and it makes the expensive one visible early. Genuine concurrent editing of one file still needs the sessions to talk, and there is no mechanism in this repository for that.
