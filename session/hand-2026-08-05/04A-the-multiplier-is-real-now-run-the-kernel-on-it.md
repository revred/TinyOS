# 04A — The Multiplier Is Real. Now Run the Kernel On It.

Session handover, written 2026-08-05 at the close of the day that
[`01A`](01A-the-need-for-speed-and-what-must-not-be-traded-for-it.md) opened by asking where
the speed actually comes from.

It comes from here. **TinyOS now boots over Ethernet.** The card swap — named in `01A` §4 as
the single largest remaining multiplier — is over. §1 is what the loop costs now, §3 is the
mandate, and §4 is the part that keeps this from decaying into speed without evidence.

---

## 0. The one-paragraph state

Fourteen commits on `main`. **`REPORT-2026-08-04-01` filed** and `LE-09` (release-blocking),
`LE-15`, `LE-24`, `LE-27` closed — the project has a hardware tier. **`STORY-P1-10-04`
board-proven** (`BOARD VERDICT 11`–`13`). **`STORY-P1-10-05` board-proven**: the die
temperature is on the wire and the `0x200` offset hypothesis held. **The spoor substrate's
overhead is measured** — stamp 138 cycles/op, announce 3101, drain 121955. **The measurement
envelope parses off the wire.** **And TinyOS netboots**, delivered by the firmware over TFTP,
emitting spoors on the cable it arrived on. Spine: **30 Features / 95 Stories / 79 Tests /
62 Reports, 76 loose ends (39 open)**, all gates green, everything pushed.

## 1. What one iteration costs now

```
cargo run -p xtask -- pi5 --fixture=measure
cp os/target/pi5/{kernel8.img,config.txt} <tftproot>/7bf18f79/
tos64-netboot --mac 88:a2:9e:11:4e:cc --root <tftproot>
# power-cycle the board
ti64dink --live 45 --text env.txt
```

Build, serve, power-cycle, read. **No card leaves the laptop.** The only physical act left is
the power cycle, and that is one action against the two card moves plus a power cycle it
replaces.

Set against where this project was two sessions ago — diagnosing Ethernet by **counting LED
blinks**, one bit of information per power cycle — that is the 10× asked for, and it is not an
estimate. Today's session ran **six board boots** and produced four board verdicts, and the
owner's total involvement was inserting a card twice, applying power, and typing one password.

## 2. The four frictions, closed

| `01A` §4 | State |
|---|---|
| The card shuffle | **Gone.** `BOOT_ORDER=0xf12`, `tos64-netboot`, image delivered over TFTP and run. |
| Password-gated `sudo` | **Gone.** `/usr/local/sbin/tos64-probe`, root-owned, read-only, `NOPASSWD` scoped to that one path. |
| Ti64Dink one-shot | **Gone.** `--until epoch-change\|rung=X\|text=Y`, `--text`, `--any`. |
| Spoor cost unmeasured | **Gone.** Three metrics in the envelope, `STORY-P1-10-02` criterion 6 discharged. |

## 3. The mandate — the kernel runs, but it does not run the machine

### Correction, filed 2026-08-05 against my own claim

**`01A` §5 and `03A` §3 say "the board has never run TinyOS's kernel". That is wrong**, and it
was wrong in two handovers and two commit messages before the register was checked properly.

The grep it rested on —

```
grep -rn "sched::|dispatch::|kernel::lock|wcet::" os/src/hal-arm64/src/*.rs   →   nothing
```

— asked the wrong crate. `hal-arm64` does not call them; **`kernel::fixture_measure_arm64`
does**, and that runs on the board. `kernel::context` carries a `#[cfg(target_arch = "aarch64")]`
context switch, and the evidence has been in every measure envelope since `BOARD VERDICT 5`:

```
D04  context_switch_yield_roundtrip_2switches   min=78     ← on silicon
D05  dispatch_select_highest_priority_ready     min=1534   ← on silicon
D05  dispatch_run_once_cooperative_round        min=1657   ← on silicon
```

A task is created, a context is switched, a dispatch round runs — on the Pi 5, a thousand times
per boot. This is the prose-versus-register class (`LE-30`, `LE-65`, `LE-70`, `LE-73`) with a
handover as the prose and the board's own envelope as the register.

### What is actually missing

**The kernel runs; it does not run the machine.** Dispatch happens only *inside*
`fixture_measure`: a timed region, interrupts masked by `STORY-P1-07-10`'s scope, results
discarded when the fixture returns. The board then falls into `hal-arm64`'s park loop, where the
tick increments a counter and **no task owns anything**.

So the mandate, stated correctly and much narrower than I had it:

**A dispatch round driven by the tick, with interrupts live, outside any measured region,
stamping a `Dispatch` spoor from a running system rather than from a fixture.**

That is a smaller piece of work than "port the kernel", and the reason it is smaller is that
the hard parts — the AArch64 context switch, the scheduler, the dispatcher — are already
proven on this hardware with numbers attached.

