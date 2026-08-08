# 20A — The board answered the question it was asked, and the ear was measured one arm at a time

The second bench session of 2026-08-08, executing [`19A`](19A-the-ear-is-deaf-on-arrival.md) §6
in its stated order. Solo in the tree; the owner was reachable and made the one
call this session could not make for itself, in §5.

**The one sentence, if only one survives:** *`M2` is done — `PING` and `STATUS`
were **answered**, not refused, and `STATUS` carried a fact only the board holds
back over the cable, which took exactly the one register bit and the one boot
`LE-122` predicted; `STORY-P1-09-16` criterion 4 then closed in full with all
five arms measured one at a time on silicon, each moving exactly its own counter
by exactly ten and the filter arm moving nothing at all; and the qualification
record's own void clause was read and **has not fired**.*

## 1. `LE-122`, closed at the MAC, exactly as the row said to

The row did the hard part yesterday by measuring the disagreement instead of
guessing at it. What was left was one bit, and the discipline about *which* bit:
`macb.h` gives `#define MACB_DRFCS_OFFSET 17 /* FCS remove */`, transcribed from
`rpi-6.12.y` rather than recalled, set in `enable_receive`'s existing `NCFGR`
read-modify-write beside the promiscuous bit it already clears, and pinned by two
host tests — one that a device reading back with the bit clear is written with it
set, one that the bit is `1 << 17` and lands clear of every field this path or
the transmit path touches.

**Fixed at the MAC and not in the glue**, which was the row's own reasoning and
is worth keeping said: this is where every other bound on this path lives — the
MAC enforces, software does not compensate. A later `-4` in software would have
inverted silently the day anything else set that bit, and every command would
then have refused as `undersize`. Same defect, opposite name, no test able to
tell.

`FRAME_CHECK_SEQUENCE_BYTES` is named in the module so the measured four has
somewhere to live. **Nothing subtracts it.**

## 2. `M2` — the board answered

```text
tos64>     sent   : verb=PING id=1 seq=1
    ANSWER : verb=PING seq=1
tos64>     sent   : verb=STATUS id=2 seq=2
    ANSWER : verb=STATUS seq=2 status=TOS64-RESULT/1 fixture=measure ok=true
tos64>     sent   : verb=WOBBLE id=0 seq=3
    REFUSED: unknown-verb seq=3
console: 0 exchange(s) went unanswered
```

`19A` proved the round trip with three refusals; this is the other half, and it
is a different claim. **`STATUS` carries a fact only the board holds** — its own
boot verdict, read out of the running kernel because a human asked for it. And
`WOBBLE` is now refused by the *verb table* rather than by the size check, which
is what makes the two answers above evidence that the deny-by-default table was
consulted at all rather than bypassed by a width.

The board's own row, one rotation later:

```text
TOS64-CMD/1 last=STATUS answered=2 refused=1 lastlen=46
```

**`lastlen=46`** against yesterday's `lastlen=50`. The row the defect was
measured on is the row that now reads right, and it moved by exactly four.

## 3. Criterion 4, five arms, ten frames each

From a counter observed settled across two consecutive rotations:

```text
settle        accepted=26 refused=67 dropped=93
ping x10      accepted=36 refused=67 dropped=103
unicast x10   accepted=46 refused=67 dropped=113
ethertype x10 accepted=46 refused=77 dropped=123
prefix x10    accepted=46 refused=87 dropped=133
notforus x10  accepted=46 refused=87 dropped=133
```

Every arm moved **exactly its own counter by exactly ten**, and the other one not
at all. `notforus` moved **nothing, including `dropped`** — the frame never
entered the ring. That is the arm that matters most: it is the only one testing
the containment argument rather than the classifier, and a moved `refused` there
would have meant the hardware address filter is not containing what
`STORY-P1-09-16` says it contains.

An earlier pass read `accepted +3` inside the `ethertype` window. It was
straggling `unicast` admissions crossing a rotation boundary, not a
misclassification — but an ambiguity has no place in a record, so the arms were
re-run from a settled counter and that is the run filed. Re-measuring was
cheaper than explaining.

Filed as [`REPORT-2026-08-08-01`](../../goals/reports/REPORT-2026-08-08-01.md)
with both raw captures.

## 4. `LE-117`'s tripwire fired for the first time, and the record survives it

The Pi OS card role booted on this bench yesterday, so the reading was owed
before more wire evidence was filed. It was taken:

```text
2026/05/26 16:01:25
version 086b83e3332dfc8927c56762771d082f3077a1ae (release)
```

**Byte-identical to `REPORT-2026-08-07-01`'s pin.** The void clause has not
fired and the qualification record stands.

**But the row's half (2) is now a measured fact rather than a worry**, and it is
an owner decision:

```text
[systemctl is-enabled rpi-eeprom-update]  enabled
[apt-mark showhold]                       (empty)
```

The EEPROM updater is **enabled with nothing held**, so the row's stated trigger
— an ordinary `apt upgrade` on the ground-truth card silently rewriting the
firmware the qualification record is pinned to — is live, today, on this bench.
The row is explicit that this is not an instruction to freeze firmware: updating
is allowed and sometimes right, it costs a new record, and the point is that the
cost gets *noticed*. Half (2) asks for a deliberate decision, recorded. **That
decision is the owner's and is not taken here.** Half (1), the tripwire itself,
is discharged for today and its reading is filed as
[`eeprom-tripwire-2026-08-08.txt`](../../goals/reports/eeprom-tripwire-2026-08-08.txt).

