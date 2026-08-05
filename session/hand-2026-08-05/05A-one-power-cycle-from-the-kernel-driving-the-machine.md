# 05A — One Power Cycle From the Kernel Driving the Machine

Session handover, written 2026-08-05 after the day that ended the card swap, filed the first
hardware Report, and corrected a claim this project had been carrying in three documents.

**The next session opens on a bench action, not a design question.** §1 is that action. §2 is
the correction, because it changes what the remaining work *is*. Read both before writing code.

---

## 0. The one-paragraph state

Eighteen commits, `main` clean, CI green. **TinyOS boots over Ethernet** — the card swap is
over. **`REPORT-2026-08-04-01` filed**, `LE-09` (release-blocking), `LE-15`, `LE-24`, `LE-27`
closed. `STORY-P1-10-04` and `-05` board-proven; the thermal offset hypothesis held on silicon.
The spoor substrate's own overhead is measured. **`FEAT-P1-11` is new and implemented**:
`kernel::board_dispatch` runs one cooperative dispatch round from the park loop with interrupts
live, host-Green with 6 tests, image `f8133b0958d3` built and served — **and never yet booted.**
Spine: **31 Features / 96 Stories / 80 Tests / 62 Reports, 77 loose ends (39 open)**.

## 1. The next session's first act: one power cycle

The image is built, gated, staged in the TFTP root and served. **It has not run.** The board is
still executing the previous netbooted image (`epoch=0x04C7D0FF`, past `seq 8300`), and because
a netbooted TinyOS has no SSH, it cannot be restarted remotely.

```
cd work/tools/netboot
./bin/Debug/net10.0/tos64-netboot.exe --mac 88:a2:9e:11:4e:cc --root C:/tmp/tftproot
# power-cycle the board by hand
cd ../ti64dink && ./bin/Debug/net10.0/ti64dink.exe --until rung=DispatchRound --timeout 90
```

What must appear:

```
Dispatch  Kernel  Select  Ok  rung=DispatchRound  cost=0
```

One per beat, from the park loop. **Three outcomes, all informative:**

| Seen | Means |
|---|---|
| `Ok` | The kernel drives the machine. `FEAT-P1-11` exit criteria 1–4 Green; Story moves off *no board evidence*. |
| `Skipped` | The scheduler is empty — `tinyos_dispatch_init` never ran or refused. Boot ordering, not the dispatcher. |
| `Failed` | A round ran and the task came back `Running`, not `Ready` — the switch went in and never returned. **The interesting one**: the same switch the fixture performs a thousand times per boot behaving differently once interrupts are live. |

There is also a single `DispatchRound` stamped from `boot.rs` right after `daifclr`, carrying
the init result — it should land in the boot certificate's neighbourhood, before the park loop
starts.

**The failure this is built to expose is not a crash.** It is a park loop calling a round every
beat, dispatching nothing, and looking exactly like one that works — the board beaconing, the
spoors flowing, the thermal rung sampling, and nothing anywhere saying otherwise. That is
`LE-71`'s shape. Hence a spoor on *every* round, including the empty ones.

## 2. The correction that reshaped the mandate

**`01A` §5 and `03A` §3 say "the board has never run TinyOS's kernel". That is wrong**, and it
was wrong in two handovers and two commit messages before anyone checked the register.

The grep it rested on asked `hal-arm64`. But `hal-arm64` is not the caller —
`kernel::fixture_measure_arm64` is, and it runs on the board. `kernel::context` has carried a
`#[cfg(target_arch = "aarch64")]` context switch throughout, and the evidence has been sitting
in every measure envelope since `BOARD VERDICT 5`:

```
D04  context_switch_yield_roundtrip_2switches   min=78     ← on silicon
D05  dispatch_select_highest_priority_ready     min=1534   ← on silicon
D05  dispatch_run_once_cooperative_round        min=1657   ← on silicon
```

I read those numbers in `REPORT-2026-08-04-01`, quoted them, and still wrote the opposite three
times. **The general lesson is the part worth keeping: a grep that returns nothing is evidence
about the pattern and the path searched, never about the claim.**

The corrected gap is narrower and much closer: **the kernel runs, but it does not run the
machine.** Dispatch happens only inside `fixture_measure` — timed region, interrupts masked,
scheduler discarded on return. `FEAT-P1-11` closes exactly that and claims nothing wider.

## 3. What was decided, so it is not re-derived

- **[`docs/tinydb-rt-scope.md`](../../docs/tinydb-rt-scope.md)** — a bounded table beside the
  kernel, and the eight things it may never do. The owner's fast Rust database as the **first
  application and RT falsifier**: every timing number this project holds comes from a harness
  with nothing contending, so a store with deterministic operations under a live scheduler is
  the first thing that *could fail informatively*. Fixed-capacity, open-addressed, a hard
  `MAX_PROBE` that **refuses** rather than probing on. No allocation (the spine forbids
  `use alloc::` outright — **if the existing implementation is `std`/`alloc`-based, that port is
  the real cost and must be scoped before commitment**), no growth, no unbounded operation, no
  persistence, no randomised hashing. Runs at EL1 in the kernel's domain, so **no containment
  evidence may be claimed for it.**
