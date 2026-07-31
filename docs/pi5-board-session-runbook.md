# Pi 5 Board-Session Runbook — the Operator's Steps for the First Silicon Capture

Status: **living runbook — owned by [`STORY-P1-07-05`](../goals/stories/STORY-P1-07-05.md); follow it verbatim the day a board, SD card and serial adapter are in hand**

One successful run of this runbook closes the board halves of `STORY-P1-07-01`
(criteria 3 and 4), the vector-install half of `STORY-P1-07-02`, and criteria 2 and 3
of `STORY-P1-07-05` — the first hardware evidence in the project's history, and the
first demo of the 08A hardware-evidence sprint.

## 0. What you need on the desk

| Item | Requirement |
|---|---|
| Raspberry Pi 5 | any revision; note the sticker text — it goes in the run record |
| SD card | any size; needs a FAT32 partition (≤32 GB formats natively on Windows) |
| USB-serial adapter | **3.3 V logic**, wired to the Pi 5's **dedicated 3-pin JST-SH debug connector** (between the two micro-HDMI ports) — **never** the GPIO header pins. The official Raspberry Pi Debug Probe is the known-good tool |
| Power | the 27 W USB-C PSU; leave it unplugged until the capture tool says power on |
| This laptop | the repo at the commit you will capture with; the tool records that commit |

TinyOS's bring-up image has no display output. The debug UART (115200 8N1) is the
only place the board can speak; without the adapter a working boot and a dead board
look identical.

## 1. Loopback-test the adapter first (`TEST-P1-07-01-A` clause 1)

**Why this exists.** Between the keyboard and the Pi's silicon there are five
independent links — adapter silicon/driver, COM-port settings, the adapter's TX/RX
circuitry, the cable/connector wiring, and the board itself — and any of them
produces *identical silence*. The loopback cuts the chain in half before the board
is involved: jumper the adapter's **TX pin to its RX pin**, so every transmitted
byte comes straight back. Echo proves links 1–3; from then on, "silence" from a
board run means the problem is on the board side. This is the repo's standing
positive-control principle applied to the instrument itself: a detector that has
never been shown to detect a byte cannot be believed when it reports nothing.

1. Plug the adapter into the laptop **with a jumper joining its TX and RX pins**
   (on the official Debug Probe, jumper the TX/RX contacts of the supplied cable's
   free end).
2. Open the enumerated COM port at 115200 8N1 with any serial terminal (or ask the
   session agent to run the echo check) and type — every character must echo back.
3. Remove the jumper. If nothing echoed, stop: fix the adapter before touching the
   board, or every later "silence" result is uninterpretable.

**Honest limit:** loopback proves the adapter, not the pigtail/crossover to the Pi —
those are proven only by the first byte that actually arrives from the board. That
residual is exactly why §4's silence triage re-checks the connector *second*, after
re-loopback and before suspecting the card.

## 1b. Wiring the Pi side

The debug socket is the 3-pin **JST-SH 1.0 mm** connector between the two
micro-HDMI ports; its pinout per the Raspberry Pi debug-connector spec is
**TX (out of the Pi) · GND · RX (into the Pi)**.

- **Official Debug Probe (recommended):** JST-SH cable from the Pi's debug socket
  into the probe's **"U" (UART) port**, probe into USB. Crossover and voltage are
  handled for you.
- **Generic adapter + JST-SH pigtail — three rules:**
  1. **Crossover:** Pi TX → adapter RX, Pi RX → adapter TX. TX-to-TX is the most
     common wiring mistake; at 3.3 V it is harmless but perfectly silent.
  2. **Common ground:** GND → GND, always — UART is single-ended and TX means
     nothing without the shared reference. Missing ground reads as garbage or
     silence.
  3. **Connect nothing else.** Never wire the adapter's VCC/5V/3V3 to the Pi — the
     board powers itself, and back-feeding the debug port is the one mistake that
     damages rather than mutes. The adapter must be **3.3 V logic**; many cheap
     ones are 5 V or carry a voltage-select jumper — check it.
- **Settings:** 115200 8N1, **no flow control** (the 3-pin connector has no
  RTS/CTS lines; the capture tool already opens the port that way).

