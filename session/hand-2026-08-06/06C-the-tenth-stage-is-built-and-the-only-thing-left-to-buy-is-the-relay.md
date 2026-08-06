# 06C — The Tenth Stage Is Built, and the Only Thing Left to Buy Is the Relay

Session handover, written 2026-08-06, directly executing
[`05C`](05C-the-loop-is-built-except-for-the-power-and-that-is-the-stall.md) §3.
Takes letter **`C`** after `05C` in the same tree.

`05C` measured the stall and named its one cause: **nine of the ten stages of
this project's board evidence loop are built and proven, and the tenth is a
human hand on a mains plug.** It costed the fix at about £15 and one C# tool.

**The tool is built.** `tos64-power` and `xtask board-run` exist, are tested,
and were falsified — eleven mutations, each seen to fail on its named test.
**What is left of `LE-95` is a purchase**, which is the one part of it a session
cannot do, and it is now the *only* part.

**The one sentence, if only one survives:** *the tenth stage is no longer
missing, it is unpowered — every ordering constraint and every fail-safe clause
on the mains path is a pure function with a test, and the first real run needs a
relay on the desk and nothing else from anybody.*

---

## 1. Why this session built the tool and did not take the boot

`05C` §6 rule 3 says **prefer the measurement to the mechanism when both fit the
session.** It does not fit: item 1 of `05C` §4 is a power cycle, this session
cannot perform one, and that is the entire finding of the document it executes.
Rule 1 — *no second unmeasured artifact while a first waits* — is the harder
one, and the honest reading is that it does not bite here either: the artifacts
waiting are waiting **on a board run**, and this is the thing that makes a board
run possible. Building a fourth blocked artifact would have broken the rule.
Building the unblocker is the rule's own purpose.

That is the whole argument, stated rather than assumed, because the next session
should be able to disagree with it if it thinks the rule was stretched.

## 2. `tos64-power` — the tenth stage

`work/tools/power/`, a C# console app under the `sdprep` pattern, per the
owner's standing rule that bench tools are C# and never scripts.

### The device, and why it is four dialects rather than one

`LE-95` fixes exactly one property: **controllable over the LAN with no vendor
cloud account.** That is a containment requirement rather than a preference — a
bench whose board cannot be rebooted while somebody else's service is down is a
*new* instrument failure, and this project has had five.

It is not a requirement for one specific product, so the tool speaks four
dialects and the bench buys whichever is in stock:

| dialect | command | readback |
|---|---|---|
| Tasmota | `GET /cm?cmnd=Power%20On` | `{"POWER":"ON"}` |
| Shelly gen1 | `GET /relay/0?turn=on` | `{"ison":true}` |
| Shelly gen2 | `GET /rpc/Switch.Set?id=0&on=true` | `{"output":true}` |
| ESPHome | `POST /switch/<id>/turn_on` | `{"state":"ON"}` |

Every one is a pure string function, because **a dialect that can only be read
with a plug on the desk cannot be reviewed before it switches mains.**

### The four fail-safe clauses, each a pure function with tests

`LE-95`'s owner path stated them as contract. They are pure — rather than
careful imperative code — for one reason: **a fail-safe whose only test is a
real board on a real relay is a fail-safe this project cannot afford to test.
It owns one Pi 5.**

1. **Never leave the board off.** `PowerPolicy.OwesRestore` over every cycle
   phase, three bounded restore attempts, then exit **4** — a code no other
   outcome shares. The subtle entry is `OffUnknown`: **an unconfirmed off is
   treated as off**, because "may be off" has to be, and reading it as
   *probably nothing happened, no restore needed* is exactly how a bench ends a
   session dark.
2. **Refuse to cycle mid-transfer.** `tos64-netboot` now writes a marker per
   *acknowledged block* — not once at the start, or a slow transfer reads as
   stale after ten seconds and the tool cycles straight through the middle of
   it. An **unreadable** marker is `Unknown`, and `Unknown` refuses. The
   staleness threshold is *derived* (five attempts × a 2 s receive timeout = the
   longest a live transfer can sit without progress), not picked, per the
   standing no-bench-tuned-constants rule.
3. **Bounds refused, not clamped.** Off-interval `[1000, 60000]` ms, on-wait
   `[1, 600]` s. `gem_receive`'s reason, verbatim: *a rounded bound is a grant
   the argument never made*. The lower bound is not style — below a second it is
   a coin toss whether the SoC saw a reset, and a cycle that did not reset the
   board yields a capture of the **previous boot**, which is a stale-evidence
   failure with no symptom.
