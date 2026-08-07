# 08G — The SWOT answered: the tree is one again, the review the marathon owed is paid, and the record's expiry has a tripwire

Session letter **G**, opened by the owner with session F's SWOT and the
instruction to address it — then widened mid-session to *address all
opportunities and mitigate risks proactively*. Sessions D and F had both ended;
their entangled uncommitted work was the tree this session inherited.

**The one sentence, if only one survives:** *The coordinated commit both
sessions owed is on `main` (`ae55ab7`, 21 files, every file read before it was
staged), the review pass caught and fixed the one defect the two sessions'
halves made together — a hard-coded Story population that went stale the moment
they merged — and the qualification record's self-declared void condition is now
a register row (`LE-117`) instead of an unwatched sentence.*

## 1. Threat 1 discharged: the coordinated commit

`07F` §7 named the debt and `06D` held the other half. This session read **every
file in the pending set** (`CONCURRENT_SESSIONS` rule 3) — the three marathon
artifacts in full, every register diff row by row, every Rust hunk, the 445-line
libsqlfs note — and landed the whole set as one commit. `git add` was by
explicit path; the hook re-ran the gates on the staged tree; nothing was left
unstaged, so the `git add -A` window is closed.

**The review pass earned its keep before the commit existed.** The full xtask
suite failed on `dashboard::tests::the_committed_page_states_the_current_story_population`:
session D's LE-108 test pinned `p0p1_stories` at a literal **85**, and session
F's `STORY-P1-13-01` made the regenerated page correctly say **86**. Neither
session was wrong alone; the defect only exists in the union — exactly the
collision class the SWOT's first threat predicted, caught at the last gate
before CI would have gone red on the runner. The fix is durable rather than a
re-sync: the test now derives its facts from
`assurance::dashboard_facts(repo_root)`, the same walk the gate uses, because a
hard-coded population count in a test is the identical defect to the five
hand-synced spine counts of 2026-07-28. *A count of how much work exists is a
floor, never a total* — now including in test literals.

## 2. Weakness 5 discharged: the review the marathon artifacts were owed

- **`REPORT-2026-08-07-01` holds.** Its first substantive sentence refuses the
  over-claim ("does NOT qualify by this record"); Q3 is presented as instrument
  proven, campaign explicitly not run; the silicon positive control is named as
  the campaign's first step, not skipped; the determinability boundary is
  stated in the ADR's own terms; every Q2 claim carries a citation a reader can
  follow. One residue found, and it is filed rather than prose: see §4.
- **`FEAT-P1-13` holds.** It defers the containment decision to the Story
  instead of presuming shape 1; its non-goals explicitly protect
  `FEAT-P1-07` §6 and refuse general DT discovery; the acceptance shape puts
  the answer on the wire (`fb_addr=… src=dtb|constant|refused`). Nothing to
  amend.
- **`STORY-P1-13-01` holds**, and its own warning is why this session did not
  start it: the containment decision is a judgement against the matrix row's
  exact text, and taking it as the last item of a coordination session is the
  rushing the Story forbids. It is the next substantive session's first-class
  work, with the sprint rule already lifted for it.

## 3. What this session found live: the cwd trap, a third and fourth time

`07F` §6 recorded a background `dotnet run` from `os/` producing an empty log.
This session hit the same class twice in its first hour — a shell whose working
directory did not survive between tool calls, and a `cd os` issued into a shell
already sitting in `os/`. Neither cost evidence this time; both are the same
lesson: **absolute paths for every bench and build command, no exceptions, and
treat a file-not-found from a relative path as this trap until proven
otherwise.**

## 4. Threat 3 mitigated: `LE-117`, the record's expiry tripwire