- **The database layering**, settled across two agents and the owner: kernel — no store, flat
  signed image, fixed-offset read-only artifact if composition is ever needed. OS applications —
  TinyDB as first service. OS mutable data — a transactional object store, later. Host — a real
  database over spoors and evidence, **in `xtask`, derived from raw captures and rebuildable**,
  because the archive is an index and never the evidence. An ADR capturing these four layers is
  offered and unwritten.
- **`SEC-01` is the ceiling on integrity claims.** A read-only offset-table image is
  integrity-*checkable*, not tamper-proof, until a root of trust exists. The Pi 5 firmware chain
  provides none.

## 4. Also owed, in rough priority

- **`LE-76`** — the measurement envelope should be spoor records, not a cycling text transcript.
  Highest-value item after §1: it removes the last channel on this bench with no loss accounting,
  and it is the **prerequisite for the host archive** (archiving the current transcript would
  bake in records with no sequence, no epoch and invisible loss).
- **`STORY-P1-07-06` criterion 1** — the envelope parsed off the wire this session; the Story
  header has not been advanced to say so.
- **`FEAT-P1-09`'s exit criterion** — Ti64Dink now harvests `TOS64-PRESENT/1`; this is a
  byte-comparison away.
- **`LE-73`** — `kernel::udp_wire` cites `STORY-P1-10-03`, which does not exist.
- **`LE-75`'s actuation half** — the fan. Sensing is in; acting is deliberately not, and the
  calibration stands as *published constants corroborated on this board* because `/dev/mem` is
  blocked and the debugfs regmap **reset the board**.
- **`tos64-netboot --serial`** — serve `os/target/pi5` directly and deployment becomes build
  plus power-cycle, with no copy step. The TFTP root is currently `C:/tmp/tftproot`, transient.
- **The netboot rough edge** — the first `kernel8.img` request is abandoned at block 1 and
  succeeds on the client's retry. A slow start, not a failure.

## 5. Discipline, carried forward

`01A` §3 holds in full. This session added five receipts, three of them my own errors, because
a handover that records only what worked teaches nothing:

- **A grep that returns nothing is evidence about the search, not the claim.** Three documents
  said the opposite of what the board's own envelope had been reporting for a day.
- **Rebuild before you believe a failure.** Six confident test failures were a stale binary.
- **Ask for a privilege only after proving you need it.** I told the owner Windows Firewall was
  blocking DHCP and asked for elevation. It wasn't; the run had missed its window.
- **A batch command that stops on first failure reports on a prefix while looking like it
  reported on all of it** — `LE-77`, and now `xtask check-lints`. It found a second real lint on
  its first run.
- **A guard earned its keep immediately.** `tos64-netboot` refuses to start without `--mac` and
  ignored five DHCP requests from the *laptop itself*.

## 6. Bench facts at close

- **Board: powered, running the PREVIOUS netbooted image** (`epoch=0x04C7D0FF`). No SSH. A power
  cycle with `tos64-netboot` running loads `f8133b0958d3`; a power cycle **without** it falls back
  to the SD card and boots Pi OS.
- **Card: in the Pi, Pi OS role** — the fallback, not the boot path. `BOOT_ORDER=0xf12`.
- **TFTP prefix `7bf18f79/`, MAC `88:a2:9e:11:4e:cc`.** A netboot root needs only `config.txt`
  and `kernel8.img`; five of eight requested files do not exist and the boot proceeds anyway.
  Beware the doubled slash in `7bf18f79//config.txt`.
- **Pi OS side:** `ssh revanur@raspberrypi.local`, key auth, and `sudo -n
  /usr/local/sbin/tos64-probe {avs|iomem|dmesg|eeprom}` needs **no password**.
- **Do not read** `/sys/kernel/debug/regmap/dummy-avs-monitor@*/registers` — it reset the board.
- **Gates:** `check-assurance-spine`, `check-spine-files`, `check-boot-images` (AArch64),
  `check-lints` (host, per package). Host `cargo clippy --workspace` is **not** a clean signal
  here.
- **Board epochs on record:** `0x049F8B28`, `0x04B328BC`, `0x04B32825`, `0x04B366E6`,
  `0x04C7D0FF`. The middle pair are 151 counter ticks apart (`LE-74`).
- **`ACT` nibble headroom: one verb.** `Rung` is at 9. `MAX_RECORDS` 181. The frame header's
  reserved padding is spent.