4. **A readback decides and nothing else does.** Not the status code, not the
   absence of an exception, and **not the command's own response body.** That
   last one is the trap worth the paragraph: **Shelly gen2's `Switch.Set`
   answers `{"was_on":false}` — the *previous* state.** A tool that accepted it
   as confirmation would report `off → on` as done at the exact moment the relay
   did nothing. There is no `was_on` key in the reader, deliberately, and a test
   asserts its absence. This is `LE-87`'s lesson — *half a success reported as a
   success* — applied before the defect rather than after it.

### The seam, tested rather than declared thin

One impure component, `PlugClient`, and the 2026-08-03 standing rule
(`LE-66`: every declared-thin I/O seam gets scripted platform-semantics tests)
is discharged against a loopback fake plug built on `TcpListener` — **not
`HttpListener`, which needs a URL ACL or elevation on Windows, and a test that
only runs elevated is a test nobody runs** (`LE-92`'s lesson, one architecture
over).

Covered: an **empty 200** (the `Ok(0)` shape, in a new direction), a 500, a body
**truncated mid-flight** with a `Content-Length` that promised more, **nothing
listening at all**, a plug that accepts the connection and then says nothing
forever, and the POST leg. Every one comes back as a reply with a message —
never an exception — because **the caller may be holding the board off when it
returns, and an exception thrown through that path is a bench left dark by a
stack unwind.**

## 3. `xtask board-run` — and why the ordering is a value

The composition `05C` §3 asked for. `verify digest → serve → POWER CYCLE →
watch → parse → leave the board ON`.

The design decision worth reading: **`board_run::plan` is a pure function
returning a `Vec<Step>`, and the ordering constraints are host tests over that
value.** Not because a plan-as-data is tidier, but because **these orderings are
safety properties on a mains path, and a safety property expressed as the shape
of an imperative function is a safety property nobody can test.**

| invariant | why it is one |
|---|---|
| every plan ends with `EnsurePowerOn` | `off` is the one state a later session cannot recover from without a hand — the exact stall this removes |
| the digest is checked before anything is served | `LE-87`: a digest read while a server serves is a digest of whatever the file is halfway through becoming |
| the server is up before power moves | a Pi 5 that DHCPs into silence retries, and the retry window is not the window the capture was sized against |
| the watch is armed after the cycle; nothing is parsed that was not captured | a `parse-meas` over a stale file is the same class as a stale image — complete, plausible, entirely wrong |

`--dry-run` prints the plan and switches nothing, which is the only way to
review this on a laptop with no relay attached.

## 4. Verification, stated plainly

- **99 C# tests** (`power.tests`, the second test project for a bench tool),
  **9 Rust tests** over the plan, `netboot.tests` still **23/23**, all seven —
  now eight — C# tools build.
- **Eleven mutations applied and each seen to fail on its named test**, per
  `ADR 0005`: unconfirmed-off owes no restore; an unreadable marker reads as
  idle; the guard admits `Unknown`; the gen2 `Set` response believed; bounds
  clamped instead of refused; an `Unknown` readback counted as confirmation; the
  ESPHome readback sent as a POST; the beacon's separator changed under the
  reader; the restore pushed before the watch; the digest checked after the
  server starts; `parse-meas` emitted with nothing harvested.
- **The writer/reader mirror.** `TransferBeacon` (in `netboot`) and
  `TransferGuard` (in `power`) live in two programs, so `power.tests`
  references both and asserts the format, the file name and the round trip from
  **both sides** — the `LE-80` shape, applied because **drift here fails OPEN,
  and failing open on this seam is the power cut.**
- **Tests were written before the implementation**, file by file, and the
  falsification sweep is the substantive check on top.

## 5. What is honestly unproven — `LE-96`, raised here

**`board_run::execute` has never run against a plug, and neither has any
dialect.** The untested surface was made as small as it could be rather than
eliminated, because eliminating it needs the relay. Filed as `LE-96` with the
specific claims listed, so the first powered run is a **checklist rather than a
discovery**:

- that `dotnet run --project` passes the child exit code through unchanged —
  which is what distinguishes exit 4 (*the board may be dark*) from anything
  else;
- that killing the netboot child leaves UDP 67/69 clear for the next run
  (`LE-87` says that is the difference between a fresh image and a stale one);
- that `ti64dink --until` returns while the board is still beaconing;
- that the per-block marker is visible **to a separate process** in time to
  refuse a cycle — the guard is tested against strings, not against a live
  server.

