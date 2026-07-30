# Handover 08A — Next-Session Mandate: The Hardware-Evidence Sprint (Pi 5 First Silicon)

The owner's reading, delivered 2026-07-30 after reviewing the V1.1 console delivery
(`9f2a39c`) and the boot/hardware white paper (`b8ab185`), captured here in the
`06A`/`17G` mandate pattern. **This document is the start-here for the next sessions
and it overrides recency bias toward the console:** the next headline is not the
Jetson GPU and not a native renderer.

## 0. The headline this sprint exists to earn

> **"TinyOS boots, diagnoses faults, and produces reproducible measurements on a
> physical Raspberry Pi 5."**

That is the next investor-credible claim. Everything in this mandate either produces a
raw hardware capture or is deferred.

**The strict rule, verbatim, binding for the next two sprints:** *no new design
surface unless it directly helps produce the next raw hardware capture.*

## 1. Verified premises (re-checked by this session, 2026-07-30)

- `cargo test -p hal-arm64` — **115/115 green** on the host.
- `cargo build -p hal-arm64 --target targets/aarch64-tinyos.json -Zbuild-std=core
  -Zjson-target-spec` — **builds clean** (note the `-Zjson-target-spec` flag; the
  invocation without it fails with a misleading error).
- The ARM64 work is therefore **not starting from zero**. The missing piece is the
  physical execution loop, exactly as `FEAT-P1-07` decomposed it.

## 2. The truth table (owner's reading — current truth → next credible "live" result)

| Area | Current truth | Next credible "live" result |
| --- | --- | --- |
| Pi 5 boot | Assembly + UART/fault code compiled and host-tested (`LE-27`: never executed) | **Serial capture from a physical Pi** |
| ARM timing | Counter arithmetic exists; MMU, GIC and board measurement remain | **Parsed hardware measurement report** (`TINYOS-MEAS/2`) |
| GPU/application resources | Architecture documents are **drafts** with open questions; no DCI/UDI runtime, no GPU driver | First resource-broker vertical slice, **clearly labelled non-GPU** |
| Console | Real host-side console (V1.1); no TinyOS renderer | **Display real Pi captures in the existing console** |
| Native graphics | Blocked on display, input, compositor, renderer (`EPIC-H2`) | **Defer** until the hardware/kernel path is credible |

## 3. The three demonstrations, in order

### Demo 1 — First byte and first deliberate fault

Start **`STORY-P1-07-05`** immediately (Red first: `TEST-P1-07-05-A`), in parallel with
physical validation of `-01` and `-02` — it is legal to parallelize (`-05` depends only
on `-01`) and it is the missing bridge between compiled code and a demonstrable
product. One command (`cargo run -p xtask -- pi5 --fixture=…`) that:

1. builds the bootable AArch64 binary and a flat, placeable Pi image;
2. prints the precise SD-card placement and `config.txt` configuration (nothing about
   the image is folklore);
3. opens the serial port and captures output;
4. distinguishes **silence, partial output, explicit failure and pass** as four
   different exit codes (silence is the *common* bring-up case; the existing UART
   pass/fail protocol, no new protocol invented, same code scheme as `qemu-x86_64`);
5. records **commit, board revision, firmware version and capture hash** into the
   versioned output.

The investor demo is: power cycle → `CurrentEL` printed first → the known
`TINYOS-BOOT/1 READY` sequence → a deliberate exception producing decoded
`ESR_EL1`/`FAR_EL1` output. **That closes the board halves of two presently half-live
Stories (`-01`, `-02`) with one visible proof.**

### Demo 2 — Caches, timer, first hardware measurement

Execute the remaining Pi ladder exactly as `FEAT-P1-07` designed it, in its
non-negotiable order:

- `-03`: flat identity MMU + caches on (Normal WB cacheable RAM, Device MMIO), with
  before/after loop evidence — timing with `SCTLR_EL1.M` clear is meaningless, not slow.
