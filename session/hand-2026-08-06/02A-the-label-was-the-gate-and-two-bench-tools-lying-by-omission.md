# 02A — The Label Was the Gate, and Two Bench Tools Lying by Omission

Session handover, written 2026-08-06. Follows
[`01A`](01A-the-decomposition-is-code-now-and-the-first-instruction-it-refutes.md).

**`01A` left a three-item mandate. Items 1 and 2 are done and are the whole of this document;
item 3 is deliberately not started, and §5 says why the finding that came out of item 2 changes
what item 3 is worth.** The one sentence to carry forward: **a metric's domain label is not a
name, it is the choice of which target column the number will be read against** — and for two
days three spoor metrics carried a label that pointed away from their own gates.

---

## 1. `LE-87` is closed, and it reproduced itself while being fixed

Mandate item 1, ~30 minutes as costed. `tos64-netboot` now **refuses to start when either of its
ports is held**, and logs **the served file's sha256 and absolute path on every transfer**.

- **Both ports are bound in `Main`, without `SO_REUSEADDR`, before either loop's thread starts.**
  That ordering is the fix, not a tidy-up: binding inside each loop is precisely how the tool
  could answer DHCP correctly while a stale instance served TFTP. One half succeeded, the other
  half was never reached, and every visible signal said the run was good.
- **A held port ends the run with exit code 3, naming every holding PID and process name**, read
  from `netstat -ano -p UDP` — `netstat` and not `Get-NetUDPEndpoint`, because on the run that
  produced `LE-87` netstat showed two PIDs and `Get-NetUDPEndpoint` showed one, which is why the
  first look did not find it. The bind is what decides; netstat is only diagnosis, and an empty
  holder list is reported as *"could not tell"*, never as *"nobody"*.
- **The full digest, not a prefix.** The question the line answers is a comparison against a
  digest the operator has in another window.

**And then the defect happened again, in front of the fix.** Mid-session two more pre-fix
instances were started ten seconds apart and both held UDP 67 **and** UDP 69 simultaneously —
`netstat` showing four rows for two processes. It was not a re-creation for the record; it was
the bench doing the thing this row exists for while the row was being written.

### The tool had no tests, and now it has 23

`work/tools/netboot.tests` is **the first test project for any bench tool under `work/tools/`**.
`LE-66`'s finding, verbatim in effect: *a seam with zero tests is not thin, it is untested*, and
this tool is three seams — UDP bind, TFTP path resolution, file service — of which every defect
it has ever produced lived in one.

The one worth naming is the **platform semantic**, stated as a test rather than remembered:

| test | what it pins |
|---|---|
| `ReuseAddress_lets_a_second_bind_of_a_held_port_succeed_silently` | the defect: Windows lets the second bind through, with no error anywhere |
| `Exclusive_bind_of_a_held_port_fails_with_AddressAlreadyInUse` | the fix, and the one error code worth branching on |
| `Two_holders_of_one_port_are_both_reported` | the observation that diagnosed it — a parser returning *the* holder would have called the collision an ordinary listener |

**Both detectors were seen to fail before being trusted**, per `ADR 0005`: reinstating
`SO_REUSEADDR`, and reinstating the naive `Path.Combine`, turned **exactly five tests red** — the
exclusivity one and the four `LE-88` spellings — and nothing else. The `LE-88` fix, made live on
the bench yesterday, had never been held by a test until now; it is.

**Nothing runs them automatically, and that is stated rather than left to be discovered.** CI
compiles no C# at all, and no `xtask` gate knows `work/tools/` exists, so these 23 tests run when
someone types `dotnet test` in `work/tools/netboot.tests`. That is still strictly better than the
zero tests the tool had, but a reader should not infer a gate from a test project.

Verified live in both directions as well as in tests: one instance starts and prints `ports held
: UDP 67 (DHCP) + UDP 69 (TFTP), exclusively`; a second exits 3 naming the first; `/config.txt`
serves 45 bytes at `acde9d81…` and `/kernel8.img` 295,897 bytes at `05f3495c…`, **both matching
`sha256sum` exactly**; `../../../secrets.txt` is still refused.

## 2. `LE-90` — the log never reached the file, which is why one server could hide from another

Found while verifying §1 and worth more than the bug it interrupted. **Redirected stdout in .NET
is buffered at 4 KiB and flushed on exit, and a netboot server does not exit.** A bench session
that redirects the tool to a log and reads it in another window saw **the first five banner lines
and nothing after them** — 240 bytes on disk with the process demonstrably live and more than 240
bytes written. Not the DHCP exchange, not one TFTP request, and not the digest line §1 had just
added, all of which exist to be read *while the board is booting*.

