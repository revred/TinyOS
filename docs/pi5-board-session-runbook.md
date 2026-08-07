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

## 0b. Pre-flight, added 2026-08-07 — binding before any capture on the wire era's bench

Three checks, each earned by a recorded incident, each cheaper than the failure
it prevents:

1. **One server, and it is the one you mean (`LE-87`).** Before serving
   anything: `netstat -abno | findstr :69` (elevated) or
   `Get-Process tos64-netboot`. Exactly one `tos64-netboot` may hold UDP 69,
   and it must be serving the root and image *you* built this sitting — a
   stale server silently winning the bind served a wrong image with a
   complete, plausible, entirely wrong envelope once already. If a server is
   already running that you did not start this sitting (one was observed live
   on 2026-08-07, `hand-2026-08-07/11A` §1), stop it and start your own.
2. **The qualification record's tripwire (`LE-117`).** If the SD card's Pi OS
   role boots for any reason this sitting — and always before filing new wire
   evidence — read the bootloader version (`vcgencmd bootloader_version` over
   ssh on the Pi OS role) and compare against
   `REPORT-2026-08-07-01`'s pinned EEPROM
   `086b83e3332dfc8927c56762771d082f3077a1ae` (2026-05-26). **A mismatch is
   not a failure; it is the record's void clause firing** — stop, and the
   response is a new qualification Report, never a quiet re-pin.
3. **If the canvas matters this sitting:** power the board while the monitor
   is *actively displaying a live source*, not merely awake — firmware scanout
   bring-up at power-on is nondeterministic (`hand-2026-08-07/07F` §7b holds
   the procedure). The wire is unaffected either way.

**Channel note.** §1's adapter loopback and the serial-capture halves of §3–§4
describe the PL011 era. Since 2026-08-07, `TEST-P1-07-01-A`'s dated amendment
retires the PL011 from the evidence path (`LE-47`): evidence rides Ethernet —
netboot with transfer digests, `ti64dink` captures (with arrival timestamps
since `LE-115`), and `xtask parse-meas` verdicts. Serial steps remain below as
the historical procedure and for a bench that someday has a working adapter.

## 0c. Commissioning the switched supply — one time, before it drives anything (`LE-95`)

The tenth stage of the board evidence loop is a LAN-controlled mains plug, and
`tos64-power` has driven four dialects since 2026-08-06 with nothing to drive.
The bench device is a **Shelly Plus Plug UK** (Type G), which speaks the Gen2
JSON-RPC surface: `--dialect shelly-gen2`. No code was needed — the dialect's
command, readback and the `was_on` trap were already written for it.

**Commission it over its own access point, not through the app**, if you can:
the plug boots into a `ShellyPlusPlugUK-XXXXXX` WiFi AP whose web UI at
`http://192.168.33.1` joins it to your network with no account anywhere. The
phone app works too, but it invites a cloud login that `LE-95` rules out at any
price. Either way, finish with the four settings below.

**The four settings, and why each one is not optional.** Everything after the
first is a way for a correctly-written fail-safe to be silently defeated by the
device underneath it, so each is written as the RPC that sets it and the RPC
that proves it:

1. **A fixed address.** DHCP reservation on the router, or a static IP on the
   plug. `board-run --plug=` names a URL; a plug that moves is a bench that
   switches whatever now holds that address. Prove it with
   `http://<ip>/rpc/Shelly.GetDeviceInfo` and check the `id` is the plug you
   mean.
2. **Power-on state must be `on`.**
   `http://<ip>/rpc/Switch.SetConfig?id=0&config={"initial_state":"on"}`.
   This is the important one. `tos64-power`'s first rule is that it never
   leaves the board off, and it keeps that rule across every path it controls —
   but it cannot keep it across a *plug* reboot. A firmware update, a brownout
   or a WiFi-driven restart with `initial_state` at `off` or `restore_last`
   leaves the board dark with no hand on the bench, which is the exact stall
   the whole tool exists to remove. Prove it with
   `Switch.GetConfig?id=0` and read `initial_state` back.
3. **No auto-off timer.**
   `Switch.SetConfig?id=0&config={"auto_off":false,"auto_on":false}`.
   An auto-off timer is a countdown to a power cut that no gate in this repo
   can see. `ADR 0005`'s Q3 residency campaign is sixty seconds of accumulated
   window time and future soaks are longer; a plug that helpfully switches off
   after an interval turns a campaign into an unexplained short capture.
4. **Cloud disabled, not merely unused.**
   `http://<ip>/rpc/Cloud.SetConfig?config={"enable":false}`. `LE-95` requires
   local control with no vendor account, and a bench that cannot reboot the
   board while somebody else's service is down is a new instrument failure —
   this bench has had five. Bluetooth may stay on for recovery; nothing in the
   loop uses it.

**Do not enable the plug's "restrict login" (HTTP auth).** `tos64-power` sends
plain unauthenticated requests and has no credential path, so enabling it turns
every call into an unreachable plug — which the tool correctly reports as
`UNKNOWN` rather than as off, but the bench stops working. If the plug must
live on a segment where auth is required, that is a code change to
`PlugClient`, not a setting.