Order note, stated rather than hidden: the reading was taken *after*
`REPORT-2026-08-08-01` was filed, not before. It happens not to matter — that
Report measures mechanism, quotes no bound, and does not cite the qualification
record — but the runbook says *before*, and this session did it in the other
order.

## 5. My own error, and the owner's call

I booted the Pi OS card role to take §4's reading — correctly, that is the only
place `vcgencmd` lives — and then could not shut it down: `sudo` on that card
gates on a password, and `systemctl poweroff` over ssh is refused by polkit.
The runbook forbids cutting mains while the Pi OS role is booted, because a
write in flight is a corrupt card and this bench owns one Pi.

So the bench was stalled on exactly the class of thing `board-run` exists to
remove, and I asked rather than cut. **The owner shut the card down by hand**,
and the board was netbooted straight back into TOS64 and left listening.

**The standing lesson: an unattended tripwire that ends in an attended shutdown
is not unattended.** The owner agreed the fix the same session, and it is now
**written down rather than remembered**: runbook
[§6b](../../docs/pi5-board-session-runbook.md) — one sudoers drop-in,
`NOPASSWD: /usr/bin/systemctl poweroff`, in the same least-authority shape as
§6's existing `tos64-probe` rule, cross-referenced from §0b item 2 where the
tripwire is actually read.

Two details in it are load-bearing rather than decorative. The verb is written
into the rule (`systemctl poweroff`, not bare `systemctl`) because the bare
binary would grant `systemctl start` and with it anything the card can be made
to run as root; and `/usr/bin/systemctl poweroff` is preferred over
`/sbin/poweroff`, which is a symlink to `systemctl` dispatching on `argv[0]` —
pinning the real binary and its verb makes the rule checkable by reading it.
What it grants is a denial of service to a key-holder who could already read
every register the probe exposes, which is a fair trade on a card behind a
direct cable and would not be on anything networked.

**The line itself is not yet installed**, because installing a sudoers drop-in
needs the root it exists to grant — it is one paste on the next Pi OS boot, and
it is §7 item 2.

## 6. `LE-121`'s remaining half: an unpinned clock is now a named refusal

The row's ask was precise — the campaign's verdict must gain a refusal for an
unpinned clock, derivable from the rate calibration's own per-window samples,
beside `no_windows`, `window_never_closed` and `pmu_dead`. It exists:
`QualRefusal::ClockUnstable`, and it fires **before any distribution is built**,
so there is no `CampaignSummary` for a future session to quote.

**The design decision worth reading, because the obvious implementation is
wrong.** The first attempt tested the rate samples' *spread* against the
arithmetic's own quantisation — clean, derivable, and it destroys the
instrument: at the campaign's real parameters that bound is two units in 44,444,
so any excursion beyond about 24 ticks would refuse, and the two existing tests
that pin *"an excursion window surfaces in the max"* went red. **A refusal that
swallows the one thing Q3 exists to report is worse than no refusal.**

The discriminator has to be the shape the row itself named — a second *mode*,
not a spread:

- Residency arrives as an **outlier tail**: a few windows, of varying depth.
- Frequency scaling arrives as a **population**: many windows, tightly grouped
  at one other rate.

So the refusal fires on **how many** windows sit below the calibrated rate, never
on how far the worst one did. Two conditions, neither a bench-tuned number:
more than 1% of windows deviate — tied to `unaccounted_p99`, the number the
instrument itself reports, because past 1% the quoted p99 has stopped describing
a tail and started describing the other clock; and at least two windows deviate,
because one window is not a mode however deep it is.

The refusal carries its own evidence onto the wire rather than just naming
itself:

```text
TOS64-QUAL/1 campaign REFUSED reason=clock_unstable rate_p50=44444 rate_min=27777 deviating_windows=40 windows=200
```

Host-green, including a test that reconstructs the 2026-08-07 campaign's exact
shape and asserts it now refuses. **It has not yet run on silicon** — see §7
item 1, and that is the honest state of this row.

## 7. What the next session does, in order

1. **Run `qual-campaign` and `qual-control` on the board once.** §6's refusal is
   host-green and unproven on silicon, and the property that matters is the
   *negative* one: on this pinned bench it must **not** fire. A refusal that
   false-fires on good data is a worse instrument than none, and one boot each
   settles it. Only then is `LE-121` closable.
2. **Install runbook §6b's line on the next Pi OS boot** — one paste, and it is
   what makes every future `LE-117` reading unattended:

   ```sh
   echo 'revanur ALL=(root) NOPASSWD: /usr/bin/systemctl poweroff' | \
     sudo tee /etc/sudoers.d/011-tos64-poweroff
   sudo visudo -c    # do not leave the session until this prints "parsed OK"
   ```

   Then **`LE-117` half (2)**: the owner's recorded decision on whether the
   ground-truth card may auto-update the EEPROM, now that §4 shows it can.
3. **`STORY-P1-09-17`'s remaining criteria** and the Story's assurance state —
   its functional criteria are now met on hardware, and nothing about its
   mapped performance, security and containment evidence has moved.
4. **The measurement sweep**, still the largest unclaimed leverage in the tree —
   `08A`'s 14 distinct measurements covering 125 gates, unblocked in principle
   since `18A` and untouched since.

## 8. Standing instruction earned

**A refusal must be tested against the data it is meant to accept, not only
against the data it is meant to reject.** §6's first implementation caught the
confound perfectly and would have made the instrument mute; the existing tests
caught it, and they only caught it because someone had previously written down
*"an excursion window surfaces in the max"* as a property rather than as a
scenario. The general shape: every guard added to an instrument narrows what it
can say, and the narrowing is the part nobody measures.
