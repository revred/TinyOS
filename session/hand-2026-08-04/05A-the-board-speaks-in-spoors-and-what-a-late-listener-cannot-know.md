# 05A — The Board Speaks in Spoors, and What a Late Listener Cannot Know

Session handover, written 2026-08-04 after the session that executed
[`03A`](03A-fixture-measure-staged-one-boot-from-le-09.md) and
[`04A`](04A-two-boots-three-defects-and-the-gates-that-missed-them.md).
Four board boots, six defects, and the first kernel spoors ever seen leaving a running
TinyOS system. Read §5 first if you only read one section — it is the next session's work.

---

## 0. The one-paragraph state

`FEAT-P1-07`'s ladder is effectively done: `STORY-P1-07-02` criterion 2, `-03` **every**
criterion, `-04` **all five**, `-06`'s board half — all Green on silicon. `FEAT-P1-10` (new,
spoors as the observability substrate) has its format, its stamping, its egress and its host
application, and `BOARD VERDICT 10` captured a fresh boot off the wire: `frame seq=0` carrying
`MmuEnabled`/`GicRouted`/`TickArmed`, 160 records, **0 refused, 0 lost**, read **unelevated**.
Ti64Dink exists. Spine: 30 Features / 93 Stories / 77 Tests, 72 loose ends (40 open). The card
is in the **laptop** carrying `b44040659702`. Everything pushed; CI green on the tip.

## 1. What the board proved

| Verdict | Image | What it settled |
|---|---|---|
| 5 | `0c709197ed26` | MMU criteria 2/3/4 — cache probe **410×**. `LE-27`, `LE-15` answered. Tick **refused** (`LE-69`). |
| 6 | `a0d1773c8f10` | `LE-69` fixed on the first try. Tick armed — and fired **once** (`LE-71`). |
| 7 | `619f40b8c076` | `count=1816 rmin=999 rmax=1000`. `-04` criterion 1, the last non-capture item. Outlier gone. |
| 8 | `fde0f2ce3f91` | `far=0x20_0000_0000` to the bit, `HALTED REASON=NO-RESUME-PATH`. `-03` c5, `-02` c2. |
| 9 | `b44040659702` | Spoor channel added, **no regression** — beacon survived a second transmit per pass. |
| 10 | `b44040659702` | **The first kernel spoors off the wire.** `seq=0`, 160 records, 0 lost, 0 refused. |

Full transcripts and per-field decodes are in
[`pios-ground-truth-2026-08-03.txt`](../../goals/reports/pios-ground-truth-2026-08-03.txt).

## 2. Three defects that were unreachable by construction

The session's pattern, and the thing worth carrying forward: **each defect hid the next**, and
none was visible until the ladder was climbed on hardware.

- **`LE-69`** — `gic.rs` asserted five implemented GIC priority bits; BCM2712 implements four
  and read back `0xF0`, so the code **refused a conforming device**. Fixed by *discovering* the
  width, not by swapping one bench constant for another.
- **`LE-71`** — `fixture_measure` masked interrupts with no counterpart, so the tick's entire
  lifetime was the ~10 ms before the fixture: one tick, and one tick is **zero intervals**.
  `-04` criterion 1 was unsatisfiable **on any board, however many times it was booted**, while
  every host test stayed green.
- **`spoor` was never compiled for AArch64** — swept into a `cfg(target_arch = "x86_64")` block
  with the modules that genuinely name `hal_x86_64` types. `LE-56`'s board half was
  unreachable by construction, not merely unimplemented.

## 3. Two gates that could not see their own subject

- **`LE-70`** — the dashboard's four headline tiles were invisible for four days across sixteen
  commits because a generated block was *relocated*. The gate byte-compares **content** and
  never asserts **placement**; proved by running the spine either side of the fix, green both
  times. Now anchored.
- **`LE-51`** — implemented, and on its first run found the project's most-cited loose end
  (`LE-09`) carrying a **dangling citation**, plus one genuinely ambiguous slot.
- **`LE-72`** — mine. Nothing in the local gate set compiles `kernel` for AArch64, so I pushed a
  broken build twice. `cargo test` + `fmt --check` + clippy all pass on a tree whose boot image
  does not link. **Until `LE-72` closes, run `cargo run -p xtask -- pi5 --fixture=measure`
  before every push that touches `kernel` or `hal-arm64`.**

History was rebased on the owner's instruction so every commit on `main` builds; verified by
building all five in a throwaway worktree.

## 4. Ti64Dink exists

`work/tools/ti64dink` — zero package dependencies, matching the rest of the C# fleet. Reads
live via P/Invoke to `wpcap.dll` (four libpcap functions, written here, **no Npcap-derived
code**), or from a capture file. Two rules it does not bend: **loss is reported, never
smoothed**, and **unknown discriminants are refused, not guessed**.

