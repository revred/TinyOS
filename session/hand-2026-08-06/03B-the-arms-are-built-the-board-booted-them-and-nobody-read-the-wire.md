# 03B — The Arms Are Built, the Board Booted Them, and Nobody Read the Wire

Session handover, written 2026-08-06. Follows
[`02A`](02A-the-label-was-the-gate-and-two-bench-tools-lying-by-omission.md) and takes
letter **`B`**: session `A` was still live in this tree during this session and appended a
section to its own `02A` while this work was in flight.

**Mandate item 3 is built, gated, and unmeasured.** The `G23` paired-arm method now
extends to `D04` and `D05`, every local gate is green, the board booted the exact image —
and **`PERF-D04-G23` and `PERF-D05-G23` still carry nothing**, because no capture was
parsed. The one sentence to carry forward is not about the arms: **the board has no
inbound path at all — GEM receive is disabled, in the source, by design — and that, not
another gate, is what stands between this project and an OS that works.** §5 is about
that.

---

## 1. What was built: the `G23` method, applied twice more

Two new phases in [`measure_phases.rs`](../../os/src/kernel/src/measure_phases.rs), each
its committed twin's body with **exactly one stamp added inside the timed region**:

| gate | disabled arm (already committed) | enabled arm (new) |
|---|---|---|
| `PERF-D04-G23` | `phase_context_switch` | `phase_context_switch_spoored` |
| `PERF-D05-G23` | `phase_dispatch_round` | `phase_dispatch_round_spoored` |

**The disabled arm is the committed function itself, never a copy** — `01A`'s rule, and
the reason the ratio can be differenced honestly. AArch64 fixture 12 → **14** metrics,
x86_64 9 → **11**, both arms of each pair in one boot, for the reason that has not
changed: `BOARD VERDICT 9` measured ~3% build-to-build movement against a 2% allowance,
so a pair from two runs cannot answer the question at all.

**`D05` is paired with the round, not with `dispatch_select`**, and the choice is
recorded at the phase rather than left to be inferred. The round is the domain's *work
unit* — selection is one step inside it — and it is the only place in this fixture that
resembles a **real** stamp site: the shipping park loop stamps inside
`tinyos_dispatch_round`. That does not make it a measurement of shipping overhead (`02A`
§5 stands — the shipping density is 1 Hz), but it makes the extrapolation a shorter one.

**`MAX_LINES` was read before the metrics were added, as instructed.** It is 24; the
envelope goes 16 → 18. Six lines spare, which is what `LE-89` bought.

### The sameness test is now the method's, not one pair's

`01A`'s source-level test held one pair to *"differs by exactly one stamp inside the
timed region"*. It is now **a table over all three pairs**, plus a second test asserting
that **every function whose name ends `_spoored` has a row in that table** — so a fourth
pair cannot be added and silently held to nothing. That second test is the `LE-89` shape
applied to a checklist instead of a capacity: a list kept *beside* the thing it checks
goes stale; one *derived from* it cannot.

**Both detectors were seen to fail before being trusted**, per `ADR 0005`. A non-stamp
line added inside the `D04` enabled arm's timed region turned the sameness test red
naming `PERF-D04-G23` and printing both regions; deleting the `D05` row from the table
turned the coverage test red naming `phase_dispatch_round_spoored`. Restored, both green.

## 2. What did not happen, stated first rather than buried

**No measurement was taken. Both new gates carry nothing.** The board booted — the
netboot log records `kernel8.img` served at
`b6dbabaea3431afa94cf9210374826bde9e5fb4efef7c5c861b92795c5006f02`, 298,089 bytes,
**transfer complete, 583 blocks** — and the session ended before a capture was parsed.
Everything needed for that boot is described in §6; it is one power cycle and one
elevated capture from being answered, and the image is already staged.