**Then verify against the real device, in this order.** The exit codes are
distinct on purpose:

```
tos64-power --plug http://<ip> --dialect shelly-gen2 status   # expect ON, exit 0
tos64-power --plug http://<ip> --dialect shelly-gen2 off      # then status: OFF
tos64-power --plug http://<ip> --dialect shelly-gen2 on       # then status: ON
tos64-power --plug http://<ip> --dialect shelly-gen2 cycle --off-ms 5000 --on-wait 20
```

`0` done and confirmed · `1` refused by a guard · `2` usage · `3` **UNKNOWN —
asked, not confirmed** · `4` **THE BOARD MAY BE OFF**. A `3` or a `4` is a
finding, not a retry: `4` in particular means a hand is owed to the bench and
the next session must know it.

**The hazard the tool does not guard, because it cannot see it.** This plug cuts
mains to the Pi's PSU — a hard cut, with no clean shutdown. `TransferGuard`
refuses a cycle while a TFTP transfer is in flight, but it knows nothing about
the SD card: **never cycle while the card's Pi OS role is booted**, because a
write in flight to the filesystem is a corrupt card and this bench owns one Pi.
The TOS64 role netboots and holds no mounted writable filesystem, so cycling it
is safe by construction; the ground-truth role of §6 is not. Shut the Pi OS role
down over ssh first, and only then cycle.

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

**One-command card preparation:** [`docs/pi5-prepare-sd.ps1`](pi5-prepare-sd.ps1)
does the format *and* the verified copy in one elevated run —
`powershell -ExecutionPolicy Bypass -File docs\pi5-prepare-sd.ps1 -DiskNumber <n>`.
It refuses disk 0, refuses non-removable disks, refuses a card carrying data
(unless `-Force`), demands a typed `YES`, formats MBR + 2 GiB FAT32 labelled
`TOS64BOOT` (the EEPROM bootloader cannot read exFAT, and Windows cannot
FAT32-format >32 GiB — hence the small partition), then copies both files and
verifies the copied `kernel8.img`'s SHA-256 against the build before declaring
the card ready. A failed verification deletes the bad copy rather than leaving
a plausible-looking card.

## 3. Wire and run

1. SD card into the Pi. Adapter into the 3-pin debug connector; USB into the laptop.
   PSU connected but **off**.
2. From `os/`:

   ```
   cargo run -p xtask -- pi5 --fixture=boot --port=COM<n> --board-rev="<sticker text>" [--firmware=<version if known>]
   ```

3. Power-cycle the board when the tool says so, and let the capture finish.

## 3b. What the screen shows (since `STORY-P1-07-07`, 2026-08-03)

A monitor on HDMI is now a first-class boot indicator: **success shows
"TinyOS" in block letters, centred at the display's native resolution**
(fallback 720p), painted strictly after the serial verdict — designed
behaviour, not yet confirmed on silicon. A quiet dark screen means *boot
possibly succeeded but the splash path failed* — only the serial capture
distinguishes that from a hang. A firmware diagnostic screen still means the
boot never reached our code.

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

## 6. The ground-truth card: one-time prep for unattended captures (2026-08-05)

`hand-2026-08-05/01A` §4 friction 2: the Pi OS ground-truth card gates `sudo` on a
password, which blocked the one thing the thermal work still needs — a paired
raw-register/`thermal_zone0` reading (`LE-75` calibration, `TEST-P1-10-05-A`
clause 7) — and makes every future SSH ground-truth capture attended. The cure is a
**one-time** change made the next time the Pi OS card is booted, after which every
capture runs unattended.

**Least authority, not blanket `NOPASSWD`.** The only step that needs root is the
`/dev/mem` mmap probe; everything else the captures read (`thermal_zone0`, sysfs,
`dmesg` via group membership) does not. So the probe gets a fixed root-owned home
and `NOPASSWD` covers exactly that path:

```
# on the Pi, over ssh revanur@raspberrypi.local, once:
sudo install -o root -g root -m 0755 /tmp/rp1rd /usr/local/bin/tos64-probe
echo 'revanur ALL=(root) NOPASSWD: /usr/local/bin/tos64-probe' | \
  sudo tee /etc/sudoers.d/010-tos64-probe
sudo visudo -c    # refuse to leave the session until this prints "parsed OK"
```

Rules, so this stays an instrument and never becomes a hole:

1. **The probe binary is root-owned at a fixed path.** `NOPASSWD` on anything under
   `/tmp` or `/home` would let any code running as `revanur` edit what root runs;
   `install` to `/usr/local/bin` first is the entire difference.
2. **`visudo -c` before logout.** A syntax error in a sudoers drop-in locks `sudo`
   out entirely, and this bench has no keyboard on the Pi to recover with.
3. **Scope stays one binary.** The day a capture needs a second privileged read,
   extend the probe, not the sudoers line.
4. This card is the **ground-truth instrument** (`tos64-cardswap pios`), reachable
   only over the direct cable's link-local address; it holds nothing but a stock
   Pi OS and the probes. The TOS64 card is untouched by any of this.
