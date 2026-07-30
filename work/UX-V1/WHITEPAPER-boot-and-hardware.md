# TinyOS White Paper — Bare-Metal Boot on Raspberry Pi 5, Hardware Resources for Applications, and How the Console Is Actually Rendered

Status: Informative white paper, owner-ordered 2026-07-30. Written against the repository
as of `9f2a39c` (UX V1.1 delivered). This document **explains and connects** decided
designs; it decides nothing new and it claims no evidence the spine does not hold.

**Honesty header, before anything else.** Every statement below carries one of three
states, in the register style (`work/UX-V1/SPEC.md` §5):

- **live** — code and evidence exist in this repository today.
- **designed** — a committed spec or ADR decides the shape; implementation is scheduled.
- **future** — direction stated in a draft spec; open questions remain.

Standing constraints that bound every claim here: no ARM64 code in this tree has ever
executed on hardware (`LE-27`); no platform is qualified, so **no worst-case timing bound
is quotable from anything in this paper** (`ADR 0005`, `goals/assurance/qualified-platforms.tsv`);
the console UX is host-side and implies no on-target graphics stack (`LE-53`).

---

## Part 1 — How TinyOS boots and loads on a bare-metal Raspberry Pi 5

### 1.1 What the board does before TinyOS exists (context — the Pi's own chain)

The BCM2712 boots the way every Raspberry Pi does: an on-die boot ROM starts the
**VideoCore** side, which loads the signed second-stage bootloader from SPI EEPROM; that
bootloader reads `config.txt` from the boot partition, loads the named kernel image and
the matching device tree into RAM, and finally releases the ARM cores into it. The
handoff follows the **Linux AArch64 boot protocol**: the kernel image is entered at its
first byte with the MMU off, caches in a known state, `x0` holding the physical address
of the device-tree blob and `x1`–`x3` reserved-zero, at `EL2` (typical Pi 5 firmware
configuration) or `EL1`.

The practical consequence: **TinyOS does not need — and does not get — its own
first-stage loader on this board.** The firmware *is* the loader. TinyOS's job begins at
the entry point of the image `config.txt` names (`kernel=` a TinyOS image instead of a
Linux one), which is exactly the seam `FEAT-P1-07` builds against.

### 1.2 The TinyOS entry — what exists in-tree today (`hal-arm64`, **designed**, host-tested halves **live**)

`os/src/hal-arm64/src/` holds the AArch64 boot path (`STORY-P1-07-01`), split in two on
purpose:

1. **The entry half (assembly + register writes, compiled only for `aarch64`).** The
   minimal stub the coding standards permit: capture `x0`–`x3` and `CurrentEL` before
   anything can clobber them, establish a stack, zero `.bss`, and enter Rust across a
   thin `extern "C"` boundary. **None of this half has executed** — it is compiled and
   reviewed, and `STORY-P1-07-01` is not Verified until a serial capture from the board
   exists. That is stated in the module header itself, not just here.
2. **The reporting half (pure Rust, host-tested — live).** The first thing the Rust side
   does is *speak*: over the PL011 UART it prints `TINYOS-BOOT/1 current_el=…` — the
   exception level **first**, then the firmware handoff (`x0`–`x3`), then the fixed
   known byte sequence `TINYOS-BOOT/1 READY 0123456789ABCDEF` so the capture is diffed,
   never eyeballed. The ordering, framing and byte-exactness are pinned by tests that
   run on the x86_64 dev host against an MMIO double — the claim "`CurrentEL` is printed
   before anything else" is machine-checked, not argued from a code reading.

Two properties of that handoff deserve emphasis because they are security posture, not
ceremony:

- **The firmware's registers are reported, never trusted.** `x0` is the DTB pointer; the
  stub prints it and drops it — it does not walk the device tree (`BND-02`, `BND-03`,
  `PD-14`). Firmware output is data crossing a boundary, and the Charter's rule that
  remote bytes are data, never code, starts at the very first instruction.
- **Every line is tagged** (`TINYOS-BOOT/1 `) so TinyOS's own output can be separated
  from whatever the firmware left on the same wire.