- `-04`: GIC + generic-timer periodic tick; the `PMCCNTR_EL0`/`CNTVCT_EL0` split
  (`LE-15`); honest-absence when `CNTFRQ_EL0` is unprogrammed.
- `-06`: batched measurements emitted as **`TINYOS-MEAS/2`** and parsed by the existing
  host tooling **with zero parser changes** (the arch-neutrality claim meets silicon).

The resulting Report may honestly say: **"TinyOS now runs and measures on ARM64
silicon"** — and it MUST still say **"unqualified platform; these are measurements,
not worst-case bounds"** (`ADR 0005`). Secure-world qualification is a separate
follow-up story; it must not delay the first silicon milestone.

### Demo 3 — The physical evidence inside the existing console

Add a read-only **Hardware Evidence** view to the host console. It **ingests the
versioned output `xtask` produces** — it never executes host commands from a webview
(the grant tables stay disjoint; a read-only ingest verb is the shape). It shows:

- board + firmware identity; git commit + capture hash;
- boot / fault / measurement verdicts;
- the raw serial transcript;
- explicit badges: **LIVE ON SILICON** · **MECHANISM EVIDENCE** · **PLATFORM
  UNQUALIFIED**.

This combines the polished console with genuine target evidence **without pretending
Windows rendering is TinyOS rendering** (`LE-53` stays honest). Fold it into the
workbench honesty states (`live`/`pending`/`absent`) rather than inventing a fourth
vocabulary; the three badges above are per-capture qualifiers, not new states.

## 4. Diligence fixes — executed by this session (2026-07-30), before external sharing

All three of the owner's diligence findings are corrected in this commit:

1. **`FEAT-P1-07` status header** no longer says "six Stories, none started"; it now
   states `-01`/`-02` In progress (host halves Green, board criteria open), `-03`…`-06`
   Specified.
2. **`STORY-P1-07-06` + `TEST-P1-07-06-A` + `FEAT-P1-07` now require `TINYOS-MEAS/2`**
   — the envelope the live kernel emits and the parser requires (`STORY-P0-01-07`
   raised it); the working parser was NOT changed back, per the owner's instruction.
   Historical logs and already-verified Story/Test documents that quote `/1` are
   records of what was true then and were left alone.
3. **The white paper no longer labels the driver/GPU architecture "designed."** Part 2
   and the closing non-claims now say **future — draft specs with open questions**;
   Part 3.3's native-renderer destination is labelled future and blocked on
   `EPIC-H2`. (If the owner later wants "designed", the path is a committed v0
   decision/ADR promoting the two drafts — that is a scope decision, not a wording fix.)

## 5. What is deferred, and what the consolation slice is

**Deferred:** Jetson GPU support (no driver runtime, no DMA/IOMMU grants, no broker,
no UMM, no Jetson boot path) and the TinyOS-native Tauri renderer (`EPIC-H2` blocks:
application ABI, display/input services, renderer). Neither is the next headline.

**After the Pi proof** (not before): a small **resource-governance slice** on the
existing Tier 0 system —

```text
application → capability check → budgeted broker → DCI test backend → audit/refusal
```

demonstrating admission, over-budget refusal, and driver-failure containment. Call it
**"the resource-governance mechanism," never "GPU support."** The Jetson backend later
reuses that live contract.

## 6. Notes for the implementing session

- Hardware runs stay manual and land in Reports; CI stays Tier 0 (recorded §7.4
  decision — do not wire the board into CI).
- The `-02`-before-`-03`/`-04` order is not negotiable (a board that cannot report a
  translation fault turns MMU bring-up into silent hangs).
- The firmware handoff, the DTB and register values are hostile inputs — report, never
  trust (`FEAT-P1-07` containment contract; no device-tree parser in this slice).
- Read `agent/CONCURRENT_SESSIONS.md`, install the hooks, stage narrowly; the soak
  logger owns `goals/reports/_soak-p0-03-01.log`.
- V1.2/V1.3 of the console UX (06A/07A) are **not cancelled — they queue behind this
  sprint** unless the owner reorders; Demo 3 is the console work this sprint permits.
