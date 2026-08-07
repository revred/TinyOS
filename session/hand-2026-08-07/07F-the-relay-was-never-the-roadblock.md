# 07F — The relay was never the roadblock: two power cycles by hand, and the qualification record goes from zero parts to Q1+Q2+Q4

Session letter **F**, concurrent with session **D** (whose `06D` and register rows
`LE-113`–`LE-116` are uncommitted in this tree — see "What is deliberately not
committed"). The owner opened this session with: *the board is online, the cable
is connected, I can cycle power — remove every critical roadblock that can be
removed now.* That sentence dissolved the premise four handovers were written
under. `LE-95`'s £15 relay automates a power cycle; it was never the only source
of one.

**The one sentence, if only one survives:** *ADR 0005's qualification record for
`rpi5-bcm2712` now holds Q1 complete, Q2 complete and Q4 stated
(`REPORT-2026-08-07-01`), the Q3 instrument `LE-103` demanded is built
test-first and proven on silicon, and the board evidence loop now produces
passive wire captures that carry their own verdict — `xtask parse-meas` exit 0,
`ok=true`, for the first time in this project's history.*

## 1. What ran on the board — two boots, both captured

Two images, both netbooted (serve log confirms each transfer digest before a
byte executed), both power-cycled by the owner's hand:

| boot | kernel8.img sha256 | new on the wire |
| --- | --- | --- |
| 1 | `77b73ea0f279…d61c` (308,360 B) | `TOS64-QUAL/1` lines: `boot_entry current_el=EL2 raw=0x8 now_at=EL1 firmware_cntvoff=0x0`, `counter_split`, `residency` |
| 2 | `223a7f90d618…2067` (308,528 B) | all of the above **plus `TOS64-RESULT/1 fixture=measure ok=true` riding the transcript** |

Captures committed as `goals/reports/wire-qual-2026-08-07.txt` and
`wire-qual-2026-08-07-verdict.txt`. The second parses to a **verdict**:
`LE-110`'s caveat ("a measurement stream without a verdict is half an
instrument") is closed by one `transcript::record` beside `report_result` —
every future passive capture is now a pass/fail, not a transcript.

## 2. `LE-103` closed — the Q3 instrument, corrected and board-proven

- `hal_arm64::timer` gains `PhysicalCounter` (`CNTPCT_EL0`), `CounterSplit`,
  and `probe_residency_window` — **windowed on the physical counter**, so the
  window's length cannot be handed to whatever owns `CNTVOFF_EL2`. TDD: the
  five host tests went red first, and one of them scripts a mid-window offset
  move and asserts it surfaces as the two counters' advances disagreeing —
  the exact blindness `LE-103` recorded, pinned as a failing case.
- On silicon (both boots): physical and virtual advances agree to 1–2
  read-skew ticks over 540,000-tick windows, `PMCCNTR` at 2400.0 MHz against
  the physical window, and the **firmware-left `CNTVOFF_EL2` read `0x0` at
  EL2** in the one instruction window this kernel has before zeroing it.
- The raising session refused to land an unrun probe; honoured — the row
  closed only after the board ran it.
- `check-boot-images` caught a real cross-target break mid-session (trait
  method ambiguity in aarch64-only code no host test compiles). The gate's
  standing instruction earned its keep again.

## 3. Q2 — determined, not "closed firmware, cannot say"

Full determination with citations in `REPORT-2026-08-07-01`. The headline that
changes the project's mental model: **the Pi 5 is not the Pi 4.** Stock EL3
hosts a resident TF-A BL31 (public source, raspberrypi/arm-trusted-firmware;
binary embedded in the closed EEPROM), live for PSCI-over-SMC at runtime — but
the GIC is configured with **no Group 0 interrupts and no SPD, so nothing
asynchronous is ever routed to EL3**. Handoff at NS-EL2, AArch64 only.
Corroborated on this bench: `current_el=EL2` on the wire, `firmware_cntvoff=0`,
and the netboot log caught the firmware requesting `armstub8-2712.bin` — the
documented hook by which TinyOS could take EL3 wholesale if the owner ever
wants the ADR's "very strong Q2" route.

## 4. What remains for qualification, exactly

Q3 alone: a campaign with stated duration, sample count, distribution,
environment — **and the silicon positive control first** (inject a known
perturbation, watch this probe see it), per ADR 0005's trap clause. The host
test is the logic's positive control; it is not the silicon's. `LE-94`'s owner
decision is now that one campaign.

## 5. Housekeeping done

- **`LE-111`**: source clauses added to `PERF-D07-G23`, `PERF-D11-G01`,
  `PERF-D11-G03` naming the metrics=12 capture and the same-boot re-read.
  Wording pins the re-read to its date, because the live wire now runs a
  *different* boot (this session's) — which is `LE-111`'s own lesson applied
  to the fix.
- `MAX_LINES` 24→28 with the derivation comment updated (21→22 lines).
- ti64dink's `--text` harvest now carries `TOS64-QUAL/1` lines (dedup'd);
  `parse-meas` ignores them by construction (sentinel is `TOS64-MEAS`).

## 6. Observations for the next session

- **The canvas is dark again** (owner observed a blank monitor on boot 2;
  boot 1 not checked). `hdmi_force_hotplug=1` IS in the staged config.txt, so
  this is not `LE-98`'s original shape; suspects are the port (must be HDMI0,
  nearest USB-C) or hotplug timing. **The wire was unaffected** — all evidence
  this session is machine-parsed from Ethernet. Diagnose before trusting any
  criterion that needs the canvas.
