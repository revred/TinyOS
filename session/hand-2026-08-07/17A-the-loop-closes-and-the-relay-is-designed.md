# 17A — The loop closes: a board power-cycled by software, and the relay designed

Follows [`16A`](16A-the-first-conversation-built.md), same session, same day.
Solo in the tree throughout; no concurrent commits arrived.

**The one sentence, if only one survives:** *`tos64-power` switched real mains,
the board rebooted and put `TOS64-RESULT/1 fixture=measure ok=true` on the wire
10.4 seconds later, sighted by `ti64dink --until` at exit 0 — the tenth stage of
the board evidence loop, and with it the whole loop, closed end to end with no
human hand (`LE-95`).*

## 1. `LE-95` closed — the stall that cost forty-eight hours of evidence

The device arrived: a **Shelly Plus Plug UK**, `SNPL-00112UK`, gen 2, firmware
`1.4.4`, `auth_en: false`, now static at `http://192.168.1.20`. It needed **no
code** — the `shelly-gen2` dialect written on 2026-08-06 against four unowned
dialects turned out to be written against exactly this RPC surface, `was_on`
trap included. Verified by reading the dialect against the device rather than
trusting the name.

What ran, against real mains, in order:

| Command | Result |
|---|---|
| `status` | `plug: OFF`, exit 0 |
| `on` | `On: confirmed by readback (ON)`, exit 0 |
| `status` | `plug: ON`, exit 0 |
| `cycle --off-ms 5000 --on-wait 20` | off confirmed → held 5 s → on confirmed, exit 0 |
| `ti64dink --until text=TOS64-RESULT/1` | **SIGHTED after 10.4 s**, exit 0 |

The last two lines together are the thing. A power cycle issued by software, a
board that came back, a verdict parsed off the wire by a machine, and an exit
code a script can gate on — with nobody touching a plug. `ADR 0005`'s `Q3`
residency campaign is a campaign *with a stated duration*, which is precisely
what a manual bench cannot run, so the single locked gate behind `0/100`
assurance-verified Stories was downstream of this switch.

**Commissioning was where the difficulty actually was**, and both failures were
silent. Runbook §0c now carries them:

- **A Windows WiFi scan lied about the AP.** `netsh wlan show networks` returned
  a partial cached list three times running with no `ShellyPlusPlugUK-*` in it,
  while the AP was broadcasting at **95% signal**. That reads exactly like a
  device not in AP mode, and it sent this session down a wrong path twice. Scan
  repeatedly before concluding anything about a radio.
- **The plug's own web UI wrote the static address into the *secondary* network
  slot.** Gen2 devices hold `sta` and `sta1`; the static configuration landed in
  `sta1` — `ssid: null`, `enable: false` — while the live `sta` stayed on DHCP.
  A device fully configured onto a slot that cannot connect, with no symptom but
  a blue LED. The phone app separately accepted WiFi settings twice and left the
  plug in AP mode both times.

Same shape as this register's oldest defect: **a write acknowledged, a readback
never taken.** Every one of the four required settings was therefore verified by
reading it back, and one of them mattered: `initial_state` **ships `off`**, and
`Switch.SetConfig` answers `restart_required: false` whether or not it took. The
ordering rule earned here is now written down — *set `initial_state` before
touching WiFi*, so a static change that strands the plug in AP mode still leaves
the board powered.

## 2. `LE-119` raised — the inbound channel is invisible on a dark canvas

A live capture of the running board carried `TOS64-MEAS/2` (fourteen metrics),
three `TOS64-QUAL/1` lines, `TOS64-RESULT/1` and `TOS64-DISPLAY/1`. It did **not**
carry `TOS64-RX/1`, because that row goes to the canvas and not to the wire —
`STORY-P1-09-16` named the debt and deferred it on the ground that an extra
transmit per beat would move the beacon cadence.

That deferral bit today. `TOS64-DISPLAY/1` read `native=1920x1080 fb=refused`,
so the canvas never painted, so **the inbound channel existed nowhere at all**.
The consequence is specific: `LE-118`'s deciding evidence is an `RX` reading on
boot 1 of the `12A` manifest, and it cannot be taken on any boot where the
firmware refuses a framebuffer — `LE-98`'s open condition, and not rare here.
The `TOS64-CMD/1` row added hours earlier by `STORY-P1-09-17` inherits the same
blindness.

