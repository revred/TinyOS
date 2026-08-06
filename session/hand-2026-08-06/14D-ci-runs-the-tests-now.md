# 14D — CI runs the tests now, and the gate that says so had to be mutated against the real file before it worked

`LE-100` closed. Executing [`13D`](13D-cover-note-for-the-next-session.md)'s
mandate, on a laptop, with no board run.

## 1. What changed

`.github/workflows/ci.yml` gains a **`host-tests` job** running
`cargo test --workspace` — its own job beside the QEMU ones, blocking from day
one. **1228 host tests** that gated a developer's machine and the pre-commit
hook, and nothing on the runner, now fail the build.

That is the easy half and it is one job block. The half worth reading is the
mechanism.

## 2. The obvious guard is circular, so the guard is a subcommand

A `#[test]` asserting *"CI runs the host tests"* is executed by the very job it
asserts the existence of. Delete the job and the test that would have objected
is no longer run either. It is not a weak guard, it is not a guard.

So `xtask check-ci-gates` (`os/src/xtask/src/ci_gates.rs`) is a step in the
fast governance job — the same reasoning that put `check-metric-labels` there
rather than leaving it a unit test (`12D`, `LE-91`) — and in
`.githooks/pre-commit` beside the other four. It refuses four things:

1. no `cargo test --workspace` anywhere in the workflow;
2. the suite sitting **inside `governance-gates`**, where the gates that exist
   to fail in seconds would queue behind it — `13D` called that placement
   wrong, and this is what stops it being undone by a tidier-looking edit;
3. a suite that cannot fail the build — `continue-on-error`, `|| true`;
4. any check named in `ci_gates::CI_ENFORCED` missing from the workflow —
   **itself included**, which is what closes the recursion. A guard nobody
   runs is the defect it was written about.

`CI_ENFORCED` is a closed list of six: `check-assurance-spine`,
`check-performance-catalogue`, `check-crate-sizes`, `check-metric-labels`,
`check-boot-images`, `check-ci-gates`. Adding a row is the deliberate act of
promising the runner executes it.

## 3. The finding: the first version of this gate did not work, and only the real file could say so

This is `12D`'s lesson landing on its own author, so it is stated plainly.

Eleven fixture tests were green. Four mutations of the **committed** workflow
were red for the right reasons. The fifth — narrow `run: cargo test --workspace`
to `run: cargo test -p kernel`, the exact way this hole would reopen quietly —
left the gate **green**.

The scan had matched the step's own `- name: cargo test --workspace` line. A
display name is prose. The fixture test for the same narrowing passed because
the fixture had no `name:` in it, which is precisely how a fixture flatters a
gate: it contains only what its author already thought of.

**The mutation that mattered made the gate stay silent, run against the exact
case it was written for.** The parser now marks only `run:` values and
block-scalar bodies as executable; five real-file mutations each fail with the
**error read** rather than the exit code; the name-only case is pinned by a
fixture test, and so is its inverse — a command inside a `run: |` block still
counts, or the fix would have been "reject `name:`" rather than "read what the
runner executes".

Do this. A gate verified only against fixtures its author wrote has been
checked against that author's imagination.

## 4. Concurrent session, and it committed mid-turn

Read this before `git log` confuses you. `13D` opened with *"nothing is
committed"* and made committing the first act. **A concurrent session did it at
23:12:25, while this session was reading the diffs** — `e273931`, both bodies of
work in one commit with each session's half named, exactly as `13D` prescribed.
`HEAD` moved from `4f5f2a4` under a session that had not staged anything.

Two things came with it that are not in `12D` or `13D`:

- `.gitattributes` is now repo-wide `* text=auto eol=lf` (goldens still
  `-text`), not the narrower `*.rs -text` that `12D` described. It renormalises
  nothing — `git ls-files --eol` reported zero committed CRLF files — and it is
  why roughly 2000 lines of pure EOL churn are absent from that commit.
- The ~1200 lines of "changes" in `SeedMVP.md`, `EPIC-P1.md` and
  `guardrail-evidence.tsv` that a reader of the pre-commit tree would have seen
  were mostly that churn. Content-only diffs were 1, 1 and 30 lines.

`CONCURRENT_SESSIONS` §"when a concurrent commit lands mid-turn, re-derive your
state" is the rule that caught it, and the thing that actually surfaced it was
`git diff` going silent between two tool calls — worth knowing, because a
diff that empties without explanation is a live session, not a bug.

## 5. What this does not do

- **It proves the workflow *asks* for the suite. It never proves a runner
  passed it.** No CI run exists for `e273931` or for this work.
- **The first Linux runs may go red on tests that pass on this bench.** This is
  a Windows host; `kernel`, `exec` and `shell` carry fixture bins gated
  `cfg(not(windows))` that no local gate compiles, and `boot_images.rs` has a
  comment saying those are "left to CI's Linux workspace run" — a run that did
  not exist when it was written. That is `LE-64`'s family. It is the reason this
  landed on its own rather than inside somebody's merge, and **red for that
  reason is expected and is the next session's, not a regression.**
- It says nothing about whether a test is meaningful, only that the harness is
  invoked. Coverage of the tests themselves is not a thing this file can hold.

## 6. Gates run

`cargo test --workspace` (1228 passed, 0 failed, 1 ignored — the deliberate
golden recorder), `cargo fmt --all --check`, `clippy --workspace --lib --tests
-D warnings`, `check-spine-files`, `check-assurance-spine` (32 Features / 100
Stories / 4050 selected contracts / 101 loose ends, **48 open**),
`check-metric-labels`, `check-citations`, `check-crate-sizes`,
`check-performance-catalogue`, `check-ci-gates`.

**Not run, and deliberately:** `check-boot-images` and `check-guest-images`.
Nothing here touches `kernel`, `hal-arm64`, `pi5-image` or any fixture source —
the changes are `xtask`, the workflow, the hook and three registers. Running one
is still not running the other (`LE-72`, `LE-92`); that trap is unchanged and
still applies to the next session that touches board code.

`check-timing-regression` untouched and still `LE-23`.

## 7. Files

| File | What |
| --- | --- |
| `os/src/xtask/src/ci_gates.rs` | new — the gate, 11 tests |
| `os/src/xtask/src/main.rs` | `mod ci_gates`, subcommand entry, match arm |
| `.github/workflows/ci.yml` | the `host-tests` job; `check-ci-gates` step |
| `.githooks/pre-commit` | `check-ci-gates` as the fifth check |
| `goals/assurance/loose-ends.tsv` | `LE-100` closed |
| `goals/index.html` | 49 open → **48 open**, tabstrip and prose |

## 8. Next

`13D` §"If you finish early" is unchanged and still in order:
**`EPIC-P1`'s Features table is missing its `FEAT-P1-11` row** (owner-approved,
pre-existing drift), then `LE-98`'s remaining half — the device-tree parse that
makes `SIMPLEFB_BASE` evidence rather than folklore — then `10C` §5 item 4,
which needs a hand on a mains plug.

**Do not start `FEAT-P1-12`.** It has a name now, and `13D` explains why the
name matters.

And one thing this session adds to that list: **watch the first `host-tests`
run.** `LE-64`'s rule is push, then watch the run, and this is the first push
in the project's history where the runner has anything to say about a test.
