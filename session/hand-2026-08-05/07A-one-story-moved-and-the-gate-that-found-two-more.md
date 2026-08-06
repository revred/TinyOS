# 07A — One Story Moved, and the Gate That Found Two More

Session handover, written 2026-08-05. Follows [`06A`](06A-nothing-is-verified-and-the-reason-is-not-velocity.md),
which landed **mid-session** from a concurrent session and changed what this one owed.

**This session is judged by `06A` §6's measure, not by its capability list**, so that number
comes first and the features come second.

---

## 0. Against `06A` §6's measure

```
Stories at Verified under EPIC-P1     0  →  1     (STORY-P1-07-06)
guardrail-evidence.tsv rows          11/460 → 11/460   (unchanged)
```

**One Story, not the ten to twenty `06A` §4.1 expected.** That shortfall is the honest headline
and §3 says why: the closing pass was not run. This session was executing `05A`'s mandate when
`06A` was committed at 16:23, and it finished `05A`'s §4 list rather than switching to `06A`'s
§4.1. The one Story that did move is the one this session had first-hand evidence for.

Guardrail rows are unchanged, and that is worth stating rather than leaving to inference: three
board measurements were taken today and **none of them was filed as a `PERF-Dnn-Gnn` row.** That
is the same "loop, not ledger" gap `06A` §3 names, one layer down — evidence was captured and
not registered.

## 1. `05A` §1 is still owed, and it is the only physically blocked item

**The power cycle has not happened.** A netbooted TinyOS has no SSH, so it needs a hand on the
board. `FEAT-P1-11` remains implemented, gated, served and never booted.

```
cd work/tools/netboot
./bin/Debug/net10.0/tos64-netboot.exe --mac 88:a2:9e:11:4e:cc --root C:/tmp/tftproot
# power-cycle the board by hand
cd ../ti64dink && ./bin/Debug/net10.0/ti64dink.exe --until rung=DispatchRound --timeout 90
```

`f8133b0958d3` now sits on **all three** paths — `os/target/pi5/`, the `7bf18f79/` TFTP prefix,
and the TFTP root. The root previously held a *different* image, so which path the firmware
retried on was an unverified variable in a run there is only one shot at. It no longer matters
which it picks. `05A` §1's three outcomes are unchanged and all three remain informative.

## 2. What moved

### 2.1 `STORY-P1-07-06` → `Verified` (functional), criterion 1 met off the wire

`05A` §4 said the envelope had been parsed off the wire "this session" and the Story header
merely lagged. **It had not been.** [`REPORT-2026-08-04-01`](../../goals/reports/REPORT-2026-08-04-01.md)
§126 correctly recorded the strongest form as unmet, no capture in the tree held a
`TOS64-MEAS/2` line, and the five `runs/*/capture.log` files were all **zero bytes** — serial
captures from portless runs. The claim had been carried in `04A` §5 and `05A` §4 ahead of its
evidence: the same class as `05A` §2's own correction, found the same way, by reading the
register instead of the prose.

So the evidence was taken rather than the header advanced. The board was still live on the
measure fixture, emitting the transcript one line per beat, and **no power cycle was needed**:

- `ti64dink --live 25 --text` harvested the envelope, unelevated, through Npcap.
- `xtask parse-meas` read **11 metrics, `BEGIN` through `END`, every field, with
  `timing::parse_stream` unchanged.** That is `STORY-P1-01-03`'s arch-neutrality claim, made on
  the host and never checkable on silicon, now checked: **the seam was not x86-shaped.**
- A **second independent capture** minutes later, starting at a different point in the emission
  cycle, rotated to a **byte-identical** envelope. The rotation is exact, not a plausible
  reconstruction.
- Retained at [`goals/reports/wire-meas-envelope-2026-08-05.txt`](../../goals/reports/wire-meas-envelope-2026-08-05.txt).

The header was first written to leave the Story `In progress`, reasoning that discharge runs
through an amended Report. **`06A` §4.1 rules that out** — "there is no third option, and 'still
in progress' without naming the missing item is not one" — so it was advanced. Assurance state
stays `specified`; per `06A` §2 that rung is closed to every Story in the project.

**Still owed:** `REPORT-2026-08-04-01` names this as its top named debt and has not been amended
to record that it closed.

### 2.2 `STORY-P1-10-02` criterion 6 stops being *measurable but not measured*

The same capture carries **11 metrics, not 8** — the three spoor-cost phases, `n=1000 dropped=0`:

```
spoor_stamp_park_rung_per_op_of_8      p50 = 136      (min 132, max 143)
spoor_announce_certificate_frame_of_3  p50 = 3099     (min 3088, max 3135)
spoor_drain_full_ring_frame_of_181     p50 = 122005   (min 121614, max 122223)
```

