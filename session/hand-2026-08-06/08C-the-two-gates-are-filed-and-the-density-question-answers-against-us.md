# 08C — The Two Gates Are Filed, and the Density Question Answers Against Us

Session handover, written 2026-08-06, executing
[`07C`](07C-the-stall-broke-on-a-manual-power-cycle-and-the-stamp-costs-110-cycles-everywhere.md)
§7 in order, in the same tree, with **no plug and no board run**. Every item
below was executable on a laptop, which is what `07C` promised and it held.

**The one sentence, if only one survives:** *`PERF-D05-G23`'s 6.7% overhead
does have a denominator in shipping code — the park loop's dispatch round and
the fixture's are the same construction with the same one-stamp-per-round
density — so `D07`'s escape does not transfer, and the only thing making it
harmless is that the beat is 1 Hz.*

---

## 1. The two gates are filed. Evidence moved for the first time since `02A`

`PERF-D04-G23` and `PERF-D05-G23` are in
[`goals/assurance/guardrail-evidence.tsv`](../../goals/assurance/guardrail-evidence.tsv),
both `measured`, both recorded as the fails they are. The capture that was at
`c:\tmp\env-2026-08-06.txt` is now committed at
[`goals/reports/wire-meas-envelope-2026-08-06-spoor-pairs.txt`](../../goals/reports/wire-meas-envelope-2026-08-06-spoor-pairs.txt)
— a separate file from the same day's `wire-meas-envelope-2026-08-06.txt`,
which is a different boot with twelve metrics and the old `D07` spoor labels.

`xtask parse-meas` reads it, reports `metrics=14`, and then says
`no usable verdict line`, which is `LE-76` and was expected (`07C` §6).

**Release-gate evidence: 23 → 25 of 460.** The first movement in four sessions.
It cost no board and no owner decision, exactly as `07C` §2 said it would, and
the *measured-and-unfiled* failure mode did not survive one session.

Both rows carry the caveats `07C` asked for. `D04`'s says on the row that 82
cycles is the smallest denominator in the tree and that the same stamp reads
+6.7% and +24.6% elsewhere in the same boot, so nobody without this document
quotes 134% as an instrumentation cost. Both carry the residue and pairing
caveats, and one thing the arithmetic made visible that no handover had noticed:

> **The 110-cycle delta is smaller than the stamp's own standalone cost.**
> `PERF-D11-G01` measures `SpoorStream::stamp` at p50 136 in this same boot, and
> the delta it adds to a context-switch round trip is 110. The stamp partly
> overlaps the switch, so **136 is an upper bound, not an addend** — anyone
> summing 136 per stamp into a path budget is over-charging it.

## 2. The `D05` density question, answered — and the answer is the awkward one

`07C` §1 left this open and costed it at *"a read of the park loop, not a
board."* That is what it cost. The answer:

**`PERF-D07-G23`'s escape does not transfer, and `D05`'s ratio is real.**

`D07`'s filed note escapes its 26% by observing the shipping park loop contains
no pool traffic at all, so the gate's ratio has no denominator in shipping code.
For `D05` the opposite holds, and not approximately:

- [`kernel::board_dispatch::tinyos_dispatch_round`](../../os/src/kernel/src/board_dispatch.rs)
  stamps `Rung::DispatchRound` **exactly once on every one of its three exits**
  — dispatched, dispatched-but-not-ready, uninitialised.
- [`measure_phases::phase_dispatch_round_spoored`](../../os/src/kernel/src/measure_phases.rs)
  stamps **exactly once per round** inside the timed region.
- The two rounds are the *same construction*: `Scheduler` of capacity 4
  (`BOARD_TASKS` == `measure_phases::TASKS`), one task at priority 11
  (`BOARD_PRIORITY`), `WcetBudgetTicks(1000)`, a yield-forever entry point that
  switches straight back, `dispatch::run_once`.

