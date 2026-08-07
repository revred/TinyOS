# 12A — Sitting B, the script: one afternoon, two locks, and every power cycle spent twice

Cover note and bench script for the next owner-attended sitting. Follows
[`10A`](10A-the-first-conversation-from-counted-frames-to-an-answered-command.md)
(the plan) and [`11A`](11A-sitting-a-executed.md) (Sitting A banked). Same
session as `08A`–`10A`. This document is written so the bench session
**executes instead of derives**: every step names its command, its expected
line on the wire, and which register row or criterion it closes.

**The one sentence, if only one survives:** *Sitting B is five boots and two
passive captures; on its far side the first platform is qualified (every `G04`
unlocked, the assurance-verified ceiling lifted), `STORY-P1-09-16` is closed
on its five predicted verdicts, and — if the owner's one sentence is spoken
before it starts — TinyOS answers a human for the first time in the same
afternoon.*

## 0. Before the sitting — the laptop work that makes the boots cheap

Two fixture arms do not exist yet and are **measurement instrumentation, not
design surface** (the sprint rule has never blocked an instrument; every
envelope arm this month is the precedent):

- **The SMC positive-control arm**: inside a residency window, issue one
  benign `PSCI_VERSION` SMC — the one documented synchronous EL3 entry this
  platform has (`REPORT-2026-08-07-01` Q2). The probe must report the
  excursion: physical-counter advance the window cannot account for. Host
  test red first with a scripted excursion, exactly `probe_residency_window`'s
  own discipline.
- **The campaign arm**: ≥1,000 windows at the proven 540,000-tick size,
  distribution of unaccounted physical ticks per window
  (`min/p50/p99/p99_9/max`) on the wire as `TOS64-QUAL/1 campaign …` lines,
  environment stated as it is, `TOS64-RESULT/1` verdict riding the same
  transcript.
