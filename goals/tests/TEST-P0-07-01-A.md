# TEST-P0-07-01-A — A Message Channel's Two Endpoints Are Directionally Enforced, Bounded, and Capability-Scoped

Status: **Verified — passing locally, 2026-07-26**
Story: [`STORY-P0-07-01`](../stories/STORY-P0-07-01.md)
Tier: Host (`cargo test -p kernel --lib`), per [Target Hardware & Test Matrix](../../README.md#target-hardware--test-matrix) — `kernel::ipc` is pure logic with no target-specific dependency.
Assurance contract: [`goals/assurance/story-contracts.tsv`](../assurance/story-contracts.tsv)
Performance domains: `D12`
Security controls: `SEC-04`, `SEC-05`, `SEC-14`, `SEC-16`, `SEC-20`
Containment classes: `C1`, `C2`, `C3`, `C4`
Boundary tests: `BND-04`, `BND-08`, `BND-09`, `BND-14`, `BND-15`, `BND-20`
Protection Domain contracts: `PD-02`, `PD-03`, `PD-05`, `PD-06`, `PD-08`, `PD-09`, `PD-13`, `PD-14`
Code admission gates: `RCG-08`, `RCG-09`, `RCG-12`, `RCG-13`, `RCG-14`
Assurance state: `baseline-debt`

## Specification

**Given** a `kernel::ipc::Channel<CAP, MSG_LEN>` created with a `sender`/`receiver` `TaskId` pair,
**when**:
- the `receiver` calls `send`, or the `sender` calls `receive`, or a third task calls either — **then** `ChannelError::NotAnEndpoint`, never silently permitted,
- `CAP` unread messages are already queued and `send` is called again — **then** `ChannelError::Full`, the buffer never grows,
- `receive` is called with nothing queued — **then** `ChannelError::Empty`, never blocks,
- a `ChannelPolicy` denies the `(sender, receiver)` pair — **then** `ChannelError::PolicyDenied` on an otherwise well-formed `send`/`receive`,
- messages are sent and received across repeated fill/drain cycles that wrap the ring buffer — **then** they arrive in the same FIFO order they were sent, with the same payload bytes,
- a payload exceeds `Message<N>`'s fixed capacity — **then** `MessageError::TooLong`, never truncated silently.

## Test type

Unit tests — `kernel::ipc`'s own `#[cfg(test)]` module. The third-task rejection test deliberately allocates `sender`/`receiver`/`outsider` from the *same* `Scheduler` instance: a `TaskId`'s equality is by pool-slot identity, so tasks from two separate `Scheduler`s could coincidentally compare equal (e.g. both being each scheduler's own first-allocated slot), which would make a naively-written version of this test pass for the wrong reason — this was caught during implementation and fixed before landing.

## Implementation location

`os/src/kernel/src/ipc.rs`.

## Reports

[`REPORT-2026-07-26-21`](../reports/REPORT-2026-07-26-21.md) — Pass.