Fixed: `Main`'s first statement replaces `Console.Out`/`Console.Error` with `AutoFlush` writers.
The same redirect now shows the whole banner two seconds in and every request as it happens.

**This is `LE-87`'s own cause, one level down.** An operator comparing two servers' logs could
only ever see one of them — whichever had most recently exited. It stays **open** on one point:
six other C# bench tools live under `work/tools/` and none was audited for the same buffering.

**Three-for-three is now four-for-four.** `LE-80` reported a live event as an absence, `LE-81`
died on the first packet it answered, `LE-87`/`LE-88` reported success for the half they did, and
`LE-90` did the work and never filed the report. Suspect the instruments before the board.

## 3. The mislabelled domain: three metrics, two days, and a gate nobody read

Mandate item 2, and the highest-yield thing available. `spoor_stamp_park_rung_per_op_of_8`,
`spoor_drain_full_ring_frame_of_181` and `spoor_announce_certificate_frame_of_3` were emitted with
`domain=D07`. **`D07` is fixed-capacity pool allocation. `D11` is "spoor stamp and journal",
which is exactly and only what those three measure.**

The label was `D07` because `STORY-P1-10-02`'s contract selected only `D07` — **the metric bent
to fit the contract instead of the contract extended to fit its subject.** The consequence was
not cosmetic. Those numbers sat on the wire from 2026-08-05, were quoted in a Report, a handover
and the Story's own status header, and **were never once compared to `D11`'s targets.**

Read against them, at `cycles_per_us=2400` on the 2026-08-06 boot:

| gate | target | measured | verdict |
|---|---|---|---|
| `PERF-D11-G01` p50 | ≤ 0.03 µs | **0.0571 µs** | **fails by 1.90×** — filed `measured`, as the fail it is |
| `PERF-D11-G02` p99 | ≤ 0.06 µs | 0.0596 µs | under by **0.7%** — filed **`refused`** |
| `PERF-D11-G03` p99.9 | ≤ 0.1 µs | 0.0600 µs | met with 40% of room — filed `measured` |

**`G02` is refused for `PERF-D03-G20`'s reason and it is the point of that kind existing.** A
0.7% margin is a quarter of the ~3% build-to-build movement `BOARD VERDICT 9` measured on
untouched code; this metric's p99 was 141 and 142 cycles on two 08-05 boots, and 142 still passes
while a rebuild 3% slower does not. **A verdict that flips on a recompile is not a verdict.**
`G01` needs no such caution — 1.9× is outside any noise this bench has shown.

**Read the three together and the shape is informative:** min 131, p50 137, p99 143, p99.9 144,
`n=1000 dropped=0`. The distribution is extremely tight, so **the substrate's problem is its
median, not its tail** — consistently expensive rather than occasionally so, which is the more
tractable of the two: there is no rare path to hunt, only a hot path to make cheaper.

Method stated with the number, because the verdict depends on it: the timed region is
`SpoorStream::stamp` alone with no drain and no wire in it; the certificate is closed by an
untimed first stamp so the once-per-boot retain path is not measured; eight stamps are timed per
sample and divided out (`LE-24`); and the calibrated 43-cycle source overhead is subtracted from
the region *before* the division — so at most ~5 cycles of residue can be in the figure, and
removing every one of them still fails `G01` by 1.8×.

What changed, in four places: `STORY-P1-10-02`'s contract now selects `D07,D11` (`D11` is
`prototype` readiness, so **no debt row**); `TEST-P1-10-02-A`'s metadata follows; the fixture
emits `D11` with the reason recorded at the `collect` site; and the Story's status header and
named debt say what fails and what nothing here closes.

**Evidence moved 21 → 23 of 460** (the refusal does not count), and `assurance-status`'
measurable-today bucket moved 129 → 127.

### `LE-91`: the mechanism, because three labels fixed by hand leaves the fourth free to be wrong

**Nothing machine-checks which domain a fixture metric is labelled with.** That is the defect
class; §3 is one instance of it. This is `09A` §5 one level up — that rule says *read the target
column before measuring*, and this says **you cannot read the right target column if the domain
label is chosen by what the contract already selects.**

The honest obstacle, recorded so nobody builds the wrong check: *fixture domains ⊆ owning Story's
contract* **is not the rule and would be wrong if asserted** — `fixture_measure_arm64` alone
emits `REF`, `D02`, `D04`, `D05`, `D07` and `D11` for several Stories, while `list-fixtures` maps
a whole fixture to one owning `TEST-*`. What is buildable: declare each metric's domain **and its
owning Story** at the `collect` site, have `xtask` parse the `collect` calls out of the fixture
source so the declaration cannot drift from the code the way `LE-80`'s mirror did, and assert the
domain is selected by that Story's contract. One session, and it closes a class.

