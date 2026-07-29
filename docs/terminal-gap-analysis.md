# Terminal Gap Analysis — TinyOS vs MS-DOS 4.0, Linux/POSIX, and Windows Terminal

Status: **Measured register, spec-level — companion to [`goals/context/terminal-gap.tsv`](../goals/context/terminal-gap.tsv)**
Ordered by: [`13F`](../session/hand-2026-07-29/13F-next-session-mandate-console-and-gap-analysis.md) §2 (Deliverable B) ·
Reported in: [`REPORT-2026-07-29-04`](../goals/reports/REPORT-2026-07-29-04.md)

**No binary-compatibility claim is made here, ever.** [`EPIC-P2`](../goals/epics/EPIC-P2.md) §2
makes that prohibition an exit criterion: parity is *ergonomic* — behaviour, switches, message
shapes, muscle memory — never the execution of DOS binaries. This document compares what the
references *do* with what TinyOS has *decided*; it confers no support status on anything.

## Method, and what "measured" means here

- **The DOS column is read from source, not memory**: Microsoft's released MS-DOS 4.0 tree at
  [`external/MsDOS`](../external/MsDOS) (`v4.0/src/CMD/…`), per [`ADR 0008`](adr/0008-external-trees-live-under-external.md)
  a reference *"for behaviour and structure, never code TinyOS builds on."* Every DOS claim in
  the register carries a file-path evidence cell, down to exact message strings
  (`"Are you sure (Y/N)?"` is quoted from `COMMAND.SKL`, not from folklore).
- **The Windows Terminal column is read from source** at
  [`external/WindowsTerminal`](../external/WindowsTerminal), on the `EPIC-P2` §6.4 terms: take
  the interaction model and the component boundaries; never the authority model — there isn't
  one.
- **The Linux column is documented POSIX/coreutils behaviour** — the references live outside
  this repository, so those cells cite no file paths and are held to spec-level confidence.
- **The TinyOS column quotes the decided record only** —
  [`docs/cli-compatibility-mvp.md`](cli-compatibility-mvp.md), `EPIC-P2` §§2–6, `LE-53` — never
  an aspiration invented for this table.

## Status discipline — every row is spec-level, and why (`LE-55`)

13F §2.3 planned live verification of the storage-free verbs (`echo`, `env-get`/`env-set`,
`task-list`, `clear-screen`, prompt behaviour) by typing at a QEMU fixture through the Stage E
operator console. **That is not executable today, and the register does not pretend
otherwise:** no shell crate exists in the `os/` workspace (`LE-48` records the filesystem half
of that absence), no fixture reads serial input, and the `hal-x86_64` serial driver is
transmit-only — there is no UART RX path anywhere in the tree. This is registered as **`LE-55`**
with its repair path (a serial-RX HAL story plus an interactive echo-class fixture, landing
with `EPIC-P2`'s shell decomposition).

One row is honestly `live-verified`: the transport asymmetry itself
(`transport:serial-asymmetry`), demonstrated end-to-end by the Stage E console — `send_line`
resolves through the signed manifest and then reports the one-way transport as an error, while
the serial *output* of a real fixture streams live into the console pane.

15 of the 22 verbs additionally presuppose a filesystem that does not exist (`LE-48`,
`EPIC-P2` §1); for those, spec-level is the ceiling regardless of the transport.

## Axis 1 — the command surface: what the DOS source actually says

The register's `verb:*` rows carry the details; these are the findings that were **not** in
[`cli-compatibility-mvp.md`](cli-compatibility-mvp.md)'s spec table and change how its DOS
bindings should be read:

1. **`MOVE` does not exist in MS-DOS 4.0.** Not in `COMMAND.COM`'s command table, no source
   directory. `REN` renames in place, two operands, never across directories. TinyOS's
   `MOVE` binding is a *post-4.0 DOS-ism* (DOS 6 era) adopted for muscle memory — correct to
   adopt, wrong to call 4.0 parity. The spec table should say so.
2. **DOS 4.0 `COPY` silently overwrites — no confirmation prompt exists in `COPY.ASM`.**
   POSIX `cp` is the same. TinyOS's stricter posture (labels survive every copy; destructive
   confirmation policy) diverges from *both* references, deliberately.
3. **`DIR` has `/P` and `/W` only** in 4.0 — no `/S`, no `/A`, no `/O`. The spec's
   `/S`↔`-R` mapping adopts a later DOS-ism. Same class as `MOVE`: fine, but not 4.0.
