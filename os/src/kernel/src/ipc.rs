//! Bounded, capability-scoped message channel between two tasks
//! (`STORY-P0-07-01`).
//!
//! A [`Channel`] is a fixed-capacity, no-heap, directional pipe between
//! exactly two [`TaskId`]s: `sender` may only [`Channel::send`], `receiver`
//! may only [`Channel::receive`] — enforced by the API itself, not by
//! caller discipline, so a third task (or the wrong-direction endpoint)
//! can't touch it at all. Modeled on [`crate::mem::Pool`]'s own
//! no-heap/fail-closed-on-exhaustion discipline: a full channel's `send`
//! fails closed rather than growing the buffer, and `send`/`receive` never
//! block — a caller that needs to wait is responsible for its own
//! retry/parking discipline, the same scope boundary [`crate::lock`]'s own
//! doc comment already draws for lock contention.
//!
//! **Capability scoping.** The real `aci` capability/policy engine (Phase 5
//! per `docs/mvp-delivery-strategy.md`'s crate map) doesn't exist in this
//! workspace yet — this module defines the same minimal standalone
//! [`ChannelPolicy`] trait shape `exec::win32_shim`'s `CapabilityPolicy` and
//! `crate::wcet`'s `OverrunHandler` already established (Dependency
//! Inversion, `agent/CODING_STANDARDS.md` §D): every function that needs a
//! policy decision takes `&impl ChannelPolicy`, never a concrete policy
//! type, so wiring in the real `aci` engine later is additive.
//!
//! **No network-addressable socket.** A [`Channel`] is purely an in-kernel
//! data structure keyed by two `TaskId`s — never a bound port, loopback or
//! otherwise, per `FEAT-P0-07`'s own scope-boundary note.

use crate::sched::TaskId;

/// Errors [`Channel::send`]/[`Channel::receive`] fail closed with, per
/// `agent/CODING_STANDARDS.md`'s "no stringly-typed errors" rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelError {
    /// The caller is neither of this channel's two endpoints, or is the
    /// endpoint that isn't permitted this operation (e.g. `receiver` calling
    /// `send`) — a channel is directional, not a shared mailbox.
    NotAnEndpoint,
    /// The capability policy denies this operation between this channel's
    /// two endpoints.
    PolicyDenied,
    /// `send` on a channel already holding `CAP` unread messages — never
    /// grown, never overwrites the oldest message.
    Full,
    /// `receive` on a channel with nothing queued.
    Empty,
}

/// Errors [`Message::new`] fails closed with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageError {
    /// The payload exceeds this message type's fixed capacity `N`.
    TooLong,
}

/// A fixed-capacity message payload of at most `N` bytes — the `no_std`
/// equivalent of a bounded byte buffer, mirroring `exec::pe::FixedBytes`'s
/// own pattern (a separate type since `kernel` has no dependency on `exec`,
/// and shouldn't gain one just to reuse a private helper).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Message<const N: usize> {
    bytes: [u8; N],
    len: usize,
}

impl<const N: usize> Message<N> {
    /// Builds a message from `payload`, failing closed with
    /// [`MessageError::TooLong`] rather than truncating silently if it
    /// exceeds this type's fixed capacity.
    pub fn new(payload: &[u8]) -> Result<Self, MessageError> {
        if payload.len() > N {
            return Err(MessageError::TooLong);
        }
        let mut bytes = [0u8; N];
        bytes[..payload.len()].copy_from_slice(payload);
        Ok(Message { bytes, len: payload.len() })
    }

    /// This message's payload bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

/// The minimal capability-check shape this Story needs standalone, since
/// the real `aci` policy engine doesn't exist in this workspace yet — see
/// this module's own doc comment for the migration path once it lands.
pub trait ChannelPolicy {
    /// Whether `sender` is currently granted the capability to send `receiver`
    /// a message over this channel.
    fn is_granted(&self, sender: TaskId, receiver: TaskId) -> bool;
}

/// A policy that grants every channel — the default standalone behavior
/// until a real per-pair, per-scope policy exists.
pub struct AllowAllPolicy;

impl ChannelPolicy for AllowAllPolicy {
    fn is_granted(&self, _sender: TaskId, _receiver: TaskId) -> bool {
        true
    }
}

/// A bounded, directional message channel between exactly two `TaskId`s:
/// `sender` may only send, `receiver` may only receive. `CAP` bounds how
/// many unread messages of at most `MSG_LEN` bytes each can queue at once.
pub struct Channel<const CAP: usize, const MSG_LEN: usize> {
    sender: TaskId,
    receiver: TaskId,
    // A fixed-capacity ring buffer: `head` is the oldest unread message's
    // slot, `len` how many slots (starting at `head`, wrapping) are
    // occupied. `None` marks an unoccupied slot; every occupied slot in
    // `[head, head+len)` (mod `CAP`) holds `Some`.
    buffer: [Option<Message<MSG_LEN>>; CAP],
    head: usize,
    len: usize,
}

impl<const CAP: usize, const MSG_LEN: usize> Channel<CAP, MSG_LEN> {
    /// Creates a channel bound to exactly `sender`/`receiver` — its only
    /// two endpoints, for the rest of this channel's lifetime.
    pub const fn new(sender: TaskId, receiver: TaskId) -> Self {
        Channel { sender, receiver, buffer: [None; CAP], head: 0, len: 0 }
    }