## 4. Not started, deliberately: mandate item 3

The `G23` paired-arm method for `D04` and `D05` is untouched. It is half a day, it needs a build
*and* a board boot to yield anything, and starting it here would have left it half-built with the
board's netboot server stopped. **The pattern is proven and the next session should just do it** —
`phase_pool_alloc_free_batched_spoored` is the template, one new gate per pair, both arms in one
boot. One thing to check first: `LE-89` made `TRANSCRIPT_CAPACITY` derived, but two more metrics
takes the envelope to 14 lines, so read `MAX_LINES` before adding them rather than after.

## 5. But read §5 before costing item 3 — the density question is answered, and it was free

`01A` §7a asked the right question with the 26.1%: *what is the real stamp density on the
shipping park loop?* **It is about five stamps per second, and no board was needed to find out —
only reading the loop.**

`hal_arm64::ethernet`'s park loop waits 100 ms per tick and runs its stamping body only on every
tenth tick, so **the beat is 1 Hz**. One beat stamps `ParkIteration`, `ThermalSample`,
`BeaconTransmitted` when beaconing, and at most two more inside `tinyos_dispatch_round`: about
five stamps of 137 cycles, so **~685 cycles per second on a 2.4 GHz core — under one part in
three million of CPU time.** And **the beat contains no pool traffic at all**, so the ratio
`G23` measures has no denominator anywhere in the shipping path.

**This does not soften the fail.** `G23` asks what instrumentation costs when it is dense, and
the answer is 26.1%. What it fixes is what the number is *for*: **a budget for whoever next puts
a stamp inside a hot loop — one stamp per ~14 operations — not a description of today's
overhead**, which is four orders of magnitude below it. Recorded on the `PERF-D07-G23` row rather
than here, because prose is where the last density claim would have gone unread.

Which is also the honest note on item 3: `D04` and `D05` paired arms will produce two more
worst-case ratios, and the shipping density will still be 1 Hz. Worth doing to have the gates
answered; not worth doing under the impression that it is measuring what the OS pays.

## 6. State at close

- **Gates:** `check-assurance-spine`, `check-spine-files`, `check-lints`, `check-crate-sizes`,
  `cargo fmt --all --check` green. **`check-boot-images` green — required this session**
  (`kernel` changed): 3 AArch64 image variants built and linted. `kernel` host tests 212 with
  `fixture-measure`. `netboot.tests` 23/23.
- **Spine:** 31 Features / 98 Stories / 81 Tests / 62 Reports, **91 loose ends (46 open)**,
  **23/460 release gates with dated evidence** (28 rows, two of them refusals that do not count).
  `assurance-status`: 197 blocked by neither qualification nor the board, **127 measurable
  today**, 70 needing a mechanism.
- **Unchanged and still true:** 5 platforms, **0 qualified**; `0/98` Stories assurance-verified;
  `06A` §4.3 undecided; `LE-86` (`G09` needs a mechanism) untouched and still the right call.
- **No board run this session.** Every number above was already on the wire from 2026-08-06;
  what changed is which target column it was read against.
- **Uncommitted.** Nothing committed, `git add -A` never used (`CONCURRENT_SESSIONS` rule 1).
- **Bench: no `tos64-netboot` is running and UDP 67/69 are clear.** Three instances were stopped
  during this session — the stale one from `01A` plus the two that collided at 01:39 — and the
  fixed binary was not left running, because a server whose log the operator cannot see is the
  shape of `LE-90`. **Start one before the next board session**, from
  `work/tools/netboot/bin/Debug/net10.0/tos64-netboot.exe --mac 88:a2:9e:11:4e:cc --root
  C:/Code/TinyOS/os/target/pi5`, and it will now refuse rather than share if one is already up.

## 7. The next session

1. **Item 3, as `01A` wrote it** — `D04` and `D05` paired arms, both arms in one boot, reading
   `MAX_LINES` first (§4). Cost it against §5.
2. **`LE-91`**, if a class is worth more than an instance this week. It is the mechanism behind
   §3 and nothing else stops the next label being bent the same way.
3. **`LE-90`'s open half** — six other bench tools, none audited for the buffering that hid a
   whole log. Cheap, and it is the fourth instrument in a row to have failed by omission.
4. **Then the 127**, choosing gates from the `target` column first. `assurance-status` prints
   which they are.
5. **`06A` §4.3 remains the owner's.** Unchanged.

**And `09A`'s instruction, still holding two sessions on:** do not report `x/460` undecomposed.
`01A` added: there is a command, so quoting it undecomposed is a choice. This session adds the
sibling rule — **`PERF-Dnn-Gnn` is only meaningful if `Dnn` is the domain of the thing you
measured**, and the register cannot tell you when it is not.