4. **`DEL`'s famous prompt fires only on an all-wildcard pattern** (all 11 FCB characters
   `?`): `"All files in directory will be deleted!"` / `"Are you sure (Y/N)?"` — and `/P`
   gives per-file `",    Delete (Y/N)?"`. There is no `/S` in 4.0 at all. TinyOS's
   confirmation-policy-on-recursive is stricter than anything 4.0 had.
5. **4.0's exit codes are a counter-example, twice.** `FIND` documents ERRORLEVEL 1 for
   "no matches" and then never assigns it (0 and 2 are the only values stored); `ATTRIB`
   *discards* its exit code entirely (a bare C `exit()` — the intended code visibly dropped,
   with the PTM number in a comment). TinyOS adopts the POSIX 0/1/2 convention that 4.0
   `FIND` documents but fails to implement — and every verb returns a meaningful code.
6. **`ECHO` prints the raw command tail from offset `82h`**, preserving case and spacing, and
   pipes force echo off — batch semantics that `.TCB` inherits knowingly.
7. **`CLS` probes for ANSI.SYS per invocation** and emits `ESC[2J` when redirected — even DOS
   treated clear-screen as a sequence when a screen wasn't guaranteed. TinyOS generalises:
   sessions emit VT; only the tab host touches the renderer.
8. **`MORE` closes stdin handle 0 and dups STDERR into it** to read keystrokes while paging a
   pipe — a hack the tab host makes unnecessary (input routes to the focused tab, §6.3).
9. **Message shapes worth honouring**: `"Press any key to continue . . ."` (with the spaced
   ellipsis), `"%1 File(s) copied"`, the `DIR` header/footer format, `"-- More --"`, TREE's
   `/A` ASCII fallback (which `EPIC-P2` §6.5 rule 2 generalises: ASCII is the canonical form).

## Axis 2 — terminal-window behaviour: the seam that is now a requirement

The `window:*` rows compare four models. The one-line history: **MS-DOS is a single
synchronous buffer** (the writer *is* the renderer; a slow screen blocks the program);
**a Linux pty decouples the application from the terminal but leaves rendering to the
emulator**; **Windows Terminal separates the text buffer from the renderer completely** — and
that separation is precisely what `LE-53`(b) promoted to a TinyOS requirement, because
`EPIC-P2` §6.6's *drop-frames-not-block* obligation is inexpressible without it.

The Windows Terminal evidence is specific and quotable (register carries paths):

- The writer's only signal to the renderer is a **relaxed atomic store**
  (`Renderer::NotifyPaintFrame`) — a writer never waits for a frame.
- Frames are **coalesced, not queued**: the render thread clears the flag before painting,
  with the in-source comment *"NotifyPaintFrame() calls are picked up and ignored. We're about
  to render a frame after all."* — unbounded write bursts collapse into one paint.
- **Every blocking wait sits outside the buffer lock** (the ~60 FPS self-throttle, the GPU
  frame-latency wait, `Present()`), so writers own the buffer while the renderer sleeps.
- The same pattern repeats at the byte level in ConPTY: double-buffered `OVERLAPPED` pipe
  writes — the producing app is never gated on the terminal draining.

TinyOS adopts the shape and strengthens the contract: the tab host runs **below the real-time
floor**, is preemptible, and **a dropped frame is reported as dropped** (Windows Terminal
drops silently, correctly for a desktop). Scrollback adopts the fixed-allocation circular
buffer (recycle-oldest, O(1), no growth) — the capacities doctrine reaching the terminal.

Where Windows Terminal is the **counter-example**, the register says so in the tab-authority
and input rows: every WT tab inherits the user's full token, and nothing stops tab content
painting a fake prompt. `EPIC-P2` §6.1 (tab = authority boundary) and §6.3 (reserved region +
secure-attention key) are the parts that must be designed here, not referenced.

## Axis 3 — execution-level verification, scoped

Blocked as stated above (`LE-48` for 15 verbs, `LE-55` for the transport), and the register's
`status` column says so row by row. The first live rows arrive when: (1) a UART RX path exists
in `hal-x86_64`, (2) an interactive echo-class fixture exists in the `qemu-x86_64` catalogue,
(3) the Stage E console types the DOS form and the POSIX form at it and records both. The
console half is already built and waiting.

## Maintaining this register

One row per behaviour; every row carries evidence; `status` is `spec-level` or
`live-verified`, nothing softer. The register is not yet machine-gated: wiring it into
`check-spine-files` requires the two-sided invariant (a fast-checked file must also be read by
`check-assurance-spine` — see `spine_files.rs`'s cross-check test), i.e. a small validator in
`assurance.rs` first. That is deliberate scope for a next session, exactly as 13F anticipated
("a register can be gated later the way everything else is").