The SWOT's sharpest *silent* threat: the qualification record is void on any
EEPROM change, Q2 rests on the published stub tree rather than a reproducible
build, and an unnoticed bootloader update invalidates the record with no signal
anywhere. That was a sentence inside the Report; it is now **`LE-117`** — open
defects live in the register, not in prose. The row asks for two cheap halves:
a bootloader-version read against the Report's pinned hash in the board-session
runbook preamble (beside `LE-87`'s stale-server check), and a deliberate,
recorded decision on whether the bench card's Pi OS role may auto-update the
EEPROM at all. It explicitly does *not* ask to freeze firmware — updating is
allowed; it costs a new record; the row exists so the cost is noticed.

Register is at **117 loose ends (58 open)**; dashboard sentence, tabstrip and
feasibility page regenerated and gate-verified against it.

## 5. Opportunity 1 prepared: the Q3 campaign, specified so the next bench session executes instead of designs

What follows is a **proposed protocol**, written from `REPORT-2026-08-07-01`
and ADR 0005's own clauses; the deciding word on shape belongs to whoever runs
it. It exists because the gap to the first qualified platform is exactly this
campaign, and the expensive resource — an owner at the bench, or `LE-95`'s
relay — should be spent executing, not deriving.

1. **Silicon positive control first, per the ADR's trap clause.** The Q2
   determination itself supplies the injectable perturbation: PSCI-over-SMC is
   live, and an SMC is the one documented synchronous entry into EL3 on this
   platform. A fixture arm that issues a benign SMC (e.g. `PSCI_VERSION`)
   *inside* a residency window must see the excursion — physical-counter
   advance the window cannot account for. If the probe does not see a real EL3
   round-trip, no zero it ever reports is a measurement. One boot, one wire
   line, and the instrument is proven able to say the other answer on silicon.
2. **Then the campaign, stated before it runs:** duration (propose ≥ 60 s of
   accumulated window time), sample count (propose ≥ 1,000 windows at the
   proven 540,000-tick size), distribution reported (min/p50/p99/max of
   unaccounted physical ticks per window), environment recorded as it is
   (idle bench, netboot, ambient — *stated*, not controlled), and the largest
   observed excursion quoted against whatever bound is then claimed.
3. **Both arms ride the existing verdict channel** — `TOS64-QUAL/1` lines plus
   `TOS64-RESULT/1`, so the capture parses to its own pass/fail like every
   capture since `07F`.
4. **`LE-117`'s tripwire runs in the same session's preamble** — the record
   this campaign completes is pinned to the EEPROM it was measured under.

Boots needed: one new image (both arms), plus power cycles. With the owner on
the bench that is one sitting; with the relay it is unattended.

## 6. Opportunity 4 prepared: which board criteria need a boot, and which need only a listen

`LE-110` asked every handover to say this; the current split of the live set:

- **Listen only (no power cycle, no relay):** re-reading committed and live
  captures against unfiled gates (`LE-104`'s read-don't-measure work); the
  park-beat cadence *after* `LE-115` lands its timestamp; any re-harvest of the
  current boot's replayed transcript.
- **New boot required:** the Q3 campaign above (new fixture arms);
  `FEAT-P1-11` criterion 3's `Skipped`/`Failed` arms; anything touching
  `TOS64-DISPLAY/1`'s future `fb_addr` field (`FEAT-P1-13`, board half);
  investigation of the two §7 findings below if their wire lines need new
  instrumentation.
- **Owner decision, not a boot:** dispatching `LE-116`'s runner-side baseline
  job; buying or not buying `LE-95`'s relay; `LE-23`.

## 7. Opportunity 3 routed: the two silicon firsts are owed to named owners

Recorded in `07F` §7c, restated here so they route rather than fade:
**`TICK RMAX=10000`** is the first live observation of ADR 0015's
masked-region condition and belongs to that ADR's owner before any timing
number near D02 is filed; **`RX STATE=STOPPED REASON=NOBUFFER ACCEPTED=0
REFUSED=0`** is the inbound arm's first observation on any channel and is owed
a disposition by `STORY-P1-09-16`'s owner. Neither is filed as a loose end:
both are evidence awaiting their owner's reading, not defects — filing them as
rows would pre-judge exactly the reading that is owed.

## 8. The remaining SWOT items, honestly dispositioned

- **Lit canvas ≠ display fixed (threat 2):** unchanged and correctly so.
  Scanout bring-up is nondeterministic, the size query answers on dark boots,
  and the durable fix is `FEAT-P1-13`. The bench procedure that lights the
  canvas is `07F` §7b. Nothing further to do until that Story's session.
- **xtask absorbing gates (threat 4):** `LE-113` part one now prints headroom
  on every run — today's figure is **14,732 counted lines, 5,268 headroom**
  (the coordinated commit itself added ~300). Part two, the seam decision, has
  roughly two comparable sessions of runway; it should be *decided* (not
  necessarily executed) in the next session that is not mid-firefight, while
  the choice is still calm.
- **ti64dink's untested changes (weakness 3):** stands as `LE-114` states it —
  the bench tool with the worst defect history has no test project behind its
  newest changes. Land the `dotnet test` CI job alone, as the row says, and
  put the harvest filter under test in the same change.
- **Publishing a hypothesis before its discriminator (weakness 2):** `07F`
  §7a/§7b is the worked example. The cheap rule this session carries forward:
  *when a discriminating experiment costs one boot and the hypothesis costs a
  committed paragraph, run the boot first.*
- **`LE-115` (weakness 4):** open, small, correctly dropped then; unblocs the
  listen-only cadence measurement above when someone touches ti64dink anyway.

## 9. Housekeeping

- The session index (`index.html`) had stopped at `05E` and its `<title>` still
  said 6 August: `02D`, `03D`, `06D`, `07F` and this document are now indexed
  and the title corrected — the front-door-drift class (`LE-107`'s family) in
  the session folder itself.
- Gates at close: spine green at 33 Features / 101 Stories / **117 loose ends
  (58 open)**, feasibility agreeing, crate sizes with headroom printed, xtask
  suite 396 green, fmt and per-crate lints clean. No `kernel`/`hal-arm64`/
  `pi5-image`/`exec`/`shell` sources were touched, so `check-boot-images` and
  `check-guest-images` were not owed by this session's changes.

## 10. Standing instructions, one addition

All previous hold, including `07F`'s *an automation gap is not an availability
gap*. The one this session earned: **a count in a test is a count in a
register** — every rule this repository has about hand-synced totals applies to
test literals, where they hide better and fire later, and the fix is the same
(derive from the walk that owns the number).
