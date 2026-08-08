# TEST-P1-09-18-A — The Verb Core Answers the Wire, and Changes Nothing By Doing It

Status: **Green — clauses 1–6 written Red first and Green 2026-08-08 in `os/src/xtask/src/boot_images.rs`, `os/src/hal-arm64/src/tos64_cmd.rs`, `os/src/pi5-image/src/wire_shell.rs` and `work/tools/ti64dink.tests/`. Clause 7 closed on silicon the same day — [`REPORT-2026-08-08-02`](../reports/REPORT-2026-08-08-02.md).**
Story: [`STORY-P1-09-18`](../stories/STORY-P1-09-18.md)
Tier: Host unit tests (build gate, classifier totality, table denial, answer bounding,
statelessness, host/board vocabulary parity) **plus** a Tier 1 board run witnessed on
the wire
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`, `D20`
Security controls: `SEC-18`, `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`
Boundary tests: `BND-03`, `BND-06`, `BND-07`, `BND-17`
Protection Domain contracts: `PD-02`, `PD-07`, `PD-10`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: `D20` is selected as stated open debt
([`goals/assurance/open-debt.tsv`](../assurance/open-debt.tsv)). This Test raises **no
timing, throughput or qualification claim**: the answer rate is a structural bound
(one line per park beat) asserted as arithmetic, never as a measured interval. `PD-02`'s
reading — why a wire peer with no identity earns a read-only, stateless shell and not a
shell — lives in the Story and binds every clause here.

## Specification

### 1. The usable OS compiles for the board, and a gate says so (`LE-123`, `LE-72`)

**Given** the AArch64 build gate, **then** `shell`'s library target is among the
packages it compiles and lints for `aarch64-tinyos`, and the gate's plan is a value
held by a host test rather than an operator's habit — a crate that compiles for the
board today and silently stops tomorrow is the failure the gate exists to catch.

**Mutation arm.** Removing `shell` from the board lint set fails a host test in
milliseconds; re-adding an unconditional `hal-x86_64` dependency to `shell` fails the
gate itself with the same wall of `invalid register` errors that was measured before
the fix.

### 2. The board image carries it, within the ceiling (`G-DX-8`)

**Given** every registered AArch64 image variant — the featureless one CI builds and
each fixture — **then** all of them link with `shell` in the image, and the flattened
`kernel8.img` stays inside the 8 MB base-image ceiling. The growth is **measured and
recorded**, not estimated: a footprint claim this project cannot reproduce is not
evidence.

### 3. The table grew by one row and by nothing else (`BND-03`, `PD-14`)

**Given** the `TOS64-CMD/1` classifier, **then**:

- `VERB_TABLE` holds exactly three rows, `PING`, `STATUS` and `SHELL`, with ids 1, 2
  and 3, no id zero, no two rows sharing an id or a wire name;
- all 65,536 verb ids resolve either to a row that exists or to `UnknownVerb`, and
  exactly `VERB_TABLE.len()` of them resolve — the one input-derived selection is
  exactly as wide as the table and no wider;
- exactly one row in the table reaches a runner, asserted over the table rather than
  over the variant;
- the classification of **every** row — which row answered, with which sequence — is
  not a function of the argument field, for every fill across its range.

**The retirement, in writing.** `-17` asserted *"no byte of the argument field steers
anything"*. That sentence is false for the `SHELL` row and was replaced by the narrower
true one above **in the test that used to hold it**, with the reason recorded there, so
no reader inherits a claim the code no longer makes. Retiring a claim silently is the
failure this clause exists to prevent.

### 4. The argument is a line, at a fixed width, and never a length (`BND-03`, `RCG-01`)

**Given** a well-formed `SHELL` command, **then** the command line is the fixed
30-octet field with padding trimmed from the end only; both fillers a sender can
plausibly produce — spaces from a human console, NULs from a zeroed buffer — resolve to
the same line, because a classifier that distinguished them would make one command mean
two things depending on who sent it. A completely blank field is an **empty** line, not
a missing one, and a completely full field is exactly the field. No length inside the
frame is read, because there is none.

### 5. One frame in, one line out, and the excess is named (`SEC-20`)

**Given** any output a runner can return, **then** the answer is exactly one line, at
most `ANSWER_CAPACITY` octets, containing exactly one terminator; output that does not
fit is carried as a **prefix** with the withheld octets counted in a ` more=` field
whose arithmetic is exact (carried + withheld = produced); and no octet a runner can
return — all 256 of them, asserted — can end the line early or reach the wire
unrendered.

**The deliberate divergence, asserted as a decision.** A `status` that will not fit is
dropped **whole** and named `none`, because it replays a verdict and a partial verdict
is a fabrication with a plausible shape. Shell output that will not fit is carried as a
labelled prefix, because it is a stream and a labelled prefix is a true statement about
the beginning of one. The label is the entire difference; an unlabelled prefix would be
the same forgery the `status` rule refuses.

**And the rate is unchanged.** A flood of `SHELL` commands emits no more lines than
there were beats, exactly as a flood of `PING`s does — the run happens inside the
bounded slot, so widening the table did not widen the work per beat.

### 6. The wire session is stateless and read-only (`PD-02`, `PD-14`)

**Given** the board's composed session, **then**:

- no command can change what the next command sees — asserted on the **wire** path by
  driving real envelopes, and over the mutations most likely to leak (a directory
  creation, a file copy, a deletion, a directory change, an environment variable);
- every verb outside the grant set is denied, enumerated over `VerbKind::ALL` so a verb
  added to the core tomorrow is denied without anyone remembering to deny it;
- every denial is **spoken** into the transcript the peer receives, audited, naming the
  session — a refusal a peer can read is a refusal it cannot mistake for a dead board;
- the grant set contains no mutating verb and no live-kernel-state verb, each named
  individually so removing one from the withheld list is a visible act;
- every input produces an answer, including the empty line, an unknown command, a
  traversal attempt and a byte string that is not UTF-8 — silence is indistinguishable
  from a dead board;
- the session's stack cost is a compile-time constant held against a declared budget
  (a quarter of the board's 64 KiB stack), so an overflow is a build failure rather
  than a guard-page fault discovered on a bench.

### 7. Board: a human types a DOS command and TinyOS answers (Tier 1)

**Given** a Pi 5 running the image, a direct cable and `ti64dink --console`, **when**
an operator types `SHELL VER`, `SHELL DIR` and `SHELL DEL README.TXT`, **then** the
capture shows `TINYCMD`'s own version and directory output for the first two and an
audited `Access denied` for the third, each matched to the sequence sent, with any
withheld octets named. The board's `TOS64-CMD/1` canvas row must show `last=SHELL` and
an `answered` count that moved by exactly the number of commands sent.

**Two-directions parity (`LE-80`'s discipline).** Ti64Dink's verb table, refusal
vocabulary and envelope arithmetic are checked against `tos64_cmd.rs` **by reading that
file**, in both directions: a row the board holds that the console cannot name, and a
row the console offers that the board would refuse, are each a red test. The console's
un-escaping is asserted as the exact inverse of the board's two reversible escape
classes, and is asserted **not** to invert the third — a `?` the board substituted is
printed as a `?`, because inventing the octet back would be the host fabricating board
output.

## Evidence

| Clause | Where | State |
|---|---|---|
| 1 | `os/src/xtask/src/boot_images.rs` | Green |
| 2 | `cargo run -p xtask -- check-boot-images`; `xtask pi5` flatten | Green — 525,624 octets, 6.3% of the ceiling |
| 3 | `os/src/hal-arm64/src/tos64_cmd.rs` | Green (host) |
| 4 | `os/src/hal-arm64/src/tos64_cmd.rs` | Green (host) |
| 5 | `os/src/hal-arm64/src/tos64_cmd.rs` | Green (host) |
| 6 | `os/src/pi5-image/src/wire_shell.rs` | Green (host) |
| 7 | `work/tools/ti64dink.tests/ConsoleTests.cs`, `ConsoleParityTests.cs` (host half); [`REPORT-2026-08-08-02`](../reports/REPORT-2026-08-08-02.md) (the wire) | **Green on silicon** — nine typed exchanges, 0 unanswered, `last=SHELL answered=17 refused=2 lastlen=144` |

## The parity gates, extended 2026-08-08 — every handshake, not most of them

Clause 7's two-directions discipline covered the **verb table**, the **refusal
vocabulary** and the **envelope arithmetic**. Two handshakes on the same path
had no gate at all, and both would have drifted silently:

- **The escape pair.** The board escapes two octets so a whole `TINYCMD`
  transcript survives one line of wire; the console inverts them. A third
  escape class added on the board would have reached an operator as literal
  backslashes with every test on both sides still green.
- **The answer's field names.** The board writes `out=` and the console looks
  for `out=`; nothing but habit held those equal. A rename would have left the
  console printing an empty transcript under a command that in fact succeeded.

Both are now read out of the Rust source in both directions, and **both were
proven to catch drift by mutation** before being trusted: renaming `out=` to
`output=` on the board fails with *"the board writes `output=` and ti64dink
would drop it unread"*, and adding a `b'\t'` escape arm fails the escape gate.
A parity test that has never been shown to fail is the trap this file's own
header records.
