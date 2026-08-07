# 11A — Sitting A executed: the instruments gated, the decision taken against the texts, and the verb Story written up to the sentence it waits on

The session that executed [`10A`](10A-the-first-conversation-from-counted-frames-to-an-answered-command.md)
§4's **Sitting A** — laptop, no board, no owner. Solo session; no concurrent
commits arrived mid-session. Letter `A` per the corrected naming rule.

**The one sentence, if only one survives:** *Every Sitting A deliverable is on
`main` — the bench instruments now gate a blocking CI job that caught a real
cross-platform containment defect on its very first runner execution, ti64dink
gained its first test project plus the arrival timestamps and Rust-source
parity that close `LE-115` and `LE-80`, `STORY-P1-13-01`'s containment
decision is recorded as shape 2 with the walker built outside every image, and
the admitted-verb Story is written whole with its charter read again — stopped,
by design, exactly at the owner's S4 sentence.*

## 1. `LE-114` closed, and the gate earned its keep in its first hour

`xtask check-tool-tests` discovers every `work/tools/*.tests` project
(discovery derived from the directory; refuses to match nothing; refuses a
`*.tests` directory with no csproj), runs as its own blocking `tool-tests` job
beside `host-tests`, and sits in `CI_ENFORCED` so `check-ci-gates` refuses a
workflow that drops it. Landed alone, as the row instructed — and both its
recorded cautions fired exactly as written:

- the runner needed `setup-dotnet` (not a one-line addition), and
- **the first runner execution went red on a real finding** (run
  `31186768795`): `TftpPaths.Resolve` refused `C:\Windows\System32\config\SAM`
  on Windows — where a drive prefix is *rooted* and lands outside the served
  root — and **resolved it inside the root on Linux**, where `C:` is just an
  odd directory name. A containment guard whose answer depends on the host OS,
  in the one function `LE-88` deliberately made one decision. The fix refuses
  the colon itself (drive root or NTFS alternate stream; nothing legitimate in
  TFTP on any host) *before* host path semantics see the name; the theory
  gained the forward-slash drive spelling and the ADS spelling. Run
  `31187664632` is green with three suites: netboot 56, power 99, ti64dink 32.

Landing alone is why that signal was attributable in one read. This is `LE-64`'s
family caught by a mechanism on its first day rather than by a bench session.

**Bench observation worth recording:** a live `tos64-netboot` (PID 3172 at the
time) holds its exe lock on this bench — presumably serving the board. The gate
builds into an isolated `BaseOutputPath` so it can run without touching a live
server; but per `LE-87`, a *stale* server silently winning the UDP-69 bind was
the project's second most expensive instrument defect. Whoever next sits at the
bench should confirm that server is the one they mean to be running.

## 2. `LE-115` and `LE-80` closed: the operator's terminal is testable, timed, and parity-guarded

`ti64dink.tests` exists — the tool with the worst instrument-defect history now
has 32 tests riding the same blocking job. Three things in the same seam:

- **Arrival timestamps (`LE-115`).** Live frames print with a monotonic
  Stopwatch stamp from capture start, printed *inside* the capture loop so a
  tailed log shows the board live. Captured beacons yield a beat-cadence
  summary whose refusals are output, not advice: the **mean over the span** is
  the only number offered; per-frame jitter is named as the host's in the
  summary's own NOTE line; a backwards seq is a reboot and no rate is computed
  across boots; a frozen seq is a refusal, not a division. The anchor error
  collapses from ±one beat interval to host jitter: the listen-only cadence
  measurement `08A` §6 queued now costs one capture (~3 min resolves the beat
  to better than 0.1 %). The measurement itself is deliberately *not* filed
  here — it belongs to whoever takes the capture, against the park-loop
  timebase and nothing else (`LE-104`'s rule).
- **The Rust-source parity test (`LE-80`'s own stated close).**
  `RungParityTests` reads `os/src/kernel/src/spoor_stream.rs` — comment lines
  stripped first, so prose cannot satisfy the scan — parses `enum Rung` and
  `fn taxonomy`, and holds `Program.Rungs` against them in both directions,
  `(category, action)` pairs included. A rung added to the kernel with no host
  row now fails a blocking job instead of surfacing as a 300-second watch
  timing out over a stream full of the event.
- **The console/`--send` paths under test** — decode bounds (the count field
  refused before it sizes a read), harvest shape rules, envelope
  rotation-plus-verdict assembly, all four watch conditions including the
  target-masquerade guard, and the send arms' exact one-field-wrong property,
  which is what makes them refusal arms at all.

## 3. `STORY-P1-13-01`: the decision is shape 2, argued against the quoted texts — and the walker exists, outside every image

Criteria 1–3 closed. The decision (recorded in full in the Story):

- **Shape 1 declined** because its required argument — that a capped FDT walk
  in the boot image "is not the complex hostile parser the matrix forbids" —
  cannot be written without re-reading one of three machine-checked texts: the
  C1 input rule (*"parse no … variable-length device format"* — the FDT is one,
  verbatim), `PD-12` (*"device-format parsers outside C1"*), and `BND-03`'s
  success criterion (*"linked executable bytes equal zero"* — a scan, not a
  judgement; no discipline inside the image makes the count zero). The
  Feature's own description concedes the format is complex and hostile. The
  strongest framing (the walk as `C0→C1` handoff verification) was attempted
  and declined in writing so the next session doesn't re-derive it.
- **Shape 2's costs are stated, not softened:** the constant keeps today's
  justification under `LE-117`'s tripwire, and `src=dtb` waits for a C4 host
  for the parse — the nearest being **off-board**: boot quarantines the DTB
  (bounded copy under a fixed-format header read, the `hdmi.rs` class) and
  ships it as data; the host walks it with the same crate. The hostile parse
  never enters the image at all.
- **The walker is built** — `os/src/fdt-walk`, pure `no_std`
  `forbid(unsafe_code)`, 38 tests red first (36 failing under a `todo!` body,
  then green with none weakened), 21 named refusals, caps pinned by test
  (totalsize 256 KiB, depth 16, tokens 32,768 — the token cap chosen so its
  refusal is *reachable inside* the size cap, because an untestable cap is a
  comment). Non-empty `ranges` is a named refusal, never a guess: **the
  board's own blob decides the second increment.** And the decision's central
  property is a test: no image crate's manifest names `fdt-walk`.

The two decisions of this session draw the same line from both sides:
fixed-offset total functions are admissible in C1 (gem admission, the mailbox
descriptor, tomorrow's `TOS64-CMD/1`); input-steered variable-length walks
never are (the FDT). That coherence is worth more than either decision alone.

**What remains on this Story:** criterion 4a (the `TOS64-DISPLAY/1`
`fb_addr=… src=constant|refused` field — small board change, sanctioned by the
Feature's sprint lift), the quarantine-copy-and-transmit increment (needs a
bounded chunking envelope — real design, next session), and criterion 5 on the
board.

## 4. The admitted-verb Story is written — `STORY-P1-09-17` — and stops exactly where it must

`10A` §3 S2's proposal is now a filed Story with its contract row, its
`TEST-P1-09-17-A` suite specification, and the charter **read again, not
cited**: `BND-03` and the matrix row answered with the fixed-width line above;
`PD-02` read honestly — *the wire peer has no identity*, so the verb table may
hold only answer-only rows disclosing what the board already broadcasts, which
is why `PING` and `STATUS` are the whole first table and a third row is a
charter re-read, not an addition; `PD-14` — the table *is* the policy; plus
one obligation `10A` did not list: **a beat-bounded answer rate**, because an
unauthenticated broadcast peer must not be able to use the board as an
amplifier. The suite (wrong magic, unknown verb, undersize, oversize,
flood/over-rate, each a distinct *spoken* refusal) is specified clause by
clause; it is deliberately not committed as code — its subject may not exist
before S4, and a test that cannot compile gates nothing. The building session
writes it red, verbatim.

**`S4` stands untouched:** the sprint rule is the owner's sentence and nobody
else's. Until it is spoken, the board keeps refusing meaning — the `-16`
absence argument remains in force and this session did not erode it by a byte.

## 5. State of the 10A showstoppers after this session

- **S1 (receive re-arm):** untouched — it is `-16`'s owner's disposition to
  write (`08A` §7), and the highest-leverage board code in the project.
- **S2 (the admitted verb):** written to the S4 boundary. §4 above.
- **S3 (the untested terminal):** discharged — `LE-114`, `LE-115`, `LE-80` all
  closed; register at **117 loose ends (53 open)**.
- **S4 (the sentence):** the owner's, verbatim from `10A`: *"Sprint rule
  lifted for the interaction chain (`-16`'s re-arm, the admitted verb,
  Ti64Dink console) and nothing else."*

**Sitting B is unchanged and now cheaper:** every power cycle still carries
qualification and interaction payloads (`-16` criterion 4's five arms, the
re-arm's first sustained listen, the Q3 campaign, `LE-117`'s preamble
tripwire) — and the passive half of the boot now also yields the beat-cadence
number for free, because the instrument finally carries timestamps.

## 6. Housekeeping

Commits this session, each gated by the pre-commit hook and pushed with the
run watched: `80b66f1` (LE-114, alone), `11d980b` (the colon fix the runner
demanded), `4218922` (ti64dink.tests + LE-115 + parity), the register close
(three rows, dashboard and feasibility advanced together), the
`STORY-P1-13-01` decision + `fdt-walk` crate, and `STORY-P1-09-17`. Spine
green at **33 Features / 102 Stories / 84 Tests / 63 Reports, 117 loose ends
(53 open)**, feasibility agreeing, three C# suites (56 + 99 + 32) green
locally and on the runner. No `kernel`/`hal-arm64`/`pi5-image`/`exec`/`shell`
sources were touched, so `check-boot-images`/`check-guest-images` were not
owed by this session's changes; `fdt-walk` is host-tested and linked into
nothing, and the suite proves it.

## 7. Standing instructions, one addition

All previous hold. The one this session earned: **a guard that delegates to
host semantics is a different guard on every host** — the TFTP escape refusal
flipped its answer between Windows and Linux because it let `Path.GetFullPath`
decide what a hostile name meant. Refuse the hostile *property of the bytes*
(the colon, the fixed offset, the cap) before any platform layer interprets
them; the platform-semantics tests `LE-66` mandates are where such guards get
caught, and the first runner execution of a new gate is where they surface.