**Order of operations, and why:** serial wired and *capturing* before power, PSU
last. The board's first bytes are the most valuable ones — `CurrentEL` printed
first is acceptance-criterion evidence and the start of `Q1` under `ADR 0005`, and
serial has no replay: a port opened after power-on has lost them forever. The
capture tool holds the port open and then tells you when to switch on, so the
transcript is guaranteed to span t=0.

## 2. Build and place the image

```
cargo run -p xtask -- pi5 --fixture=boot        # from os/
```

This builds `kernel8.img` **at the current commit** (never reuse a stale image — the
run record binds commit to capture hash) and prints the placement. Copy to the SD
card's FAT32 boot partition:

- `kernel8.img` (the tool prints its size and sha256 — the record keeps both);
- `config.txt` containing **both** lines:
  `os_check=0` and `kernel=kernel8.img`.
  Without `os_check=0` the Pi 5 firmware relocates the image to `0x200000` and
  execution starts mid-image — total silence, and no test can catch it because
  `config.txt` lives on the card.

The Pi 5's bootloader is in on-board EEPROM; no other firmware files are needed.

## 3. Wire and run

1. SD card into the Pi. Adapter into the 3-pin debug connector; USB into the laptop.
   PSU connected but **off**.
2. From `os/`:

   ```
   cargo run -p xtask -- pi5 --fixture=boot --port=COM<n> --board-rev="<sticker text>" [--firmware=<version if known>]
   ```

3. Power-cycle the board when the tool says so, and let the capture finish.

## 4. Read the result — five distinguishable exits

| Exit | Meaning | What you do |
|---|---|---|
| **0** | Pass. Transcript shows `current_el=` **first**, the `TOS64-BOOT/1 READY` sequence, `vbar … match=yes`, then `TOS64-RESULT/1 fixture=boot ok=true` | Celebrate briefly, then §5 |
| **1** | The board's own verdict said failure | Read the transcript; the board is talking, which is already progress — file what it said |
| **2** | Harness error (port wouldn't open, bad arguments) | Fix the host side; the board is not implicated |
| **3** | **Silence** — not one byte. The *common* bring-up case | Triage **in this order**: adapter (re-loopback) → connector seating/muxing → `os_check=0` actually on the card (divergence record §§1–4). Do not reorder the triage |
| **4** | Spoke, then stopped — bytes but no trustworthy verdict | Read the partial transcript; the last line printed is the first clue to where it died |

A capture from an image built before 2026-07-31 shows `TINYOS-*` prefixes and fails
the current parser — that is the TOS64 rename working, not a board fault; rebuild.

## 5. After a pass — evidence, statuses, and what not to claim

1. Every run already wrote `capture.log` + `record.tsv` (commit, port, baud, board
   revision, firmware, image and capture SHA-256, end reason, outcome, exit code,
   timestamp) under `os/target/pi5/runs/` — that attributable pair **is** the
   evidence; never quote a screen-scrape instead.
2. File the dated Report quoting the raw transcript; move the Story/Feature/dashboard
   statuses together (the `LE-44` machinery checks them jointly): `-01` criteria 3–4
   close, `-02`'s vector half closes, `-05` criteria 2–3 close.
3. The entry exception level printed first is the beginning of **Q1** qualification
   evidence under `ADR 0005` — record it in the Report.
4. **Claims discipline:** this is mechanism evidence on an **unqualified platform**.
   No timing figure, no worst-case bound, no "hard real-time" statement — `LE-09`
   itself closes only on `STORY-P1-07-06`'s measured Report, and bounds additionally
   need the platform qualification that no board yet has.
5. Next rungs, in the non-negotiable order: `-02`'s deliberate-fault board criterion,
   then `-03` (MMU/caches), `-04` (GIC/timer), `-06` (`TOS64-MEAS/2` on silicon).

## If it goes wrong repeatedly

Silence after a clean triage pass usually means the image or the card, not the code:
re-image the card from scratch, verify `kernel8.img`'s sha256 against the tool's
printout, and confirm the card's first partition is FAT32 (not exFAT). The transcript
of *any* speaking run — even a failing one — is worth committing as an asset; a board
that talks is a board that can be debugged.
