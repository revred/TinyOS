# 01A — Cover Note for the Next Session

Short by design. [`14D`](../hand-2026-08-06/14D-ci-runs-the-tests-now.md) is the
last full handover and this does not repeat it. This is the orientation a fresh
agent needs in five minutes — and it exists mainly because **the one thing
`14D` told you to watch for cannot happen**, and nobody has been told why.

## Where the project actually is

Everything is committed and pushed. `dff7b1d` on `origin/main`, tree clean, and
the arc that ran from `12D` to `14D` closed three loose ends with mechanisms
rather than corrections:

- **`LE-91`** — a fixture metric's domain is machine-checked against the Story
  it names (`check-metric-labels`, 40 metrics, 6 fixtures).
- **`LE-100`** — CI runs the host suite (`host-tests`, 1228 tests), and
  `check-ci-gates` refuses a workflow that quietly stops asking for it.
- **`LE-101`** — the repo is LF everywhere, so a worktree stops failing
  source-level guards for a reason that is not the property.

The Pi 5 is powered and beaconing, untouched since the 2026-08-06 boot.
**There is no plug on the desk.** Every item below runs on a laptop.

## Your task: run creation stopped, and the outage no longer explains it

`14D` §8 asked the next session to **watch the first `host-tests` run**. There
is nothing to watch. Verified 2026-08-07:

```text
last run created:  420e875   2026-08-06T01:13:00Z
runs since:        none, for b4a7010, cb9b27b, 4f5f2a4, e273931, dff7b1d
Actions component: operational        (was major_outage on 2026-08-06)
workflows:         CI active, fork-advisories active
permissions:       enabled: true, allowed_actions: all
push events:       registered — e273931 at 2026-08-06T22:13:17Z, actor revred
```

So: **the push lands, GitHub records the event, the workflow is active, Actions
is healthy — and no run is created.** `10C` §3 diagnosed this as the platform
outage and was right at the time. The outage is over and the symptom is not.
That diagnosis has expired; do not re-apply it.

**Five commits' worth of gates have never executed on a runner.** That includes
`host-tests` itself, which means `LE-100` — closed on the grounds that a gate
must run where it counts — currently has a mechanism that has never run where it
counts. Until this is resolved, treat `12D`'s and `14D`'s CI-side work as
**asserted, not demonstrated**.

### Do not re-check these — `10C` §3 ruled them out and they still hold

Push landed (`git ls-remote`), Actions not disabled, workflow not disabled,
`.github/` untouched by the commit, no `paths:` filter, and the
`GITHUB_TOKEN`/App-actor recursion rule (the push is an OAuth user token,
`gho_`, same actor as pushes that did create runs).

### But one instrument in that list is broken, and this is the part to read

`10C` ruled out *"run exists but hidden"* with
`actions/runs?head_sha=b4a7010` → `total_count: 0`. **That query returns `0`
for `420e875` too** — a commit whose run demonstrably exists
(`31062093883`, and it is the newest run in the list). The probe returns zero
regardless, so it ruled out nothing.

The conclusion still stands — there really is no run, and the full run list is
what shows it — but it was reached with an instrument that cannot distinguish
its two answers. **Use the list and match client-side:**

```sh
gh api "repos/revred/TinyOS/actions/runs?per_page=10" \
  --jq '.workflow_runs[] | "\(.created_at)  \(.head_sha[0:7])  \(.conclusion)"'
```

This is `LE-80`'s family for the third time in four sessions: a tool that
answers confidently and means nothing. If you rule something out this session,
first check the instrument can produce both answers.

### Where to look next

The remaining shape is *"pushes register, runs are not created, nothing errors"*
— silent and by omission, which is this project's recurring signature. The
candidate `10C` could not reach is **account-level**: Actions spending limit or
billing state, which suppresses run creation repository-wide with no message
anywhere. Check the owner's billing page and
`gh api users/revred/settings/billing/actions` (needs a token scope this
session did not have — that is why it is unresolved rather than answered).