    /// Enqueues `message`, sent by `caller` to this channel's `receiver`.
    ///
    /// Fails closed with [`ChannelError::NotAnEndpoint`] if `caller` isn't
    /// this channel's `sender`, [`ChannelError::PolicyDenied`] if `policy`
    /// denies this pair, or [`ChannelError::Full`] if `CAP` unread messages
    /// are already queued — never blocks, never grows the buffer.
    pub fn send(
        &mut self,
        policy: &impl ChannelPolicy,
        caller: TaskId,
        message: Message<MSG_LEN>,
    ) -> Result<(), ChannelError> {
        if caller != self.sender {
            return Err(ChannelError::NotAnEndpoint);
        }
        if !policy.is_granted(self.sender, self.receiver) {
            return Err(ChannelError::PolicyDenied);
        }
        if self.len == CAP {
            return Err(ChannelError::Full);
        }
        let tail = (self.head + self.len) % CAP;
        self.buffer[tail] = Some(message);
        self.len += 1;
        Ok(())
    }

    /// Dequeues the oldest unread message, received by `caller` from this
    /// channel's `sender`.
    ///
    /// Fails closed with [`ChannelError::NotAnEndpoint`] if `caller` isn't
    /// this channel's `receiver`, [`ChannelError::PolicyDenied`] if `policy`
    /// denies this pair, or [`ChannelError::Empty`] if nothing is queued.
    pub fn receive(
        &mut self,
        policy: &impl ChannelPolicy,
        caller: TaskId,
    ) -> Result<Message<MSG_LEN>, ChannelError> {
        if caller != self.receiver {
            return Err(ChannelError::NotAnEndpoint);
        }
        if !policy.is_granted(self.sender, self.receiver) {
            return Err(ChannelError::PolicyDenied);
        }
        if self.len == 0 {
            return Err(ChannelError::Empty);
        }
        let message =
            self.buffer[self.head].take().expect("occupied slot per this type's own invariant");
        self.head = (self.head + 1) % CAP;
        self.len -= 1;
        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sched::{Priority, Scheduler, WcetBudgetTicks};

    #[allow(clippy::empty_loop)]
    extern "C" fn dummy_entry() -> ! {
        loop {}
    }

    fn priority(value: u8) -> Priority {
        Priority::try_new(value).expect("value is in range")
    }

    fn two_tasks() -> (TaskId, TaskId) {
        let mut sched: Scheduler<4> = Scheduler::new();
        let a = sched.create_task(priority(1), WcetBudgetTicks(1000), dummy_entry).unwrap();
        let b = sched.create_task(priority(1), WcetBudgetTicks(1000), dummy_entry).unwrap();
        (a, b)
    }

    // STORY-P0-07-01 AC1: only the two endpoints can operate on the
    // channel at all, each in only the direction it was created for.
    #[test]
    fn only_the_sender_may_send_and_only_the_receiver_may_receive() {
        let (sender, receiver) = two_tasks();
        let mut channel: Channel<4, 16> = Channel::new(sender, receiver);

        assert_eq!(
            channel.send(&AllowAllPolicy, receiver, Message::new(b"hi").unwrap()),
            Err(ChannelError::NotAnEndpoint)
        );
        assert_eq!(channel.receive(&AllowAllPolicy, sender), Err(ChannelError::NotAnEndpoint));
    }

    // A third task (neither endpoint) can't send or receive either. All
    // three tasks come from the same `Scheduler` — a `TaskId`'s equality is
    // by pool-slot identity, so tasks from two separate `Scheduler`
    // instances could coincidentally compare equal (e.g. both being each
    // scheduler's own first-allocated slot), which would make this test
    // pass for the wrong reason.
    #[test]
    fn a_third_task_cannot_send_or_receive() {
        let mut sched: Scheduler<4> = Scheduler::new();
        let sender = sched.create_task(priority(1), WcetBudgetTicks(1000), dummy_entry).unwrap();
        let receiver = sched.create_task(priority(1), WcetBudgetTicks(1000), dummy_entry).unwrap();
        let outsider = sched.create_task(priority(1), WcetBudgetTicks(1000), dummy_entry).unwrap();
        let mut channel: Channel<4, 16> = Channel::new(sender, receiver);

        assert_eq!(
            channel.send(&AllowAllPolicy, outsider, Message::new(b"hi").unwrap()),
            Err(ChannelError::NotAnEndpoint)
        );
        assert_eq!(channel.receive(&AllowAllPolicy, outsider), Err(ChannelError::NotAnEndpoint));
    }

    // A well-formed send/receive round-trips the payload exactly.
    #[test]
    fn a_sent_message_is_received_with_the_same_payload() {
        let (sender, receiver) = two_tasks();
        let mut channel: Channel<4, 16> = Channel::new(sender, receiver);

        channel.send(&AllowAllPolicy, sender, Message::new(b"hello").unwrap()).unwrap();
        let received = channel.receive(&AllowAllPolicy, receiver).unwrap();
        assert_eq!(received.as_bytes(), b"hello");
    }

    // STORY-P0-07-01 AC2: a full channel's send fails closed rather than
    // growing the buffer.
    #[test]
    fn send_on_a_full_channel_fails_closed() {
        let (sender, receiver) = two_tasks();
        let mut channel: Channel<2, 16> = Channel::new(sender, receiver);

        channel.send(&AllowAllPolicy, sender, Message::new(b"a").unwrap()).unwrap();
        channel.send(&AllowAllPolicy, sender, Message::new(b"b").unwrap()).unwrap();
        assert_eq!(
            channel.send(&AllowAllPolicy, sender, Message::new(b"c").unwrap()),
            Err(ChannelError::Full)
        );
    }

    // Receiving from an empty channel fails closed rather than blocking.
    #[test]
    fn receive_on_an_empty_channel_fails_closed() {
        let (sender, receiver) = two_tasks();
        let mut channel: Channel<2, 16> = Channel::new(sender, receiver);
        assert_eq!(channel.receive(&AllowAllPolicy, receiver), Err(ChannelError::Empty));
    }

    // Messages are received in the order they were sent (FIFO), and
    // draining then refilling the channel proves the ring buffer wraps
    // correctly rather than only working for a single fill/drain pass.
    #[test]
    fn messages_are_received_in_fifo_order_across_repeated_wraps() {
        let (sender, receiver) = two_tasks();
        let mut channel: Channel<2, 16> = Channel::new(sender, receiver);

        for round in 0..5u8 {
            channel
                .send(&AllowAllPolicy, sender, Message::new(&[round, round + 1]).unwrap())
                .unwrap();
            channel.send(&AllowAllPolicy, sender, Message::new(&[round + 2]).unwrap()).unwrap();
            assert_eq!(
                channel.receive(&AllowAllPolicy, receiver).unwrap().as_bytes(),
                &[round, round + 1]
            );
            assert_eq!(
                channel.receive(&AllowAllPolicy, receiver).unwrap().as_bytes(),
                &[round + 2]
            );
        }
    }

    // STORY-P0-07-01 AC3: a denying policy rejects an otherwise well-formed
    // send/receive, mirroring `win32_shim`'s own `PolicyDenied` precedent.
    #[test]
    fn a_denying_policy_rejects_send_and_receive() {
        struct DenyAllPolicy;
        impl ChannelPolicy for DenyAllPolicy {
            fn is_granted(&self, _sender: TaskId, _receiver: TaskId) -> bool {
                false
            }
        }
        let (sender, receiver) = two_tasks();
        let mut channel: Channel<4, 16> = Channel::new(sender, receiver);

        assert_eq!(
            channel.send(&DenyAllPolicy, sender, Message::new(b"hi").unwrap()),
            Err(ChannelError::PolicyDenied)
        );
        // Even a message actually queued (via the allowing policy) is
        // rejected on receive once the policy denies the pair.
        channel.send(&AllowAllPolicy, sender, Message::new(b"hi").unwrap()).unwrap();
        assert_eq!(channel.receive(&DenyAllPolicy, receiver), Err(ChannelError::PolicyDenied));
    }

    // A payload longer than a message's fixed capacity fails closed rather
    // than truncating silently.
    #[test]
    fn a_payload_exceeding_capacity_is_rejected() {
        assert_eq!(Message::<4>::new(b"toolong").err(), Some(MessageError::TooLong));
    }
}