So the fixture arm is a **replica** of the shipping stamp site, not a scaled
model of it, and `07C`'s hedge — *"the park loop's round is nearly idle and the
fixture's is 1650 cycles"* — does not survive the read. The fixture's round is
also a single task that yields immediately. 1650 cycles is what that costs.

### What saves it is cadence, and the gate's two clauses split

`G23` reads *"adds <= 2% p99 **and** <= 2% CPU cycles"*. Those two clauses now
disagree by seven orders of magnitude on the same physical act:

| clause | shipping value | verdict |
|---|---|---|
| p99 overhead per round | +110 cycles on 1650 = **+6.7%** | **fails by 3.3×** |
| CPU cycles | 110 cycles/second on 2.4 GHz | **passes, ~1 part in 22 million** |

`hal_arm64::ethernet`'s park loop waits 100 ms per tick and runs its stamping
body only on every tenth tick, so the beat is 1 Hz and the board executes **one
dispatch round per second**. The per-round ratio is genuinely breached and the
CPU-cycles clause is not remotely close to breached, and both statements are
about the same 110 cycles.

**Filed as `LE-99`**, because the load-bearing part is a constraint on the
future and not a defect in the present: *nothing guards the assumption.* No
test, no gate and no comment ties the park beat to this budget. Raising the
cadence, adding a second stamped call site inside the round, or landing the
preemptive scheduler `D05` readiness already anticipates would breach it
silently — with no symptom on the wire, which is this project's recurring shape.

Do **not** chase the 110 cycles. Three independent pairs measured 110/110/117 in
one boot; `G23` constrains density.

## 3. `LE-97` — closed, and it grew a second defect on the way

All three of `LE-97`'s owner-path items are done and the row is `closed`.