Also worth one cheap test, because it is decisive either way: **push a trivial
commit and see whether a run appears.** If it does, the missed runs are simply
not backfilled after an outage and the whole thing is closed by re-pushing. If
it does not, the cause is live and account-level.

**File it as a loose end before you diagnose it.** It is not on the register
yet — this note is prose, and prose is the thing this project keeps filing
loose ends about (`LE-65`, `LE-70`).

## When a run does appear, expect red, and know which red is yours

- **`host-tests` may go red on its first Linux run.** This bench is Windows;
  `kernel`, `exec` and `shell` carry fixture bins gated `cfg(not(windows))` that
  no local gate compiles. That is `LE-64`'s family, it is expected, and `14D`
  landed it alone precisely so it would not surface inside somebody's merge.
- **`check-timing-regression` will name three unbaselined metrics.** That is
  `LE-23`, an owner decision declined by four independent sessions.
  `--update-baseline` is **not** the fix: it rewrites the whole file, replacing
  CI-runner rows with Windows-host rows and re-creating the exact offset the row
  exists to record.

**Red for any other reason is new and is yours.**

## Four traps, current

- **A gate that would not have caught the defect it was written for has not been
  checked against its own instance.** Twice now, on consecutive sessions:
  `LE-91`'s rule passed the defect it was filed for until rule 6 was added, and
  `check-ci-gates`' first version stayed green when `cargo test --workspace` was
  narrowed to `-p kernel`, because it had matched the step's display `name:`.
  Both were caught by mutating the **real file** and reading **which** error came
  back. A fixture contains only what its author already thought of.
- **A source-level scan matches its own text.** `metric_labels.rs` failed on its
  own doc comment and again on its own error string.
- **`check-boot-images` and `check-guest-images` are siblings** and running one
  is not running the other (`LE-72`, `LE-92`). `14D` correctly ran neither —
  it touched no board code. The next session that touches `kernel`,
  `hal-arm64`, `pi5-image`, `exec` or `shell` must run both.
- **A concurrent session can commit mid-turn.** It happened on 2026-08-06 at
  23:12:25, moving `HEAD` under a session that had staged nothing. A `git diff`
  that empties between two tool calls is a live session, not a bug.

## Do not start

`FEAT-P1-12` — the RT reserve. It has a name now, and that is the point: four
handovers called it *"`FEAT-P1-05`'s RT reserve"*, and the ambiguity is what let
it get started while listed as do-not-start. Also `G09`/`LE-86` and `06A` §4.3.
**Do not add design surface** — the hardware-evidence sprint rule from
2026-07-30 has not been lifted.

## The standing instructions

Stated in full at the end of
[`10C`](../hand-2026-08-06/10C-the-work-is-on-main-and-the-runner-is-in-an-outage.md),
with `12D` §8's tenth. The three most likely to save you an hour here:

- **A gate is only as strong as the weakest place it is actually executed.**
  `LE-100` closed on that sentence; the runner has not executed it yet.
- **Check what the code does on the machine before writing down what it does** —
  and this session extends it: *check that your instrument can return both
  answers before you believe the one it gave you.*
- **Build the unblocker rather than the next blocked artifact, and say so.**

## If you finish early

In order: **`EPIC-P1`'s Features table is still missing its `FEAT-P1-11` row**
(owner-approved on 2026-08-06, pre-existing drift, carried unactioned through
two handovers — summarise `FEAT-P1-11` as board-proven but not Complete and name
what it waits on). Then `LE-98`'s remaining half — the device-tree parse that
makes `SIMPLEFB_BASE` evidence rather than folklore, and removes the fault
path's named exception with it. Then the board checklist in
[`10C`](../hand-2026-08-06/10C-the-work-is-on-main-and-the-runner-is-in-an-outage.md)
§5 item 4, which is a checklist rather than a discovery and needs a hand on a
mains plug that nobody has yet.
