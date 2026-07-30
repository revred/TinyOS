# TEST-P1-08-01-A — One Epoch In, One Epoch Out, Whole or Not at All

Status: **Verified (Host), 2026-07-30** — every clause Green (51 host tests, written Red first and observed failing as a 119-error compile-stage Red). Specification unchanged since it was written before implementation.
Story: [`STORY-P1-08-01`](../stories/STORY-P1-08-01.md)
Tier: Host unit tests (`cargo test -p motion`) — no Tier 0 fixture, no board, no bus, per the Story's scope and the 08A mandate
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D21`
Security controls: `SEC-19`, `SEC-20`
Containment classes: `C1`, `C2`, `C3`
Boundary tests: `BND-03`, `BND-14`, `BND-15`, `BND-17`
Protection Domain contracts: `PD-05`, `PD-07`, `PD-08`, `PD-12`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

Applicable guardrails: none closeable — `D21` readiness is `design` (no field-I/O
subsystem exists), so the selection is stated open debt per `LE-35` and this Test files
no guardrail evidence. What this Test establishes is **Code-live** on the delivery
contract's claim ladder, and nothing above it.

## What this test is for

A 16-axis machine whose axes sample at different instants, or whose commands latch on
different cycles, is not synchronised no matter how fast each axis is. The invariant
that makes coupled control possible is *whole-or-nothing at both boundaries*: one
coherent feedback epoch in, one atomic command commit out. This Test pins that
invariant into the type system and the transport contract before any fieldbus exists,
so that EtherCAT later has to conform to a proven contract instead of the contract
quietly conforming to EtherCAT.

Every safety detector here requires a positive control: a clean run alone is not
evidence that missing feedback, stale quality, repeated epochs, partial staging, token
reuse or late commits would be detected. Each rejection arm is driven deliberately.

## Specification

### 1. Identity is typed and bounded

**Given** the public motion types,
**then** group, axis, feedback, epoch and time are distinct types that do not
interchange; axis constructors refuse index ≥ 16 and feedback constructors refuse
index ≥ 32; and no public constructor or accessor lets a raw integer index cross the
motion boundary unchecked.

### 2. Epoch order is wrap-aware and explicit

**Given** two epochs,
**then** successor-ness is decidable across the numeric wrap (the maximum epoch's
successor is epoch zero), a repeated epoch and an out-of-order epoch are distinguishable
rejections, and wrap can never make an old frame appear current.

### 3. A complete epoch is accepted exactly once

**Given** a frame carrying every mandatory feedback bit with matching group,
profile-consistent per-sample identity, all-`Valid` quality on mandatory bits and the
expected successor epoch,
**then** validation accepts it, and the accepted epoch becomes the new order baseline.

### 4. Every invalid epoch is rejected whole, with a typed reason (`BND-03`, `PD-12`)

**Given** a frame with (a) any mandatory feedback bit absent, (b) a non-valid quality
on a mandatory bit, (c) a repeated epoch, (d) an out-of-order epoch, (e) a wrong group
identity, or (f) a sample whose identity disagrees with the profile,
**then** the whole epoch is rejected with a distinct typed reason per arm — never a
boolean, never a partially-accepted frame — and rejection changes no state an accepted
epoch would have changed. Frames are fixed-layout hostile input from a compromisable
transport; there is no variable-length parsing on this path.

### 5. Staging is atomic (`BND-14`, `PD-05`)

**Given** an actuation frame in which any mandatory axis command is absent, or whose
mask and command contents disagree, or whose apply epoch is not the successor of its
`based_on` epoch,
**then** `stage` refuses and **nothing** from the frame becomes staged — verified by
observing the double's staged output, not by trusting the returned error. A staged
frame conveys data only; no authority accompanies it.

### 6. A commit token is single-use and bound (`SEC-20`, `PD-08`)

**Given** a successfully staged frame and its token,
**then** the token commits exactly once for exactly that frame and epoch; a second
commit with the same token is refused; a token cannot commit a different epoch than
the frame was staged for; and tokens do not accumulate (fixed capacity, typed refusal
at the bound).

### 7. A late commit fails closed (`PD-07`)

**Given** a token whose apply epoch has already passed in the double's timeline,
**then** the commit is refused, the command is **not** emitted, and it is **not**
relabelled for a later epoch — observed from the double's committed record.

### 8. The double is deterministic and the invariants are observable (`BND-17`, `SEC-19`)

**Given** the same script,
**then** the in-memory double produces the identical sequence of delivered frames,
refusals and committed frames on every run; every staged and committed frame is
inspectable by tests; and the whole crate is memory-safe by construction —
`#![forbid(unsafe_code)]`, `no_std`, allocation-free outside `#[cfg(test)]`.

### 9. What this test explicitly does **not** establish

- **No timing claim of any kind.** No cycle period, skew, feedback age, WCET, commit
  margin, latency or jitter figure exists at this tier (delivery contract §3 and §9).
- **No transport claim.** EtherCAT, Distributed Clocks, working counters, PDO layouts,
  the NIC, DMA and the C2/C3 process image are later work packages (`LE-62`, `LE-26`).
- **No scheduler binding.** Periodic phase-aligned release is `MFS-02`, on
  `FEAT-P1-04`'s machinery, not here.
- **No safety claim.** Hardware e-stop/STO independence is physically out of scope of
  a host-tier data-contract Story and stays with the delivery contract's §2 boundary.

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/motion/src/`), written Red first.

## Implementation location

- `os/src/motion/` — identities, frames, masks, validation, transport contract, and
  the deterministic in-memory double.

## Reports

[`REPORT-2026-07-30-04`](../reports/REPORT-2026-07-30-04.md).
