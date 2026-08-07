# TEST-P1-13-01-A — One Fact from a Hostile Tree, or a Named Refusal

Status: **In progress — host clauses written Red first 2026-08-07 and Green the same
session; clauses 8 and 9 await the consumption increment and the board**
Story: [`STORY-P1-13-01`](../stories/STORY-P1-13-01.md)
Tier: Host unit tests (header discipline, caps, structure walk, refusal taxonomy,
linked-absence) **plus** a Tier 1 board half deferred to the consumption increment
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D22`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C4`
Boundary tests: `BND-02`, `BND-03`, `BND-17`
Protection Domain contracts: `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: `D22` is selected as stated open debt
([`goals/assurance/open-debt.tsv`](../assurance/open-debt.tsv)) — the display domain's
subsystem does not exist at measurable maturity and no guardrail closes on a parser
refusing a blob. This Test raises no timing, throughput or qualification claim.

## What this test is for

The flattened device tree is the first *complex* hostile format this project has ever
had a reason to read — `STORY-P1-09-16`'s admission filter deliberately reads six bytes
at fixed offsets precisely so that no such reader would exist. The Story's containment
decision (shape 2, recorded in the Story) keeps the walker **out of every image**: it
is a pure leaf crate whose consumers are a future C4 domain and, nearer, the host as a
disposable off-board inspector. The subject of this Test is therefore not "does the
walker work" but two properties with teeth:

1. **Totality.** Every byte string maps to exactly one answer or one *named* refusal,
   in bounded work, with no read outside the blob and no state left behind.
2. **Absence.** The walker's bytes are linked into nothing the board boots — asserted
   against the image crates' own manifests, not inferred from intent.

## Specification

### 1. The header is believed only within its own caps (`SEC-20`, `BND-02`)

**Given** `simple_framebuffer_reg` over any byte slice,
**then** a blob shorter than the 40-byte header, a wrong magic, a version below 17, a
`totalsize` above the crate's hard cap, and a `totalsize` larger than the bytes
actually presented are each a **distinct named refusal** — the size-lying arm before
any region is read, because `totalsize` is the one field every later bound derives
from.

### 2. Regions are validated before they are walked (`SEC-20`, `BND-02`)

**Given** the struct and strings region descriptors from the header,
**then** an out-of-bounds or misaligned struct region, an out-of-bounds strings
region, and **overlapping** struct/strings regions (the self-referential arm of the
Feature's adversarial list) are each refused by name before the first token is read.

### 3. The caps are hard, and each one refuses (`SEC-19`, `SEC-20`)

**Given** a blob built to exceed exactly one cap,
**then** nesting deeper than `MAX_DEPTH` refuses `DepthOverCap`; a token stream longer
than `MAX_TOKENS` (a NOP flood inside a legal region) refuses `TokenOverCap`; and both
caps are compile-time constants a test asserts, so a "temporary" widening is a diff in
a reviewed file, not a config drift. This is the mutation evidence the Story's
criterion 3 demands: each cap is exercised by an input that violates only it, and the
refusal named is that cap's own.

### 4. The structure walk is total (`SEC-19`, `BND-03`)

**Given** hostile structure,
**then** each of the following is a distinct named refusal, never a partial answer or
a wrong one: a token the format does not define; a region exhausted mid-token; a node
name with no terminator before the region ends; a property whose declared length runs
past the region; a property name offset outside the strings region or unterminated
within it; `END_NODE` at depth zero; `END` with nodes still open; content after the
walk that never reaches `END`.

### 5. The one fact is extracted exactly, and ambiguity is refused, not resolved (`RCG-01`)

**Given** a well-formed blob holding a node whose `compatible` string list contains
exactly `simple-framebuffer`,
**then** its `reg` is returned as `(base, size)` interpreted under the **parent's**
`#address-cells`/`#size-cells` (spec defaults 2/1 when absent); cells counts outside
1..=2, a missing `reg`, and a `reg` too short for one (address, size) pair are each
named refusals — a matched node that cannot be read is hostile input, not a candidate
to skip. The first node whose definition completes wins, deterministically. A
`compatible` that merely *contains* the text (`simple-framebuffer-extended`) does not
match; the comparison is whole-entry.

### 6. Address translation is refused, never guessed (`SEC-19`)

**Given** a matched node with an ancestor carrying a **non-empty** `ranges`,
**then** the result is `RangesUnsupported` — a named refusal recording that the
answer would require bus-address translation this increment does not implement. An
**empty** `ranges` (identity by spec) and an absent one (the `simple-framebuffer`
binding's own convention: its `reg` is a CPU-visible physical address) pass through
untranslated. If the target firmware's blob takes the refusal arm, that is the board
telling us the second increment's shape — the refusal is the honest answer until then.

### 7. The walker is linked into nothing the board boots (`BND-03`, `PD-14`)

**Given** the image crates' own manifests (`kernel`, `hal-arm64`, `pi5-image`, `os`,
`exec`, `shell`, `hal`, `hal-x86_64`, `motion`),
**then** none declares a dependency on `fdt-walk` — asserted by a test that reads the
manifests, so the decision's central property ("privileged hostile-parser linked bytes
equal zero" holds *by absence*) fails loudly the day someone wires the walker into an
image instead of into a C4 host.

### 8. Board (deferred): the source field speaks (`BND-17`)

`TOS64-DISPLAY/1` gains `fb_addr=… src=constant|refused` — the honest today-values —
in the consumption increment, board-side, under this Feature's sprint lift. Not this
session's clause; recorded so its absence is a stated gap rather than an implied
completion.

### 9. Board (deferred): the boot's own tree justifies the canvas base

The quarantined-DTB transmit and the host-side walk of the captured blob (`src=dtb`
by the off-board path, per the Story's decision) — the clause `LE-98` closes on.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/fdt-walk/src/lib.rs`), written Red first
per the TDD mandate — red as a compile-clean `todo!()` body under the full suite, then
green with no test weakened.

## Implementation location

- `os/src/fdt-walk/src/lib.rs` — the whole crate: header discipline, region
  validation, the bounded token walk, the refusal taxonomy, the linked-absence test.

## Reports

None yet. Clauses 8 and 9 file with the board run that carries them.
