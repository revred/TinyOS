//! The narrow contract between the motion group and whatever moves its bytes
//! (`ADR 0010`): EtherCAT is *an implementation* of this trait, the
//! deterministic in-memory double is another, and the coupled control code
//! depends only on this boundary — never on PDO indices, EtherCAT object
//! numbers or a concrete NIC.
//!
//! Required invariants (delivery contract §4.4), enforced by type shape
//! where possible and by every implementor's conformance tests otherwise:
//!
//! - `stage` accepts the entire frame or changes nothing;
//! - a [`CommitToken`] is single-use and tied to exactly one staged frame —
//!   single use is *type-enforced*: `commit_at` consumes the token by value,
//!   and the token is neither `Clone` nor `Copy`, so a second commit with
//!   the same token is not writable in safe Rust;
//! - `commit_at` cannot change the frame or its epoch;
//! - no axis has a separate public "write now" escape path — this trait is
//!   the only output path, and it moves whole frames;
//! - a late commit fails closed;
//! - feedback acquisition and command staging are bounded and non-blocking;
//! - an implementation cannot hide dropped or repeated process images (the
//!   epoch discipline in [`crate::validate`] surfaces both).

use crate::command::ActuationFrame;
use crate::feedback::FeedbackFrame;
use crate::ident::{Epoch, MAX_AXES, MAX_FEEDBACK};
use crate::validate::StagingRejection;

/// Why a transport operation failed. Every arm fails closed: no variant
/// leaves a partial frame delivered, staged or emitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportFault {
    /// No feedback frame is available this cycle.
    NoFrame,
    /// The double's fixed-capacity script is full (test-side configuration,
    /// still a typed refusal — nothing in this crate grows).
    ScriptFull,
    /// A frame is already staged and uncommitted; the transport holds at
    /// most one staged frame, so the caller must commit (or lose) it first.
    StagePending,
    /// The frame failed whole-frame validation; nothing was staged.
    Refused(StagingRejection),
    /// The commit named a different epoch than the token was staged for; the
    /// staged frame is discarded, not emitted.
    EpochMismatch,
    /// The apply epoch has already passed; the command is not emitted and is
    /// never relabelled for a later epoch.
    LateCommit,
}

/// Single-use, non-copyable proof that exactly one frame is staged.
///
/// Only a transport implementation can mint one, and [`commit_at`]
/// consumes it — see the module documentation for why that makes reuse
/// unwritable rather than merely checked.
///
/// [`commit_at`]: MotionGroupTransport::commit_at
#[derive(Debug, PartialEq, Eq)]
pub struct CommitToken {
    id: u32,
    apply_epoch: Epoch,
}

impl CommitToken {
    /// Mints a token for one staged frame. Crate-internal: callers receive
    /// tokens from [`MotionGroupTransport::stage`], never construct them.
    #[must_use]
    pub(crate) const fn new(id: u32, apply_epoch: Epoch) -> Self {
        Self { id, apply_epoch }
    }

    /// The staged-frame identity, for the transport's own bookkeeping.
    #[must_use]
    pub(crate) const fn id(&self) -> u32 {
        self.id
    }

    /// The epoch the staged frame applies at.
    #[must_use]
    pub const fn apply_epoch(&self) -> Epoch {
        self.apply_epoch
    }
}

/// One motion group's cyclic transport: coherent feedback in, atomic
/// time-tagged commands out. EtherCAT is the first physical implementation;
/// the deterministic in-memory double ([`crate::double`]) is the first test
/// implementation; coupled control depends only on this trait.
pub trait MotionGroupTransport {
    /// Receives the next whole-group feedback frame, bounded and
    /// non-blocking: if no frame is available the call returns
    /// [`TransportFault::NoFrame`] rather than waiting.
    fn receive_epoch(&mut self) -> Result<FeedbackFrame<MAX_FEEDBACK>, TransportFault>;

    /// Stages one whole actuation frame: accepted entirely (yielding the
    /// single token that can commit it) or refused with nothing staged.
    fn stage(&mut self, frame: ActuationFrame<MAX_AXES>) -> Result<CommitToken, TransportFault>;

    /// Commits the staged frame for exactly `apply_epoch`. The token is
    /// consumed either way; a failed commit discards the staged frame
    /// rather than leaving it armed without a live token.
    fn commit_at(&mut self, token: CommitToken, apply_epoch: Epoch) -> Result<(), TransportFault>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ident::Epoch;

    #[test]
    fn a_commit_token_names_the_apply_epoch_it_was_staged_for() {
        let token = CommitToken::new(7, Epoch::new(42));
        assert_eq!(token.apply_epoch(), Epoch::new(42));
    }
}
