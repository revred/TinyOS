# 04C — The Board Can Listen, and Three Instruments Stopped Lying

Session handover, written 2026-08-06. Follows
[`03B`](03B-the-arms-are-built-the-board-booted-them-and-nobody-read-the-wire.md)
and takes letter **`C`**: `03B` was written by session `B` in this same tree and
its §5 is the entire brief for this one.

**`03B` §5 named the bottleneck — *the board cannot be told anything* — and Step 1
of its ordered path is built, gated and unwitnessed.** The one sentence to carry
forward is not that receive works. It is this: **the containment argument for an
inbound path on a machine with no IOMMU is that nothing interprets the bytes, and
that argument expires the moment a received frame is allowed to mean something.**
§2 is about that, and Step 2 is where it falls due.

The other half of the session paid down the bench tax. **`LE-90`, `LE-92` and
`LE-93` are all closed**, and `LE-92`'s gate was proven by reintroducing the exact
defect that raised it and watching three existing gates stay green.

---

## 1. `STORY-P1-09-16` — GEM receive, one frame, fail-closed

The board now arms an inbound path. Not a stack, not a session, not a command: a
ring of **one** wrapped descriptor into a **second** pinned region, behind a
hardware address filter and a MAC-enforced size bound, with every frame
classified by a total function and *counted* on the canvas.

It lives in a new [`gem_receive.rs`](../../os/src/hal-arm64/src/gem_receive.rs)
**beside** `gem.rs` rather than inside it, and that is a design decision rather
than tidiness. `gem.rs`'s scripted double asserts on every test in it that the
receive-enable bit is never set — `TEST-P1-09-03-A` clause 4's contracted
absence. Keeping the modules apart means **that assertion is still true and still
enforced**: the transmit path does not enable receive, and the claim stayed
checkable instead of being quietly widened into a lie. `gem.rs`'s module doc now
scopes its two absence claims to itself and points one module over.

### The four things that are the containment, each a test

1. **Enable order pinned, `NCR.RE` strictly last.** Filter bottom, filter top,
   `NCFGR`, `DMACFG` (64-bit addressing *and* the size bound), queue base low,
   queue base high, stale status cleared, **then** enable. `RE` before `RBQP`
   hands the MAC whatever that register held at reset and lets it write there;
   `RE` before the size field lets it write past the end of a region that is
   correctly addressed. Both are single-write mistakes with **no symptom** on a
   bench where the register happens to read zero, which is why it is an ordered
   assertion and not a comment.
2. **A separate pinned region.** Receive does not share `BEACON_MEMORY`. The two
   directions have opposite writers, and aliasing them would let a confused
   inbound write corrupt the frame the board is about to transmit — **turning an
   inbound fault into an outbound lie**, when every piece of evidence this
   project owns is an outbound frame.
3. **Bounds the device does not get to choose.** The `RXBS` encoding is refused
   rather than rounded (a rounded-up bound is a grant the argument did not
   make); a misaligned buffer address is refused because the low two bits *are*
   the ownership and wrap flags; and the descriptor's reported length is
   bounds-checked against the region **anyway**, even though the size field
   should make that unreachable.
4. **Nothing interprets the bytes.** `admit` compares a destination, an
   EtherType and six payload bytes. No value taken from a frame selects a
   branch, an address, an offset or a size anywhere in the image.

### `LE-67` re-argued, and two other documents that had borrowed its sentence

`03B` §5 required the containment note be *re-argued rather than inherited*. The
honest core of the replacement, after reading `SECURITY_CHARTER.md` in full:

**A malicious device already had bus-master DMA with no IOMMU, so receive does
not widen what a compromised device can reach — arbitrary has no wider setting.**
What receive newly admits is a **remote peer as an input source**, the first in
this project's history, and the four points above are the containment for *that*
because device isolation does not exist on this path and cannot be claimed.

Point 4 is the load-bearing one. `C1` gained an input path and gained **no
parser**, which is `BND-03` satisfied by absence, and it is why a no-IOMMU
receive path can be honest: there is no reachable behaviour for a crafted frame
to reach.