### 1.3 From stub to kernel — the load path (**designed**, mirrored from the live x86_64 path)

The Pi 5 bring-up deliberately generalizes the boot architecture that already runs under
QEMU on x86_64 (the Tier 0 CI gate, **live**): exception vectors installed before any
interrupt can arrive (`vectors.rs` / the 256-entry IDT equivalent on x86), fail-closed
unserviced vectors, architectural timer armed (`timer.rs` / local-APIC on x86), fault
reporting decoded from `ESR_EL1` (`esr.rs`), then the kernel proper: fixed pools — no
heap, no demand allocation — W^X-validated sections, spoor journal, and the task set.
There is no initrd, no module loader, no second link stage: **TinyOS loads as one
statically linked image**, and everything that will ever be executable is either in that
image or must later pass the full code-admission gate chain
(`goals/security/code-admission-gates.tsv`) — the boot path itself can never become a
code-injection path.

### 1.4 What the Pi 5 may and may not be used to claim (**decided — ADR 0004/0005**)

The Pi 5 is `EPIC-P1`'s first physical timing target, and ARM64 is the real-time tier of
record — x86_64 is structurally disqualified from worst-case claims because System
Management Interrupts are invisible, unmaskable and unattributable to the OS
(`ADR 0004`, argument preserved in `ADR 0005`). But `ADR 0005` makes the ARM64 tier
**conditional on per-platform secure-world qualification**: a GIC with secure interrupt
groups routed to `EL3` via `SCR_EL3.FIQ` can preempt `NS-EL1` *irrespective of
`PSTATE.I/F`* — the same hole, on the other architecture — and a non-secure kernel
cannot even read `SCR_EL3` to enumerate what was routed away from it. So the boot story
above ends not with "and then bounds are quotable" but with a **qualification record per
platform** (`rpi5-bcm2712` is registered, `unqualified`): characterize the board's
secure-world/firmware residency behaviour, then and only then state bounds from it. A
white paper that skipped this paragraph would be marketing.

---

## Part 2 — How hardware resources (GPU cores on a Jetson) reach the applications on top

The committed second board is the **Jetson Orin Nano Super** (`jetson-orin-nano-super`
in the platform register — the current-generation successor to the original Jetson Nano;
same unified-memory architecture, which is what matters here). The pipeline below is how
any device resource reaches an application; the GPU is the worked example. States:
architecture **designed** (`docs/universal-driver-model.md`,
`docs/inference-architecture.md`); implementation **future** (Phase 6 for inference);
the governing gate pattern (ACI, one gate for every caller) is **live** today in
host-side form — it is the same pattern the V1 console's signed manifest enforces.

### 2.1 The layer stack (bottom-up)

```text
                        Application / inference runtime / console
                                        │  ACI capability calls (typed, budgeted,
                                        │  rate-limited, audited — the ONLY gate)
        ┌───────────────────────────────▼────────────────────────────────┐
        │  C3 supervisor (e.g. the inference runtime) — proposes, never   │
        │  owns hardware; budget-charged per action                       │
        └───────────────────────────────┬────────────────────────────────┘
                                        │  brokered, capability-scoped
        ┌───────────────────────────────▼────────────────────────────────┐
        │  C2 broker services: GPU broker · storage broker · display      │
        │  compositor — each a resource-budgeted userspace task           │
        └───────────────────────────────┬────────────────────────────────┘
                                        │  Driver Capability Interface (DCI)
        ┌───────────────────────────────▼────────────────────────────────┐
        │  Universal Driver Interface (UDI) class contracts:              │
        │  gpu/compute · display · storage · network · CAN · sensor       │
        │  — generic class driver mandatory, vendor extensions additive   │
        └───────────────────────────────┬────────────────────────────────┘
                                        │  DMA/IRQ/MMIO grants (scoped, enumerated)
        ┌───────────────────────────────▼────────────────────────────────┐
        │  HAL: bus enumeration + unified hardware manifest               │
        │  (ACPI and Device Tree normalized to ONE topology model)        │
        └────────────────────────────────────────────────────────────────┘
```