**Smallest honest increment: one task created, one dispatch round, one `Dispatch` spoor off
silicon.** Then the tick drives the dispatch round and the board stops being parked and starts
being an OS. `Category::Dispatch` and the call sites already stamp on the host path, so the
moment those subsystems *run*, the stream carries them for free — which is why the substrate
was built first.

This crosses from observability into execution and wants a new Feature under `EPIC-P1`,
decomposed just-in-time.

**And the loop is now fast enough to do it properly.** A dispatch bug on silicon used to cost
a card swap to observe. It now costs a power cycle and a 30-second capture, with a `--until
rung=...` watch that exits the moment the event you care about appears.

## 4. What speed must not cost — carried forward

`01A` §3 holds in full and is not restated. Today added four receipts of its own:

- **The tool was right and my harness was wrong.** Six confident test failures in the `--until`
  review were a stale binary. *Rebuild before you believe a failure.*
- **A diagnosis stated confidently was wrong.** I told the owner Windows Firewall was blocking
  the DHCP exchange and asked for elevation. It was not — the run had missed its window. The
  fix was to test the hypothesis (socket and packet capture side by side across one reboot),
  not to act on it. **Ask for a privilege only after proving you need it.**
- **A guard earned its keep on its first run.** `tos64-netboot` refuses to start without
  `--mac` and ignored five DHCP requests — from the *laptop itself*. Without that rule this
  bench tool would have answered its own host's DHCP.
- **The owner saw the gap before the tool did.** *"Capture starting mid cycle is a symbol of
  tooling gap"* — exactly right, and now `LE-76`: the text transcript carries no sequence, no
  epoch and no framing, so a mid-cycle capture is a *rotation* of the envelope, a lost line is
  invisible, and a capture spanning a reboot would interleave two boots indistinguishably. The
  fix is to carry the envelope **as spoors**, where all of that is already solved.

## 5. Also owed

- **`LE-76`** — the measurement envelope should be spoor records, not a cycling text
  transcript. Highest-value item after the mandate: it removes the last channel on this bench
  with no loss accounting.
- **`LE-73`** — `kernel::udp_wire` cites `STORY-P1-10-03`, which does not exist; then the gate
  that refuses a `STORY-*` citation with no filed Story.
- **`LE-75`'s actuation half** — the fan, driven from a validated reading. Sensing is in;
  acting is deliberately not. The calibration stands as *published constants corroborated on
  this board*, and `/dev/mem` and the debugfs regmap are both closed routes (the latter
  **reset the board** — do not repeat).
- **`FEAT-P1-09`'s exit criterion** — the beacon byte-compared against the frame builder.
  Ti64Dink now harvests `TOS64-PRESENT/1` lines, so this is a comparison away.
- **`STORY-P1-07-06` criterion 1** — the envelope parsed off the wire this session; the Story
  header has not yet been advanced to say so.
- **Netboot rough edges**: the first `kernel8.img` request is abandoned at block 1 and succeeds
  on the client's retry (a slow start, not a failure); and the TFTP root is currently
  `C:/tmp/tftproot`, which is transient. **The obvious next tooling win is a `--serial` flag so
  `tos64-netboot` serves `os/target/pi5` directly** — deployment then becomes build plus
  power-cycle, with no copy step at all.

## 6. Bench facts at close

- **Card: in the Pi 5, Pi OS role.** It is the *fallback*, not the boot path. `BOOT_ORDER=0xf12`
  is network-then-SD, so **stopping `tos64-netboot` boots Pi OS** — proven twice.
- **Board serial `d7bd1a077bf18f79`, TFTP prefix `7bf18f79/`, MAC `88:a2:9e:11:4e:cc`.**
  A netboot root needs only `config.txt` and `kernel8.img`; five of the eight requested files
  do not exist and the boot proceeds regardless. Beware the **doubled slash** in
  `7bf18f79//config.txt`.
- **A netbooted TinyOS has no SSH**, so returning to Pi OS is: stop the server, power-cycle.
- **Pi OS side:** `ssh revanur@raspberrypi.local`, key auth, and `sudo -n
  /usr/local/sbin/tos64-probe {avs|iomem|dmesg|eeprom}` needs no password.
- **Last image `92fa283a6d20`** (measure fixture, 11 metrics, spoor egress, boot epoch,
  retained certificate, thermal sampling).
- **Board epochs on record:** `0x049F8B28`, `0x04B328BC`, `0x04B32825`, `0x04B366E6`,
  `0x04C7D0FF`. The middle pair are 151 counter ticks apart — `LE-74`.
- **`ACT` nibble headroom: one verb.** `MAX_RECORDS` 181. The frame header's reserved padding
  is spent.
- Host `cargo clippy --workspace --all-targets` cannot build `kernel`'s `[[bin]]` on this
  Windows machine (`LE-64`'s class). **`check-boot-images` is the clean local signal.**
