# Handover 27 — `STORY-P1-07-02`: the Fault Path That Exists Before the Fault Does

Follows: [`26-next-session-mandate.md`](26-next-session-mandate.md), which asked for exactly this
and gave the arithmetic — four of five clauses host-testable — as the reason not to wait for an
adapter. Companions: [`24-story-p1-07-01-host-half.md`](24-story-p1-07-01-host-half.md), whose split
this repeats, and [`23-bcm2712-divergence-record.md`](23-bcm2712-divergence-record.md), which is
still the hardware reference and is untouched by this session.

**On the folder date.** This repository's document dates run one day ahead of the clock, per
Handover 13 §"A note on dates". Do not read a date here as evidence of when anything happened.

**No adapter arrived.** No Raspberry Pi 5 has executed one instruction from this repository, and
nothing below changes that.

## Concurrency

Per [`agent/CONCURRENT_SESSIONS.md`](../../agent/CONCURRENT_SESSIONS.md) rule 7, stated up front.

This session started at `1faabf7`. No other session's commits arrived on `main` while it ran, but
**another session was live in this tree at the time of writing** — it had taken slot 28 and had
uncommitted work in four paths this session did not write:

- `session/hand-2026-07-28/28-analysis-response-and-le-33.md`
- `docs/competitive-position.md`
- `goals/assurance/loose-ends.tsv` (an `LE-33` row)
- `goals/reports/_soak-p0-03-01.log`, modified before this session began

**None of it was staged** (rules 1 and 3): this commit is by pathspec and contains no file this
session did not write. Slot 27 was claimed by creating the file before writing it (rule 4), which is
why there is no renumbering paragraph here. The `.githooks` gate was installed before any commit
(`git config core.hooksPath .githooks`), so the gates ran after staging.

Two consequences worth knowing, both of them the "counts are floors, not totals" rule in action:

- the spine counts quoted below were taken over the **combined** tree, so they include whatever that
  session has not committed. `check-assurance-spine` being green over the mixture means it is green
  over this session's subset; the reverse inference does not hold;
- the loose-ends line below says 32 rows / 21 open, which is what the checker reported when it ran.
  If `LE-33` lands from the other session, that count is already stale and theirs is right.

## What was built

The mandate's split, applied again: build the half a dev host can prove, and let the board gate the
Green. Three new modules in `hal-arm64`, one wiring change, and no new crate.

| Deliverable | Where |
|---|---|
| `ESR_EL1` decoding — class, `IL`, class-specific `ISS`, `FAR` validity | `os/src/hal-arm64/src/esr.rs` |
| The sixteen vector slots, their routing, and `VBAR_EL1` alignment | `os/src/hal-arm64/src/vectors.rs` |
| The fault frame, the bounded report, the vector table, `install`, two deliberate faults | `os/src/hal-arm64/src/fault.rs` |
| `install()` called at the end of the `EL1` boot path | `os/src/hal-arm64/src/boot.rs` |

`hal-arm64` went from **64 host tests to 115**; the workspace suite from **498 to 549**.
`TEST-P1-07-02-A` carries the per-clause table and **not one clause was edited**.

**`STORY-P1-07-02` is `In progress`, not `Verified`.** Clauses 1, 3, 4, 5 and 6 are Green on the
host; clause 2 is untouched, and the Test document says in bold that there is no version of this
Story that passes without it.

## Four things a reviewer should look at first

**1. "Every entry present" is a different claim on AArch64, and it is enforced by the assembler.**
On x86_64 the table is *data* — `Idt::every_entry_present` reads 256 descriptors and answers. On
AArch64 the table is *code*: sixteen slots of 128 bytes, branched to by offset, with nothing to
inspect at run time. So the claim is established twice, both times before the board runs:
`.org tinyos_vector_table + 0x80 * \index` places every entry, and the assembler **refuses to move
`.org` backwards**, so an over-long entry is a build failure rather than a table whose second half
is silently displaced by one slot. `llvm-objdump -h` confirms `.text.vectors` is exactly `0x800`
bytes with sixteen branch relocations.