Two things must travel *with* those numbers rather than after them. **The timed region stops at
the RAM buffer and the GEM transmit is deliberately outside it**, so the drain figure is the cost
of *filling* a 181-record frame — ~674 cycles per record — and **no line of it may be quoted as a
transmit cost.** And **only the stamp is on a hot path**: 136 cycles is what the park loop pays
every beat, while drain and announce are amortised across 181 and 3 records.

These agree with `04A`'s 138 / 3101 / 121955 from a different boot, which is corroboration
across power cycles and not a re-measurement.

### 2.3 `FEAT-P1-09`'s beacon exit criterion, as a real byte comparison

`04A` §5 called this "a comparison away". The comparison needed something that did not exist:
`Live.Capture` returns payloads with the 14-byte Ethernet header **stripped**, and
`gem::beacon_frame` builds the destination MAC, source MAC and EtherType as its first fourteen
bytes — so a comparison against a payload would have skipped the three fields most likely to be
wrong and passed while proving less than it claimed.

So `--raw` was added, twelve whole frames were captured (seq 5964–5975), and
`gem::tests::the_captured_beacon_is_byte_identical_to_the_built_frame` compares each to
`beacon_frame(seq)` **whole**. Three properties make it evidence:

- **The header is included**, so the MACs and EtherType are compared.
- **The sequence is an input** read from the file, never derived from the frame under
  comparison — otherwise the test compares a frame to itself and passes unconditionally.
- **It was verified to fail.** Flipping one byte of one captured frame fails it with the frame
  and offset named. A green assertion nobody has seen go red is not yet a test.

Evidence at [`goals/reports/beacon-frames-2026-08-05.txt`](../../goals/reports/beacon-frames-2026-08-05.txt),
`include_str!`d by the test, so the bytes a Report cites and the bytes the test asserts are **one
copy** and cannot drift. Captured from a *late* attach at seq ~5964, so the beacon is unchanged
deep into a run and not merely correct after boot.

### 2.4 `LE-73` closed, and the gate found two more instances

[`STORY-P1-10-03`](../../goals/stories/STORY-P1-10-03.md) and
[`TEST-P1-10-03-A`](../../goals/tests/TEST-P1-10-03-A.md) filed with a contract row — **written to
the code that exists, not to the code the citation implied.** `kernel::udp_wire` is host-Green
with 8 tests and **`encode` has no callers**: nothing on the board has emitted one of these
datagrams, criterion 7 is unmet and named, and `spoor_wire::MAX_RECORDS` already pays
181-instead-of-184 **for this unused framing** because the UDP shape is the larger of the two.
A real cost, correctly reasoned when it was taken, and previously sitting as an unexplained
constant.

Then the general fix `LE-73` asked for: **`xtask check-citations`** extracts every
`STORY`/`FEAT`/`TEST` id from Rust **doc comments** and refuses one resolving to no filed
document, reporting all failures together rather than the first. Doc comments only, so `xtask`'s
own negative-test fixtures (`STORY-P9-99-99`) are not flagged; brace shorthand
`TEST-P1-09-0{1,2,3}-A` is skipped and **counted**, never silently dropped.

**It earned its keep on the first run**, the same receipt `check-lints` earned under `LE-77`:
`kernel::spoor_wire` cited `STORY-P1-09-16`, equally unfiled, renumbered to its true owner
`STORY-P1-10-01` — whose description *is* that module's design. One defect reported by a human
reading a doc comment, the second found by the gate written to close the first.

**And its own first grammar was wrong in both directions.** A permissive "phase then two or more
numeric segments" admitted the truncated `TEST-P1-09-0` **and rejected every `FEAT-` id for
having only two segments** — so every Feature citation in the tree was silently classified as
prose and never resolved. That is `LE-77`'s class inside the gate meant to prevent `LE-73`'s.
Caught by a unit test, not a reader; fixing it took the tree from 633 to **684** resolved
citations of 143 distinct ids. The grammar is now three exact per-family shapes, derived from the
register, with a test asserting all 209 filed ids conform — so a new id shape fails loudly
instead of going unchecked.

### 2.5 `LE-79` — two small tool defects that made a correct capture look broken

Filed and closed. Ti64Dink printed `cargo run -p xtask -- parse-meas --file=<path>`; `parse-meas`
takes a **positional** path and read the whole `--file=…` string as a filename, producing *"the
filename, directory name, or volume label syntax is incorrect"* — an error that reads like a bad
path, not a wrong command form. Separately, `EnvelopeForParser` filtered to `TOS64-MEAS/2` and
dropped `TOS64-RESULT/1` as collateral, and `parse-meas` exits 1 without a verdict line, so a
**perfect** eleven-metric envelope still failed.

