# 25A — A crate that gains its first test changes a gate, and says nothing

**Aim, stated first, per [`21A`](21A-the-destination-and-the-three-steps-to-it.md) §5
item 4:** *this session fixed no feature. It read why CI went red on a tree whose local
gates were all green, and closed the gate hole that let it.*

Follows [`24A`](24A-the-gui-gets-a-board-tab-and-a-regression-that-no-gate-could-see.md),
same date, and is its sibling rather than its sequel: `24A` found an artifact **outside
the workspace** that no gate compiles; this one found an artifact **inside** it that no
gate linted. Same shape, two directions.

**The one sentence, if only one survives:** *`pi5-image` was absent from the local lint
list — correctly, for the crate's whole life as a packaging stub — and
`STORY-P1-09-18` silently ended that by giving it its first `#[cfg(test)]` module, so
its tests were linted by nobody until the runner; the list is now enumerated and
length-pinned, and the gate was mutated until it failed before being believed.*

## 1. CI went red on a green tree

Run `31256380768`, on `f06a5c2`:

```text
error: this assertion has a constant value
   --> src/pi5-image/src/wire_shell.rs:404      (bin "pi5-image" test)
   --> src/hal-arm64/src/tos64_cmd.rs:786       (lib test)
```

Two `clippy::assertions_on_constants`, both runtime `assert!`s over operands that are
entirely constant. Fixed in `895bc82` as `const { assert! }`, and **the fix is better
than the code it replaced** rather than merely quieter: a bound on a board stack and a
frame width that must never be paddable are properties that should refuse to *compile*,
not fail a test run. `tos64_cmd.rs` already carried that exact note from an earlier
session; the new assertions did not follow it.

That is the whole of the defect, and it was one commit. **The rest of this handover is
about why it reached the runner**, because that was not fixed by the same commit and is
worth more than the two lines were.

## 2. `LE-126` — the local lint gate did not lint `pi5-image`

`check-lints` lints `--lib --tests` per package, precisely so test code cannot escape
it (`LE-77`). It would have caught both assertions. It did not, because **`pi5-image`
was not in `HOST_LINT_TARGETS`** — and that was *correct* for the whole life of the
crate, which was a packaging stub with no tests to lint and no library to point `--lib`
at.

[`STORY-P1-09-18`](../../goals/stories/STORY-P1-09-18.md) ended that and nothing
announced it. Making `pi5-image` the composition root gave it its first `#[cfg(test)]`
module — the wire shell's grant set, its seed, its stack budget — which moved the crate
inside this gate's remit while the list still said otherwise.

**The general form, which is the durable part: a crate that gains its first test has
just entered a lint gate's remit, and nothing in cargo announces that.** The moment a
package acquires a test module is the moment somebody must ask which gates now own it,
and there is no tooling anywhere that raises the question.

Closed test-first. The red said what it needed to say:

```text
pi5-image is not linted locally, so its warnings reach CI first
```

Two tests carry it now:

- `every_crate_this_gate_is_responsible_for_is_in_the_list` enumerates the owned crates
  **and asserts the list's length**, so a ninth cannot arrive without a decision. It
  names `fdt-walk` and `os` as deliberate absences, so their absence reads as a choice
  rather than as this same oversight a second time.
- `only_the_bin_only_crate_is_marked_as_one` became
  `bin_only_is_set_exactly_for_the_crates_that_have_no_library`. `bin_only` is a **fact
  about a crate**, and it had been sitting there phrased as though it were a lint
  preference — which is exactly how a later session talks itself into setting it on a
  crate that does have a library.

`pi5-image` is `bin_only` because it genuinely has no library, and `--all-targets` is
safe for it in a way it is not for the fixture bins in `exec`, `shell` and `kernel`:
those name `hal_x86_64` items gated `cfg(not(windows))` and cannot build on this host
at all, which is why `LE-77` scoped them and why widening that scope would make the
whole gate fail permanently for reasons unrelated to the code under review.

`check-lints` now covers 9 packages, up from 8.

## 3. The mutation, and the mutation that lied

The widened gate was not believed until it was made to fail. Reintroducing the exact
assertion reproduces CI's error, locally, verbatim:

```text
error: this assertion has a constant value
   --> src\pi5-image\src\wire_shell.rs:409
xtask: lint check failed: clippy refused: pi5-image
```

