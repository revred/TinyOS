# 09A — The Window Is Poisoned Whole: the Inbound Path Is Indicted, and the Next Capture Convicts It

Session: 2026-08-03, late night (follows [08A](08A-first-light-the-board-speaks.md) and its
postscript). Commits this session: `e2e8e35` (STORY-P1-09-12 delivered, host-Green, CI green)
and `241755f` (the board verdict postscript). Both pushed, both CI-green.

This handover is deliberately exhaustive. It is the single document the next session needs:
the full evidence corpus, the surviving and refuted hypotheses each with its discriminating
experiment, the exact capture manifest for the next Pi OS boot, the operational runbook that
was debugged tonight keystroke by keystroke (SSH, card, probe, gotchas), the code map after
tonight's refactor, the spine mechanics that bit twice, and the skeleton of the next story.
Read it end to end before touching anything; every subsection earns its place by naming a
mistake that was actually made tonight or a fact that was actually measured.

---

## 0. The one-paragraph state

TinyOS boots on the Pi 5, draws its blue canvas console at 1920×1080, pulses the lamp,
heartbeats on serial, and self-diagnoses its Ethernet bring-up on screen and lamp
simultaneously. The discovery pipeline is proven to the far edge of PCIe config space: link
gates pass, the outbound window validates, enumeration completes with **both vendor identity
gates green on silicon**. Every **memory-space** read through the RP1 window returns RP1's
fabric poison `0xDEADDEAD` — first seen at the GEM (`ID-MODULE 0xDEAD`, 08A postscript), and
tonight proven at the **clocks block too**, which kills the per-peripheral clock-gating
theory: the poison covers the whole 4 MiB peripheral window. Config reads work; memory reads
poison; therefore the fault lives on the **inbound memory path** — the endpoint BAR contents
and/or RP1's internal PCIe-to-fabric translation. The next Pi OS capture (manifest in §5) is
designed to convict the exact register. The SD card is already in the Pi OS role; SSH key
auth is installed on the card; the mmap probe is compiled at `/tmp/rp1rd` on the Pi.

---

## 0.5 SAME-NIGHT ADDENDUM — the capture already happened; H1 is CONVICTED

The owner powered the Pi OS card back up before this handover was committed, and the §5
manifest was executed on the spot. **The next session does not start at "capture" — it starts
at "write STORY-P1-09-13", and the story is fully specified from tonight's numbers.**

The working chain under Pi OS, every register captured raw
(full transcript at the tail of the ground-truth file):

```
CPU 0x1F_0000_0000 + off
  → outbound WIN0 (LO/HI = 0x0/0x0, BASE_LIMIT 0xfff00000, BASE_HI/LIMIT_HI 0x1f)
  → PCI bus address 0x0 + off                    [dts pcie2 ranges agree: CPU 0x1f_0000_0000 → PCI 0x0]
  → endpoint BAR1 = 0x00000000 claims it          [raw dword at config 0x14; COMMAND = 0x0406]
  → RP1 fabric 0x4000_0000 + off
```

Cross-checked against TinyOS's `pcie.rs`: `BUS_NUMBERS` (00/01/01), `MEM_WINDOW`
(`0x00400000` → bridge forwards bus `0x0..0x4FFFFF`), `COMMAND_ENABLE` (0x6), and all five
WIN0 window-program constants are **byte-identical to the live working values**. Exactly one
class of register in the chain is never written by TinyOS: **the endpoint's BARs.** The
`WindowPci` doc comment in `pcie.rs` records the fatal assumption verbatim — "where the
firmware assigns RP1's peripheral BAR" — and the Pi OS dmesg refutes it: Linux found BAR1
unassigned at probe time and assigned it itself. With `pciex4_reset=0`, TinyOS inherits
unprogrammed BARs, emits perfectly-formed TLPs to bus `0x0+off`, nothing claims them, and
the read comes back `0xDEADDEAD`.

**STORY-P1-09-13, now fully pinned from capture:** inside `establish`, via the existing
`EXT_CFG_INDEX` endpoint-config path (the same one that already writes `EP_COMMAND`):
size-probe each BAR (write all-ones, read the mask — dmesg-confirmed `0xffc00000` for BAR1's
4 MiB), then assign the capture values **BAR0 = 0x00410000, BAR1 = 0x00000000,
BAR2 = 0x00400000**, then believe each from readback — noting that BAR1's happy readback is
zero, so belief for BAR1 rests on the size-probe mask, not the zero. Order: BARs before the
memory-enable in `EP_COMMAND` (which the code already writes after). New refusal codes 19+
(BAR silent / BAR not held), decisive halves per the §7.2 conventions. The expected board
outcome: the canvas walks past `CLK-SILENT` — through the clock rung (whose enable writes
will now actually land) — to `rp1=present id=0x0109` and a live PHY scan, with linkwatch
catching the training and the beacon transmitting for the first time.

The §3 hypothesis ranking, §5 capture manifest, and §9 skeleton below are retained as
written *before* the capture, as the record of how the conviction was designed — read them
as history, not as work remaining. §5.4's driver-source read is now optional curiosity;
the `PCIE_APBS`-style block at BAR0 was captured (four 16-byte indexed entries, values in
the ground-truth file) and is likely irrelevant if the BAR fix clears the poison.

---

## 1. What happened tonight, in order (so the narrative is auditable)

1. **CI triage.** `b9be18b` (rustfmt-only fix) confirmed green; the three preceding reds were
   formatting-only, exactly as the 08A close believed. Lesson kept: `cargo fmt --all` before
   every commit; the pre-push mirror is cheap and the CI round-trip is not.
