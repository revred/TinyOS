# 03A — Two Boots, and the Wire Becomes the Deployment Path

Session handover, written 2026-08-05 after
[`01A`](01A-the-need-for-speed-and-what-must-not-be-traded-for-it.md) §4 was worked top to
bottom and [`02A`](02A-the-report-that-closes-the-tier-debt.md) filed the tier Report.

The owner's direction: *"take the OS for a spin — build, test and fix all aligned, and we can
deploy better OS variants without a card swap, through the Ethernet wire, in the next
iterations."*

**Every remaining item now needs the bench, and it needs it exactly twice.** §1 is those two
boots, in order, with what each one closes. §4 is the deployment goal and the one distinction
that must not blur. Read §1 first — it is the whole session.

---

## 0. The one-paragraph state

Seven commits on `main`, **none pushed**. `REPORT-2026-08-04-01` is filed and **`LE-09`
(release-blocking), `LE-15`, `LE-24` and `LE-27` are closed** — the project has a hardware
tier. `STORY-P1-10-04` is board-proven across `BOARD VERDICT 11`–`13`; `STORY-P1-10-05` puts
the die temperature on the wire and has never booted. All four frictions from `01A` §4 are
moved: two fixed in code (`--until` watch mode, spoor-cost measurement machinery), two reduced
to single bench steps (the sudo prep, the netboot investigation). Spine: **30 Features /
95 Stories / 79 Tests / 62 Reports, 75 loose ends (38 open)**, every gate green. The card is
**in the laptop, TOS64 role, `f06bfa8ac7ec`**.

## 1. The two boots — this session's whole bench cost

### Boot A — Pi OS card, ~20 minutes, closes two frictions at once

`tos64-cardswap pios`, insert, power. Then, in one SSH session:

1. **Runbook §6 — the least-authority sudo prep.** Root-owned probe at a fixed path, `NOPASSWD`
   scoped to *exactly that binary*, `visudo -c` **before logout**. The bench has no keyboard on
   the Pi; a broken `sudoers` is recovered only by moving the card. Do not widen the scope
   "just for now".
2. **Netboot question 1** — read the EEPROM's `BOOT_ORDER` and its config schema
   ([`docs/pi5-netboot-investigation.md`](../../docs/pi5-netboot-investigation.md)).
3. **The `LE-75` calibration**, now unattended: the paired raw AVS word against
   `thermal_zone0/temp` at two distinct temperatures. This is what turns Ti64Dink's
   `~xx.xC(unverified)` into a number the project may quote.

Questions 2–5 need the board *attempting* a netboot, which is a separate power cycle and can
follow in the same session.

### Boot B — TOS64 card, `f06bfa8ac7ec`, one capture closes four owed items

`tos64-cardswap tos64`, insert, power, then **one** command:

```
cd work/tools/ti64dink
dotnet run -- --live 45 --text env.txt
```

That single capture settles:

| Owed item | What to look for |
|---|---|
| `STORY-P1-10-05` c7 / `LE-75` | `Thermal Kernel Observe … avs=0x……` — does the raw word behave and drift like a die temperature? |
| `STORY-P1-07-06` c1, strongest form | `---- TOS64 text frames ----` carrying the `TOS64-MEAS/2` envelope, then `xtask parse-meas --file=env.txt`. **This is `REPORT-2026-08-04-01`'s top named debt.** |
| `STORY-P1-10-02` c6 | The envelope now carries **11 metrics, not 8** — the three spoor costs. Criterion 6 stops reading *measurable-but-not-measured*. |
| `FEAT-P1-09` exit | The `TOS64-PRESENT/1` beacon line, byte-compared against the frame builder. |

**Four owed items, one power-up.** That is what the last two sessions were building toward,
and it is worth noticing that none of it needs a human to read a screen.

Use `--until` rather than guessing a window when you want a specific event:
`--until rung=ThermalSample`, `--until text=METRICS`, `--until epoch-change`. Exit 0 sighted,
1 timed out, 2 refused. **Do not pipe Ti64Dink through `tail`** — that is how `BOARD VERDICT 13`
lost its `BOOT CHANGED` line.

## 2. Review of `c7f2a6d` — what I checked rather than accepted

The `--until` matrix was re-run independently against synthetic captures, including one built
specifically to attack the strongest claim:

```
epoch-change        (two-boot capture)              exit 0  sighted
rung=ThermalSample  (absent)                        exit 1  timed out
rung=Bogus                                          exit 2  refused, not guessed
rung=ThermalSample  (present in stream)             exit 0  sighted
rung=ThermalSample  (present ONLY in a retained frame)  exit 1  correctly NOT sighted
text=qualification / text=nonesuch                  exit 0 / 1
```

**The retained-frame case is the one that matters** and it holds: a re-announcement replays
records already sent, so it must never fire a watch. A watch that triggered on the boot
certificate would report a five-second-old reboot as a live event every five seconds forever.

One correction to my own process, not theirs: my first run showed six failures and they were
**a stale binary** — I had used `--no-build` against a build predating the commit. Rebuilt, all
seven paths correct. Worth recording because "the tool says it's broken" was wrong and the tool
was right.

The three measure phases hold up on reading: the stamp phase closes the certificate with an
untimed park rung first (so the once-per-boot retain path is not what gets measured) and
**checks `next_sequence()` against the count it expects** — a phase that measured something
other than what it names fails rather than reports; the drain phase asserts the frame came out
at exactly `MAX_PAYLOAD`, so "worst case" is verified and not assumed; the announce phase walks
the `ANNOUNCE_EVERY - 1` refusals untimed. Batching at 8 applies `LE-24`'s lesson *before* the
number is quoted rather than after, which is the right order and the one this project learned
the hard way.