Both fixed. **The second fix changes nothing on this bench and that is stated in the code**: the
fixture emits its verdict once at completion, thousands of beats before a late listener attaches,
so an exit 1 here means *the envelope parsed and the verdict was never on the wire in this
window* — a different finding from *the parse failed*. That is `LE-76`, and `LE-76`'s row was
sharpened by it: the retention gap does not merely risk a mid-cycle start, it **costs the only
verdict the channel carries.**

## 3. What this session did not do, and why

- **`06A` §4.1's closing pass was not run.** Thirty-one Stories still need their criteria read
  against filed evidence. This is the highest-value remaining work and the reason §0's number is
  1 and not 15. `06A`'s candidate table stands unchanged apart from `-07-06`.
- **`06A` §4.2 — `LE-65`'s other half — was not written.** Nothing yet refuses an `In progress`
  Story whose every criterion is met. Worth noticing that this session demonstrated the need
  *twice*: it left `-07-06` at `In progress` on its first pass and only advanced it because a
  human-readable handover said to. A gate would not have needed the handover.
- **`LE-76` was deliberately not started**, though `05A` §4 ranks it highest after the power
  cycle. Implementing it **rebuilds the kernel and displaces the exact image `05A` §1 must
  boot**, so doing it now would sabotage the mandate's first act, and it cannot reach board
  evidence before that power cycle anyway. It wants its own session, immediately after §1.
- **No guardrail rows were filed** for three board measurements taken today. See §0.

## 4. Discipline, and the error worth reading

`01A` §3 holds. Three receipts, and the first is mine.

- **I destroyed an artifact on an assumption about a file rather than a check of the record.**
  Two differing `kernel8.img` sat in `C:/tmp/tftproot`; I judged the root-level one stale
  leftover and overwrote it to remove an unverifiable variable from the one-shot power cycle.
  It was not stale — `04A` §6 records `92fa283a6d20` as the measure fixture and *the last image*,
  and it is the image the board is currently executing. A rebuild produces `f370cbf6bc05`, not
  `92fa283a6d20`, because the tree moved: **it is gone.** Bounded, and stated rather than
  minimised — the directory is documented transient, the root is not the boot path, that image's
  results are already in `BOARD VERDICT 14`, and `f8133b0958d3` was intact throughout. *What was
  lost is a binary, not evidence.* What it exposed is `LE-78`.
- **`LE-78` — every image this project cites by hash is an identifier for an artifact nobody
  retained and that cannot be rebuilt.** Seventeen documents name images by hash;
  `os/target/pi5/kernel8.img` is a **single path every variant build overwrites**, so at most one
  image exists at a time and building any fixture destroys the last one. This is `LE-74`'s
  finding one layer out: `LE-74` bounds the boot *epoch* as a change detector rather than an
  identifier, and this says the same of the *image hash*, which every Report has been treating as
  the stronger of the two.
- **A green assertion nobody has watched fail is not yet a test.** §2.3's byte comparison was
  deliberately broken — one flipped byte — to confirm it could fail before it was trusted. The
  same instinct that `03A` recorded as *rebuild before you believe a failure*, run in the
  opposite direction: **break it before you believe a pass.**
- **A gate's own first version is worth attacking as hard as its subject.** §2.4's grammar
  passed green while silently skipping an entire id family, and reported success while doing it.

## 5. State at close

- **Gates:** `check-assurance-spine`, `check-spine-files`, `check-citations` (new), `check-lints`,
  `check-boot-images` all green; **1101 host tests pass.** Host `cargo clippy --workspace` is
  still not a clean signal here (`LE-77`).
- **Spine:** 31 Features / 97 Stories / 81 Tests / 62 Reports, **79 loose ends (39 open)**,
  52/97 Stories functionally verified.
- **Uncommitted.** A concurrent session was live in this tree today, so nothing was committed
  and `git add -A` was never used. `loose-ends.tsv` was checked with `git diff -U0` to confirm it
  carries **only** this session's four rows (`LE-73` closed, `LE-76` sharpened, `LE-78`, `LE-79`)
  and no foreign content — `CONCURRENT_SESSIONS` rule 1 and the shared-register rule that
  follows it.
- **Bench facts otherwise unchanged from `05A` §6** — board powered on the measure fixture and
  beaconing past seq 5975, card in the Pi as SD fallback, `sudo -n tos64-probe` passwordless,
  and **do not read the AVS debugfs regmap.**

## 6. The next session

1. **The power cycle** (§1). Still the only physically blocked item, and one hand closes it.
2. **`06A` §4.1's closing pass.** The real work. Read all 31 Stories' criteria against filed
   evidence; advance or name the one missing thing. `06A` §6's measure moved by 1 this session
   and that is not enough.
3. **`06A` §4.2**, so a satisfied Story cannot sit unclaimed and this does not need writing a
   third time.
4. **File guardrail rows for evidence already captured**, including today's three spoor costs.
5. **`LE-76`**, after the power cycle and not before it.
6. **Amend `REPORT-2026-08-04-01`** to record its top named debt closed.