2. **Ground station.** Card verified in Pi OS role (full Pi OS bootfs on `D:\`, `pios-backup/`
   intact). Fresh `tos64-linkwatch` armed (a stale instance from the afternoon was killed —
   two watchers double-log; check `Get-Process tos64-linkwatch` at session start and keep one).
3. **Desk research before the board was touched.** The RP1 clock driver
   (`raspberrypi/linux`, branch `rpi-6.12.y`, `drivers/clk/clk-rp1.c`) and `rp1.dtsi` were
   read via `raw.githubusercontent.com` and transcribed into
   [`goals/reports/pios-ground-truth-2026-08-03.txt`](../../goals/reports/pios-ground-truth-2026-08-03.txt):
   full register maps for `clk_eth`, `clk_eth_tsu`, `clk_sys`, and the `pll_sys` block
   (§2.2 below). Two load-bearing facts fell out immediately: **RP1 has no reset controller
   node at all** (so no held-reset theory survives), and **`clk_sys` — the GEM's register-bus
   clock — is `CLK_IS_CRITICAL`, "always enabled in hardware"** (so "the bus clock is off"
   was already dead before the board was powered; the theory that survived to be tested was
   "the two *gateable* clocks are off").
4. **Pi OS boot + live capture over the cable.** After an SSH ops battle (§6 — worth reading,
   every trap is real), the working system's clock tree and live registers were captured:
   `clk_summary` (94 lines, committed as
   [`goals/reports/pios-clk-summary-2026-08-03.txt`](../../goals/reports/pios-clk-summary-2026-08-03.txt))
   and a 20-register mmap dump including **`GEM_MID = 0x00070109` read live through the very
   window TinyOS reads as poison** (§2.3). The delta under Pi OS: `CLK_ETH_CTRL` and
   `CLK_ETH_TSU_CTRL` both `0x10000800` (ENABLE bit 11 + running-status bit 28).
5. **STORY-P1-09-12 delivered** with the full spine ritual (contract TSV row, story, test doc,
   feature table, dashboard splice, `check-assurance-spine` green at 29 Features / 87 Stories
   / 71 Tests). New module `rp1_clocks.rs`: block pre-flight before belief, enable believed
   only from readback, running polled under an attempt budget, three new refusal codes 16/17/18.
   231 hal-arm64 tests (up from 221), fmt clean, clippy clean host + `aarch64-unknown-none`.
6. **Owner orders executed mid-session:** (a) no Python anywhere in the tooling — the on-Pi
   register reader became a 30-line compiled C probe (`/tmp/rp1rd.c`, source in §6.5), host
   orchestration stays thin PowerShell/ssh one-liners, real host tools stay C# in `work/tools/`;
   (b) `ethernet.rs` split so the pipeline reads as a pipeline — refusal taxonomy to
   `etherrors.rs`, diagnostic instruments to `ethernostics.rs` (§7.2).
7. **Board boot with the new kernel** (`kernel8.img` sha256 `9672f517…`, cardswap-verified).
   The canvas answered:

   ```
   TINYOS
   TOS64-LINK/1 RP1=ABSENT REASON=CLK-SILENT DETAIL=0xDEADDEAD BEACON=SKIPPED
   TOS64-BEAT/1 SEQ=50,51,52... STATE=PARKED FB=REFUSED
   CODE 16 DETAIL 57005
   ```

   The pre-flight gate fired exactly as designed: it read `CLK_SYS_SEL`, got `0xDEADDEAD`,
   refused **before writing a single register into a poisoned block**, and spoke the refusal
   on every channel at once. Criterion 5 of the story was answered in its refusal arm; the
   verdict is recorded in the story's progress table, the test doc, the feature table, and
   the ground-truth file (`241755f`).
8. **Card swapped back to Pi OS role** at session close (backup retained; cardswap verified
   the restore). The board is powered off. The cable is still connected laptop↔Pi.

The night's yield in one sentence: **one theory promoted to code, tested on silicon, and
honestly refuted in ninety minutes — with the refuting number spelled by the board itself.**

---

## 2. The evidence corpus (verbatim, so nothing needs re-deriving)

Everything below is committed in
[`goals/reports/pios-ground-truth-2026-08-03.txt`](../../goals/reports/pios-ground-truth-2026-08-03.txt)
(968 lines before tonight, ~1080 after). This section restates the decisive subset so the
next session does not need to grep for it.

### 2.1 Addressing (all confirmed against `rp1.dtsi` and the live capture)

| Thing | RP1 bus address | CPU address (through the kept window) | Source |
|---|---|---|---|
| RP1 peripheral window | `0x4000_0000..0x4040_0000` (4 MiB) | `0x1F_0000_0000..0x1F_0040_0000` | `board.rs` (`RP1_WINDOW_BASE`, `RP1_WINDOW_MIN_SPAN`), Pi OS `/proc/iomem` |
| Clocks block | `0x4001_8000`, span `0x10038` | `0x1F_0001_8000` | `rp1.dtsi` `clocks@18000`, live dump |
| GEM (Ethernet) | `0x4010_0000`, span `0x4000` | `0x1F_0010_0000` | `rp1.dtsi` `ethernet@100000`, `board.rs` `RP1_GEM_OFFSET` |
| xosc | 50 MHz crystal | — | `rp1.dtsi` `clock-frequency = <50000000>` |

GEM's four clock consumers (from `rp1.dtsi`, the exact `clocks`/`clock-names` pairing):
`pclk = RP1_CLK_SYS`, `hclk = RP1_CLK_SYS`, `tsu_clk = RP1_CLK_ETH_TSU`,
`tx_clk = RP1_CLK_ETH`. `phy-mode = "rgmii-id"`. **No `resets` property; no reset controller
node exists anywhere in `rp1.dtsi`.**

### 2.2 The clock register map (from `clk-rp1.c`, `rpi-6.12.y`)

Offsets are within the clocks block (add `0x1F_0001_8000` for CPU addresses):

```
GPCLK_OE_CTRL        0x00000
CLK_SYS_CTRL         0x00014   src field bits[1:0] (mask 0x3); parents: xosc, -, pll_sys
CLK_SYS_DIV_INT      0x00018
CLK_SYS_SEL          0x00020   ONE-HOT READBACK of selected parent; read-only in effect
CLK_ETH_CTRL         0x00064   aux parents: pll_sys_sec(0), pll_sys, pll_video_sec, clksrc_gp0-5
CLK_ETH_DIV_INT      0x00068   8-bit integer divider (max 0xff); max_freq 125 MHz
CLK_ETH_SEL          0x00070
CLK_ETH_TSU_CTRL     0x00134   aux parents: xosc(0), pll_video_sec, clksrc_gp0-5
CLK_ETH_TSU_DIV_INT  0x00138   8-bit; max_freq 50 MHz
CLK_ETH_TSU_SEL      0x00140
PLL_SYS_CS           0x08000   LOCK = bit 31; low bits = refdiv
PLL_SYS_PWR          0x08004
PLL_SYS_FBDIV_INT    0x08008
PLL_SYS_FBDIV_FRAC   0x0800c
PLL_SYS_PRIM         0x08010   postdiv1 bits[18:16], postdiv2 bits[14:12]
PLL_SYS_SEC          0x08014   sec divider bits[12:8] (mask 0x1f00), valid 8..19

CLK_CTRL_ENABLE      = BIT(11)     (the request bit software writes)
running-status       = BIT(28)     (hardware's answer; observed set on every enabled clock)
CLK_CTRL_AUXSRC      = bits[9:5]   (mask 0x3e0)
CLK_DIV_FRAC_BITS    = 16
```

No magic/password constant exists anywhere in the RP1 clock write path (unlike the RP2040's
`0x5A` pattern) — plain MMIO writes. `clk_sys` carries `CLK_IS_CRITICAL` with the driver
comment "Always enabled in hardware".

### 2.3 The live register dump under working Pi OS ethernet (the crown jewel)

Captured ~23:15 with the C mmap probe while `eth0` was UP with a trained 1 Gbps link:

```
0x1f00018000 GPCLK_OE_CTRL       = 0x00000000
0x1f00018014 CLK_SYS_CTRL        = 0x00000002   src=2 (pll_sys); NO enable bit — hw-critical
0x1f00018018 CLK_SYS_DIV_INT     = 0x00000001   200 MHz / 1
0x1f00018020 CLK_SYS_SEL         = 0x00000004   one-hot: parent 2  <-- the pre-flight expectation
0x1f00018064 CLK_ETH_CTRL        = 0x10000800   ENABLE + running; AUXSRC=0 (pll_sys_sec)
0x1f00018068 CLK_ETH_DIV_INT     = 0x00000001   125 MHz / 1
0x1f00018070 CLK_ETH_SEL         = 0x00000001
0x1f00018134 CLK_ETH_TSU_CTRL    = 0x10000800   ENABLE + running; AUXSRC=0 (xosc)
0x1f00018138 CLK_ETH_TSU_DIV_INT = 0x00000001   50 MHz / 1
0x1f00018140 CLK_ETH_TSU_SEL     = 0x00000001
0x1f00020000 PLL_SYS_CS          = 0x80000001   LOCKED, refdiv=1
0x1f00020004 PLL_SYS_PWR         = 0x00000004
0x1f00020008 PLL_SYS_FBDIV_INT   = 0x00000014   = 20 → 50 MHz × 20 = 1000 MHz VCO
0x1f0002000c PLL_SYS_FBDIV_FRAC  = 0x00000000
0x1f00020010 PLL_SYS_PRIM        = 0x00051010   ÷5 ÷1 → 200 MHz pll_sys
0x1f00020014 PLL_SYS_SEC         = 0x80000800   sec ÷8 → 125 MHz pll_sys_sec
0x1f001000fc GEM_MID             = 0x00070109   ← THE register TinyOS reads as 0xDEADDEAD
0x1f00100000 GEM_NCR             = 0x0010001c
0x1f00100004 GEM_NCFGR           = 0x0156044a
0x1f00100008 GEM_NSR             = 0x00000006
```

And the matching `clk_summary` rows (enable-count, prepare-count, protect, rate):

```
xosc            7 7 0   50000000  Y
clk_eth_tsu     1 1 0   50000000  Y   consumer: 1f00100000.ethernet tsu_clk
pll_sys_core    3 3 0 1000000000  Y
pll_sys_sec     1 1 0  125000000  Y
clk_eth         1 1 0  125000000  Y   consumer: 1f00100000.ethernet tx_clk
pll_sys         2 2 0  200000000  Y
clk_sys         4 4 0  200000000  Y   consumers: 1f00188000.dma cfgr-clk, ethernet hclk+pclk
pll_sys_pri_ph  1 1 0  100000000  Y
```

Every consistency check passes: 50 MHz × 20 = 1000; ÷5 = 200 (`clk_sys`); ÷8 = 125
(`clk_eth`). The PLL tree is the firmware's work — locked before any kernel runs, because the
RP1 fabric itself runs on `clk_sys` which hangs off it.

### 2.4 The board's answer with the new kernel (TinyOS, kernel `9672f517…`)

```
canvas:  TOS64-LINK/1 RP1=ABSENT REASON=CLK-SILENT DETAIL=0xDEADDEAD BEACON=SKIPPED
         TOS64-BEAT/1 SEQ=50,51,52... STATE=PARKED FB=REFUSED
         CODE 16 DETAIL 57005
lamp:    the same sentence spelled in decimal groups
link:    the laptop NIC never trained (linkwatch silent throughout the boot)
```

Decode: code 16 = `ClockRefused::BlockSilent` (the pre-flight arm); detail 57005 = `0xDEAD`
(the high half of the readback, exactly as the arm's decisive-bits contract says); the report
line carries the full 32-bit readback `0xDEADDEAD`. Note the sequence numbers — the beat was
in the 50s when transcribed, i.e. the pipeline refused within the first second and the park
loop had been heartbeating for ~50 s by the time it was read. `FB=REFUSED` is the known
mailbox posture (08A postscript): the canvas runs on the firmware's `simple-framebuffer`
handoff at `0x3F800000`, not on a mailbox grant.

### 2.5 The Pi OS PCI facts already in the can (captured earlier sessions, decisive now)

From `dmesg` (ground truth lines ~299–309):

```
pci 0002:01:00.0: BAR 0 [mem 0xffffc000-0xffffffff]        <- probe-time contents
pci 0002:01:00.0: BAR 1 [mem 0xffc00000-0xffffffff]        <- probe-time contents (4 MiB)
pci 0002:01:00.0: BAR 2 [mem 0xffff0000-0xffffffff]
pci 0002:01:00.0: BAR 1 [mem 0x1f00000000-0x1f003fffff]: assigned
pci 0002:01:00.0: BAR 2 [mem 0x1f00400000-0x1f0040ffff]: assigned
pci 0002:01:00.0: BAR 0 [mem 0x1f00410000-0x1f00413fff]: assigned
```

From `lspci -vv` (ground truth lines ~744–746):

```
Region 0: Memory at 1f00410000 (32-bit, non-prefetchable) [size=16K]
Region 1: Memory at 1f00000000 (32-bit, non-prefetchable) [virtual] [size=4M]
Region 2: Memory at 1f00400000 (32-bit, non-prefetchable) [virtual] [size=64K]
```

Read those two together carefully — they are the reason the next capture is designed the way
it is. **BAR1 is a 32-bit BAR.** A 32-bit BAR cannot contain `0x1F_0000_0000`. lspci's
`[virtual]` tag means precisely "the address printed is the CPU-side resource, and the BAR
register itself does not hold it" — on this platform the root complex's outbound window
translates CPU `0x1F_xxxx_xxxx` to some 32-bit PCI bus address, and the BAR register holds
*that*. What bus address Linux actually programmed into config offset `0x14` is **not in any
capture we have**. That raw dword is the single highest-value number the next boot must
produce.

From `dmesg` (macb lines, for later rungs):

```
macb 1f00100000.ethernet eth0: Cadence GEM rev 0x00070109 at 0x1f00100000 irq 106 (88:a2:9e:11:4e:cc)
macb 1f00100000.ethernet eth0: PHY [...ffffffff:01] driver [Broadcom BCM54213PE] (irq=POLL)
macb 1f00100000.ethernet eth0: Link is Up - 1Gbps/Full - flow control tx
```

---

## 3. Hypotheses: refuted, surviving, and how each survivor dies or is convicted

### 3.1 Refuted on silicon (do not resurrect these)

- **"The GEM's clocks are gated; enable `clk_eth`/`clk_eth_tsu` and identity clears."** Dead.
  The clocks block itself — a different peripheral, whose own bus clock is hardwired-on —
  reads the identical poison. Tonight's boot, code 16.
- **"A reset line holds the GEM."** Dead twice over: `rp1.dtsi` has no reset controller and no
  `resets` property on the ethernet node, and the poison is window-wide anyway.
- **"The PCIe link/window/enumeration is bad."** Dead. Both vendor identity gates
  (root `0x2712_14E4`, endpoint `0x0001_1DE4`) pass on silicon through config space, on the
  same boots that poison every memory read. The blink ladder 3→4→9 through the earlier
  evening was precisely the sequential proof of link gates, window, and enumeration.
- **"The window span/base is unmapped so reads float."** Dead by the *value*: a floating or
  unclaimed read returns `0xFFFFFFFF` (that is what code 7 `id-floating` exists for) and an
  unbacked-but-decoded read returns zeros (code 8). `0xDEADDEAD` is neither — it is a
  *completer answering with RP1's own poison pattern*, which means the TLP reaches RP1 and
  RP1's internal fabric generates the answer.

### 3.2 Survivors, ranked (each with the observation that would convict or clear it)

**H1 — BAR1 contents mismatch the bus address our TLPs carry (leading).**
The root complex's outbound window translates CPU `0x1F_0000_0000+off` to PCI bus address
`B+off` for whatever base `B` the window was programmed with. RP1 claims the TLP iff
`B+off` falls inside BAR1's programmed range; the internal fabric address is then
`0x4000_0000 + (B+off − BAR1)`. If our enumeration wrote BAR1 a value different from the
`B` our window emits — or wrote it with a stale/incorrect value pinned from a capture that
recorded CPU-side numbers — every access lands at a *constant wrong offset* in RP1's
internal map, or inside RP1's "not a peripheral here" space, and RP1 answers its poison for
all of them uniformly. **This fits every observed fact:** uniform poison across two widely
separated blocks (0x18000 and 0x100000), config space unaffected, and the completer clearly
being RP1. *Convicted by:* raw BAR1 dword under Pi OS ≠ what our code writes, or ≠ our
window's PCI base. *Cleared by:* all three numbers agreeing.

**H2 — RP1's internal inbound translation (`PCIE_APBS` block) needs programming and Pi OS's
driver does it silently.** The RP1 PCIe endpoint has its own APB-visible configuration block;
if inbound BAR1→fabric translation is not a hard-wired `0x4000_0000` mapping but a
programmable one, then with `pciex4_reset=0` we inherit whatever state the *firmware* left,
which may differ from what the *Linux rp1 driver* establishes before peripherals become
readable. *Convicted by:* diffing the `PCIE_APBS` register block between (a) live Pi OS with
ethernet working and (b) what TinyOS sees at the same offsets — if TinyOS can read that block
at all (it may live behind BAR0/BAR2 rather than BAR1; see §5.3). *Also strongly informed
by:* reading `drivers/misc/rp1-pci.c` / `drivers/mfd/rp1.c` (name varies by branch) in
`rpi-6.12.y` for any write it performs before children probe.

**H3 — Bridge memory-decode aperture excludes our addresses (weak).** The type-1 bridge's
memory base/limit registers must cover the bus addresses of the TLPs for them to be forwarded
downstream at all. If they didn't, the RC would master-abort and we'd read `0xFFFFFFFF`, not
RP1's poison — which is why this is ranked low. It survives only in the exotic variant where
the aperture covers the addresses but maps them oddly. *Cleared by:* the same capture as H1
(the bridge's live config under Pi OS vs ours).

**H4 — RP1 requires a "wake"/handshake write before its fabric un-poisons (dark horse).**
Folklore-shaped, but cheap to check while we're in the sources: if the Linux rp1 platform
driver performs any magic doorbell (e.g. clearing a `PCIE_APBS` status, setting an inbound
enable bit) before peripherals read sanely, it will be visible as the *first* MMIO write in
that driver's probe path. *Convicted/cleared by:* the driver source read (§5.4) plus the
`PCIE_APBS` dump diff.

The capture manifest in §5 is constructed so that **one Pi OS boot answers H1, H3 fully and
H2, H4 to the extent hardware state can** — the remainder of H2/H4 comes from a driver-source
read that needs only the laptop and WebFetch.

---

## 4. Why config space works while memory space poisons (the mechanism, precisely)

Worth stating once so the next session doesn't re-derive it. Config reads (`CFG0` TLPs) are
generated by the root complex's config mechanism — on this controller, through the
`EXT_CFG_INDEX`/config-window path our `pcie.rs` already drives — and are routed by
**bus/device/function**, not by memory address. They do not consult BARs, do not pass through
the bridge's memory base/limit decode, and do not involve RP1's inbound *memory* translation.
That is exactly why enumeration and both vendor gates can be perfectly green while every
memory TLP misroutes. The boundary between "what works" and "what poisons" is *precisely* the
boundary between the ID-routed and address-routed transaction classes — which is what points
the finger at the address-routing state: window PCI base, BAR contents, bridge aperture, and
RP1's inbound translation. Nothing else distinguishes the two classes.

---

## 5. THE CAPTURE MANIFEST — next Pi OS boot (card is already in the Pi OS role)

Everything below runs over SSH from the laptop. §6 has the connection runbook. Capture
**in this order** (cheap and read-only first), appending everything to
`goals/reports/pios-ground-truth-2026-08-03.txt` (or a dated `-04` file if the session rolls
past midnight — keep one file per calendar date, the spine's report references are by date).

### 5.1 Raw config space of the RP1 endpoint (the H1 conviction evidence)

```sh
# The full 4 KiB config space, hex, straight from the kernel's view of 0000:01:00.0.
# NOTE the domain: on Pi OS the RP1 sits at domain 0002 (dmesg says 0002:01:00.0), but
# /sys enumerates it as 0002:01:00.0 — confirm with: ls /sys/bus/pci/devices/
sudo hexdump -C /sys/bus/pci/devices/0002:01:00.0/config | head -20

# The three BAR dwords, labeled (offsets 0x10, 0x14, 0x18), plus command/status:
sudo setpci -s 0002:01:00.0 COMMAND         # expect memory-space enable bit set (bit 1)
sudo setpci -s 0002:01:00.0 0x10.l          # BAR0 raw dword
sudo setpci -s 0002:01:00.0 0x14.l          # BAR1 raw dword  <-- THE NUMBER
sudo setpci -s 0002:01:00.0 0x18.l          # BAR2 raw dword
```

`setpci` ships in the `pciutils` package that provided `lspci`, already present. If the
domain prefix differs, `lspci -D` prints it. **Record all of it verbatim.** The expected
shape, if H1 is right: BAR1 holds a 32-bit bus address (low nibble masked — bits [3:0] are
type flags) that our TinyOS enumeration either does not write or writes differently.

### 5.2 The root complex's live outbound window and bridge decode (H1/H3)

The bridge's own config space (the root port, `0002:00:00.0`):

```sh
sudo hexdump -C /sys/bus/pci/devices/0002:00:00.0/config | head -8
sudo setpci -s 0002:00:00.0 0x20.l   # memory base/limit dword (type-1 header)
sudo setpci -s 0002:00:00.0 0x24.l   # prefetchable base/limit
sudo setpci -s 0002:00:00.0 PRIMARY_BUS.b SECONDARY_BUS.b SUBORDINATE_BUS.b
```

The controller's outbound window registers (`WIN0_*`) live in the *controller's* MMIO block,
not config space — TinyOS's `pcie.rs` already knows the offsets (`WIN0_BASE_LIMIT`,
`WIN0_BASE_HI`, `WIN0_LIMIT_HI`; the healthy capture values are pinned in the `HealthyRc`
test double: `WIN0_BASE_LIMIT = 0x03F0_0000`, `WIN0_BASE_HI = WIN0_LIMIT_HI = 0x1F`). Capture
them live from Pi OS with the mmap probe against the PCIE2 controller base (`board::PCIE2_BASE`
in `board.rs` names the CPU address — read it from the source before the session; it is the
same block the probe interrogates on TinyOS). **The specific question these answers:** what
PCI bus address does the window emit for CPU `0x1F_0000_0000`? The `WIN0` register pair
encodes CPU base and PCI base; our code programmed it "from the capture's recorded mapping"
(STORY-P1-09-09) — verify the *PCI-side* half of that mapping against BAR1's raw dword from
§5.1. If they disagree, H1 is convicted and the fix is a one-register story.

### 5.3 RP1's `PCIE_APBS` inbound-translation block (H2)

The RP1 endpoint controller's own registers are visible from the host — BAR0 (16 KiB at CPU
`0x1f00410000` per lspci) is the strongest candidate for the `PCIE_APBS` block (BAR2, 64 KiB
at `0x1f00400000`, is the shared-memory/MSI-X region on most RP1 documentation). Dump both
headers with the probe:

```sh
sudo /tmp/rp1rd 0x1f00410000 0x1f00410004 0x1f00410008 0x1f0041000c \
                0x1f00410010 0x1f00410014 0x1f00410018 0x1f0041001c
sudo /tmp/rp1rd 0x1f00400000 0x1f00400004 0x1f00400008 0x1f0040000c
```

Then widen: dump the first 0x100 of BAR0 in 4-byte steps (the probe takes many addresses per
invocation; generate the list with a shell loop *on the Pi*, not PowerShell — §6.6 explains
why). Look for registers whose values embed `0x40000000` (the fabric base) or the BAR
addresses — those are the translation registers. **Also capture the same range's TinyOS-side
readback next boot for the diff.** If BAR0 reads poison under TinyOS too, that itself is
decisive: it means even the endpoint controller's own block is behind the broken translation,
which re-ranks H1 further up (BAR0's dword from §5.1 then tells us where TLPs must land).

### 5.4 The driver source read (laptop-only, no board time needed)

WebFetch these from `raw.githubusercontent.com/raspberrypi/linux/rpi-6.12.y/`:

- `drivers/misc/rp1-pci.c` — if 404, try `drivers/mfd/rp1.c` and
  `drivers/firmware/rp1.c`; one of them is the platform driver that binds `0002:01:00.0`,
  maps the BARs, and creates the child platform devices from the DT overlay.
- Ask specifically: **every MMIO write the driver performs before children probe** — inbound
  ATU setup, MSI-X init, a "wake" doorbell, anything touching the `PCIE_APBS` block; and how
  it computes the peripheral base handed to children (does it assume BAR1+0 ↔ 0x40000000?).
- Also `arch/arm64/boot/dts/broadcom/bcm2712-rpi-5-b.dts` for the `pcie2` node's `ranges`
  property — that is the authoritative CPU→PCI translation Linux uses, i.e. the exact bus
  address `B` that BAR1 must equal for offset-0 alignment. **This single property may answer
  H1 from the armchair before the board is even powered** — fetch it first.

### 5.5 The comparison TinyOS boot (after the analysis, not before)

Only after §5.1–5.4 are in hand and a fix hypothesis exists: build, `cardswap tos64`, boot,
read the canvas. Do not burn a boot cycle on "let's just see" — every swap is minutes of
operator time and the board only speaks in one refusal at a time. The expected outcomes:

- Fix right → canvas moves past `CLK-SILENT`; likely all the way to `rp1=present id=0x0109`
  with a PHY scan, because everything downstream of the poison is already board-tested code.
  Watch linkwatch for the training; if the wire trains, the **beacon transmits** — have a
  packet capture ready if desired (the beacon is a raw Ethernet II broadcast with a
  local-experimental EtherType; byte-identical to the host tests' pinned frame).
- Fix wrong → a *different* number. Codes 16/17/18 all carry their readbacks; the ladder
  continues on whatever it says.

### 5.6 Capture hygiene

- Append to the ground-truth file with clear `===== section =====` headers and the date/time;
  the file is the court record and tonight's session leaned on line-number references into it.
- The `clk_summary` path is `/sys/kernel/debug/clk/clk_summary` — **with `/clk/`**; the 08A
  note omitted it and tonight lost a round-trip to "No such file".
- `sudo` needs `echo sonu | sudo -S <cmd>`; debugfs needs root; `/dev/mem` mmap needs root
  (`O_SYNC`; `read()` on MMIO gives EFAULT — that is why the probe exists).

---

## 6. Operations runbook (every item below was learned the hard way tonight)

### 6.1 Reaching the Pi

- **Address:** `fe80::375c:1a61:f858:2034%16` — the Pi's stable link-local on the direct
  cable, interface index 16 = the laptop's Realtek NIC ("Ethernet"). This survived a reboot
  tonight (NetworkManager stable-privacy addressing is per-connection-stable). The Pi also
  says it itself on its console: "My IP address is … fe80::375c:1a61:f858:2034".
- **mDNS (`raspberrypi.local`) is untrustworthy.** It resolved for the first two commands of
  the night and then flatly refused for the rest, including while the same host answered ping
  by address. Never build a wait loop on the name; always use the fe80 literal with `%16`.
- **If the address ever changes:** `Get-NetNeighbor -AddressFamily IPv6 -ifIndex 16` after a
  multicast ping (`ping -6 ff02::1%16` — note the Pi did *not* answer that multicast tonight;
  the neighbor cache route is more reliable), or read the previous session's
  `known_hosts`/ground-truth lines, or just read the Pi's console message.
- **The Pi is NOT on WiFi.** A full subnet sweep of 192.168.1.0/24 found no Raspberry Pi OUI
  and no SSH answering our key. The cable is the only path. (The laptop's WiFi stays up and
  is not involved.)

### 6.2 SSH auth (fixed permanently tonight — but know the traps)

Key auth now works: `ssh -o BatchMode=yes "revanur@fe80::375c:1a61:f858:2034%16" <cmd>`.
The key is `~/.ssh/id_ed25519` on the laptop; the pubkey is installed in
`/home/revanur/.ssh/authorized_keys` on the **Pi OS card**. Three traps burned ~40 minutes:

1. **PowerShell `ssh-keygen -N '""'` sets the passphrase to literally two quote characters.**
   The symptom is maddening: `ssh -vv` shows "Server accepts key" then "Permission denied" —
   the *signing* step fails silently in BatchMode. Generate keys with
   `cmd /c 'ssh-keygen -t ed25519 -f "%USERPROFILE%\.ssh\id_ed25519" -N "" -q'`.
2. **Piping a pubkey through PowerShell appends CRLF.** `Get-Content key.pub | ssh 'cat >>
   authorized_keys'` leaves a trailing `\r` and sshd rejects the line silently. Install with
   `printf '%s\n' '<pubkey>'` on the remote side, or strip with `tr -d '\r'` after.
3. **Password fallback without a TTY:** OpenSSH 9.5 honors `SSH_ASKPASS` +
   `SSH_ASKPASS_REQUIRE=force`. The shim `askpass.bat` (`@echo sonu`) lives in the session
   scratchpad; recreate it in 5 seconds if needed. Password is `sonu`, user `revanur`.

### 6.3 The link used to bounce — fixed, but re-verify

Pi OS's NetworkManager was DHCP-cycling `eth0` on the peer-to-peer cable: ~45 s up, then
down, in a loop (visible in linkwatch's log at 19:19, 22:08, 22:50). Tonight's fix, applied
persistently on the card: `nmcli con mod 'Wired connection 1' ipv4.method link-local`. After
it, `eth0` held `169.254.133.66/16` + the fe80 and stopped cycling. **First action after the
next Pi OS boot: confirm the link is steady** (linkwatch shows one `Down -> UP` and then
silence) before starting captures; if the cycling is back, re-apply the nmcli line, `nmcli
con up 'Wired connection 1'`, and expect one more bounce as it re-activates.

### 6.4 The card (single physical card, two roles, hash-verified)

- `work\tools\cardswap\bin\Release\net10.0\tos64-cardswap.exe status|tos64|pios`
- **Current state at handover: Pi OS role**, `pios-backup\` retained on card, staged TOS64
  build is `kernel8.img` sha256 `9672f517047d52336f9cbff97519f4f8f81a70fc8c460649e1f7f263787c37b0`
  (the code-16 kernel — includes STORY-P1-09-12; rebuild before the next TOS64 swap only if
  code changed).
- The image is built by `cargo run -p xtask -- pi5 --fixture=boot` (from `os/`); it stages
  `kernel8.img` + `config.txt` and prints the placement contract (`os_check=0`,
  `kernel=kernel8.img`, `pciex4_reset=0` — all three are load-bearing; the printout explains
  why each).
- cardswap says "role: unknown kernel" when the card is in Pi OS role — it just means the
  on-card kernel hash matches no staged TOS64 build; it is not an error.

### 6.5 The on-Pi register probe (C, compiled on the Pi — the owner's no-Python rule applies)

`/tmp/rp1rd` exists on the Pi OS card from tonight. `/tmp` may or may not survive a reboot
(Pi OS default keeps it on disk, but do not rely on it). The full source, for re-creation:

```c
#include <stdio.h>
#include <stdlib.h>
#include <fcntl.h>
#include <sys/mman.h>
#include <unistd.h>
int main(int argc, char **argv) {
    int fd = open("/dev/mem", O_RDONLY | O_SYNC);
    if (fd < 0) { perror("open"); return 1; }
    long pg = sysconf(_SC_PAGE_SIZE);
    for (int i = 1; i < argc; i++) {
        unsigned long long a = strtoull(argv[i], 0, 0);
        unsigned long long base = a & ~(unsigned long long)(pg - 1);
        volatile unsigned int *m = mmap(0, pg, PROT_READ, MAP_SHARED, fd, base);
        if (m == MAP_FAILED) { printf("0x%llx = MAP_FAILED\n", a); continue; }
        printf("0x%llx = 0x%08x\n", a, m[(a - base) / 4]);
        munmap((void *)m, pg);
    }
    return 0;
}
```

Ship it via heredoc over ssh and `gcc -O1 -o /tmp/rp1rd /tmp/rp1rd.c` (gcc is on the card).
Run as root. **Why it exists:** `dd if=/dev/mem` uses `read()`, which returns EFAULT for MMIO
on this kernel; `devmem` is not installed and the card has no internet to install it; Python
is banned from the stack by owner order (01A for TinyTile, restated tonight for tooling).
A future `work/tools/` C# host app could wrap the whole capture (ssh + probe + append) —
sdprep is the pattern — but the thin-ssh approach worked and the owner's priority is the
diagnosis, not the tooling.

### 6.6 PowerShell → ssh quoting (two bites tonight)

Any remote command containing `$` **must** travel in a single-quoted here-string
(`$cmd = @'...'@; ssh host $cmd`), or PowerShell interpolates `$((a))`, `$v`, `$a` into
nothing before ssh ever sees them. Symptom tonight: a register loop that printed empty values,
and a `sed 's/\r$//'` whose `$` was eaten so it matched nothing while reporting success —
which then masqueraded as the CRLF fix having failed for a *different* reason. When a remote
one-liner misbehaves, the first check is: what did the remote shell actually receive?

### 6.7 linkwatch

`work\tools\linkwatch\bin\Release\net10.0\tos64-linkwatch.exe` in background at session
start; log at `...\net10.0\logs\watch.log`. Kill stale instances first (two log
double-entries). It is the boot-verdict instrument for "did the wire train" — during TinyOS
boots it is the *only* network-side instrument until the beacon exists.

---

## 7. The code map after tonight (what lives where, and why)

### 7.1 The discovery pipeline (`os/src/hal-arm64/src/`)

- **`ethernet.rs`** — the pipeline and nothing else: `Discovery` (the outcome enum),
  `discover()` (gates → **clock rung** → identity → release → PHY scan → link), `BeaconField`,
  `link_line()` / `heartbeat_line()` (the pinned TOS64 wire lines), `beacon_eligible`,
  `watch_from`/`watch_step` (the park-loop link watch), `reprobe_due`, and the aarch64 glue
  (`announce_and_park`) that owns real addresses and the 10 Hz park loop. Re-exports the two
  satellite modules for API stability.
- **`etherrors.rs`** (new tonight) — the refusal *taxonomy*: `blink_code()` (Discovery → code
  1–18, exhaustively matched so a new arm is a compile error, no code ever shared) and
  `blink_detail()` (Discovery → the sixteen decisive bits). Nothing else.
- **`ethernostics.rs`** (new tonight) — the *instruments*: the plain blink pattern
  (`blink_lamp_at`), the seven-group decimal sentence (`Sentence`, `sentence_for`,
  `sentence_lamp_at`, `sentence_period`, zero = 1.5 s steady burn), the `SentenceLatch`
  (a sentence in flight is never replaced), `LampAction`/`lamp_action`, and `refusal_text`
  (the `CODE NN DETAIL NNNNN` canvas line). All pure functions of (sentence, tick).
- **`rp1_clocks.rs`** (new tonight) — STORY-P1-09-12's rung: `register::*` map,
  `CTRL_ENABLE`/`CTRL_RUNNING`, `enable_ethernet_clocks()` (pre-flight `CLK_SYS_SEL`
  one-hot credibility gate → per-clock exactly-once pinned write pair → readback-believed
  enable → `RUN_POLL_LIMIT`-bounded running poll; idempotent, zero writes on the
  already-running pass), `ClockRefused` (three arms). **This is the rung that fired tonight,
  correctly, at its first gate.**
- **`pcie.rs`** — gates, window validation + programming fallback (-09), enumeration at
  bring-up size with both vendors verified (-10). *The next story almost certainly lands
  here or beside it* — the window's PCI-side base and the endpoint BAR writes are its
  territory. Read it before writing -13.
- **`board.rs`** — the address constants, each with provenance:
  `RP1_WINDOW_BASE = 0x1F_0000_0000`, `RP1_GEM_OFFSET = 0x10_0000`,
  `RP1_CLOCKS_OFFSET = 0x1_8000` (new tonight), sizes, `PCIE2_BASE`, `STAT_GPIO_BASE`,
  `RP1_DMA_RAM_BASE = 0x10_0000_0000` (the *outbound-from-RP1* translation for beacon DMA —
  do not confuse it with the inbound path under investigation).
- **`gem.rs`** — identity parse, MDIO port, PHY scan (BCM54213PE expected), latched link
  read, bounded transmit. All downstream of the poison; all already board-adjacent-tested.

### 7.2 The refusal code table (transcription reference — print this at the bench)

| Code | Arm | Decisive 16 bits |
|---|---|---|
| 1 | port not RC | status word low half |
| 2 | PCIe PHY down | status word low half |
| 3 | data-link down | status word low half |
| 4 | window base refused | address in MiB, low 16 |
| 5 | window PCI refused | address in MiB, low 16 |
| 6 | window span refused | span in MiB, low 16 |
| 7 | identity read floating (0xFFFFFFFF) | 0xFFFF |
| 8 | identity read zeros | 0 |
| 9 | wrong module | the module field |
| 10 | PHY release stuck | 0 |
| 11 | PHY absent | 0 |
| 12 | management port wedged | 0 |
| 13 | PHY unknown | ID1 |
| 14 | root vendor refused | vendor half |
| 15 | endpoint vendor refused | vendor half |
| **16** | **clocks block silent (pre-flight)** | **readback high half — poison = 57005** |
| **17** | **clock enable did not hold** | **readback low half** |
| **18** | **clock never ran (budget exhausted)** | **readback high half** |

Sentence shape: seven groups, least-significant first — two code digits then five detail
digits; zero = one long 1.5 s burn; groups separated by a 1.2 s dark, sentence ends with a
3.5 s dark. Health = plain 1 Hz pulse, never a sentence.

### 7.3 Test surface

231 hal-arm64 host tests. The new ones: 8 in `rp1_clocks` (pre-flight refusals for
0/all-ones/poison with a write-panicking double; exactly-one-write-pair; enable-not-held;
zero-writes-when-running; exact-budget exhaustion; k-attempt pass; register map pinned
against the capture) and 2 pipeline tests in `ethernet` (poisoned clocks stop the pipeline
before the GEM with codes/details/line pinned, incl. the exact line the board then printed
tonight — `reason=clk-silent detail=0xdeaddead` — a pinned string that silicon reproduced
byte-for-byte an hour later; and the 17/18 arms' codes/details/names). The taxonomy and
instrument tests moved wholesale into their new modules; total count and every test name
preserved.

---

## 8. Spine mechanics that bit tonight (avoid the same three round-trips)

1. **Status headers:** the state token must be followed by a terminator — `**`, ` —`
   (space + em-dash), `,`, ` (`, or `.`. `Status: **Specified 2026-08-03 — …**` fails the
   parse (`Specified` runs into ` 2026`); `Status: **Specified — 2026-08-03, …**` passes.
   Same grammar applies to the Feature table's status *cells*.
2. **LE-44 criteria matching:** if the Story header says "criterion 5 …", the Feature table
   cell for that story must mention the same criteria set (the checker extracts the numbers
   from both and diffs). Tonight's postscript failed once with table `{}` vs header `{5}`.
3. **The index.html footnote counts are gated:** moving one story Specified → In progress
   requires the `list-status` footnote sentence to change (`14 Specified, 22 In progress`
   after tonight), the "N Features / N Stories / N Tests" strings (there are **two** of
   them), and the generated tile blocks (`cargo run -p xtask -- emit-dashboard` prints them;
   splice verbatim). The full ritual for a new story: contract TSV row → story doc → test
   doc → feature table row + feature status header → emit-dashboard splice → footnote counts
   → `check-assurance-spine`.
4. `cargo run -p xtask -- …` must run from `os/`; the pre-commit hook re-runs the spine check
   and crate-size gates on the staged tree, so a green local run is necessary but commit only
   proves the *index* was green.

---

## 9. STORY-P1-09-13 — the skeleton (write it against the capture, not before)

Do not commit this until §5's numbers exist; the story's whole design must be pinned from
the capture per the house rule. But the shape is predictable enough to sketch:

- **Working title:** "The introduction was incomplete: the window's far end — BAR and inbound
  translation believed only from readback." (Or, if H1 convicts cleanly: "The two ends of one
  window: the outbound base and the endpoint BAR are the same number or neither is believed.")
- **The rung:** strictly inside `pcie::establish` (it is enumeration's unfinished business,
  not a discovery-pipeline stage): after the vendor gates, read the endpoint's BAR1 raw
  dword; if it does not equal the outbound window's PCI base (the value our WIN0 programming
  emits), write it (exactly once, from the capture-pinned value), re-read, believe the
  readback; verify memory-space enable in the endpoint's COMMAND register the same way. If
  the driver-source read (§5.4) reveals a required `PCIE_APBS` write, that becomes a second
  pinned write with its own readback gate and its own refusal.
- **Refusal codes:** 19+ (the space is open; `blink_code`'s exhaustive match will force the
  wiring). Each refusal carries the offending readback; the decisive-half convention follows
  §7.2's pattern.
- **The decisive test double:** a `HealthyRc` variant whose BAR1 initially reads the
  probe-time garbage from dmesg (`0xffc00000`-shaped) and accepts the corrective write —
  pinning that the pipeline *notices* and *fixes* rather than assumes; plus an arm where the
  write does not hold.
- **Board criterion:** the canvas moves past `CLK-SILENT` — to `rp1=present id=0x0109` if the
  chain is now whole, or to the next honest number.
- **Spine:** next contract row is after STORY-P1-09-12's in `story-contracts.tsv`; test doc
  `TEST-P1-09-13-A`; feature table row 13; counts go to 88 Stories / 72 Tests; footnote
  `15 Specified` (or straight to In progress if delivered same-session, keeping 14/23).

**A note on the `[virtual]` BAR trap when pinning values:** the capture's lspci CPU-side
addresses (`1f00000000`) are *not* BAR values. The BAR value to pin is the §5.1 raw dword —
a 32-bit bus address. Getting this wrong would reproduce exactly tonight's class of bug one
layer down, and the whole reason -13 exists is that recorded-mapping beliefs and hardware
state diverged somewhere on this path.

---

## 10. Standing orders in force (restated because they bound -13's design)

- **Wire-first diagnostics (owner, 08A amendment):** the screen is bootstrap only. The moment
  the link trains, diagnosis moves onto the cable as `TOS64-*` envelopes into Ti64Dink. Do
  not grow new canvas/lamp surface beyond what the current refusal ladder needs.
- **No bench-tuned constants (owner, standing):** attempt budgets, not durations; watch,
  don't sample. Tonight's `RUN_POLL_LIMIT` follows `MDIO_POLL_LIMIT`'s "convert a hang into a
  return" rationale — keep that phrasing discipline in -13.
- **Host tooling is C# (owner):** `work/tools/` only, sdprep as the pattern; PowerShell as
  thin invocation only; **no Python anywhere**, including on the Pi (compiled C probes are
  the sanctioned exception for on-target capture).
- **TDD without exception:** the failing test precedes the code. The spine ritual is not
  optional overhead — tonight it caught two real drift bugs (footnote counts, LE-44 join)
  before CI would have.
- **Priorities unchanged:** (1) Ethernet for Ti64Dink — this ladder; (2) micro-HDMI
  splash→OS. Everything else queues.
- **`TOS64-*` prefix** on all wire envelopes; never emit or parse `TINYOS-*`.

---

## 11. Session-start checklist for the next session (mechanical, in order)

1. `gh run list --limit 3 --branch main` — confirm `241755f` still green (it was at close).
2. `Get-Process tos64-linkwatch` — kill stale, arm fresh, note the log baseline.
3. `cardswap status` — expect Pi OS role, backup present, staged `9672f517…`.
4. **Fetch `bcm2712-rpi-5-b.dts` `pcie2` ranges first** (§5.4 last bullet) — it may answer H1
   before the board is even powered, and it shapes what §5.2 should expect.
5. Boot the Pi (card in), wait for linkwatch's `Down -> UP`, confirm the link *stays* up
   (§6.3), ssh by fe80 literal.
6. Run the §5 manifest in order; append verbatim to the ground-truth file with headers.
7. Read the rp1 platform driver source (§5.4) against the captured `PCIE_APBS` values.
8. Write STORY-P1-09-13 from the numbers; spine ritual; TDD; gates
   (`fmt` → `clippy --all-targets` → `clippy --target aarch64-unknown-none` → tests → spine).
9. Build, `cardswap tos64`, boot, transcribe the canvas. The ladder continues on whatever
   number it shows.
10. Commit narrative style: what the number was, what it convicted, what the board said.

---

## 12. What "done" looks like for this arc

The arc closes when the canvas prints `rp1=present id=0x0109 phy=0x600d84a2 link=up
speed=1000 duplex=full beacon=running`, linkwatch logs the training, and a packet capture
on the laptop shows the pinned broadcast frame — at which point `FEAT-P1-09`'s exit
criterion is met on silicon, `FEAT-P2-10` (Ti64Dink host side) unblocks, and the owner's
priority 1 has its first end-to-end proof. Every rung between here and there is already
written and host-tested except the one the next capture will name. The board has stopped
being a mystery and become a witness; keep asking it questions it can answer with a number.

— session close, 2026-08-03, with the card in the Pi OS role and the cable still connected.