**The first mutation attempt passed, and that is the half worth recording.** It mutated

```rust
assert!(core::mem::size_of::<World<'static>>() <= STACK_BUDGET);
```

which *reads* like a constant assertion and is not one to clippy, because `size_of` is
not a literal. The lint fires on the other line, whose operands are literals throughout.
A session that had stopped at that green would have filed the gate as proven on the one
arm that could never have failed it — and would have been more confident than before it
started.

`20A` §8 earned *"a refusal must be tested against the data it is meant to accept, not
only against the data it is meant to reject."* This is its twin and it is the sharper
of the two: **a gate must be mutated with the defect it exists for, not with something
that resembles it.** The resemblance is what makes the false proof feel like a real one.

## 4. Four instances, one shape

`24A` §1 makes this point for the console; here is the whole family, because the fourth
instance is what turns a pattern into a property of how this project fails.

| Row | The artifact no gate covered | Found by |
|---|---|---|
| `LE-72` | the featureless AArch64 image | three red pushes |
| `LE-92` | the x86_64 Tier 0 fixture binaries | an `E0308` on the runner |
| `LE-125` | the Tauri operator console | someone building it by hand, two commits later |
| `LE-126` | `pi5-image`'s first unit tests | a red CI run on a green tree |

**In none of the four was the compiler wrong. In all four the list was.** And the
detection method degrades as the artifact moves further from the workspace: `LE-92` and
`LE-126` took one runner failure each; `LE-72` took three pushes; `LE-125` took *a
person deciding to build something for an unrelated reason*, which is not a detection
method at all.

## 5. `LE-125`'s cost, measured — `24A` §4 item 1 discharged in part

`LE-125` records that the cheap options were untried because the cost of compiling
`stage-e-console` in CI had **not been measured**, and that the choice between them was
a real one rather than a formality. Measured on this bench:

```text
$ cargo check -p stage-e-console
   Compiling tauri-build / tauri-codegen / tauri-macros / tauri / tauri-runtime …
    Finished `dev` profile in 47.62s
```

It also **compiled clean**, which independently confirms `24A`'s repair of the seventh
construction site.

**Read the number carefully, because it flatters the option it appears to support.**
47.6 s is a *warm* check on a bench whose `target/` already holds the vendored fork. CI
is cold, and the units it pulled in — `tauri-build`, `tauri-codegen`, `tauri-macros`,
`tauri`, `tauri-runtime`, `webview2-com` — are proc-macro and build-script heavy, which
is the worst shape for a cold runner. What the measurement *does* establish is narrower
and still useful: **once the fork is built, checking the console is cheap.** So the open
question is dependency-build cost and cache strategy, not the check itself — and the
seam option `LE-125` already names (a workspace-member smoke target constructing every
`os/src` type the PoC consumes) needs no fork build at all and is **not** argued against
by this number. The row stays open and its decision stays a decision.

## 6. What the next session does, in order

1. **`LE-125`'s choice**, with §5's half-measurement in hand: either a cold-cache
   measurement on the runner, or the seam smoke target — which §5 shows is the option
   the warm number does not weigh against.
2. **`21A` §5 item 3** — make `emit-dashboard` and `emit-feasibility` *write*. Still the
   cheapest open item in the mandate; `22A` paid its cost seven times and `24A`/`25A`
   have each paid it again.
3. `20A` §7's four items, which `22A` §9 left deliberately unblocked: `LE-121`'s silicon
   run, `LE-117` half (2), `STORY-P1-09-17`'s remaining criteria, and the measurement
   sweep.

## 7. Standing instruction earned

**A gate's coverage is a list, and a list is a claim about a set that changes without
telling anyone.** Gates report on what they *did* check and never on what they *should*
have, so a gap in one is invisible from its own output — which is why all four rows in
§4 were found by something breaking rather than by anything noticing. The two moments
that silently change a gate's remit are **a crate gaining its first test** and **an
artifact living outside the workspace**, and neither raises so much as a warning.

The practical form, and the reason `LE-126` was closed with an enumeration rather than
with one more entry: **pin the list's length, not only its contents.** A membership
assertion catches a removal. Only a length assertion catches the crate somebody adds
without asking which gates now own it.