- **`LE-115` deliberately not done** (ti64dink arrival timestamps): touching a
  bench instrument that gates nothing (`LE-114`) for a measurement nothing
  this session needed was the wrong trade. It remains open and small.
- **A cwd trap cost one capture window:** a background `dotnet run --project
  work/tools/…` launched from `os/` failed on a relative path and produced an
  empty log that read like a dead capture. Absolute paths for background bench
  commands.
- Stale-server discipline held: the first netboot server was stopped before
  the rebuilt image was re-served (`LE-87`).

## 7. What is deliberately not committed, and why

Session D's work is live and uncommitted in this tree: `06D`, register rows
`LE-113`–`LE-116`, `governance.rs` (LE-113 part one), assurance-module edits,
netboot doc comment. Per `CONCURRENT_SESSIONS.md` rules 1/3, none of it is
staged here. **Consequence:** this session's register close (`LE-103`),
dashboard counts (57 open / 63 Reports) and regenerated `feasibility.html`
cannot be committed coherently until session D's rows land — the spine gates
tie the Report count and the register state together. The code, the Report,
the captures and this handover are committed; the register/dashboard/
feasibility trio waits for one coordinated commit after session D lands.
Whoever makes that commit: the counts in the working tree are already green
(`check-assurance-spine` clean at 116/57).

## 7a. Added later the same session: the blank monitor diagnosed from the wire

The owner wanted the monitor alive. Instead of guessing, the display outcome
was put on the wire — `TOS64-DISPLAY/1` (`ethernet::display_line`, TDD, one
transcript line per boot) — and a third hand power-cycle answered:

```text
TOS64-DISPLAY/1 native=1920x1080 fb=refused
TOS64-RESULT/1 fixture=measure ok=true
```

**The firmware sees the display perfectly** (native size answered = panel up,
EDID read, monitor awake on HDMI0), the canvas gate is open, TinyOS is
painting — and the screen is blank. That is `LE-98`'s documented unfinished
half by its own words: *a display existing is not the same as a framebuffer
existing at `SIMPLEFB_BASE`* — a constant captured from a Pi OS **SD** boot.
The record's pattern agrees: every canvas ever photographed (2026-08-04
verdicts 5–9) was an SD boot; every netboot (`LE-98`'s raising run included)
has been dark. Working hypothesis, now with wire evidence behind it: **the
firmware's scanout allocation differs between SD boot and netboot**, and the
DT parse that would resolve it is the fix `02D`/`06D` explicitly deferred.
Do not start it as a side effect; the diagnosis line is in the tree and the
next LE-98 session starts from evidence instead of a dark screen. To *see*
the OS state today, SD-boot the same image (`sdprep`/`cardswap`) — that path
painted every canvas this project has photographed.

## 7b. Resolved later still: the splash lit, and the constant was never wrong

The owner ordered the display fixed regardless of boot path. The elimination
run that followed — netboot vs SD boot (both dark, both `native=1920x1080`),
config without `hdmi_force_hotplug=1` (dark), the exact 2026-08-04 two-line
config queued — ended unexpectedly: after the card's Pi OS role booted to a
visible desktop and the board was cycled once more, **the TinyOS splash
appeared on the monitor**, from a netboot of the current image with the
pinned four-line config, `verdict ok=true` on the same boot's wire.

So: `SIMPLEFB_BASE` is still correct, every config flag is exonerated, and
the discriminating variable was **the monitor's state at the instant of
power-on** — "awake with its OSD up" did not get scanout brought up;
"actively driven moments earlier" did. The firmware's display bring-up at
power-on is nondeterministic in a way `TOS64-DISPLAY/1` cannot yet see:
`native=` answered 1920x1080 on dark boots and lit boots alike, so the size
query and scanout setup are **separate firmware facts**, and the wire only
carries the first. `LE-98` stays open with its hazard unchanged (the 4 MB
write to an unverified constant), and the owner has ordered its durable fix:
a C4-contained DT-parse Feature (option 1 of the row's three), sprint rule
lifted for it — decision taken 2026-08-07, this session, recorded here.

Bench procedure that lights the canvas, until then: power the board while
the monitor is actively displaying a live source (not merely awake), and if
it comes up dark, drive the monitor from another source and cycle again.

## 8. Standing instructions, one extension

All previous instructions hold. The extension this session earned: **a blocker
without its scope becomes a blocker on everything near it** (`LE-110`) now has
a corollary — *an automation gap is not an availability gap.* The relay
automates the power cycle; with an owner on the bench the whole loop runs
today. Ask what the blocker actually blocks before queueing work behind it.
