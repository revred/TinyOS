# 19A — The ear was deaf on arrival, the fix works, and the board answered

The first bench session of 2026-08-08, executing what `18A` §5 left and
`15A` §V had been waiting on since it was written. Solo in the tree; the
owner was present at the bench and made the one decision this session could
not take for itself.

**The one sentence, if only one survives:** *TinyOS answered a human — three
commands typed at `ti64dink --console`, three distinct sequence-matched
refusals spoken back over the cable — which proves `M1`'s round trip on
silicon; the ear that had been deaf on arrival for its whole existence is
alive and counting; and the single remaining gap to `M2` is a measured fact
rather than a mystery: the board receives exactly four octets more than the
host sends, because the descriptor's frame length includes the FCS.*

## 1. The ear was deaf before anything was sent, and `LE-119` is why we know

`17A` raised `LE-119` — the inbound channel reported only to a canvas this
bench's firmware refuses — and said, correctly, that a dark-canvas boot
therefore could not close `LE-118`. This session put both live rows on the
wire (slots in the transcript rotation, so the beat still carries one text
frame and the cadence a capture window is sized against does not move), and
**the very first capture that could carry the row carried the answer**:

```text
TOS64-DISPLAY/1 native=1920x1080 fb=refused fb_addr=0x000000003f800000 src=constant
TOS64-RX/1 state=stopped reason=nobuffer accepted=0 refused=0
TOS64-CMD/1 last=none answered=0 refused=0
```

A freshly netbooted, digest-verified board, nothing sent to it, already
stopped. Three `ping` frames then moved neither counter. **The ear was not
deafened by our traffic; it was deaf before it.** That is `LE-118`'s
deciding evidence, taken on a boot that could not have produced it the day
before — and `STORY-P1-13-01` criterion 4a rode along in the same line.

## 2. The owner's decision, and the mechanism behind it

`read_status` checked `OVR`, then `BNA`, and **returned before it ever
looked at `REC`**. So `REC|BNA` — one frame safely in the ring plus a second
the MAC had nowhere to put — discarded the good frame and disabled receive
for the boot. And `BNA` is precisely the signature of two frames in one
beat, which on a segment carrying ordinary Windows broadcast is the median.

The owner ruled: **`BNA` becomes a counted drop; `OVR` stays terminal.** The
two bits are different failures wearing one word. `OVR` is the MAC's FIFO
overflowing — a torn frame, broken accounting, and deaf is the right safe
state. `BNA` is backpressure about a frame that never entered the ring, so
it says nothing about the frame that did.

`ReceiveError::BufferUnavailable` is **removed rather than left
unreachable**: a terminal error that cannot fire is a taxonomy that lies,
and the canvas would have kept advertising a `stopped reason=nobuffer` state
the board can no longer enter. The drop is counted and spoken —
`TOS64-RX/1` gains `dropped=N` — because on a one-descriptor ring polled
once per beat that count *is* the measure of how contended the slot is.

**It worked on the first boot that carried it:**

```text
TOS64-RX/1 state=listening accepted=0 refused=19 dropped=19
```

`listening`, not stopped. Nineteen frames classified and declined, nineteen
more dropped — the contention now a number instead of a fatality. Then five
pings: `accepted=5`. **`STORY-P1-09-16` criterion 4's accept arm is met on
silicon.**

## 3. The first conversation

`ti64dink --console`, three lines typed, on a running board:

```text
tos64> PING     sent: verb=PING id=1 seq=1     REFUSED: oversize seq=1
tos64> STATUS   sent: verb=STATUS id=2 seq=2   REFUSED: oversize seq=2
tos64> WOBBLE   sent: verb=WOBBLE id=0 seq=3   REFUSED: oversize seq=3
console: 0 exchange(s) went unanswered
```

**Every command was answered.** Not accepted — answered: a distinct spoken
refusal, named, matched to the sequence it belonged to, back over the cable
within the timeout. That is `M1`'s round trip, and it is the first time in
this project's history that TinyOS has done something because a human asked.
`STORY-P1-09-17` criterion 4's *refusal* arms close on this capture.

