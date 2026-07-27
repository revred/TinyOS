# STORY-P0-07-01 — Bounded, Capability-Scoped Message Channel Between Two Tasks

Status: **Verified**
Feature: [`FEAT-P0-07`](../features/FEAT-P0-07.md)
Introduced in: [`FEAT-P0-07`](../features/FEAT-P0-07.md), this session (2026-07-26)
Implemented in: [`session/hand-2026-07-26/26-feat-p0-07-local-ipc.md`](../../session/hand-2026-07-26/26-feat-p0-07-local-ipc.md)

## Description

A bounded, fixed-capacity message channel between exactly two `TaskId`s — the "socket communication" half of `FEAT-P0-07`'s local-IPC scope, in the local (never network-addressable) sense. `kernel::ipc::Channel<CAP, MSG_LEN>` is a directional pipe: the `sender` endpoint may only `send`, the `receiver` endpoint may only `receive`, enforced by the API itself rather than caller discipline, so a third task (or the wrong-direction endpoint) can't touch it at all. Modeled on the same `Pool<T, N>`-backed, no-heap, fail-closed-on-exhaustion discipline every other Phase 0 subsystem already follows: a full channel's `send` fails closed rather than growing the buffer.

## Depends on

`STORY-P0-02-01` (tasks to be channel endpoints), `STORY-P0-03-01` (the `Pool<T, N>` discipline this Story's own fixed-capacity ring buffer mirrors, though `Channel` itself is a plain array-backed ring, not a `Pool` instance).

## Acceptance criteria (final, superseding the original draft)

1. A channel is created bound to exactly two `TaskId`s — its `sender` and `receiver` — and each may only perform its own permitted operation; a third task, or the wrong-direction endpoint, is rejected with `ChannelError::NotAnEndpoint`. **Met**: `only_the_sender_may_send_and_only_the_receiver_may_receive`, `a_third_task_cannot_send_or_receive`.
2. `send`/`receive` never allocate and never block — a full channel's `send` fails closed with `ChannelError::Full` rather than growing the buffer; an empty channel's `receive` fails closed with `ChannelError::Empty` rather than blocking. **Met**: `send_on_a_full_channel_fails_closed`, `receive_on_an_empty_channel_fails_closed`.
3. Every channel operation is capability-scoped via a standalone `ChannelPolicy` trait (the same Dependency Inversion stand-in `exec::win32_shim`'s `CapabilityPolicy` and `crate::wcet`'s `OverrunHandler` already established, since the real `aci` engine doesn't exist yet) — a denying policy rejects an otherwise well-formed `send`/`receive`. **Met**: `a_denying_policy_rejects_send_and_receive`.
4. No network-addressable socket (loopback or otherwise) is ever created — a channel is purely an in-kernel data structure keyed by two `TaskId`s. **Met by construction**: `kernel::ipc::Channel` has no I/O, no port, no networking dependency of any kind.
5. Messages are received in FIFO order, correctly across repeated fill/drain wraps of the ring buffer, and a message's payload round-trips exactly. **Met**: `a_sent_message_is_received_with_the_same_payload`, `messages_are_received_in_fifo_order_across_repeated_wraps`.
6. A payload exceeding a message type's fixed capacity is rejected, not truncated silently. **Met**: `a_payload_exceeding_capacity_is_rejected`.

## Tests

`os/src/kernel/src/ipc.rs`'s `#[cfg(test)]` module — host-testable pure logic (a bounded channel over two known task endpoints has no target dependency), including a third-task rejection test that deliberately allocates all three tasks from the same `Scheduler` (a `TaskId`'s equality is by pool-slot identity, so tasks from two separate `Scheduler` instances could coincidentally compare equal, which would make a naive version of that test pass for the wrong reason). See [`TEST-P0-07-01-A`](../tests/TEST-P0-07-01-A.md) and [`REPORT-2026-07-26-21`](../reports/REPORT-2026-07-26-21.md).

## Goals verified

`G-PC-3` (no privileged bypass — a channel's two-endpoint, single-direction scoping is exactly the kind of "ambient access nobody explicitly granted" this goal exists to prevent), `G-AI-3` (identical mediation regardless of caller type, once a real capability check exists).
