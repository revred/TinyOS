# 02A — STATE=BEACONING: the Exit Is One Elevated Capture, and the Screen Work Is Next

Session handover, written 2026-08-04 after the morning session that executed 01A start to
finish. Read this top to bottom, then act. The repo tree is **clean** — nothing uncommitted,
nothing untracked, four CI runs green through `8b58602`. The SD card is **in the laptop, in
the TOS64 role**, carrying the beaconing kernel. The board is powered off.

---

## 0. The one-paragraph state

`STORY-P1-09-15` landed exactly as 01A shaped it and the first boot answered **criterion 5
in its success arm**: the beat line read `STATE=BEACONING` where every prior boot stopped or
parked (BOARD VERDICT 4 in the ground-truth file, ~02:07), and the laptop's linkwatch found
the wire already trained at 1000 Mbps before the watch even armed. The one recurring disease
of the whole marathon is dead — all three instances of *state Linux programs and the
firmware does not* (outbound window `-09`, endpoint BARs `-13`, inbound DMA windows `-15`)
are now written by `pcie::establish`, each believed only from readback. Spine: 29 Features /
**90 Stories / 74 Tests**, 246 hal-arm64 tests, codes 1–22, fmt + both clippy targets clean.
The `work/tools/` C# fleet is committed (sources only; `bin`/`obj` ignored) and the
perpetual soak log is checked in — `git status` is empty.

## 1. What this session proved and recorded (all on `main`, all CI-green)

| Commit | What |
|---|---|
| `04af8f2` | `STORY-P1-09-15` — the `inbound` module in `pcie.rs`: twelve dwords derived from the captured `dma-ranges` triples through the driver's own size encoding (pinned as a transcription test), written once after enumeration, readback-believed, idempotent; refusal codes 21 (`ibw-held`) / 22 (`ibw-remap`), decisive low halves; TDD Red-first |
| `d727ece` | the verdict — BOARD VERDICT 4 appended to `pios-ground-truth-2026-08-03.txt`; story/test/feature statuses advanced to every-criterion-Green |
| `de37c1d` | dashboard — the Epics tab had drifted three sessions: `FEAT-P1-09` had **no row at all**, `FEAT-P1-07` still read "blocked on an adapter"; both fixed, EPIC-P1 now reads 9 Features / 42 Stories |
| `8b58602` | `work/tools/` fleet committed (sdprep, imgwrite, cardswap, linkwatch, serialwatch — owner settled commit-or-ignore); `.gitignore` learns `bin`/`obj`/`out`/smoketest |

`pktmon` was retried from an unelevated shell and is **still access-denied** — same refusal
as 08-03. That is the only thing between `FEAT-P1-09` and its exit criterion.

## 2. Step 1 — the beacon capture (needs an ELEVATED shell; ~10 minutes of board time)

The exit criterion: *the beacon frame appears in a stock host packet capture and is
byte-identical to the pinned frame the host tests assert.* The frame to expect
(`gem.rs`, pinned word-for-word by `the_beacon_frame_is_exact_bytes_…`):

- destination `FF:FF:FF:FF:FF:FF`, source `02:54:4F:53:36:34` (locally administered,
  ASCII `TOS64`), EtherType `0x88B5` (IEEE local experimental)
- payload `TOS64-PRESENT/1 board=pi5-bcm2712 seq=<decimal>`, zero-padded to 60 bytes
- one frame per park-loop period, `seq` monotonically increasing

The runnable sequence:

1. Boot the board with the card as it is (it already carries the beaconing kernel
   `7be16dee…` — no rebuild needed unless code changed; if it did, §4 is the card workflow).
2. **Elevated** PowerShell on the laptop (Start → "PowerShell" → Run as administrator):

   ```powershell
   pktmon filter remove
   pktmon filter add -d 0x88B5          # EtherType filter: only the beacon
   pktmon start --capture --pkt-size 0 --file-name $env:TEMP\beacon.etl
   # wait ~30 s while the beat line ticks STATE=BEACONING
   pktmon stop
   pktmon etl2txt $env:TEMP\beacon.etl -o $env:TEMP\beacon.txt -v 3
   Select-String TOS64-PRESENT $env:TEMP\beacon.txt
   ```

   (If `pktmon` still refuses even elevated, Wireshark/tshark on the laptop NIC with
   display filter `eth.type == 0x88b5` is an acceptable stock capture; the point is the
   bytes, not the tool.)
3. Byte-compare a captured frame against the pinned shape above. Then:
   - file `REPORT-2026-08-04-01` with the raw capture lines quoted verbatim;
   - `FEAT-P1-09`'s exit criteria are met — walk the Feature and its fifteen Stories'
     board criteria against the evidence now in hand (most cite exactly this capture or
     the linkwatch training already recorded); statuses advance per the LE-44 join
     discipline (story header and feature cell must name the same criteria — 09A §8);
   - the spine ritual for status moves: headers → feature cells → `emit-dashboard`
     splice → the two count strings → `check-assurance-spine`.