**One race is worth recording because it nearly became evidence.** The first power cycle
pulled `cffa3507…`, an image staged minutes earlier, while a rebuild replaced it with
`b6dbabae…` mid-fetch. The delta was a comment in `ethernet.rs` and the numbers would
have been sound — but the digest would have matched nothing a rebuild of this tree
produces, and **evidence recorded against an unbuildable digest is unreproducible
evidence**. The capture was stopped and the board re-cycled against the staged digest.
`02A`'s rule did exactly its job: *compare the digest, not the byte count*. The rule's
next refinement is smaller and is in §6 — **stage, then verify, then boot; never rebuild
while a server is serving.**

## 3. Two findings outside the mandate

### 3a. The capture-window comment was stale in the `LE-89` way

[`ethernet.rs`](../../os/src/hal-arm64/src/ethernet.rs) told every operator that *"a
capture of any dozen-odd seconds holds the whole envelope"*. The park loop transmits
**one transcript line per beat, and the beat is 1 Hz**, so a full cycle is `line_count`
seconds — 16 when that sentence was written, **18 now**. A 30-second window sized from
the stale sentence is how an operator concludes a line was never transmitted when it
simply had not come round yet: *the same class as `LE-89`, in prose instead of a
constant, and it would have been diagnosed as a kernel or transport fault.* The comment
now states the window as a function of `line_count` and says to size the capture from it.

**This is the third member of a family worth naming.** `LE-89` was a capacity beside its
consumers; this was a duration beside its producer; `LE-91` is a domain label beside its
subject. All three are *a number written down next to the thing that determines it,
rather than derived from it*, and all three stayed correct until someone added one item.

### 3b. `check-timing-regression` is red on `main`, and this session made it redder

**This gate was not in the mandate and was not run by the previous session.** It fails:

```text
xtask: `D04/context_switch_yield_roundtrip_2switches_spoored` was measured
       but has no baseline; commit one (`--update-baseline`) rather than leaving it ungated
```

`01A` added a ninth x86_64 metric with no baseline row; this session added two more.
**Three metrics now have no baseline**, and the gate is doing precisely its job.

**It was deliberately not fixed here, and the reason is a decision, not effort.**
`--update-baseline` **rewrites the whole of
[`goals/performance/baselines/tier0-x86_64.tsv`](../../goals/performance/baselines/tier0-x86_64.tsv)**
— it cannot append — so taking it on this laptop replaces rows recorded on the CI runner
on 2026-07-29 with a Windows laptop's. That is **`LE-23` by name**, and session `A`
independently reached the same conclusion and deferred it in its own appended section.
Two sessions declining the same shortcut for the same recorded reason is the register
working.

**The prior question, which nobody has answered and which the next session should put to
the owner rather than resolve:** the gate compares **same-run ratios** to a reference and
is designed to be host-independent, but the `min_cycles`/`p50_cycles` columns beside them
are not. **Nobody has said which of the two a reader is entitled to trust** — and until
someone does, regenerating on any machine is a coin-flip about which half of the file
means something. `LE-23` should probably be discharged by recording on the runner.

## 4. State at close

- **Gates:** `check-boot-images` **green** (required — `kernel` and `hal-arm64` both
  changed; 3 AArch64 variants built and linted), `check-assurance-spine`,
  `check-spine-files`, `check-lints`, `check-crate-sizes`, `check-citations`,
  `cargo fmt --all --check` all green. Full workspace suite green. **`check-timing-regression`
  RED** — §3b, deliberately.
- **Spine, undecomposed figures deliberately avoided:** 31 Features / 98 Stories / 81
  Tests / 62 Reports, **92 loose ends (47 open)**, **23 of 460 release gates carrying
  evidence** — unchanged by this session, because this session measured nothing.
  `assurance-status`: **197** blocked by neither qualification nor the board, **127**
  measurable today, **70** needing a mechanism. 5 platforms, **0 qualified**; **0/98**
  Stories assurance-verified.
- **Uncommitted.** Nothing committed, `git add -A` never used (`CONCURRENT_SESSIONS`
  rule 1). Five files modified: `measure_phases.rs`, `fixture_measure.rs`,
  `fixture_measure_arm64.rs`, `transcript.rs`, `ethernet.rs`. **Session `A` also has
  changes in this tree** — its appended section in `02A` — so stage by path, never `-A`.