That guard was **proven able to fail** before it was trusted — the entry body was temporarily padded
past 128 bytes and the build rejected it with `invalid .org offset '128' (at offset '204')`. A gate
nobody has watched fail is a gate nobody has tested; that is `fixture-broken-boot`'s discipline
applied to a build-time assertion.

**2. Target-only clippy caught a defect the host lint cannot see — the first time it has.**
The first implementation had each vector entry build a `FaultFrame` on the stack and hand the Rust
entry point a `*const FaultFrame`. `cargo clippy -p hal-arm64 --all-targets -- -D warnings` passes on
that code, because the entry point is `cfg(target_arch = "aarch64")` and the host never compiles it.
The AArch64 clippy invocation Handover 26 added rejected it: `clippy::not_unsafe_ptr_arg_deref`.

That is `LE-12` paying for itself, and it is worth knowing that the second command earned its place
on the first target-only Story to add code after it existed.

**The fix is better than the code it replaced**, which is why it is recorded rather than buried. Each
entry now loads the slot index and the four describing registers into `x0`–`x4` and branches; the
entry point takes five `u64` arguments. Six instructions, no `sub sp`, no raw pointer, no
`#[repr(C)]` layout invariant between assembly and Rust — and, the part that matters, **no store to
the stack** on a path one of whose causes is a stack that is no longer valid.

**3. The x86_64 disposition policy consumed an AArch64 fault unmodified.**
`TEST-P1-07-02-A` clause 4 says a second architecture is where "the disposition depends only on where
the fault happened" either holds or turns out to have been x86-shaped. It held, and needed **no
change at all**: `kernel::fault::Disposition`, `FaultReport` and `audit` are exercised from
`hal-arm64`'s host tests against every slot × a matrix of `ESR_EL1` values, and reach one
disposition. The `vector` field carries a vector *slot index* instead of an x86 vector number, and
because the policy never reads it, nothing else had to move.

State that as a result rather than a convenience. The policy survived a second architecture because
it reads exactly one field. Had it consulted an error code, the invariant would have been discovered
to be x86-shaped at the moment it was hardest to fix.

**4. `FAR_EL1` is refused for every class that does not update it.**
The direct restatement of `hal_x86_64::fault`'s refusal to report `CR2` for a `#GP`, and it matters
more here: `FAR_EL1` is a *register*, not a value pushed with the frame, so it holds whatever the
last exception that updated it left there. The report prints `far=invalid` rather than a number for
an `SP` alignment fault, for any abort with `FnV` set, and for every non-abort class. A test asserts
a planted `0xDEADBEEFDEADBEEF` never reaches the wire.

Note the pair the tests keep adjacent: a **PC** alignment fault puts the misaligned PC in `FAR_EL1`
and an **SP** alignment fault does not update it at all. They are one line apart in the architecture
and are the most likely two cases to be treated as one.

## One interpretation recorded rather than quietly applied

`TEST-P1-07-02-A`'s "Implementation location" names `os/src/kernel/` for "the deliberate-fault
fixture and its spoor emission". The fixture half is correct and still unbuilt. **The spoor emission
cannot be there**, structurally: `kernel` depends unconditionally on `hal-x86_64`, so building it
for `aarch64-tinyos` would mean building the x86_64 HAL for AArch64. No code that runs on the Pi can
call into `kernel` as this workspace is arranged.

What was done instead keeps the claim honest rather than duplicating it: `kernel::fault::audit` and
the real `Spoor` encoding are exercised **from `hal-arm64`'s host tests** (`kernel` is already a
dev-dependency, for `TEST-P1-01-03-A` clause 5's own reasons), so the audit path an AArch64 fault
would take is proven against the real policy, not a second copy of it. Two faults at one slot audit
*identically* however different their frames — the strongest available statement that no `ESR_EL1`
or `FAR_EL1` is smuggled into the atom.