Reading order for the five load-bearing rules:

1. **Drivers are userspace, not kernel.** A GPU driver is a resource-budgeted task
   holding exactly the DMA regions, IRQ lines and MMIO ranges its manifest names —
   never blanket physical memory. A crashing driver restarts through the deploy
   protocol's health-check machinery; it cannot fault the kernel and cannot block an RT
   task. On the Jetson, the vendor's CUDA-class SDK code is confined to `-sys` binding
   crates behind the DCI trait, like any other vendor boundary.
2. **Class driver first, vendor extension second.** A device that identifies as
   `gpu/compute` gets baseline function from the mandatory generic class driver;
   vendor extensions (better DMA paths, vendor-specific queues) are additive and
   removable. Friction — needing a vendor blob for basic function — is designed out.
3. **GPU work is admission-controlled, never RT-scheduled.** This is the single most
   important sentence in this part. GPU completion latency is coarse and scheduled by
   vendor firmware outside TinyOS's authority, so a GPU submission requests a budget
   (VRAM footprint, submission rate) and is **admitted, throttled or refused by
   policy** — it is never given scheduler priority that could sit in front of an RT
   deadline, and no RT-context code path ever waits on a GPU fence
   (Non-Negotiable 6). A stalled model degrades or errors through the ACI; it cannot
   make a motion-control loop late. The V1 console's Agent tab already renders exactly
   this contract (admission panel: VRAM, rate, verdict) ahead of the runtime existing,
   so the runtime cannot land without its gate.
4. **Unified memory is a managed handle, not a shared pointer.** On Jetson-class
   hardware, CPU and GPU address the same DRAM. The **Unified Memory Manager** exposes
   that as typed, ownership-tracked buffer handles: one writer at a time, explicit
   fenced hand-off between CPU and GPU access, no silent aliasing — the RT kernel's
   own memory discipline extended to heterogeneous memory. On hardware *without*
   unified memory the same handle API falls back to explicit host↔VRAM copies, so
   applications never branch on the memory model. Model weights follow the
   mmap/pointer path: pay the SSD latency once per page on first touch, then bare
   pointer dereference — with bounds/format validation before any pointer derived from
   an untrusted model file is trusted.
5. **Applications never see any of the above directly.** An application — including a
   locally hosted LLM runtime — reaches hardware only as **ACI capabilities**: typed,
   pre-registered, rate-limited, budget-charged, audited. The `APP-03` platform row
   binds this: a C3 inference supervisor uses C2 GPU/storage brokers while prompts,
   model files and generated content remain C4 inputs. A capability not in the grant
   table is *refused at the grant table* — the exact behaviour the console's Agent tab
   displays today with its `storage.format` card.

### 2.2 Why this shape and not "expose the device"

Every alternative — ioctl surfaces, direct device nodes, root-owned vendor daemons —
lets the busiest caller or the buggiest driver set the system's worst case. TinyOS's
founding intent is the opposite ordering (safety > security > correctness >
performance), and the stack above is that ordering drawn as boxes: the RT core's time
is never in the gift of a GPU queue, authority is enumerated rather than inherited, and
every hardware touch has an audit atom. The cost is honest too: brokered access adds a
hop, and Phase 6 will have to *measure* what that hop costs (D08/D14 domains) rather
than assert it away.

---

## Part 3 — How the console UX is actually rendered, and where hardware acceleration comes from

### 3.1 Today (**live**): the host's GPU stack does the work, and we say so

The V1 operator console (`external/tauri/tinyos-poc/stage-e-console-app`) is a **Tauri
(vendored fork) application on the Windows host**. The rendering pipeline, precisely:

```text
V1 chrome (HTML/CSS/JS, no framework)
   → WebView2 (Chromium): Blink layout → Skia paint/raster
        → Chromium GPU process (ANGLE → Direct3D 11) composites layers
   → DirectComposition hands the swapchain to DWM
   → Windows' display driver + GPU scan out the frame
```

