# STORY-P1-07-07 — The Boot Splash: a Dark Screen Is Not a User Interface

Status: **In progress — criteria 1, 2 and 3 met; **criteria 4 and 5's missing thing is a firmware that grants the request, and this one refuses it.** Host half Green 2026-08-03. Criterion 2 — the response validated as hostile input, whole — is the one this board actually exercised, and it exercised the **refusal** arm: every boot from `BOARD VERDICT 1` onward reports `FB=REFUSED`, most recently on 2026-08-05's netbooted run, so the rejection path resolves cleanly and leaves the boot protocol untouched exactly as specified. That is a real result and it is the useful one: **this firmware refuses the legacy mailbox framebuffer path**, settled by data rather than by conjecture, and `STORY-P1-07-09` answered the same question from the other side by painting the firmware's own canvas where the mailbox would not answer. **Criterion 5 asks that the screen say TinyOS via *this* path, and it never has** — the title on the monitor comes from `STORY-P1-07-09`'s scan-out surface, not from a mailbox-allocated framebuffer, and crediting this Story with it would be crediting a claim to the wrong mechanism. Criterion 4's protocol-invariance half is unobservable for the separate reason that serial has never produced a byte (`LE-47`), though the canvas transcript is byte-stable with the splash code present. **This Story may not be closeable on this hardware at all**, and that is the finding rather than a delay: the criteria are written against a firmware behaviour this board does not offer. The decision owed is whether to retire it against `STORY-P1-07-09`, re-scope it to a board whose firmware grants the exchange, or leave it open as stated debt — an owner call, not a diff. **2026-08-07 sharpened the finding without changing it:** the lit-canvas boot (`hand-2026-08-07/07F` §7b) reported `FB=REFUSED` on the wire while `-09`'s scan-out surface painted the monitor — the mailbox refusal and a working display observed together on one boot, so the refusal is this firmware's policy against *this path*, not a display absence, and no amount of scanout debugging will change this Story's answer. The owner call stands. **Not Verified.**
Feature: [`FEAT-P1-07`](../features/FEAT-P1-07.md)
Introduced in: owner order, 2026-08-03 — *"the screen goes and stays dark/quiet — that is the worst UX"* — an explicit owner re-scope of the Feature's display non-goal, recorded per that non-goal's own rule that pulling display in is a scope decision

## Description

Until now the bring-up image's success state was a silent screen: correct by the
evidence rules, indefensible as an experience — an operator (or investor) watching
the board cannot tell triumph from death without a serial adapter. This Story puts
**"TinyOS" in block letters on the HDMI output at boot**, using the one display
mechanism that needs no driver stack: asking the GPU firmware for a framebuffer over
the VideoCore **mailbox property interface** and writing pixels into it.

The discipline that keeps this from corrupting the Feature:

- **The splash is subordinate to the evidence.** It runs *after* the
  `TOS64-RESULT/1` verdict is on the wire, every mailbox wait is bounded, and every
  failure path is silent-and-continue into the same `park()` — a missing splash can
  never delay, alter or hang the serial protocol that Stories `-01`/`-02`/`-05`
  close on.
- **The firmware's mailbox response is hostile input** (`FEAT-P1-07`'s containment
  stance, `BND-02`/`PD-12`/`RCG-01`): a framebuffer descriptor is believed only
  after typed validation — response code, tag presence, non-zero base, sane
  dimensions, pitch consistent with width and depth — and a rejected descriptor
  means no splash, never a wild pointer write.
- **A splash is not evidence.** Nothing here contributes to any Story's capture,
  measurement or qualification claims; the claim it earns is UX, and the Report
  evidence for criterion 5 is a photograph plus the unchanged serial capture.
- **Blind-flight caveat, stated up front:** the Pi 5 bare-metal
  mailbox-framebuffer path is not a documented-stable contract. Until the serial
  adapter arrives, a dark screen with a green verdict capture cannot be debugged
  further; the Story's board criteria wait for exactly that tooling.

## Acceptance criteria

1. **The property messages are host-pinned.** Both the native-size query
   (get-physical-display-size) and the framebuffer request (physical/virtual
   size at the chosen mode, depth 32, allocate at 4096 alignment, get pitch,
   end tag) are built by pure functions whose exact word layouts host tests
   pin, 16-byte aligned by construction. The chosen mode is the display's
   native resolution when its (hostile-validated) answer is sane, 1280×720 as
   fallback — the splash adapts to the panel and centres on it (owner
   direction, 2026-08-03).
2. **The response is validated as hostile input, whole.** A typed rejection per
   arm — wrong response code, missing tag, zero/implausible base or size,
   depth mismatch, pitch/width inconsistency, dimensions beyond bounds — each
   driven by a host test; any rejection yields no splash and an untouched boot
   protocol.
3. **The renderer is pure and host-tested.** Block-glyph "TinyOS" centred on a
   filled background, rendered through a surface seam; host tests pin bounds
   (no out-of-surface write), coverage (the text actually paints), and layout
   (spot-checked pixels).
4. **Board: the splash cannot perturb the protocol.** The serial transcript with
   the splash code present is byte-identical to before it (same READY sequence,
   same verdict line), splash success or failure alike — proven by the board
   capture when the adapter arrives.
5. **Board: the screen says TinyOS.** Powering the boxed board with a monitor
   attached shows the splash within firmware-boot time — the UX moment this
   Story exists for, evidenced by photograph beside the capture.

## Progress, 2026-08-03 (evening)

The adaptive-mode image (`18a28448…54d9`) was staged to the card and powered on
the physical board. **The board outcome is unconfirmed** — an initial sighting
report was withdrawn by the owner moments later ("too early to celebrate"), so
nothing is recorded as observed. The criteria close on committed evidence
(photograph beside a capture; serial byte-identity), and none exists yet.

## Named debt this Story leaves open

- No general display driver, no compositor, no text console, no double
  buffering, no EPIC-H2 claim — this is one static frame painted once.
- `LE-27`-shaped caveat: until a board runs it, the AArch64 halves are compiled,
  host-tested and never executed.
- The mailbox module is bring-up-scoped: commissioning it into a real C2 device
  service with contracts is future work if display ever becomes a platform
  claim.

## Tests

[`TEST-P1-07-07-A`](../tests/TEST-P1-07-07-A.md) — written before implementation,
per the TDD mandate.
