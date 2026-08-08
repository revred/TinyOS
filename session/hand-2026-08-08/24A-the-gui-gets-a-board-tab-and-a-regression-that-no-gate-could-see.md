# 24A — The GUI gets a board tab, and a regression no gate could see

Follows [`23A`](23A-the-keyhole-the-clamp-and-the-lie.md), same date. Taken at
the owner's direction: *make the Tauri tabbed terminal real on the board*, with
the standing priorities **(1) TinyOS as a real-time OS, (2) human usability**.

**The one sentence, if only one survives:** *The operator console can now open a
tab whose commands run **on the Raspberry Pi** rather than in-process on the
laptop — the seam is a `BoardLink` trait with seven host tests behind a scripted
double, so the tab model is provable with no Pi on the desk — and building it
exposed that **nothing in CI or in any local gate compiles the console at all**,
which is how `23A` broke it and pushed the break to `main` past a fully green
gate set.*

## 1. The regression, and why it is the more important half

`LE-124` added a `platform` field to `shell::verbs::World`. Six construction
sites were found by grepping `os/src` and fixed. **The seventh is
`external/tauri/tinyos-poc/stage-e-console/src/tabs.rs`** — the tab model of the
planned GUI — and it is outside `os/`, outside the cargo workspace, and reached
by no gate:

- `cargo test` at `os/` does not compile it;
- `check-boot-images` and `check-guest-images` do not compile it;
- CI's `cargo clippy --workspace --all-targets` does not reach it;
- `check-external-isolation` gates the **direction** of the dependency and says
  nothing about whether the consumer still builds.

So a type change in `os/src/shell` broke the GUI, every gate stayed green, and
it was found two commits later only because this session tried to *build* the
console for an unrelated reason. **This is `LE-72` and `LE-92`'s exact shape, on
its third instance** — nothing compiled the artifact, so a type error reached
`main` past a green gate set — and this time the artifact is the one the owner
named as the route to human usability.

Filed as **`LE-125`**, and the row does not pretend the fix is obvious: gating
`stage-e-console` pulls the vendored Tauri fork into CI and **that cost has not
been measured**. The alternative — a workspace-member smoke target that
constructs every `os/src` type the PoC consumes — is cheaper and narrower. The
choice is real and is written down as one rather than assumed.

Build fixed the same session: a host tab supplies `Platform::TIER0_X86_64`,
because a host tab genuinely runs TINYCMD on this laptop and should say so.

## 2. `TabKind::Board` — the integration itself

`21A` §3 step 3 landed in `22A` through `ti64dink --console`, the CLI. This puts
the same exchange in the window:

- **`TabKind::Board`** joins `Dos` and `Parity`. A `Dos` tab runs the verb core
  *on this laptop*; a board tab **runs nothing locally** — the line crosses the
  cable as one `TOS64-CMD/1` frame and the board's own `TINYCMD` answers it.
  They render identically because it is the same crate, which is exactly what
  `FEAT-P2-01`'s `fmt::Write` sink bought — **but only one of them is TinyOS**,
  and the chrome names it `BOARD` so an operator can tell which is answering.
- **`BoardLink`** is a trait, not a concrete transport. That is what makes the
  tab model testable with no Raspberry Pi, and it keeps the raw-Ethernet half in
  one implementation instead of spread through session logic. The real
  implementor drives `ti64dink`, which already owns the frame builder, the Npcap
  capture and the answer parser — all gated by `check-tool-tests`.
- **`BOARD_LINE_LIMIT` is `shell::capacities::MAX_LINE`**, named rather than
  restated. `23A`'s lesson one layer out: a console carrying its own literal
  would drift from the board the day the board changed.
- **A board tab cannot be opened without a link.** `open(TabKind::Board)`
  refuses, because a session with no transport is indistinguishable — to an
  operator — from a dead board.
- **Every failure is rendered, never swallowed.** `BoardError` has three named
  arms and each reaches the transcript. An empty transcript under a command that
  failed is the silent loss this project refuses everywhere else.

Seven tests, and the two that carry the argument: *a board tab runs nothing
locally* (an unanswered command must **not** quietly execute on the laptop —
the failure mode a future "fall back to the local world" change would
introduce), and *a line wider than a frame is refused here and never reaches the
wire* (so the operator reads what they typed, not the board's word for the
envelope). Mutation-verified: removing the width guard puts the long line on the
wire and reds that test by name.

## 3. Where this leaves human usability, measured rather than hoped

The transport, the runner, the width and now the GUI seam are all done. **What
binds is unchanged from `23A` §5 and neither item moved here:**

1. **~1 Hz.** The board answers **one line per park beat**. A terminal a human
   calls responsive needs sub-100 ms. That rate bound *is* `SEC-20`'s
   amplification containment, so raising it is a security decision with an
   argument to make, not a constant to edit.
2. **256 octets.** The answer rides a text frame bounded by
   `transcript::MAX_LINE_BYTES`; a two-file `DIR` already truncates with
   `more=`. Unlike the command envelope this bound is real — `LE-120`'s
   derivation — so it wants a continuation protocol or a larger frame, not a
   bigger number.

A tabbed terminal over a one-line-per-second, 256-octet channel is a working
integration and an unusable terminal. Both facts are true and the second is not
a reason to skip the first: the seam is what makes the two constraints
*measurable from the UI* instead of argued about.

## 4. What the next session does, in order

1. **The two constraints in §3**, because they are now the whole of human
   usability. The answer width first — it is bounded design work; then the beat,
   which needs a `SEC-20` argument rather than a patch.
2. **`LE-125`'s choice**: measure the cost of compiling `stage-e-console` in CI,
   or build the cheap smoke target instead. Until one exists, every `os/src`
   change can silently break the GUI again.
3. **The `LE-124` re-confirmation boot** (`23A` §6 item 1), still owed.
4. **Wire the real `BoardLink`** to `ti64dink` and open a board tab in the
   running window — the seam is done and tested; this is the transport plumbing
   behind it.

## 4b. Attribution correction for `bbf7ac4`

**The commit that carried this document carried a second session's work too, and
its message does not say so.** `bbf7ac4` contains
[`25A`](25A-a-crate-that-gains-its-first-test-changes-a-gate-and-says-nothing.md),
the `HOST_LINT_TARGETS` repair in `os/src/xtask/src/boot_images.rs`, and
`LE-126` — none of them this session's, all of them swept in by a `git add -A`
run while that session's work sat uncommitted in the tree.

That is `CONCURRENT_SESSIONS` rule 3, broken by the session that had refused the
same sweep two commits earlier and said so in writing. Recorded here rather than
by rewriting pushed history, because the correction a reader needs is *what the
commit contains*, and that is a fact about the record rather than about the
tree. **`LE-126` and `25A` are that session's, not this one's.**

One thing the collision itself demonstrated, worth keeping: the spine refused
two `24A-*` files with *"a citation must name exactly one"*. Two sessions
filing at one number is the exact case `session/README.md`'s letter rule
exists for, and the gate caught it before either document could be cited
ambiguously.

## 5. Standing instruction earned

**A gate that checks a relationship is not a gate that checks the thing.**
`check-external-isolation` has been green for weeks and means precisely one
thing: the PoC does not depend on `os/` backwards. It was read — by me, this
week — as evidence that the PoC was healthy. Every gate should be read as the
sentence it actually asserts, and the sentence should be short enough that
misreading it is hard.