## 3. Then: take the OS for a spin — one task dispatched on silicon

`01A` §5's finding stands and is the next build, once the two boots above are done:

```
grep -rn "sched::|dispatch::|kernel::lock|wcet::" os/src/hal-arm64/src/*.rs   →   nothing
```

**The board has never run TinyOS's kernel.** It boots, maps memory, arms a tick, measures, and
parks. The scheduler, dispatcher, priority-inheriting locks and WCET budgets are Tier 0 only.

Smallest honest increment: **`kernel::sched` creates one task, `kernel::dispatch` selects it,
and a `Dispatch` spoor arrives on the wire from silicon.** Then the tick drives the dispatch
round, and the board stops being parked and starts being an OS. Note that `Category::Dispatch`
and the call sites already exist and already stamp on the host path — so the moment the
subsystems *run*, the stream carries them for free. That is why the substrate was built first.

This crosses from observability into execution and is almost certainly a new Feature under
`EPIC-P1`, decomposed just-in-time.

## 4. Deployment over the wire — the distinction that must not blur

The owner's goal is right and it is reachable. **One line separates the version that costs
nothing from the version that costs the Security Charter.**

- **Charter-neutral, and the path to take: Pi 5 *firmware* netboot.** The bootloader fetches
  `kernel8.img` over TFTP **before TinyOS exists**. No TinyOS code parses it, nothing is
  admitted at runtime, and [rule 9](../../agent.md) — *remote bytes are data, never code* — is
  never engaged. This is what
  [`docs/pi5-netboot-investigation.md`](../../docs/pi5-netboot-investigation.md) scopes, as
  five questions each paired with the bench step that closes it, with **no code and no Feature
  written yet** — deliberately, because building a DHCP/TFTP server before question 2's capture
  is exactly the design-before-ground-truth mistake `01A` §2 warns against.
- **Not charter-neutral, and not to be reached for as a shortcut: TinyOS receiving an image and
  executing it.** That requires reversing `no_path_in_this_module_ever_enables_receive`,
  replacing the containment argument `LE-67` currently rests on (GEM DMA, no IOMMU), a bounded
  command vocabulary with a replay counter and per-record MACs, and **all fourteen `RCG-*`
  gates**. It is a Feature with adversarial tests, not a convenience.

The distinction is worth restating whenever "deploy over Ethernet" is said out loud, because
the two sound identical in a sentence and differ by an entire charter. **Question 4 of the
investigation is the bench-safety one — does a failed netboot always fall back to SD? — and it
should be answered before anything depends on netboot, or a bad image costs a card swap to
recover rather than saving one.**

## 5. Also owed

- **`LE-73`** — `kernel::udp_wire` cites `STORY-P1-10-03`, which does not exist. Then the gate
  that extracts `STORY-*` citations from source and refuses one with no filed Story.
- **`LE-75`'s actuation half** — the fan, driven from a *validated* reading, never a hardcoded
  duty cycle. Sensing is in; acting is deliberately not.
- **`LE-56`'s shell-lane half** — the board half is evidenced; the console lane is untouched.
- **`udp_wire`** is written, tested, and still not wired to the board.
- **`STORY-P1-10-04` criterion 6** — the certificate's write-once bound is host-Green only.
- **`fault_brk_capture` varies ~10%** across boots (109/120/111), unexplained — the metric to
  distrust first if any absolute `D02` figure is quoted.
- **Six unbatched metrics carry unquantified calibration residue.** Relative ordering is
  evidence; absolute values are indicative until each has a batched twin.

## 6. Discipline carried forward

`01A` §3 holds unchanged and is not restated here. Two additions from this session:

- **Rebuild before you believe a failure.** A stale binary produced six confident false
  failures in this session's review. The tool was right and the harness was wrong.
- **Verify another session's strongest claim, not its weakest.** The `--until` review was worth
  running only because it attacked the retained-frame case, which is the one place the design
  could have been subtly wrong and still passed every obvious test.

## 7. Bench facts at close

- **Card: in the laptop, TOS64 role, `f06bfa8ac7ec`.** Note this image predates `c7f2a6d`'s
  three new metrics — **rebuild before Boot B** (`xtask pi5 --fixture=measure`) or the envelope
  will carry 8 metrics instead of 11.
- **Board unpowered.** Per `LE-75`, power for a run and down after; TinyOS still has no thermal
  response, and the sensing half has never executed.
- **Pi OS side:** `ssh revanur@raspberrypi.local`, key auth, link-local IPv6 over the direct
  cable. `sudo` password-gated until runbook §6 runs.
- **Ti64Dink:** `--live <s>`, `--until <cond> [--timeout N]`, `--text <file>`, `--dev`,
  `--list`. Unelevated via Npcap. Never pipe it through `tail`.
- **Three board epochs on record:** `0x049F8B28`, `0x04B328BC`, `0x04B32825` — the last two
  151 counter ticks apart (`LE-74`).
- **`ACT` nibble headroom: one verb.** `MAX_RECORDS` 181. The frame header's reserved padding
  is spent; `flags` has 15 bits.
- Host `cargo clippy --workspace --all-targets` cannot build `kernel`'s `[[bin]]` on this
  Windows machine (`LE-64`'s class). **`check-boot-images` is the clean local signal** — and it
  earned its keep again in `c7f2a6d`, catching an `E0277` in the AArch64 fixture that no host
  build can see.
- **Nothing is pushed.** Seven commits on `main` locally.