A dialect that is wrong about real firmware fails at the **readback**, which is
the safe direction, but it fails as `UNKNOWN` rather than as *wrong dialect*.

## 6. The owner decision queue — one item shorter, and unchanged otherwise

`05C` §5 collected three so they could be answered in one sitting rather than
re-argued in a fourth handover. This handover does not re-argue them. **Decision
3's engineering half is done and only the purchase remains.**

| # | Decision | State after this session |
|---|---|---|
| 1 | Tier 0 baseline off the CI runner, and `min_cycles`/`p50_cycles` versus the ratios (`LE-23`, `LE-19`) | **Unchanged.** `check-timing-regression` still red on `main` |
| 2 | Does an `ADR 0005` Q1–Q4 campaign run for the Pi 5? (`LE-94`) | **Unchanged** — but Q3 is a residency campaign with a stated duration, and a bench that can cycle on demand is what makes it runnable at all |
| 3 | **Buy a LAN-controllable relay for the board's supply** | **This is now the whole of it.** The tool, the guards, the composition and the tests are in the tree; `LE-95` is `owned` and open on one purchase of about £15 |

## 7. The next session, in order

1. **The batched board session, still four deliverables and still unspent**, and
   this is now the *only* item that needs a hand: `03B` §6 item 1 (digest
   `b6dbabaea3431afa94cf9210374826bde9e5fb4efef7c5c861b92795c5006f02`, 298,089
   bytes — **start the server first, verify, then power on**; capture 60 s,
   `parse-meas`, expect `metrics=14`, file `PERF-D04-G23` and `PERF-D05-G23`
   with `D04`'s residue caveat and a large `D04` fail expected);
   `STORY-P1-09-16` criterion 4 (five `ti64dink --send` arms, **both** arms,
   and `notforus` must move neither counter); `STORY-P1-06-02` criterion 4;
   `LE-82`.
2. **If the relay has arrived: `LE-96`'s checklist, cheapest first.** A `cycle`
   with no image staged and no watch exercises both power legs and the exit-code
   path with nothing to lose if the board does not come up. Then a full
   `board-run`. Then the marker-visibility check on its own: serve an image,
   request it, and run `tos64-power off` against the live root expecting exit 1.
3. **`LE-91`**, unchanged and still the right mechanism before the 127. A gate
   filed through an unchecked labelling path is 127 chances to be wrong.

**Do not start:** `FEAT-P1-05`'s RT reserve, `G09`/`LE-86`, `06A` §4.3 — all
unchanged, all still correctly sized as owner decisions or Feature-sized work.
And **do not add design surface**: the hardware-evidence sprint rule from
2026-07-30 has not been lifted.

## 8. State at close

- **Gates:** `check-assurance-spine`, `check-spine-files`, `check-lints`,
  `check-citations`, `check-crate-sizes`, `check-image-size`,
  `check-boot-images`, `check-guest-images`, `cargo fmt --all --check` all
  green; full workspace suite green. All eight C# tools build; `netboot.tests`
  23/23, `power.tests` 99/99. **`check-timing-regression` RED, unchanged** —
  §6 decision 1's reason, untouched here.
- **Spine:** 31 Features / 99 Stories / 82 Tests / 62 Reports, **96 loose ends
  (48 open)**, **23 of 460** release gates carrying evidence, **0/99** Stories
  assurance-verified, 5 platforms **0 qualified**. **Evidence did not move this
  session either** — and it is worth saying plainly that this is the third such
  session, which is precisely why the thing built was the unblocker rather than
  a fourth blocked artifact.
- **Uncommitted.** Nothing committed, `git add -A` never used
  (`CONCURRENT_SESSIONS` rule 1). `03B`'s, `04C`'s and `05C`'s files remain
  uncommitted in this tree alongside this session's, so **stage by path**.
- **Bench:** no `tos64-netboot` running, UDP 67/69 clear. The staged image is
  still `b6dbabae…` (298,089 bytes), untouched by this session. **No plug on the
  desk.**

**And the standing instructions, all still holding:** do not report `x/460`
undecomposed — there is a subcommand. `PERF-Dnn-Gnn` is only meaningful if `Dnn`
is the domain of the thing you measured. Verify the digest and size the window
before you spend the boot. A gate written for one architecture, one tool, or one
direction does not generalise itself. **This session adds a fifth: when the
thing that blocks a session is a thing a session cannot do, build the unblocker
rather than the next blocked artifact — and say so in the handover, so the
choice can be argued with rather than inherited.**