Two other documents carried `LE-67`'s borrowed sentence and would have gone
quietly stale — **`FEAT-P1-10`'s contract row** (*"one pinned bounded transmit
grant with receive left disabled"*, which was a claim about the *board*, now
scoped to that egress path) and its **hostile-inputs declaration** (*"None on the
board today because no receive path exists"*). `FEAT-P1-10` also *predicted* that
reversing the absence would need adversarial tests, a bounded command
vocabulary, authenticity and a replacement containment argument. It got three of
four and **owed the fourth nothing**: there are no commands, so there is no
vocabulary to bound. Step 2 is where that prediction holds in full.

### Verification, stated plainly

22 host tests in the new module, 2 for the canvas row, 324 in the crate. **Seven
mutations applied and each seen to fail on the named test** per `ADR 0005`:
enable-before-queue-base, rounded size bound, dropped alignment check, believed
over-length, preserved promiscuous bit, swallowed overrun, skipped prefix check.

Two things to be straight about. **The Rust tests and implementation were written
in one pass**, with that falsification sweep as the substantive check rather than
a literal red-then-green sequence; `TEST-P1-09-16-A` was written first. And
**`check-boot-images` caught a real defect host clippy could not see** —
`dangerous_implicit_autorefs` on the mutable static, a lint that fires only for
the board target. `LE-72`'s gate doing exactly its job, one session after
`LE-92` proved the same gate had a mirror hole.

## 2. The needle, restated for whoever takes Step 2

`03B` §5's path was: receive one frame → answer one command → the two costed
board items. Step 1 is done to the extent a laptop can do it.

**Step 2 is not more of the same work, and the argument in §1 is why.** Every
sentence justifying a no-IOMMU inbound path rests on the frame *meaning nothing*.
The moment `report your rungs` is a request the board acts on, a value from a
remote peer selects a branch, and points 1–3 alone do not carry that. The
`SECURITY_CHARTER.md` read has to happen **again**, not be cited, and
`FEAT-P1-10`'s prediction — bounded vocabulary, authenticity, adversarial tests —
comes due in full.

That is not an argument for delay. It is the reason Step 2 is a Story of its own
rather than a follow-on commit to this one.

## 3. `LE-93`, raised and closed the same day

`STORY-P1-09-16` criterion 4 needs the board to count a frame a host sent, and
**nothing on this bench could send one**: `ti64dink` captures and does not
transmit, and the other five tools are card, image, link, serial and SD
utilities. The first inbound path in the project's history was host-Green and
*unwitnessable* — the instrument missing, not the board, for the sixth time.

`ti64dink --send <arm>` now exists: a fifth P/Invoke (`pcap_sendpacket`,
unelevated) and a table of five arms, each stating what the canvas `TOS64-RX/1`
row must do.

| arm | sends | the canvas must |
|---|---|---|
| `ping` | broadcast, `0x88B5`, `TOS64-` | `accepted` +1 |
| `unicast` | to the board's own MAC | `accepted` +1 — the filter admits *us*, not only broadcast |
| `ethertype` | `0x0800` instead | `refused` +1 |
| `prefix` | `TINYOS-` instead | `refused` +1 |
| `notforus` | to `02:aa:…`, otherwise valid | **neither counter moves** |

**`notforus` is the arm worth reading twice.** It expects nothing to happen,
because the GEM's hardware address filter should drop the frame before DMA — so a
moved `refused` count is not a smaller success, it is the filter failing to
contain what §1's argument assigns it.

**Two of the board's refusals are recorded as unreachable rather than omitted.**
`TooShort` cannot be put on a wire at all: the NIC pads every frame to 60 octets
below any software this tool can reach, so it is host-test-only, and that is a
property of Ethernet rather than a gap in the tool. The three *descriptor*
refusals (fragment, zero length, over length) are statements about what the GEM
writes into the ring; only a lying device reaches them.

### The part that makes criterion 4 predictable rather than discovered

The arm frames are emitted to
[`goals/reports/rx-arms-2026-08-06.txt`](../../goals/reports/rx-arms-2026-08-06.txt)
by `ti64dink --send-frames`, and **a Rust host test asserts `gem_receive::admit`
returns exactly the verdict each arm predicts.** The `LE-80` mirror shape, and
the same trick the captured-beacon test uses in the outbound direction: without
it the sender's expectations live in its prose and the filter lives in Rust, and
the two can drift with no symptom until an operator is standing over a powered
board wondering which is wrong. Falsified in both directions — an arm predicting
`Accepted` where the filter refuses, and one predicting a refusal it does not
make, each turn the test red.

## 4. `LE-90` closed, and the audit's own rule

The five tools that lacked it now set `AutoFlush` on stdout and stderr as their
first statements: `cardswap`, `imgwrite`, `linkwatch`, `sdprep`, `serialwatch`,
plus `ti64dink`, whose `--live` capture runs for minutes and was the most exposed
of the lot.

**Applied uniformly, including to the two short-lived tools that are not bitten
today**, and the reason is the family `03B` §3a named: whether a program runs
long enough for this to matter is a property of its loops, and loops get added. A
fix conditional on that is *a fact recorded beside the thing that determines it
rather than derived from it* — `LE-89`, `LE-91` and the stale capture-window
comment, a fourth time.

**Verified in the field, per this defect's own lesson.** `tos64-linkwatch`
redirected to a file showed **364 bytes and three lines while the process was
demonstrably still running**. All seven tools build; `netboot.tests` still 23/23.

## 5. `LE-92` closed, and the gate was proven against the defect that raised it

`xtask check-guest-images` compiles every registered x86_64 Tier 0 fixture
binary — **22 distinct artifacts, compilation only, no QEMU.** QEMU is what makes
these fixtures CI-side; the *compile* is the half no local gate performed, and a
gate that needed QEMU is a gate nobody runs, which is how the hole stayed open.

The plan is a **pure function of the `FIXTURES` register** `list-fixtures`
already prints, with a host test asserting every registered fixture is in it — so
a fixture added tomorrow is compiled by this gate or fails a test today on a
laptop. Same shape as `boot_images.rs`, for the same reason: the defect is a
*coverage* failure and coverage is a property of the list.

**And it was proven rather than asserted.** `fb3f36c`'s literal was reintroduced
verbatim — a ten-element `[None, …]` beside `METRICS = 11`, a second declaration
of a count the type already carries:

| gate | verdict |
|---|---|
| `cargo test -p kernel` | **green** — blind to it |
| `check-lints` | **green** — blind to it |
| `check-boot-images` | **green** — blind to it |
| `check-guest-images` | **RED**, `E0308` |

That table is the whole content of `LE-92`. `agent.md` and `CLAUDE.md` now say
the two gates are siblings and that **running one is not running the other**.

## 6. State at close

- **Gates:** `check-boot-images` **green** (required — `kernel` and `hal-arm64`
  both changed), `check-guest-images` **green** (new, 22 artifacts),
  `check-assurance-spine`, `check-spine-files`, `check-lints`, `check-citations`,
  `check-crate-sizes`, `check-image-size`, `cargo fmt --all --check` all green.
  Full workspace suite green. All seven C# tools build; `netboot.tests` 23/23.
  **`check-timing-regression` RED, unchanged** — `03B` §3b's three baseline-less
  metrics, untouched here and still gated on the owner's decision.
- **Spine:** 31 Features / **99** Stories / **82** Tests / 62 Reports,
  **93 loose ends (45 open)**, **23 of 460 release gates carrying evidence** —
  unchanged, because this session measured nothing either. `assurance-status`
  unchanged: **197** blocked by neither qualification nor the board, **127**
  measurable today, **70** needing a mechanism. 5 platforms, **0 qualified**;
  **0/99** Stories assurance-verified.
- **Uncommitted.** Nothing committed, `git add -A` never used
  (`CONCURRENT_SESSIONS` rule 1). `03B`'s five files are still uncommitted in
  this tree alongside this session's, so **stage by path**.
- **Bench:** no `tos64-netboot` running, UDP 67/69 clear. The staged image is
  still `b6dbabae…` (298,089 bytes) and `03B` §6 item 1 is still one power cycle
  away, untouched by this session.

## 7. The next session, in order

1. **One board session now closes three things at once**, and that is new — it
   was two separate trips before this session. Start the server first, verify the
   digest, then:
   - `03B` §6 item 1: capture **60 s**, `parse-meas`, expect `metrics=14`, file
     `PERF-D04-G23` and `PERF-D05-G23` with `D04`'s residue caveat on the row.
   - `STORY-P1-09-16` criterion 4: five `ti64dink --send` commands, reading the
     canvas `TOS64-RX/1` row against each arm's stated expectation. **Both arms
     required** — an accepted count alone proves only that the board can hear.
   - `STORY-P1-06-02` criterion 4 and `LE-82` if the boot is already spent.
2. **Put `03B` §3b to the owner**, unchanged and still blocking: is a Tier 0
   baseline recorded off the CI runner one this project wants committed, and
   which of `min_cycles`/`p50_cycles` versus the ratios is a reader entitled to
   trust? Then regenerate **once**, with `--date=`, for all three metrics.
3. **The qualification ceiling, as a decision rather than a status line.**
   `0/99` assurance-verified is not a backlog — `verified` requires a qualified
   platform, `qualified-platforms.tsv` holds five with **zero** qualified, and
   the Pi 5's firmware column reads `unknown`. No Story work moves that number.
   Either an `ADR 0005` campaign runs, or the project stops presenting one
   blocked prerequisite as ninety-nine pending tasks.
4. **Step 2 — one command end to end**, as its own Story, with the charter read
   again rather than cited (§2).
5. **`LE-91`**, unchanged and still the right mechanism before the 127. A gate
   filed through an unchecked labelling path is 127 chances to be wrong.

**Do not start:** `FEAT-P1-05`'s RT reserve, `G09`/`LE-86`, `06A` §4.3 — all
unchanged from `03B`, all still correctly sized as owner decisions or
Feature-sized work.

**One register observation for whoever does §7 item 3.** **40 of the 48 open
loose ends were `unowned`** at the start of this session. A defect register where
five in six live items have no owner is recording rather than driving, and that
is `LE-65`'s still-open half wearing a different hat. It costs a session of
*assignment*, not of work.

**And the standing instructions, all still holding:** do not report `x/460`
undecomposed — there is a subcommand. `PERF-Dnn-Gnn` is only meaningful if `Dnn`
is the domain of the thing you measured. Verify the digest and size the window
before you spend the boot. **This session adds a fourth: a gate written for one
architecture, one tool, or one direction does not generalise itself** — `LE-92`
was `LE-72` mirrored, `LE-90` was one tool's defect latent in six, and `LE-67`'s
sentence had been borrowed by two documents that were not looking.