**`tos64-netboot` refuses to start rather than falling back.** The decision moved
into [`work/tools/netboot/ServerAddress.cs`](../../work/tools/netboot/ServerAddress.cs),
a pure function with the NIC walk separated out, so it is testable without a
bench, a board or a power cycle (`LE-66`'s seam rule). Six outcomes: `Named`,
`Discovered`, and four refusals — `Malformed`, `Unusable` (an explicit
`0.0.0.0`, refused on the way *in* as well as out), `NoCandidate`, `Ambiguous`.
**Every refusal carries no address**, asserted over all of them at once, and
every refusal names `--server`, because a stop with no next step gets worked
around rather than fixed.

Ambiguity is refused, not resolved. This host has four link-local addresses and
first-one-wins fails with a *plausible* address in the log, which is strictly
harder to catch than `0.0.0.0`.

The deferred re-guess inside the DHCP loop is **deleted**. It existed because
startup could produce `Any`; it was a second mechanism for one decision, and the
one that fired late could still pick wrong. There is now exactly one place that
decides, and it decides before any port is claimed.

### The defect the test found on its own

`IPAddress.TryParse("169.254.113")` **succeeds**, and yields `169.254.0.113`.
.NET still honours the historical shorthand forms, so a truncated address does
not fail — **it becomes a different, valid, wrong address**, which the tool
would then have printed in the same confident column. That is `LE-97`'s own
shape one layer down, and it was found by a test written to assert the boring
case. Both sides of the seam now require four dotted octets, and each checks it
rather than trusting the other, because they are separate programs and nothing
links them (the same reason `POWER_EXIT_LEFT_OFF` is duplicated by value).

**`board-run` requires `--server=`**, and the argument is not caution — it is a
consequence of an ordering the tests already pin. `plan` puts `StartNetboot`
before `PowerCycle`, so at the moment the server starts **the board is off and
the bench NIC has no link**. Discovery cannot work on this path, by
construction. `BoardRun::server` is therefore not an `Option`,
`board_run::server_address` validates the value **before the plan runs** (a bad
one otherwise costs a wasted mains cycle), and `execute` passes `--server`
through. `describe` prints it, so the plan an operator reviews before power
moves shows the address it will serve from.

On the automated path there is no human reading the printed line — which is the
only thing that saved the 2026-08-06 boot.

### Three things review caught in the first cut, all of them `LE-97`'s own shape

**`--server` was validated for syntax and never for existence.** Four octets,
digits, 0–255, not `0.0.0.0` — and `--server=169.254.113.249`, one digit off,
passed all of it, entered the `OFFER`, and the board fails to fetch looking like
a board fault. `Choose` was already computing `Candidates` and throwing them
away on the `Named` path. It now compares, and **warns rather than refuses**,
because `LE-97`'s own ordering trap says there is nothing to compare against on
the run that matters: *when there is evidence, use it; when there is none, stay
quiet.* The existence check enumerates **all** host IPv4 addresses, not the
link-local subset, or a bench on a real subnet is warned on every correct run
and learns to ignore the one warning that counts.

**Say what this does not cover.** On a *cold* `board-run` the server starts
before power moves, so nothing is enumerable and the check is silent — exactly
the unattended run a typo is worst on. It fires on the interactive path (the
2026-08-06 recovery shape) and on any `board-run` following another, since every
run leaves the board ON. Closing the cold case needs evidence that does not
exist at that moment: a recorded bench address, or a first run that learns one.

**The two paths disagreed about what an address may be.** Discovery filtered
hard to 169.254/16 with a test asserting `192.168.1.20` is not a candidate,
while `--server 127.0.0.1` was accepted — the board told to fetch from *itself*
— as were `255.255.255.255` and `224.0.0.1`. Routable unicast stays allowed and
that is deliberate; loopback, `0/8`, multicast, reserved and the limited
broadcast are now `Unusable`, which is the category the enum's own words already
described. Both sides of the seam.

**`Explain()`'s acceptance arms were unreachable**, called only under
`if (!choice.CanServe)` while the success line printed the raw outcome — dead,
untested strings in a file whose entire subject is a tool saying what it chose.
It is printed on every path now.

**And the mirror test that `08C` argued for and did not build.** The four-octet
and `0.0.0.0` rules were duplicated across C# and Rust with the duplication
justified (`POWER_EXIT_LEFT_OFF`'s precedent) and then left unasserted — which
is precisely what `TransferBeacon`/`TransferGuard` got a mirror test to avoid.
[`server-address-cases.tsv`](../../work/tools/netboot/server-address-cases.tsv)
holds 22 cases; both suites read it and assert the same accept/refuse verdict,
and both fail if they read fewer than 20 rows, because *nothing was wrong* and
*nothing was looked at* are different results.

It found a real disagreement immediately: a leading-zero octet. `.NET` rejects
`169.254.0.01`; the Rust side accepted it and would have said yes to a value the
server then refuses — after power had moved. Both refuse it now.

Tests: `netboot.tests` **23 → 49**, `board_run` **9 → 20**.

## 4. `LE-98` — the cheap half only, and it stays open

`config.txt` is generated by the build (`pi5::CONFIG_TXT`, pinned by test), so
`hdmi_force_hotplug=1` was added there and to the placement instructions, which
also now name HDMI0 and the power-on ordering. **No rebuild**: `kernel8.img` and
its digest are untouched.

**Read what this line claims, because it is deliberately the weaker claim.** It
is *not* that the monitor comes back — a display cabled to HDMI1 is not helped by
forcing HDMI0's hotplug, and nothing here has been seen on a board. It is that
**the firmware allocates a framebuffer at all**, so the fixed physical address
the kernel paints into is memory the firmware owns rather than memory nobody
assigned. A 4 MB write to a constant physical address on a machine with no
IOMMU is a safety question, and safety precedes correctness here.

It is unverified on Pi 5 firmware and it cannot make a working display stop
working (an unrecognised key is ignored). **`LE-98` stays open**: the real fix is
that the paint must be conditional on evidence — read the framebuffer back, or
take the address from the firmware rather than from a file, and refuse to paint
when neither is available. `STORY-P1-09-16` criterion 4 is still blocked on it.

While there: `tos64-sdprep` printed `config.txt (os_check=0,
kernel=kernel8.img)` — two of what are now four lines. It reads the staged file
now. Same lying-by-omission shape as `02A`'s two bench tools, found by looking.

## 5. One thing done that was not on the list

**The stale `tos64-netboot` from `07C` was still running** (PID 26668, started
20:24, holding UDP 67/69 and the build output). `07C` §8 said to stop it before
the next run or `LE-87` collects its second instance. It is stopped. It was
found because it blocked a `dotnet build`, not because anyone remembered — which
is worth noticing, since the reason `07C` wrote that line down is that nothing
else would have caught it.

## 6. The next session, in order, still with no plug

1. **`LE-99`'s guard** (§2). Pin the park beat's tick divisor and the stamp count
   inside `tinyos_dispatch_round` together, so raising the cadence or adding a
   stamp fails a host test rather than a board run nobody re-measures. Laptop
   work, and it is the only thing standing between `LE-99` and rediscovery.
2. **`LE-98`'s real half** (§4). The paint must be conditional on evidence. This
   is the one with a safety argument behind it, and `STORY-P1-09-16` criterion 4
   is behind it too.
3. **`LE-91`**, unchanged and still the right mechanism before the 127.
4. **The cold-`board-run` typo case** (§3). The existence check is silent on a
   run that powers the board itself, because nothing is enumerable before the
   link comes up. It needs evidence that does not exist at that moment — the
   cheapest source is the bench's own address recorded once and checked against,
   which is a config decision rather than code.
5. **When the board next runs**: `hdmi_force_hotplug=1` and HDMI0 are worth one
   power cycle to confirm, and `LE-96`'s remaining half — a *separate process*
   seeing `.tos64-transfer` *during* a transfer — is catchable on any boot.

**Do not start:** `FEAT-P1-05`'s RT reserve, `G09`/`LE-86`, `06A` §4.3. The
hardware-evidence sprint rule from 2026-07-30 has not been lifted, and no design
surface was added here.

## 7. State at close

- **Gates:** `check-assurance-spine` green, `check-boot-images` green,
  `check-guest-images` green, workspace suite green, `cargo fmt --check` clean.
  `check-timing-regression` RED and unchanged, for the `LE-23` owner decision.
  All ten C# projects build; `netboot.tests` 49/49, `power.tests` 99/99.
- **Spine:** 31 Features / 99 Stories / 82 Tests / 62 Reports, **99 loose ends
  (50 open** — `LE-99` raised, `LE-97` closed**)**, **25 of 460** release gates
  carrying evidence.
- **Bench:** board left **powered and beaconing** — untouched this session. The
  stale `tos64-netboot` is stopped, so UDP 67/69 are clear for the next run.
  **No plug on the desk.**
- **Uncommitted.** Nothing committed, `git add -A` never used
  (`CONCURRENT_SESSIONS` rule 1). `03B` through `08C` are all uncommitted in this
  tree, so **stage by path**.

**The standing instructions, all holding.** Do not report `x/460` undecomposed.
`PERF-Dnn-Gnn` is only meaningful if `Dnn` is the domain of what you measured.
Verify the digest and size the window before you spend the boot. A gate written
for one architecture, one tool or one direction does not generalise itself.
Build the unblocker rather than the next blocked artifact, and say so. A tool
that prints the value it chose is not the same as a tool that refuses a value it
cannot justify.

**And a seventh, from this session:** *an escape argument is a property of one
denominator and does not generalise to the next one* — `PERF-D07-G23` escaped
its 26% because the shipping path contains no pool traffic, and the identical
argument was one source read away from being wrong about `D05`, where the
shipping path contains the stamp site exactly.