What is **not** proven is that a board emits one. That is recorded as blocked with clause 2, not
counted as Green. Wiring a real emission needs a crate both architectures can depend on — a
`kernel-fault`-shaped split, or the AArch64 binary crate `STORY-P1-07-05` must introduce anyway.
**That decision is deliberately not made here**: `FEAT-P1-07` §6's rule is that a seventh concern
means re-decomposing, not extending.

## What this deliberately did not do

- **No `kernel8.img` and no fixture.** Trap 7 of the mandate. Clause 2's fault needs an image, and
  producing one is `STORY-P1-07-05`. The *triggers* — `deliberate_breakpoint` and
  `deliberate_alignment_fault` — are written and compiled so that **one board session can close
  `STORY-P1-07-01`'s capture and this Story's fault injection together**, which was the mandate's
  whole argument for doing this now.
- **No MMU, no GIC, no `CPACR_EL1.FPEN`.** `SimdFpAccessTrap` is *decoded* because it is the fault
  this board is most likely to take from code nobody wrote. Decoding it is diagnostics; enabling
  FP/SIMD would be a scope change with no test behind it.
- **No measurement, and none is available to quote.** Trap 5. The MMU is off, every access is
  Device-nGnRnE, nothing was measured, and `LE-09` is untouched and open.
- **No seventh Story.** No RP1, no PCIe, no GPIO, no device-tree parser, no address spaces, single
  core.
- **`LE-31` and `LE-23` not taken.** Board-adjacent work was possible, so the fallback was correctly
  not taken. `LE-31` is still the clearest non-hardware work in the project.

## One ordering decision worth knowing about

`install()` is called **after** `report_ready` in `continue_at_el1`, not before. The reason is
evidence, not correctness: `TEST-P1-07-01-A` clause 4's known byte sequence is `STORY-P1-07-01`'s
pending Green, and it should be produced by the same code path that was specified for it. Installing
vectors first would put two new lines ahead of that sequence and make one Story's untaken evidence
depend on another Story's code.

The consequence is stated in `boot.rs` rather than left implicit: everything between `_start` and
that call still runs with no fault reporting. A Story that installs a vector table cannot cover the
code that precedes its own installation, and pretending otherwise would be the same silent gap in a
better disguise.

## What the next session does

**Unchanged, and the case is now stronger.** Buy two USB-serial adapters and loopback-test one before
the board is ever blamed (`TEST-P1-07-01-A` clause 1). Then:

1. `config.txt` with **`os_check=0`** — Handover 23 §3, the divergence with no test behind it and
   the one that presents as total silence.
2. Read the first line. It says `current_el=`.
3. Quote the capture verbatim into `TEST-P1-07-01-A` clause 4, and only then move `-01` to Verified.
4. **In the same session**, inject a fault and quote `TINYOS-FAULT/1` into `TEST-P1-07-02-A` clause 2.
   Both triggers already exist. What does *not* exist is the image that calls one — decide
   deliberately whether that is `STORY-P1-07-05` arriving early or a minimal harness, and say which.

If there is no board time, `LE-31` is still first: two handovers reached it independently from
opposite directions, and a wrong belief about what blocks `verified` compounds in every later
session.

## State at the close

```
main                    1faabf7 + this work
assurance spine         green: 23 Features, 57 Stories, 44 Tests, 45 Reports
                        32 loose ends (21 open), 83 status headers
                        10 release gates with dated evidence
host tests              549 passing across the workspace; hal-arm64 64 -> 115
aarch64                 hal-arm64 builds and lints clean against the target spec, in CI
                        .text.vectors is exactly 0x800 bytes, sixteen entries
EPIC-P1                 4 Features of 7 complete
STORY-P1-07-01          In progress — criteria 1 and 5 Green, 3 half, 2 and 4 need the board
STORY-P1-07-02          In progress — clauses 1, 3, 4, 5, 6 Green; clause 2 needs the board
Stories verified        0 / 56
LE-09                   OPEN
LE-12                   OPEN, and it earned its keep this session
LE-31                   OPEN, and still the clearest non-hardware work in the project
```

A fault path that has never handled a fault is not a fault path yet. What exists now is that the
board has somewhere to land when it does — and, for the first time in this Feature, a deliberate
fault to land it with.