## 4. `LE-122`: the disagreement, measured rather than guessed

Every command refused as `oversize`, and the host builds exactly
`14 + 46 = 60` octets — the Ethernet minimum, chosen so no NIC padding can
exist. A fixed width is the one thing that should make this impossible, so
the board was made to state its own number rather than have one inferred.

The first reading was `lastlen=47`, and **47 was the copy-out cap, not the
width** (`ADMITTED_CAPACITY` was `COMMAND_PAYLOAD_BYTES + 1`, saturating one
past the limit). A refusal that cannot say how far over a frame was is a
refusal that names nothing — this project's own defect class — so the buffer
was widened to `+16` and the board answered:

```text
TOS64-CMD/1 last=none answered=0 refused=1 lastlen=50
```

**Fifty against forty-six: a delta of exactly four, the frame check
sequence.** The GEM descriptor's length includes the CRC.

The row recommends fixing it **at the MAC** rather than subtracting four in
the glue: a later FCS-strip would silently invert a software subtraction and
every command would then refuse as `undersize` instead. The `NCFGR` bit is
deliberately *not* written from memory — this repository transcribes
register bits from `macb.h` and pins them by test, and the session that
found the defect is exactly the wrong one to relax that.

## 5. Also landed, and one inherited red cleared

- **`LE-120`'s general half.** `text_frame` now refuses over-long lines and
  `TEXT_FRAME_CAPACITY` is *derived* from `transcript::MAX_LINE_BYTES`
  rather than a bare `192` beside a recorder that accepts 256. Writing the
  producer test found what the row did not know: a `METRIC` row needed only
  seven-digit latency tails to overflow the old bound, so every
  fault-latency outlier was one bad tail from the campaign line's fate. An
  over-long line now rides as a self-describing `TOS64-TRUNC/1` refusal —
  never truncated, never silently dropped.
- **`main` was red and it was not a defect.** An xtask test asserted
  `qualified_count() == 0`; when `18A` qualified the first platform it went
  red *describing the project's biggest result as a violation*. A population
  count in a test literal, which is `08A` §1's rule exactly. It now asserts
  the invariant `ADR 0005` actually protects: every qualified row cites a
  Report that exists.
- **My own error, named:** a bare `tos64-power cycle` boots whatever the SD
  card holds, because `board-run`'s server exits with it. `board-run` orders
  `StartNetboot` before `PowerCycle` precisely to prevent this; I stepped
  outside it and spent two captures diagnosing a Pi OS login as a silent
  board. **A power cycle without a server is a boot into the card role.**

## 6. What the next session does, in order

1. **`LE-122`** — one register bit, transcribed and pinned, then one boot.
   The same console `PING` must move `TOS64-CMD/1` to `last=PING
   answered=1` with `lastlen=46`. That is `M2`, and everything else in the
   interaction chain is already built and proven.
2. **`-16` criterion 4's remaining arms** — `unicast`, `ethertype`,
   `prefix`, `notforus`. The accept arm is done; these are four `--send`
   commands and one capture.
3. **`LE-117`'s tripwire** is owed: the Pi OS card role booted on this bench
   today, so the bootloader hash needs reading against
   `REPORT-2026-08-07-01`'s pin before further wire evidence is filed.
4. **`LE-121`'s remaining half** — the clock-instability refusal, untouched
   by this session.

## 7. Standing instruction earned

**A cap that saturates measures nothing beyond itself.** `ADMITTED_CAPACITY`
was sized to make a refusal *reachable* and, in doing so, made its width
*unknowable* — the board could say "too wide" and never "by four". The same
shape as `LE-120`'s truncation and `LE-115`'s missing timestamp: an
instrument that is bounded exactly at the question stops being able to
answer it. Leave headroom wherever a refusal is expected to name a number.