Licensing is settled and written down in [`external/README.md`](../../external/README.md):
Npcap is source-available, not open source; `external/npcap188/` is **git-ignored** because
untracked survives exactly until someone types `git add external/` — a dry run confirmed that
command would have staged the whole tree into an MIT repository.

## 5. The next session's work — what a late listener cannot know

**This is the mandate.** It comes from the owner's question at the close of this session, and it
is a real hole in the substrate rather than a polish item.

**If frame 0 is lost, the boot rungs are gone forever.** They stamp exactly once, the drain
clears the ring, nothing re-sends them. A host would see a sequence gap and know *how many*
records it lost but never *what they were* — and boot state is the least repeatable, most
diagnostic part of the whole stream. `BOARD VERDICT 10` only exists because a capture happened
to be listening across a power cycle.

**Worse: a listener joining late cannot tell which boot it joined.** At `seq=25138` there is no
way to distinguish "continuing normally" from "joined after a reboot I never saw". Sequence
numbers alone cannot express that.

Three answers, in the order they should be taken:

1. **Re-announce the boot epoch.** Hold the boot rungs in a small immutable buffer *separate*
   from the ring and re-emit them every N park passes — a retained birth certificate. Any
   listener, joining at any time, learns the boot state within a bounded window. **Egress-only:
   no receive, no charter change, no new `LE-67` exposure**, one small frame every few seconds.
2. **A boot epoch field in the frame header.** Nearly free: `spoor_wire` already reserves a
   `flags` `u16` and 4 bytes of padding at offsets 18–24, with the comment *"so a future field
   does not have to move the records."* This is that field. A 32-bit epoch fixed at boot lets
   every frame self-identify, so a host can tell boot #7 from boot #8 and knows when it has
   joined mid-stream and must wait for the next re-announcement.
3. **Two-way query/response — the expensive answer.** "Host asks, board replies" is genuinely
   better than broadcasting hopefully, and it requires **enabling GEM receive**: the one thing
   `gem.rs` enforces against with a dedicated test, and the thing `LE-67` records as *the*
   containment story while there is no IOMMU. Needs the bounded vocabulary, replay counter and
   per-record MAC specified in
   [`docs/spoor-transport-architecture.md`](../../docs/spoor-transport-architecture.md) §7.

**The sequencing matters: 1 + 2 remove most of the need for 3.** A retained, re-announced,
epoch-tagged stream means a listener never *has* to ask — which is worth establishing before
spending charter work on making the board listen.

## 6. Also owed, in rough priority order

- **`REPORT-2026-08-04-01`**, closing `LE-09` (release-blocking), `LE-15`, `LE-24`, `LE-27`.
  Everything it needs now exists. **Do not write it from the photographs** — Ti64Dink and
  `xtask parse-meas` can quote machine-parsed bytes, which is the whole reason to prefer them.
- **`FEAT-P1-09`'s exit criterion** — the beacon byte-compared against the frame builder. Now
  trivially reachable: Ti64Dink already captures the beacon frames alongside the spoors.
- **`STORY-P1-10-02` criterion 6** — the per-stamp and per-drain cost is **unmeasured**.
  A stated "not measured" passes; an unstated assumption does not. An observability substrate
  whose overhead is unknown is one that gets switched off in the run that mattered.
- **`LE-72`** — make the AArch64 kernel build reachable from a habitual command.
- **`LE-56`'s shell-lane half** — the board half is evidenced; the console lane is untouched.
- **`udp_wire`** is written and tested (8 tests) but **not yet wired to the board**. It is no
  longer urgent now that Npcap works, but it is what makes the stream readable on any machine
  with no driver at all, and it costs the board nothing in attack surface — the board parses
  nothing either way.

## 7. Bench facts at close

- **Card: in the laptop, TOS64 role, `b44040659702`** (spoor-emitting measure image).
  `pios-backup\` retained. Board powered, beaconing.
- **Npcap installed**, unelevated capture works: `ti64dink --live 30`.
- `MAX_RECORDS` was reduced 184 → **181** so the payload fits an MTU in *both* framings
  (raw 1486, UDP 1514). The board image on the card predates that change; rebuild before
  relying on the UDP path.
- **~3% build-to-build timing variation exists** independent of any change to the measured
  path (`BOARD VERDICT 9`) — almost certainly code layout. No absolute figure should be quoted
  to better than that, and no timing gate should be tighter.
- `LINK=DOWN BEACON=SKIPPED` on the report row is the **expected** boot snapshot, as in verdicts
  2–10. The live `TOS64-BEAT/1 STATE=BEACONING` row is the truth. Do not re-diagnose it.
