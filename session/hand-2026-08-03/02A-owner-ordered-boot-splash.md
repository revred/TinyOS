# Handover 02A (2026-08-03) — Owner-Ordered: the Boot Splash (`STORY-P1-07-07`)

The owner watched the first physical boot of the staged card end in the designed
success state — a dark, silent screen — and rejected it in plain words: *"that is
the worst UX."* The boxed board also hides the ACT LED. This session delivered the
fix as a real Story, not a hack: **"TinyOS" in block letters on HDMI at boot**, via
the one display mechanism needing no driver stack — a framebuffer requested from
the GPU firmware over the VideoCore mailbox property interface.

## What exists now

`os/src/hal-arm64/src/hdmi.rs` (+ one call in `boot.rs` after the verdict):

- **Property message host-pinned word-for-word** (1280×720, depth 32, allocate at
  4096, get pitch; 16-byte aligned by type).
- **The firmware's descriptor is hostile input**: eight typed rejection arms
  (response code, missing tags, zero base, bad size/depth/pitch/dimensions), each
  driven by a host test; any rejection paints nothing.
- **Pure renderer behind a `Surface` seam**: background fill + centred 8×8
  block-glyph "TinyOS" at computed scale; host tests pin bounds (zero
  out-of-surface writes at five geometries), coverage, centring, and
  too-small-surface → background-only.
- **Every board wait is bounded** (`BoundedPoll`, host-tested countdown; both
  volatile spins consume it). Every failure silently continues into the same
  `park()`.
- **Evidence order is law**: the splash runs strictly *after*
  `TOS64-RESULT/1` — a splash failure can never delay or alter a protocol byte
  (`TEST-P1-07-07-A` clause 5 will prove byte-identity on the board).

Spine artifacts: `STORY-P1-07-07` + `TEST-P1-07-07-A` + contract row
(`D01`/`SEC-19,SEC-20`/`C0,C1`/`specified`); `FEAT-P1-07` is now seven Stories —
the seventh-Story re-scope its own notes anticipated, decided by the owner.

## Gates

`hal-arm64` 118 → **125 host tests** (Red first: 44-error compile-stage), workspace
16/16 suites green, fmt clean, host clippy and **cross-target AArch64 clippy**
clean (the LE-64 lesson applied), spine green at 28 Features / 73 Stories /
57 Tests / 61 Reports. Image rebuilt: `kernel8.img` **89,036 bytes, sha256
`bbb85bd0…ca49a`** — the SD card staged earlier carries the pre-splash image and
must be re-staged before the next boot.

## The blind-flight caveat, stated plainly

The Pi 5 bare-metal mailbox-framebuffer path is **not a documented-stable
contract** (BCM2712 mailbox base taken from the bcm2712 device tree;
VC bus-alias masking assumed as on prior Pis). If the screen stays dark, the boot
still parks green — and nothing can be debugged further until the serial adapter
arrives. Criteria 4 and 5 wait on the board; criterion 5's evidence is a
photograph beside an unchanged capture.

## Also this session (uncommitted by design)

`work/tools/sdprep/` — a C# card-prep tool (owner preference over PowerShell):
auto-detects the removable disk, refuses the system disk / boot-flagged
partitions / OS-marker volumes / >1 GiB data, demands the operator type the
drive letter *and* shows a per-volume content manifest before the typed YES,
then diskpart-formats and stages with SHA-256 verification. Left untracked: the
committed reference stays `docs/pi5-prepare-sd.ps1`, because non-Rust in-tree
code needs an ADR the owner has not (yet) ordered. It successfully prepared the
first card (TOS64BOOT, verified `7e9d…` — now superseded by the splash image).