- **Criterion 4a of `STORY-P1-13-01`** (sanctioned by `FEAT-P1-13`'s lift):
  `TOS64-DISPLAY/1` gains `fb_addr=… src=constant|refused` — small, and it
  rides whichever image boots first.
- **If the S4 sentence has been spoken** (§4): the `-16` re-arm and the
  `TOS64-CMD/1` admitted-verb implementation, written red-first against
  `STORY-P1-09-17`'s already-specified suite. If it has not: skip; nothing
  else in this script depends on it.

`check-boot-images` before anything is served — the standing instruction that
has caught a real cross-target break every week it has existed.

## 1. Pre-flight at the bench

Runbook §0b, added today, is the checklist: **one server and it is yours**
(`LE-87`; a live `tos64-netboot` was left running on this bench on
2026-08-07 — stop it and start your own), **the `LE-117` tripwire** if the
Pi OS role boots (bootloader hash against the Report's pin — a mismatch is
the void clause firing, stop and re-record), and **monitor-live** if the
canvas matters (`07F` §7b). Absolute paths for every background command — the
cwd trap has fired four recorded times.

## 2. The boot manifest — each cycle spent twice

**Boot 1 — the current image, before anything new.** Two payloads, no build:

1. *The receive probe, which is also the `NOBUFFER` diagnosis.* Send **one**
   `ping` arm: `ti64dink --send ping`, then read `TOS64-RX/1` on the wire.
   - **Counted** (`ACCEPTED=1`) → run the remaining four arms
     (`unicast`, `ethertype`, `prefix`, and the fifth), each verdict already
     predicted by a host test against `gem_receive::admit` — **five matches
     closes `STORY-P1-09-16` criterion 4** and the Story with it.
   - **`STATE=STOPPED REASON=NOBUFFER ACCEPTED=0`** on a fresh boot → the
     inbound arm dies before its first frame, the `07F` §7c finding is a
     boot-time condition rather than an exhaustion, and *that capture is the
     finding* — S1's diagnosis, free, on the boot you were going to burn
     anyway. `-16` criterion 4 then waits on the re-arm fix (§4).
2. *The cadence capture, passive.* Three minutes of beacons through the
   timestamped ti64dink (`11A` §2): the beat resolves to better than 0.1%,
   mean-over-span only, the number filed by this sitting against the
   park-loop timebase and nothing else (`LE-104`'s rule).

**Boot 2 — the positive-control image.** One power cycle. Expected on the
wire: the SMC excursion **seen** by the probe (a non-zero where every idle
window reads zero), beside idle windows that still read zero. This is ADR
0005's trap clause satisfied on silicon: the instrument has now produced both
answers where it counts. A control that sees nothing **stops the campaign** —
a zero from an instrument never shown to produce a non-zero stays an absence
of measurement, and the sitting files the failure instead.

**Boot 3 — the campaign image.** One power cycle, then wait out the stated
duration. Expected: the campaign distribution lines plus `verdict ok=true`,
largest excursion inside the bound the record will claim. **Filing this
closes Q3** — see §3.

**Boot 4 and 5 — only if S4 is spoken and §0's code exists.** The re-arm's
first *sustained* listen (frames counted, then more frames counted — the ear
proven, S1 closed), then `PING` answered and `STATUS` answered with a refusal
arm alongside (`--send` an unknown verb; the refusal must be *spoken*, named,
and rate-bounded). **That is M1 and M2 — the first conversation** — and the
capture that carries it parses to its own verdict like every capture since
`07F`.

Every boot: transfer digest confirmed in the serve log before a byte
executes; every capture committed with its boot's image sha; the display
line's new `fb_addr` field noted whichever way it reads.

## 3. Filing, the same day — the part that turns boots into state

1. **The qualification record completes**: a new Report (Q3 section — control
   seen, campaign distribution, largest excursion, environment as-stated)
   citing `REPORT-2026-08-07-01` for Q1/Q2/Q4;
   `goals/assurance/qualified-platforms.tsv` gains row one; the `0 / 5` tile
   moves for the first time. `LE-94` and `LE-95` get their dispositions
   (the relay automates what the owner's hand did; its row should say what it
   still buys — unattended sittings — and nothing more).
2. **The `G04` unlock is work, not a switch**: `bound_provenance` starts
   accepting rows from the qualified platform — file the first `G04`s from
   the campaign boot's own envelope, under ADR 0015's `irq_state` honesty
   (masked-region numbers say `masked`; nothing is quoted as live that was
   not measured live).
3. **Stories advance on what closed**: `-16` Verified if the five arms
   matched (or its re-arm gap stated); `STORY-P1-07-05`'s two remaining exit
   arms if a failing-fixture boot was spared a cycle for them; the cadence
   number against the park timebase.
4. **The dashboards move themselves** — this week made every count derived;
   run the gates and splice what they print.

## 4. The one decision this script cannot take

`S4`, verbatim from `10A`: *"Sprint rule lifted for the interaction chain
(`-16`'s re-arm, the admitted verb, Ti64Dink console) and nothing else."*
Everything is staged so that the sentence, once spoken, costs one laptop
evening (§0's fourth bullet — the charter reading is already done, the suite
already specified) and turns Boot 4/5 from conditional to scheduled. Without
it, Sitting B still closes Q3, still closes or diagnoses `-16` criterion 4,
still banks the cadence number — the sitting is worth having either way,
which is exactly why the decision is not being forced by it.

## 5. What this script refuses to include

No relay dependency — the owner's hand ran five cycles on 2026-08-07 and this
script needs five. No serial. No TCP/IP. No DT parse on the board (shape 2 is
decided; the quarantine-and-ship increment needs its chunking envelope
designed and is *not* smuggled in here). No `G04` filed from any capture that
predates qualification. And no new loose ends opened by intent: this sitting
exists to close `LE-94`'s decision, `-16`'s criterion, and the qualification
arc that `LE-103`'s instrument made possible — the register should be
*smaller* on its far side.
