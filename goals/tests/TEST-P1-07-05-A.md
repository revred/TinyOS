# TEST-P1-07-05-A — One Command, Three Distinguishable Outcomes, and No Second Harness

Status: **Partially Verified (Host), 2026-07-30** — every host-testable clause Green (31 host tests, written Red first and observed failing; the one command built an 82,916-byte `kernel8.img` whose first bytes are the divergence record's pinned `A4 00 38 D5`); the Tier 1 live-board capture through this path has not happened. **Specification unchanged since it was written before implementation.**
Story: [`STORY-P1-07-05`](../stories/STORY-P1-07-05.md)
Tier: Host unit tests (capture parsing, verdict extraction, timeout handling, exit-code mapping, fixture registration) **plus** a Tier 1 hardware run driving a real board over the debug UART, per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix)
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D01`
Security controls: `SEC-14`, `SEC-19`, `SEC-20`
Containment classes: `C0`, `C1`
Boundary tests: `BND-01`, `BND-02`, `BND-03`, `BND-17`
Protection Domain contracts: `PD-07`, `PD-12`, `PD-14`
Code admission gates: `RCG-01`, `RCG-13`, `RCG-14`
Assurance state: `specified`

Applicable guardrails: `PERF-D01-G09`, `PERF-D01-G17`, `PERF-D01-G18` — image footprint, cold start and warm restart. The run path makes them observable on this board; it closes none.

## What this test is for

Hardware evidence that no tool can reproduce is anecdote. And a second, divergent harness beside the Tier 0 one is a shape this project has already paid for once: `LE-06`, `pool-bench`, closed by folding it back in rather than maintaining two.

The protocol this path consumes already exists. UART-borne pass/fail shipped inside `STORY-P1-01-02` for one stated reason — *a gate that can only read a QEMU exit code can never gate hardware*. This Story is where that foresight is either collected or found to have been wrong, and the second outcome is worth recording as loudly as the first.

## Specification

### 1. One command produces a placeable image

**Given** `cargo run -p xtask -- pi5 --fixture=<name>`,
**then** it builds the image for the target spec `STORY-P1-07-01` committed and prints exactly which artifacts go where on the boot partition.

**And automating the physical SD swap is out of scope.** Manual swap is acceptable and expected. The command's job is that nothing about the image layout is folklore held by whoever did it last.

### 2. The exit-code scheme is the Tier 0 scheme

**Given** a completed run,
**then** the exit code follows the same scheme as `qemu-x86_64`, and the UART pass/fail protocol is the one that already exists — **no new protocol is invented for hardware**.

**And** a reader who trusts the Tier 0 path can read this one without learning anything new. That is the requirement, not a nicety: divergence here is how the two paths start disagreeing about what "passed" means.

### 3. Silence, truncation and failure are three outcomes

**Given** a run,
**then** the tool distinguishes: a board that **never speaks** (no bytes before the timeout), a board that **speaks and stops** (bytes, then silence, no verdict), and a board that **reports failure** (a well-formed failing verdict). Each exits differently and each prints what it saw.

**And this clause matters more here than anywhere in the Tier 0 path**, because on this hardware during bring-up silence is the *common* case. A run path that reports silence as "still working" wastes exactly the session it was built to save — and one that reports it as "failed" sends the next hour to the wrong hypothesis.

### 4. The captured text is hostile input (`BND-03`, `PD-12`)

**Given** whatever bytes arrive on the serial port,
**then** they are parsed defensively: no unbounded buffering, no assumption of well-formedness, no assumption of framing, and a partial or corrupt verdict line is a *failure to read a verdict*, never a verdict.

**And** capture size is bounded (`SEC-20`). A board stuck in an output loop must not exhaust the host.

### 5. The run is registered like every other fixture

**Given** `cargo run -p xtask -- list-fixtures`,
**then** the Pi 5 run path appears with its owning `TEST-*`, per `STORY-P0-01-04`'s rule that **a fixture nobody runs is an unverified fixture that looks verified.**

### 6. The host-side logic is board-free (`SEC-19`)

**Given** the implementation,
**then** capture parsing, verdict extraction, timeout handling and exit-code mapping are pure functions over captured text with host unit tests covering clause 3's three outcomes plus malformed input. Only the serial-port open is I/O, and it is the seam.

### 7. Every run is attributable (`SEC-14`, `BND-17`)

**Given** a completed run,
**then** the tool records what it built, which fixture, which serial device, which baud, and the verdict — enough that a Report quoting the capture can be traced back to the invocation that produced it.

### 8. What this test explicitly does **not** establish

- **No CI integration.** Recorded §7.4 decision (b): hardware runs stay manual and land in Reports; CI stays Tier 0. The ratio baselines therefore stay Tier 0, and `LE-23` is unaffected either way.
- **No deploy loop.** This is a bring-up run path, not `EPIC-P1_5`'s deploy tooling; it makes no signing, atomicity, or rollback claim.
- **No Ethernet, no PCIe, no RP1.** `LE-26` is raised by this Feature and routed around, not answered: on a Pi 5 the recorded peer-to-peer Ethernet transport implies PCIe bring-up plus an RP1 driver plus a NIC driver before a single byte can be deployed.
- **No SD-card driver.** The firmware loads the image; TinyOS never touches the SD controller.
- **`LE-09` stays open.**

## Test type

Host unit tests (`#[cfg(test)]` in `os/src/xtask/src/`) plus a Tier 1 hardware run.

## Implementation location

- `os/src/xtask/` — the `pi5` subcommand, image assembly, serial capture, verdict parsing, exit-code mapping, fixture registration.

## Reports

To be filed when the Story goes Green.
