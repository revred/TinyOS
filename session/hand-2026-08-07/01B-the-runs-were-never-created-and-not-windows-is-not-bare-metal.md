# 01B — The runs were never created, and `not(windows)` is not `bare metal`

Executes [`01A`](01A-cover-note-for-the-next-session.md). Two findings, one
of them a live defect that `01A` predicted the shape of and nobody could have
predicted the mechanism of, because no machine in this project can see it.

## 1. The premise `01A` opened with was wrong, and that was the whole answer

`01A` §"Where the project actually is" states: *"Everything is committed and
pushed. `dff7b1d` on `origin/main`, tree clean."*

```text
git ls-remote origin main   ->  e2739316...  refs/heads/main
git rev-parse HEAD          ->  dff7b1dc...
```

`origin/main` was at `e273931`. **`dff7b1d` was committed on this bench and
never pushed**, so the absence of a run for it was never evidence of anything —
it was the absence of a push. That does not explain the other four, and the
rest of §1 is about those.

The cheap decisive test `01A` named was run, and it answered in the first of
the two directions `01A` laid out. Pushing `e273931..dff7b1d` created run
[`31161538569`](https://github.com/revred/TinyOS/actions/runs/31161538569) at
`2026-08-07T08:24:08Z`, within seconds of the push.

So **run creation is not broken.** The four pushes of 2026-08-06 evening
(`b4a7010` 20:52:07Z, `cb9b27b` 21:12:19Z, `4f5f2a4` 21:15:55Z, `e273931`
22:13:17Z) fell inside the platform outage window, and GitHub does not
retro-create runs for pushes whose run creation it dropped. `10C` §3's
diagnosis was correct; what expired was not the diagnosis but the expectation
that recovery would backfill. The remedy for a dropped push is another push.

**The residual, and it is weaker than the fix looks.** `dff7b1d`'s tree
contains all four dropped commits, so the gates now execute on the *cumulative*
state and never on the individual trees of `b4a7010`, `cb9b27b`, `4f5f2a4` or
`e273931`. A bisect across those four has no runner evidence behind any of its
steps and never will.

Filed as **`LE-102`**, closed, with the reasoning above and the two corrections
below. `01A` was right that it belonged on the register rather than in prose.

### The account-level candidate is ruled out on structure, not on a probe

`01A` pointed at an Actions spending limit and recorded it unresolved because
the session token lacked the scope. The scope is moot twice over:
`users/revred/settings/billing/actions` now returns **HTTP 410** (endpoint
moved), and more decisively `revred/TinyOS` is **public** (`private: false`)
with every job on `ubuntu-latest`. Standard runners on public repositories are
unmetered, so no spending limit can suppress them. Confirmed alongside it:
`actions/permissions` → `enabled: true`, `allowed_actions: all`; both workflows
`state: active`.

One correction to `10C` §3's ruled-out list, which changes no conclusion but
should not be inherited: **`.github/` was not untouched** — `e273931` and
`dff7b1d` both edit `ci.yml`. It changes nothing because the *earliest* silent
push, `b4a7010`, touched no workflow file at all. `fork-advisories`' silence
over the same window is explained by its `paths:` filter and is not evidence
either way.

### The instrument: `01A`'s correction was half right, and the other half matters more

`01A` reports that `actions/runs?head_sha=` *"returns `0` regardless, so it
ruled nothing out."* The probe works. It works **only with a full 40-character
SHA**:

```text
head_sha=420e875                                    -> total_count: 0
head_sha=420e8758c94c1296e5e088c1d6a84db87f0b1115   -> total_count: 1
```

`10C` abbreviated the SHA. Its probe was not answering about the run at all —
it was answering, correctly, about a commit identifier the API accepts and can
never match. `LE-80`'s family a fourth time, and it sharpens `01A`'s own
standing rule: checking that an instrument *can* return both answers is not
enough if it can be handed an argument it accepts and silently cannot match.
The list-and-match-client-side form `01A` recommends is correct and is what
carried the conclusion here.

## 2. `host-tests` went red on its first Linux run, and not for the reason `01A` expected

`01A` predicted this and named its class: *"`kernel`, `exec` and `shell` carry
fixture bins gated `cfg(not(windows))` that no local gate compiles. That is
`LE-64`'s family."* The class is right. The mechanism is worse, and it is not
a test failure at all:

```text
rust-lld: error: duplicate symbol: _start
  >>> defined at /usr/lib/gcc/x86_64-linux-gnu/13/../../../x86_64-linux-gnu/Scrt1.o:(_start)
  >>> defined at hal_x86_64...cgu.4.rcgu.o:(.boot+0x0)
error: could not compile `hal-x86_64` (lib test)
error: could not compile `exec` (lib test)
error: could not compile `kernel` (lib test)
error: could not compile `hal-arm64` (lib test)
```

**Not one host test in the workspace ran.** The suite died at the linker in
four crates.

`hal_x86_64::boot` is gated `#[cfg(not(target_os = "windows"))]`, and its
`global_asm!` defines `_start` in section `.boot`. **A Linux host satisfies
`not(target_os = "windows")` exactly as `x86_64-tinyos` (`"os": "none"`)
does.** The gate's stated intent, recorded in `lib.rs`'s own module doc, was
"not assemblable under a COFF-flavored host assembler" — a *build* property.
It was never the property the code needs, which is "not linked into a hosted
binary", and the two are the same condition when read from a Windows bench.
Every local gate agreed, because every local gate is Windows.

It was harmless for the entire life of the project because CI only ever ran
`clippy`, **which does not link**. `LE-100` added `cargo test --workspace` on
2026-08-06, and the defect became reachable the moment a runner tried to
produce an executable.

That is `LE-100`'s own sentence landing on `LE-100`: *a gate is only as strong
as the weakest place it is actually executed.* The first time the host suite
executed where it counts, it found that it could not execute at all.

### The fix, and why it is narrower than the obvious one

The obvious fix is to gate the four modules (`boot`, `interrupts`,
`qemu_exit`, `serial`) on `target_os = "none"`. **It was written, and then
rejected on evidence.** `kernel`'s `[[bin]]` writes `use hal_x86_64::boot as
_;` and calls `hal_x86_64::interrupts::init`, `::serial::SerialPort::init` and
`::qemu_exit::panic_report` *ungated*, and the Linux governance job's `clippy
--workspace --all-targets` compiles that bin. Tightening the module gate turns
a currently-green job red with seven `E0432`/`E0433`s — which is exactly what
running plain `clippy --workspace --all-targets` on this bench prints, both
before and after the change, and that is itself worth recording: **plain
`clippy --workspace --all-targets` is not a local gate on Windows.** It is red
on the pristine tree. `check-guest-images` and `check-boot-images` are the
local gates, as `CLAUDE.md` says.

So the gate moved to the one thing that actually emits the symbol: both
`global_asm!` blocks inside `boot.rs` are now `#[cfg(target_os = "none")]`.
The module still exists for an ELF-native host and is empty there. Nothing is
lost — assembling that block on a host never proved anything
`check-guest-images` does not prove properly, against the real target.

`core::arch::global_asm!` is now spelled in full at both sites rather than
imported, because a `use` whose only consumers are `cfg`'d out is an unused
import and this workspace builds with `-D warnings`.

### The guard, and the two traps it was written around

`hal_x86_64::gate_tests` (in `lib.rs`, not in `boot.rs`) asserts every
`global_asm!` invocation in `boot.rs` is immediately preceded by
`#[cfg(target_os = "none")]`.

- **It lives in `lib.rs` deliberately.** `boot` is itself
  `#[cfg(not(target_os = "windows"))]`, so a `#[test]` written inside it does
  not exist on this project's only development bench and would gate nothing
  where the mistake is made.
- **Comment lines are excluded, and not as tidiness** — `01A` trap #2. The
  comment in `boot.rs` explaining this very gate contains the string
  `global_asm!`, so a scan that did not skip comments would match the prose
  describing the fix and demand a `#[cfg]` above a sentence. That is
  `metric_labels.rs`'s self-match twice over.
- **Verified by mutating the real file** — `01A` trap #1. Removing the
  `#[cfg(target_os = "none")]` from `boot.rs` itself (not from a fixture) makes
  the guard fail naming line 67 with the `LE-102` message, not with an exit
  code. A companion test pins the positive case *and* refuses the wrong gate:
  a scan that accepted any `#[cfg(...)]` would have passed the committed defect
  unchanged, and that is the version a careless author writes first.

### What the guard cannot see, stated because a closed row reading as fully guarded is worse than an open one

1. **It is a text scan of one file.** `LE-99`'s residue verbatim: a
   `global_asm!` emitted by a macro, or reached through a renaming re-export,
   counts zero. A second file acquiring an ungated `global_asm!` is invisible
   to it — the scan is `boot.rs` by name because `boot.rs` is where `_start`
   is, not because that is a general property.
2. **It guards the symbol, not the class.** `interrupts`, `qemu_exit`, `serial`
   and the `not(target_os = "windows")` gates throughout `gdt.rs`, `paging.rs`,
   `pci.rs`, `fault.rs` and `exec/address_space.rs` still say "not Windows"
   while meaning "bare metal". None of them currently emits a colliding symbol.
   Nothing stops the next one from doing so, and the honest reason this session
   did not convert them is the one above: converting them reddens the Linux
   governance job, and the change that makes that safe is a change to how
   `kernel`'s bin is built, which is design surface the 2026-07-30 sprint rule
   has not lifted.
3. **Whether `cargo test --workspace` links the `no_main` `[[bin]]`s on Linux
   is still unknown.** The first run aborted at the four lib-test link failures
   and never reached them. If cargo builds those bins during `cargo test`, a
   `#![no_main]`/`#![no_std]` binary with no `_start` cannot link for a hosted
   target and `host-tests` will go red again for a *new* reason. **That is the
   next red to expect, and it is not `LE-64`'s family.**

## 3. Which red was whose, on run 31161538569

| job | verdict | whose |
| --- | --- | --- |
| `Format, lint, size, assurance, missing_docs` | success | — |
| `governance-fixture-smoke-test` | success | — |
| `host-tests` | **failure** | §2 above — fixed here |
| `kernel-boot-x86_64` | **failure** | `LE-23`, owner decision |
| `record-timing-baseline` | skipped | `workflow_dispatch` only |

`kernel-boot-x86_64` failed exactly where `01A` said it would, at
`check-timing-regression`, with

```text
xtask: `D04/context_switch_yield_roundtrip_2switches_spoored` was measured but
has no baseline; commit one (`--update-baseline`) rather than leaving it ungated
```

One correction to `01A`: it says the gate *"will name three unbaselined
metrics."* It names **one** and exits 2 on the first. All three spoored metrics
are unbaselined; only the first is ever printed. That is worth knowing before
someone reads a single name and concludes two of them got baselined. The gate
is otherwise behaving correctly and `--update-baseline` remains **not** the
fix, for the reason `01A` and the `ci.yml` comment both give.

## 4. What this session did not do

`01A`'s "if you finish early" list is untouched: `EPIC-P1`'s missing
`FEAT-P1-11` row, `LE-98`'s device-tree half, and `10C` §5 item 4. The whole
session went into §1 and §2, and §2 was not optional — `host-tests` is blocking
by design and `main` was red.

The do-not-start list was honoured: no `FEAT-P1-12`, no `G09`/`LE-86`, no new
design surface.

## 5. For the next session

1. **Read run 31161538569's successor first.** The `_start` fix is committed
   on the evidence of a local mutation and the local suite; whether it clears
   the Linux linker is a fact only the runner holds, and §2's residue #3 names
   the specific way it can still fail.
2. **`origin/main` and `HEAD` are not the same thing.** This session's entire
   §1 existed because a handover asserted a push it had not made. `git
   ls-remote origin main` is one command and it is the only one that answers.
3. `EPIC-P1`'s `FEAT-P1-11` row is now carried unactioned through **three**
   handovers.

---

**Written 2026-08-07.** Loose ends: `LE-102` raised and closed. Gates run:
`cargo fmt --all --check`, `cargo test --workspace`,
`cargo run -p xtask -- check-guest-images` (22 x86_64 Tier 0 binaries),
`cargo run -p xtask -- check-boot-images` (3 AArch64 variants),
`check-assurance-spine`. Plain `cargo clippy --workspace --all-targets` was
**not** run as a gate, and §2 records why it cannot be one on this bench.