**What is unaffected, and the distinction is worth keeping straight:**
`-17`'s answers and refusals ride the wire as text frames, so `M1` and `M2`
remain witnessable on a dark canvas. It is the channel's *state and counters*
that vanish, not the conversation.

The deferral's stated reason is now answerable: `-17` established a bounded
transmit slot, host-tested against a 10,000-command flood, so the rows can ride
the transcript cycle's own rotation at no cadence cost. Until they do, **a
dark-canvas boot cannot close `LE-118`, `-16` criterion 4, or `-17` criterion
4's refusal arms**, and any bench step whose evidence is an RX or CMD counter
must confirm `TOS64-DISPLAY/1 fb=` is not `refused` first.

## 3. The relay: a protocol for two agents, one baton

Written at the owner's ask and filed as [`agent/RELAY.md`](../../agent/RELAY.md),
binding alongside `CODING_STANDARDS.md` and `CONCURRENT_SESSIONS.md`.

`CONCURRENT_SESSIONS.md` closes by naming what it cannot fix — *"there is no
mechanism in this repository for that"*. The relay does not build that
mechanism; it **removes the need for one**, because only one agent ever writes.
The problem it actually solves is the one that ends every session here:
**context exhaustion**, and the fact that the handover carrying state forward is
written by the least capable version of the agent that did the work, at the
moment it has least room to be careful.

Six invariants, in the priority ordering: one baton and one writer; the tree is
never handed over red; **reset is gated on the receiver's ACCEPT, never on send**;
the handover is written continuously from the first tool call; prose proves
nothing that a command cannot re-derive; findings go to the register, not the
prose.

The cycle is **two context lifetimes per agent per turn** — a small, capped
AUDIT context that re-derives every claim, repairs the handover and accepts or
rejects; then a **reset**; then a large WORK context that reads only the repaired
document. That middle reset is the design's point rather than its overhead: it
*proves* the handover was sufficient, it maximises the budget that matters, and
it stops an auditor's reconstruction from being inherited as a plan.

Rejected alternatives are recorded with it, the sharpest being *sender resets on
send* — which destroys the only context that could answer the receiver's
questions, and reports a handoff complete on the strength of having sent it.
That is `LE-87` with knowledge in place of a relay state, and it is the single
thing the protocol most exists to prevent.

**One trap named explicitly:** the handover letter marks concurrency at a number
and nothing else (the owner's 2026-08-07 amendment). A relay has one writer, so
its handovers are sequential — `17A`, `18A`, `19A` — and an agent's identity
belongs in a `Held by:` header field, never in the filename. Using the letter as
an agent name is how `08G` happened.

## 4. Direction recorded, not yet designed

Two owner statements arrived this session and are recorded here so they are not
lost between contexts. Neither has been designed and neither should be started
without the decisions below being taken.

**The first goal**, in the owner's words: *get the OS to a point where the
planned Epics all the way to human use are complete, and there is evidence of
fitness for this OS to be a real-time OS.* Two separate gates — the interaction
ladder (`M1`–`M5`), and an RT fitness claim which under `ADR 0005` is quotable
only from a qualified platform. The qualified count is zero, so the second half
is downstream of the `Q3` campaign, not of more features. §1 is what makes that
campaign runnable at last.

**The filesystem**: based on the concept of a **Cluster** — the owner first said
"constellation" and corrected it in the same exchange — and **heavily derived
from ZFS**. Direction only: no Epic, Feature, Story, ADR or contract row exists,
and no code. Two questions decide its whole shape and are owed before anything
is written:

1. Does *Cluster* mean a **storage** grouping (ZFS's vdev/pool analogue) or a
   **machine** grouping (several boards sharing one namespace)? The word carries
   both, and this project has fleet ambitions that make the ambiguity real.
2. **Which ZFS properties are actually wanted?** End-to-end checksums and
   copy-on-write sit comfortably beside a determinism goal. The ARC, ZFS's memory
   appetite and an unbounded transaction-group flush do not — and an RT fitness
   claim and an unbounded write path cannot both hold. Deciding this is an ADR,
   and it comes before a Story.

## 5. Read next

The bench half of [`15A`](15A-the-first-conversation-the-workload.md) is now
cheap in a way it has never been: the boots are automatable. `LE-119` says which
of its steps still need a lit canvas, and `LE-118` says what to read before
calling a `NOBUFFER` a defect.