4. **The owner's amendment activates the moment the capture exists:** diagnosis moves onto
   the cable as `TOS64-*` envelopes into Ti64Dink, and the screen returns to bootstrap.
   `FEAT-P2-10` (Ti64Dink's promotion — the capture path that would have replaced pktmon)
   is unblocked and becomes the natural next Feature on priority 1.

## 3. Step 2 — "the next version of the OS painted on the screen" (owner priority 2)

What the screen shows **today** (verified this morning, BOARD VERDICT 4): slow blue fill →
`TinyOS` title → the `TOS64-LINK/1` report line → the ticking `TOS64-BEAT/1` line with
`STATE=BEACONING`. That is `STORY-P1-07-07`'s splash plus `-09`'s canvas — bootstrap
instruments, deliberately.

The next screen version is **owner priority 2: micro-HDMI splash → OS**, and it has
deliberately *not* been decomposed (just-in-time rule). The session that starts it must:

- Re-read the standing order first: **wire-first diagnostics** (08A amendment) — no new
  *diagnostic* canvas surface; the screen work is UX, not evidence. The two priorities do
  not conflict: the beacon capture (Step 1) satisfies priority 1's exit, and the screen arc
  is what priority 2 always was.
- Read [`docs/whole-system-context.md`](../../docs/whole-system-context.md) (destination
  architecture and horizon labels) and the UX V1 mandate (satellite architecture,
  hand-2026-07-30/07A; V1.2 generators and V1.3 responsive remain open) before proposing
  the Feature shape. Remember the Pi 5's *destination* display is remote (Ti64Dink over
  the cable — the headless direction); the local micro-HDMI "splash → OS" is the boot
  experience the owner ordered, not a compositor commitment.
- Decompose under the spine: new Feature (or extension of `FEAT-P1-07`'s display seam)
  with its contract row **before code**, test doc first, `hdmi.rs`'s canvas as the seam —
  it already owns geometry, glyphs and the hostile-descriptor gates.
- Bring a proposal to the owner before building anything beyond the existing canvas: what
  "OS painted on the screen" shows (status? console? the Ti64 surface mirrored?) is a
  product decision the owner has not yet specified beyond "splash → OS".

## 4. The card workflow (runnable, end to end — this is the loop you asked for)

Everything runs from the repo root except the xtask build (from `os/`). The card must be
in the laptop (it is).

```powershell
# 1. status — know the role before touching anything
.\work\tools\cardswap\bin\Release\net10.0\tos64-cardswap.exe status

# 2. rebuild ONLY if code changed since 8b58602 (current staged kernel: 7be16dee…)
cd os; cargo run -p xtask -- pi5 --fixture=boot; cd ..
#    prints the placement contract: os_check=0, kernel=kernel8.img, pciex4_reset=0
#    (all three load-bearing; the printout explains each)

# 3. stage the TOS64 role (idempotent; verifies sha256 after copy)
.\work\tools\cardswap\bin\Release\net10.0\tos64-cardswap.exe tos64

# 4. arm the instrument BEFORE power (kill stale instances first)
Get-Process tos64-linkwatch -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Process .\work\tools\linkwatch\bin\Release\net10.0\tos64-linkwatch.exe

# 5. safely eject → card into the Pi 5 → monitor on → power.
#    Expect on screen within ~5 s: blue fill → TinyOS → TOS64-LINK/1 … → TOS64-BEAT/1
#    STATE=BEACONING. Expect linkwatch: one Down -> UP at 1000 Mbps, then silence.

# 6. transcribe the two canvas lines into the session log / ground-truth tail
#    (append-only, keep the ===== BOARD VERDICT N ===== header discipline).

# 7. after the session: poweroff (unplug), card back to the laptop.
#    cardswap pios restores the Pi OS role if a ground-truth capture is ever owed again.
```

Notes that bite: cardswap prints `role: unknown kernel` when the card is in the Pi OS
role — not an error (09A §6.4). If the tools are missing binaries after a fresh clone,
`dotnet build -c Release` in each `work/tools/<tool>/` — binaries are no longer committed.

## 5. Bench facts at close

- **Card: laptop, TOS64 role**, kernel `7be16dee5f410504fce348551e1f6ef7b6e3169dcf3b08bad221def9e06ff8a2`
  (the `-15` beaconing build) verified on card; `pios-backup\` retained.
- Board off; cable connected. The wire has now trained on **three consecutive boots**; the
  beacon transmits every period. Boot-to-canvas is ~5 s.
- linkwatch stopped cleanly after logging the power-off `UP -> Down` at 02:11:06.
- CI green through `8b58602`; `git status` empty; the stash created and dropped mid-session
  left nothing behind (verified by diffstat before dropping).
- `pktmon` needs elevation — that is Step 1's whole prerequisite. Nothing else blocks.
- Deep operational context (SSH runbook, probe source, quoting traps, contingency trees):
  [hand-2026-08-03/09A](../hand-2026-08-03/09A-window-poisoned-inbound-path-indicted.md).
  Spine ritual gotchas: 09A §8. This note supersedes 01A's "what next".

The method holds: ask the board a question it can answer with a number, believe only
readbacks, and let each verdict choose the next story. Tonight the answer was a word —
`BEACONING` — and the next question belongs to a packet capture and then to the screen.
