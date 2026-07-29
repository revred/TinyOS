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

### Staging narrowly is not enough for a *shared* file

Later on 2026-07-28, a session staged `goals/assurance/loose-ends.tsv` **by path** — exactly what this
rule asks for — and swept in another session's appended `LE-48` row, which landed in `8b8f703`, a commit
about priority inheritance, under the wrong authorship. The other session's `goals/index.html` edit did
not travel with it, so the register and the dashboard disagreed for as long as it took someone to notice.

**Path-level staging is file-level staging.** `git add <path>` takes *every* change in that file,
including the ones you did not make. For the handful of append-only shared registers — the assurance
TSVs above all — narrow staging is therefore **not** the same as narrow *content*.

When you must commit a shared register while another session has edits pending in it, stage the content
rather than the path:

```sh
# verify YOUR row over clean HEAD first (see rule 8), then stage exactly HEAD + your row
SHA=$(git hash-object -w --path=goals/assurance/loose-ends.tsv <verified-file>)
git update-index --cacheinfo 100644,$SHA,goals/assurance/loose-ends.tsv
git diff --cached --numstat -- goals/assurance/loose-ends.tsv   # MUST show only your lines
```

That last line is the check that makes this safe, and it is the one to actually run: **read
`git diff --cached` before every commit, not after.** It is also how a directory-wide `git add -- session/…`
was caught mid-turn on the same day, one tool call before it would have committed another session's
handover.

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

**8. Never leave a machine-checked shared file invalid between tool calls.**

Later on 2026-07-28, a hand-built edit to [`goals/assurance/loose-ends.tsv`](../goals/assurance/loose-ends.tsv)
consumed the tab separating two fields, leaving a 7-field row in an 8-field file. The session that
made it caught the break with its own ad-hoc field-count check and repaired it several steps later. In
between, `check-assurance-spine` failed for a **different** session that had changed nothing — in a
file they were correctly leaving alone, for a reason they could not diagnose from their own tree.

Rule 2 worked: their pre-commit caught it rather than letting a broken spine ship. But no rule
prevented it. So: **when you hand-edit a machine-checked file, validate it before your next tool
call** — `cargo run -p xtask -- check-spine-files` — not several steps later when you happen to look.
The edit is not finished until the file parses. (That command did not exist when this rule was
written; the next section is why it does now, and why a field count alone was the wrong instrument.)

And when you *hit* someone else's broken row, the response is the one the `STORY-P1-07-02` session
used, which is better than this document previously asked for:

- **Do not repair it.** It is mid-edit, not abandoned. This is rule 5's sibling.
- **Do not reach for `--no-verify`.** The gate is right; your tree is just not the whole tree.
- **Verify your own subset in a throwaway worktree over clean `HEAD`**, then wait for the row to
  complete.

### The second incident, later the same day: the rule was half-right

`LE-43` was written **twice**, by two sessions, into the same file. For a period `loose-ends.tsv` held
two `LE-43` rows and `check-assurance-spine` was red with `duplicate id LE-43`. Three things this
incident corrects, all of them cheap:

**1. A field-count pass would not have caught it, and did not.** Both duplicate rows were well-formed
at eight fields — a duplicate id is a different defect class from a consumed separator. The session
that made the edit ran its field check, the check passed, and it was right to pass. What actually
surfaced the break was **re-inspecting `git` state the moment a concurrent commit appeared**: noticing
a new `HEAD`, diffing the working tree against it, and reading both rows. Credit the mechanism that
worked, because the next session will reach for the one it is told about.

So rule 8's "validate before your next tool call" needs its instrument named: **validate with the
check that would fail** — not a field count alone. `LE-36` asked for a field-count guard and was, on
this evidence, **under-specified**: field counting is necessary and demonstrably not sufficient.

**The instrument now exists.** `STORY-P0-01-07` closed `LE-36` with:

```text
cargo run -p xtask -- check-spine-files
```

It checks header agreement, field count, **key uniqueness and id contiguity** across all 15
hand-edited spine TSVs and does nothing that requires opening a second file, so it returns fast
enough that skipping it has no excuse. It is a strict subset of `check-assurance-spine`: it can never
pass where the full check would fail on the same file. Run it after every hand edit; run
`check-assurance-spine` before you commit.

**2. Guard the write, not only the result.** A second session read the broken row, built a repair, and
gated the write on the file's line count — which **refused to fire**, because by then the other session
had already withdrawn its row and the file had changed under it. That one `[ … ] || exit 1` is the only
reason two sessions did not write the same file in the same second. Validating *after* writing catches
a break; guarding *before* writing prevents one. Do both.

**3. When a concurrent commit lands mid-turn, re-derive your state before continuing.** `git status`,
`git log`, and the diff of anything you were about to touch. Nothing in a shared tree entitles you to a
fact you established a tool call ago, and mtimes are enough to tell a live session from an abandoned
one when you need to know.

## The rule for counts and totals

The five spine-count re-syncs have one cause worth naming separately, because it recurs anywhere two sessions share a repository:

**A count of how much work exists is a floor, never a total.**

`os/src/xtask/src/assurance.rs` now splits the two: closed catalogues (5 classes, 20 controls, 14 gates, …) are asserted exactly, because changing one is a deliberate charter amendment that *should* break a test; population counts (Features, Stories, Tests, Reports, loose ends) are asserted as floors, because they grow with every Story — including one landing in a tree you cannot see. A floor still catches the failure that matters: documents are added, never deleted, so a shrinking count means something was lost.

## What this does not solve

Nothing here prevents two agents editing the same file. It makes the *cheap* failures — bad staging, unread commits, number collisions, brittle totals — stop happening, and it makes the expensive one visible early. Genuine concurrent editing of one file still needs the sessions to talk, and there is no mechanism in this repository for that.