So yes — the console is hardware-accelerated end to end, and **none of that
acceleration is TinyOS's**. It belongs to the host OS's graphics stack; `LE-53` exists
precisely so a screenshot of this console is never mistaken for an on-target graphics
claim. TinyOS's contribution today is everything *behind* the pixels: the signed verb
manifest, the per-webview identity resolver, the host-owned system line, and the real
shell transcripts the renderer colours.

What TinyOS-side discipline buys even on this borrowed renderer is the **render
budget** (`SPEC.md` §6): one DOM node per transcript line, append-only (never
re-render), a 5 000-line ring, one 4 Hz clock paused when hidden, ~400–480 nodes at
boot, paint ticks around a millisecond — numbers printed live on the system line
(`nodes · ms`) so a regression shows up in a screenshot. The console is cheap on a
Chromium; that is what makes it *plausible* on something much smaller.

### 3.2 Why the smoke run needs no window at all (answering the operational question)

Every functional property of the console is exercised **through IPC state, not
pixels** — `read_tab`/`read_console` snapshots, and `window.smokeKey` dispatching
in-page events. The only pixel-dependent artifact is screenshot evidence, captured via
`PrintWindow`/`PW_RENDERFULLCONTENT` (DWM renders the window's own surface regardless
of z-order), so the unattended run holds no always-on-top, steals no focus, and works
fully occluded (`51e635c`). Rendering and verification are deliberately separable
concerns.

### 3.3 The destination (**designed shape, honestly costed**): the same UI over a TinyOS renderer

Tauri's runtime seam makes the port well-shaped: `tauri-runtime` is a trait set
(`Runtime`, `WebviewDispatch`, `WindowDispatch`…) with `wry`/`tao` as the sole current
implementation — so an on-target console is, structurally, `impl Runtime for
TinyOsRuntime` (`docs/tauri-internals-review.md` §4, `ADR 0007`). The review is equally
clear about what the seam does **not** shrink: Tauri ships no renderer, so a native
implementation needs a window/input/**compositor** service and a browser-class engine
(`EPIC-H3`) — or, more likely first, a far smaller path: the V1 console deliberately
uses no framework, no webfonts, a fixed palette, and a node budget small enough that a
purpose-built retained-DOM-subset renderer over the UDI `display` class driver could
carry it. In *either* future, hardware acceleration on-target arrives the Part 2 way:
a GPU class driver under the DCI, a compositor running as a C2 broker with a scoped
DMA/IRQ grant, admission-controlled like every other GPU client — the console's frames
queue behind the same policy gate as the LLM's tensors, and neither may ever make an
RT deadline late.

---

## Source map (where each claim lives)

| Claim area | Authority |
| --- | --- |
| Pi 5 entry stub, handoff report, never-executed status | `os/src/hal-arm64/src/boot.rs`, `STORY-P1-07-01`, `LE-27` |
| x86_64 tier-0 boot primitives (IDT, APIC, paging, W^X, pools) | `docs/whole-system-context.md` "Current implementation truth" |
| RT-tier attribution and qualification precondition | `docs/adr/0004…`, `docs/adr/0005…`, `goals/assurance/qualified-platforms.tsv` |
| Driver stack: HAL → UDI → DCI, userspace drivers, class-first | `docs/universal-driver-model.md` |
| GPU admission control, UMM, mmap model loading, distributed inference | `docs/inference-architecture.md` |
| App-facing gate (ACI), containment classes for inference | `SECURITY_CHARTER.md`, `goals/context/application-platforms.tsv` `APP-03` |
| Console rendering, render budget, host-side honesty | `work/UX-V1/SPEC.md` §6, `LE-53`, `REPORT-2026-07-30-03` |
| Tauri runtime seam and its true cost | `docs/tauri-internals-review.md`, `ADR 0007/0008/0009` |

**Non-claims, restated once:** nothing in this paper is `PD-*`/`TG-P*`/timing evidence;
no number here is a bound; the Jetson and Pi 5 sections describe committed designs and
registered-but-unqualified platforms, not shipped capability.