- **Bench: `tos64-netboot` was stopped at close and UDP 67/69 are clear**, verified.
  Its log is at `C:\tmp\netboot-03A.log` (5,610 bytes) and is worth keeping until the
  measure boot is done: it holds the served digest of both boots and is the record that
  settles §2's race. **The `LE-90` fix works in the field** — the log was read live,
  mid-run, from another window, which is the thing that was impossible two days ago.

## 5. The needle: what stands between this and an OS that works

Asked directly at session close, and it deserves a direct answer rather than another
gate count. **The register is not the bottleneck. The bottleneck is that the board cannot
be told anything.**

Today the Pi 5 boots to MMU on, GIC routed, tick armed, gigabit link up, canvas painted
on micro-HDMI, spoor stamps and a measurement transcript cycling onto the wire. It is a
genuinely instrumented machine. **It is also strictly one-directional: it speaks and
cannot listen.** The source says so in
[`ethernet.rs`](../../os/src/hal-arm64/src/ethernet.rs#L1241) — *"the device is granted
one pinned region and **receive stays disabled**"* — and there is no console input path
either: the guest kernel has never had one. Every interaction this project has with its
own OS happens by rebuilding the image and power-cycling.

That single fact explains several things that otherwise look unrelated:

- **It gates the owner's own first priority.** Ti64Dink is a *remote desktop*. A remote
  desktop is bidirectional by definition. Every frame the board has ever sent is a
  monologue, so `FEAT-P2-10` cannot start on top of what exists.
- **It is why `0/98` Stories are assurance-verified.** Verification of behaviour needs
  the behaviour to be *exercisable*; a system that can only be configured at build time
  is one whose test surface is the fixture set, which is exactly what this project has.
- **It is why every board session costs a power cycle**, and why three of them were spent
  on a stale image (`LE-87`) rather than on the OS.

### The ordered path, and it is short

**Step 1 — GEM receive, one frame, fail-closed.** The RX ring, one pinned region, an
EtherType filter, and a bound on what is accepted. Nothing above it. The success
criterion is one `TOS64-*` frame arriving from the host and being *counted* on the
canvas. This is where the Security Charter earns its keep and where `LE-67`'s containment
note (one pinned region, no IOMMU) has to be re-argued rather than inherited — **receive
widens what a confused device can reach in a way transmit never did**, so it is a
`SECURITY_CHARTER.md` read-in-full change, not a driver change.

**Step 2 — one command, end to end.** Host sends a framed request; board acts; board
answers. The first command should be the most boring one available — *report your
rungs* — because it is already stamped, already framed, and already on the wire, so
Step 2 tests only the new half. **That round trip is the first thing this project could
honestly call an operating system responding to a user**, and it is also the moment the
whole `TOS64` envelope family stops being a diagnostic and becomes a protocol.

**Step 3 — the two board items that have been costed and deferred for days**, both of
which are builds rather than decisions and both of which get much cheaper once Step 2
exists: `STORY-P1-06-02` criterion 4 (`Rp1CommandLines` compiles for AArch64 and
**nothing calls it** — no fixture drives actuation, no probe on GPIO 20..27) and `LE-82`
(`Rung::FaultTaken` is declared in both vocabularies and **stamped by nothing**, so no
`mmu-fault` boot proves anything until it is stamped at the `hal-arm64` fault-report
site).

**What this path deliberately does not do is add gates.** 127 are measurable today and
they will still be there. A gate measured on a machine nobody can talk to is evidence
about a fixture; the same gate measured on a machine that answers commands is evidence
about an OS. **Step 1 and 2 change what every subsequent measurement means**, which is
why they come before the 127 and before `LE-91`, notwithstanding that `LE-91` is the
better *mechanism* work.

## 6. The next session, in order

1. **Close item 3 — one boot, and everything is staged.** ~20 minutes.
   - Start the server **first**, and do not build afterwards:
     `work/tools/netboot/bin/Debug/net10.0/tos64-netboot.exe --mac 88:a2:9e:11:4e:cc
     --root C:/Code/TinyOS/os/target/pi5`. It refuses rather than shares if one is up.
   - **Verify the staged digest before powering the board**, and expect the server's
     transfer line to name the same one:
     `b6dbabaea3431afa94cf9210374826bde9e5fb4efef7c5c861b92795c5006f02` (298,089 bytes).
     If it differs, something rebuilt — stop, do not spend the capture (§2).
   - Power-cycle, then capture **60 seconds, not 30** — 18 lines at 1 Hz is an 18-second
     cycle (§3a):
     ```powershell
     pktmon filter remove
     pktmon filter add -d 0x88B5
     pktmon start --capture --pkt-size 0 --file-name $env:TEMP\meas.etl
     # 60 s
     pktmon stop
     pktmon etl2txt $env:TEMP\meas.etl -o $env:TEMP\meas.txt -v 3
     ```
   - Parse through the **unchanged** parser: `cd os; cargo run -p xtask -- parse-meas
     $env:TEMP\meas.txt`. Expect `metrics=14`.
   - **Read the `target` column before computing anything** (`09A` §5) **and check the
     domain label is the subject's** (`LE-91`, §3b of `02A`). The ratio is
     `(spoored p99 - twin p99) / twin p99` for each pair, against `<= 2%`.
   - **File both gates with the caveat, or the number will be misread.** `D04`'s disabled
     arm is *unbatched* and measured 80 cycles p50 on `BOARD VERDICT 7`, which is
     `LE-24`'s residue regime. Both arms subtract the same calibration, so **the
     difference is the stamp and is sound; the denominator carries `D04`'s existing
     residue caveat**. Say so on the row. Expect a large fail on `D04` — a 137-cycle
     stamp on an 80-cycle round trip is not close — and record it as one.
2. **Put §3b to the owner** before touching the baseline. One question: *is a Tier 0
   baseline recorded off the CI runner one this project wants committed, and which of
   `min_cycles`/`p50_cycles` versus the ratios is a reader entitled to trust?* Then
   regenerate **once**, with `--date=`, for all three metrics together.
3. **Step 1 of §5 — GEM receive.** Read `SECURITY_CHARTER.md` and `LE-67` in full first;
   this is the first change in the project's history that lets external bytes reach a
   pinned region. Test-first, fail-closed, and the containment argument written down
   before the ring is.
4. **`LE-91`**, unchanged and still the right mechanism — declare each metric's domain
   *and* owning Story at the `collect` site, have `xtask` parse the `collect` calls out
   of the fixture source (the `LE-80` mirror shape), assert the domain is selected by
   that Story's contract. `02A` records why *fixture domains ⊆ owning Story's contract*
   is **not** the rule and would be wrong if asserted — read that before building it.
5. **`LE-90`'s open half** — six C# bench tools under `work/tools/` (cardswap, imgwrite,
   linkwatch, sdprep, serialwatch, ti64dink), none audited for the stdout buffering that
   hid an entire live log. Cheap, and it is the fourth instrument in a row to have failed
   by omission.

**Do not start:** `FEAT-P1-05`'s RT reserve (Feature-sized, files nothing in a session;
scope settled in `STORY-P1-05-01`). `G09`/`LE-86` (needs per-feature section attribution
that does not exist — a decision, not a session). `06A` §4.3 (the owner's, and correctly
sized).

**And the two standing instructions, both still holding:** do not report `x/460`
undecomposed — there is a subcommand, so quoting it undecomposed is a choice. And
`PERF-Dnn-Gnn` is only meaningful if `Dnn` is the domain of the thing you measured; the
register cannot tell you when it is not. **This session adds a third, from §2 and §3a:
verify the digest and size the window before you spend the boot** — both of this
session's near-misses were the bench lying quietly, not the board, which makes it
five instruments in a row.
