# Handover 04A (2026-08-03) — Owner Direction: TinyRemote — the Cable Is the Signal, the Window Is the Machine

Owner order, recorded as the product shape for the headless Pi 5 (extends
[`03A`](03A-headless-pi5-direction.md), which fixed *headless*; this names what
replaces the screen). Direction and decomposition map — **nothing here is
implemented, and nothing below may be labelled live until its evidence exists.**

## The experience, as ordered

> If the Pi 5 has TinyOS on it and it is connected to another machine with a
> peer-to-peer Ethernet cable, that is a **direct signal** to render a remote
> desktop in a **TinyRemote** application (host application on the laptop) with
> the UX of TinyOS — as if TinyOS is in that window. The user can choose within
> TinyRemote whether they want the TinyOS desktop or something else.

Three product commitments fall out of that sentence:

1. **Plug-and-see.** Inserting the cable is the discovery event. No IP
   configuration, no DHCP server, no dialogs about adapters — link-local
   addressing on both ends, TinyRemote listening for the link and the device
   announcing itself. The *feel* is: cable in, window appears.
2. **The window is the machine.** TinyRemote renders TinyOS's own UX —
   the DOS-inheritance console surface and whatever UX phases exist — with
   input flowing back, full-fidelity enough that "as if TinyOS is on that
   window" is not marketing. The rendering hardware is the laptop's; the
   machine identity, state and authority are the device's.
3. **TinyRemote is a chooser, not just a viewer.** The desktop is the default
   view; beside it sit the other things a connected operator wants: the
   Hardware Evidence view (08A Demo 3), spoor stream, deploy/update, run
   records, board diagnostics. "Something else" is a first-class menu, not an
   easter egg.

## The security line that must never move

"The cable is the signal" is a **discovery** statement, never an **authority**
statement. A bare socket that renders whatever the wire claims would violate
the charter on arrival. Binding rules, all pre-existing:

- The session rides the deploy-protocol/WCI authenticated pairing and session
  model — the cable triggers *discovery + offer*; rendering starts after the
  session authenticates. Physical possession of the cable may be a pairing
  *factor* (as it is for the P2P deploy loop), never the whole authorisation.
- Remote bytes are data, never code, in both directions (`RCG-01`); frames
  and input events are typed, bounded, hostile-input-validated at both ends.
- On-device, the UX stream source and input sink go through the same policy
  seams as every other surface — no privileged side channel for the renderer
  (the Ti64 console's grant-table discipline extends to the wire).
- TinyRemote on the host renders classified output; payload-controlled escape
  sequences stay inert (`LE-59`'s rule, now over a network).

## What TinyRemote is, relative to what exists

The Ti64 console **is** the embryo of TinyRemote: it already renders TinyOS
surfaces host-side through narrow seams (`open_tab`/`run_line`/`read_tab`/…),
already has tab identity, grant tables, and the honesty vocabulary
(live/pending/absent). TinyRemote = that application grown a **transport**:
the same seams carried over an authenticated link instead of an in-process
call, plus a connection manager (link watch → discover → pair → choose view).
This is why the satellite/same-origin-bus architecture was the right call —
the seams were always going to become a wire.

## Dependency chain (unchanged by enthusiasm)

1. **Serial-first bring-up** (`FEAT-P1-07` ladder) — the UART debugs the
   drivers Ethernet needs; the current blocker is physical (adapter wiring /
   JST-SH cable; three silent captures, `LE-66`'s reader fix landed).
2. **`LE-26` closure as Stories**: PCIe controller → RP1 → Ethernet MAC/DMA
   as a C2 device service with containment contracts.
3. **Link-local + authenticated session** per `docs/deploy-protocol.md` /
   WCI model (the `deployer`-capability shape; the ACI seam).
4. **The UX stream**: TinyOS-side frame source + input sink behind policy;
   host-side render in the console/TinyRemote shell; spoors and deploy verbs
   on the same session.
5. **The chooser UX**: view selection, device identity display, the honesty
   badges (LIVE ON SILICON / MECHANISM EVIDENCE / PLATFORM UNQUALIFIED).

Milestones 2–5 each get Features/Stories with contract rows before code, per
the spine; none may jump the queue ahead of the 08A silicon evidence unless
the owner reorders again. The link-up "direct signal" also gives the dev loop
its aliveness ping long before the full stream exists — the first Ethernet
Story can deliver *board-present detection* as its earliest visible win.

## Naming

**TinyRemote** — the host application (Windows/macOS/Linux laptop side).
The name TinySpot remains the catalogue's remote-UX concept; whether
TinyRemote subsumes TinySpot's row or implements it is settled when the
Feature is promoted, not here.
