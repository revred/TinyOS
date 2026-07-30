//! The deterministic in-memory transport double (`MFS-03`, minimal): the
//! first implementor of [`crate::transport::MotionGroupTransport`], scripted
//! and repeatable, existing so that every stage/commit invariant is proven by
//! *observing the double's output* — not by trusting a returned error — and
//! so that EtherCAT later conforms to a contract already proven without it.
//!
//! Determinism is structural: the double holds a fixed-capacity script, no
//! clock, no randomness and no I/O, so the same script always produces the
//! same delivery, refusal and commit sequence. Its timeline is the epochs it
//! has delivered — a commit is *late* exactly when it names an apply epoch
//! that is not the successor of the last delivered one.

use crate::command::ActuationFrame;
use crate::feedback::FeedbackFrame;
use crate::ident::{Epoch, EpochStep, MAX_AXES, MAX_FEEDBACK};
use crate::profile::GroupProfile;
use crate::transport::{CommitToken, MotionGroupTransport, TransportFault};
use crate::validate::validate_actuation;

/// Fixed capacity of the double's feedback script and committed-frame
/// record. A compile-time bound like every other capacity in this crate.
pub const SCRIPT_CAPACITY: usize = 8;

struct StagedFrame {
    frame: ActuationFrame<MAX_AXES>,
    token_id: u32,
}

/// The deterministic, scripted, fixed-capacity in-memory transport.
pub struct InMemoryTransport {
    profile: GroupProfile,
    script: [Option<FeedbackFrame<MAX_FEEDBACK>>; SCRIPT_CAPACITY],
    scripted: usize,
    delivered: usize,
    current_epoch: Option<Epoch>,
    staged: Option<StagedFrame>,
    next_token_id: u32,
    committed: [Option<ActuationFrame<MAX_AXES>>; SCRIPT_CAPACITY],
    committed_count: usize,
}

impl InMemoryTransport {
    /// An empty transport for one group profile.
    #[must_use]
    pub const fn new(profile: GroupProfile) -> Self {
        Self {
            profile,
            script: [None; SCRIPT_CAPACITY],
            scripted: 0,
            delivered: 0,
            current_epoch: None,
            staged: None,
            next_token_id: 0,
            committed: [None; SCRIPT_CAPACITY],
            committed_count: 0,
        }
    }

    /// Appends one frame to the delivery script (test-side configuration,
    /// not part of the transport contract). Refuses beyond
    /// [`SCRIPT_CAPACITY`] — the double never grows.
    pub fn push_frame(&mut self, frame: FeedbackFrame<MAX_FEEDBACK>) -> Result<(), TransportFault> {
        if self.scripted == SCRIPT_CAPACITY {
            return Err(TransportFault::ScriptFull);
        }
        self.script[self.scripted] = Some(frame);
        self.scripted += 1;
        Ok(())
    }

    /// The last delivered epoch — the double's timeline position. `None`
    /// before the first delivery.
    #[must_use]
    pub const fn current_epoch(&self) -> Option<Epoch> {
        self.current_epoch
    }

    /// The staged-but-uncommitted frame, if any — observable so tests prove
    /// "stages nothing" by looking, not by trusting the returned error.
    #[must_use]
    pub fn staged(&self) -> Option<&ActuationFrame<MAX_AXES>> {
        self.staged.as_ref().map(|staged| &staged.frame)
    }

    /// How many frames have been committed — the double's emitted record.
    #[must_use]
    pub const fn committed_count(&self) -> usize {
        self.committed_count
    }

    /// The `index`-th committed frame, in commit order.
    #[must_use]
    pub fn committed(&self, index: usize) -> Option<&ActuationFrame<MAX_AXES>> {
        if index < self.committed_count {
            self.committed[index].as_ref()
        } else {
            None
        }
    }
}

impl MotionGroupTransport for InMemoryTransport {
    fn receive_epoch(&mut self) -> Result<FeedbackFrame<MAX_FEEDBACK>, TransportFault> {
        if self.delivered == self.scripted {
            return Err(TransportFault::NoFrame);
        }
        let frame = self.script[self.delivered].ok_or(TransportFault::NoFrame)?;
        self.delivered += 1;
        self.current_epoch = Some(frame.epoch);
        Ok(frame)
    }

    fn stage(&mut self, frame: ActuationFrame<MAX_AXES>) -> Result<CommitToken, TransportFault> {
        if self.staged.is_some() {
            return Err(TransportFault::StagePending);
        }
        validate_actuation(&self.profile, &frame).map_err(TransportFault::Refused)?;
        let token_id = self.next_token_id;
        self.next_token_id = self.next_token_id.wrapping_add(1);
        self.staged = Some(StagedFrame { frame, token_id });
        Ok(CommitToken::new(token_id, frame.apply_epoch))
    }

    fn commit_at(&mut self, token: CommitToken, apply_epoch: Epoch) -> Result<(), TransportFault> {
        // Fail closed on every arm: the staged frame is taken up front, so a
        // refused commit discards it rather than leaving it armed without a
        // live token (the token is consumed by this call either way).
        let Some(staged) = self.staged.take() else {
            // A token cannot outlive its staged frame in safe code (stage
            // refuses while one is pending, and every commit consumes the
            // token); typed defensively rather than panicking on an RT path.
            return Err(TransportFault::EpochMismatch);
        };
        if staged.token_id != token.id() || staged.frame.apply_epoch != token.apply_epoch() {
            return Err(TransportFault::EpochMismatch);
        }
        if apply_epoch != token.apply_epoch() {
            return Err(TransportFault::EpochMismatch);
        }
        let on_time = match self.current_epoch {
            Some(current) => apply_epoch.step_from(current) == EpochStep::Successor,
            // No epoch has ever been delivered: there is no timeline to be
            // on time against, and fail-safe beats fail-operational.
            None => false,
        };
        if !on_time {
            return Err(TransportFault::LateCommit);
        }
        if self.committed_count == SCRIPT_CAPACITY {
            return Err(TransportFault::ScriptFull);
        }
        self.committed[self.committed_count] = Some(staged.frame);
        self.committed_count += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{ActuationFrame, AxisCommand, CommandLimits, CommandMode};
    use crate::feedback::{FeedbackFrame, FeedbackQuality, FeedbackRole, FeedbackSample};
    use crate::ident::{
        AxisId, Epoch, FeedbackId, MotionGroupId, MotionTime, MAX_AXES, MAX_FEEDBACK,
    };
    use crate::profile::GroupProfile;
    use crate::transport::{MotionGroupTransport, TransportFault};
    use crate::units::{Position, Torque, Velocity};
    use crate::validate::StagingRejection;

    const GROUP: MotionGroupId = MotionGroupId::new(1);

    fn axis(index: u8) -> AxisId {
        AxisId::new(index).expect("test index in range")
    }

    fn feedback(index: u8) -> FeedbackId {
        FeedbackId::new(index).expect("test index in range")
    }

    fn one_axis_profile() -> GroupProfile {
        let mut profile = GroupProfile::new(GROUP);
        profile.require_feedback(feedback(0), axis(0), FeedbackRole::MotorPosition);
        profile.require_axis(axis(0));
        profile
    }

    fn scripted_frame(profile: &GroupProfile, epoch: Epoch) -> FeedbackFrame<MAX_FEEDBACK> {
        let mut frame = FeedbackFrame::new(GROUP, epoch, MotionTime::new(epoch.raw() as u64));
        let binding = profile.binding(feedback(0)).expect("declared");
        frame.place(FeedbackSample {
            feedback_id: feedback(0),
            axis_id: binding.axis,
            role: binding.role,
            position: Position::new(5),
            velocity: Velocity::new(1),
            quality: FeedbackQuality::Valid,
        });
        frame
    }

    fn command_frame(based_on: Epoch) -> ActuationFrame<MAX_AXES> {
        let mut frame = ActuationFrame::new(GROUP, based_on);
        frame.place(AxisCommand {
            axis_id: axis(0),
            mode: CommandMode::CyclicSynchronousPosition,
            target_position: Position::new(9),
            target_velocity: Velocity::new(2),
            target_torque: Torque::new(0),
            limits: CommandLimits { max_velocity: Velocity::new(10), max_torque: Torque::new(10) },
        });
        frame
    }

    fn transport_with(epochs: &[u32]) -> InMemoryTransport {
        let profile = one_axis_profile();
        let mut transport = InMemoryTransport::new(one_axis_profile());
        for &raw in epochs {
            transport
                .push_frame(scripted_frame(&profile, Epoch::new(raw)))
                .expect("script capacity not exceeded");
        }
        transport
    }

    // --- delivery ------------------------------------------------------------

    #[test]
    fn scripted_frames_are_delivered_in_order_then_no_frame() {
        let mut transport = transport_with(&[3, 4]);
        assert_eq!(transport.receive_epoch().expect("scripted").epoch, Epoch::new(3));
        assert_eq!(transport.receive_epoch().expect("scripted").epoch, Epoch::new(4));
        assert_eq!(transport.receive_epoch().unwrap_err(), TransportFault::NoFrame);
    }

    #[test]
    fn delivery_advances_the_transport_timeline() {
        let mut transport = transport_with(&[3]);
        assert_eq!(transport.current_epoch(), None);
        transport.receive_epoch().expect("scripted");
        assert_eq!(transport.current_epoch(), Some(Epoch::new(3)));
    }

    #[test]
    fn the_script_is_fixed_capacity_with_a_typed_refusal() {
        let profile = one_axis_profile();
        let mut transport = InMemoryTransport::new(one_axis_profile());
        for raw in 0..SCRIPT_CAPACITY as u32 {
            transport
                .push_frame(scripted_frame(&profile, Epoch::new(raw)))
                .expect("within capacity");
        }
        assert_eq!(
            transport.push_frame(scripted_frame(&profile, Epoch::new(99))),
            Err(TransportFault::ScriptFull)
        );
    }

    // --- staging -------------------------------------------------------------

    #[test]
    fn staging_a_complete_frame_yields_a_token_for_its_apply_epoch() {
        let mut transport = transport_with(&[3]);
        transport.receive_epoch().expect("scripted");
        let token = transport.stage(command_frame(Epoch::new(3))).expect("complete frame stages");
        assert_eq!(token.apply_epoch(), Epoch::new(4));
        assert!(transport.staged().is_some());
    }

    #[test]
    fn an_incomplete_frame_stages_nothing_at_all() {
        let mut transport = transport_with(&[3]);
        transport.receive_epoch().expect("scripted");
        // Mandatory axis 0 absent: an empty frame.
        let empty = ActuationFrame::new(GROUP, Epoch::new(3));
        assert_eq!(
            transport.stage(empty),
            Err(TransportFault::Refused(StagingRejection::MissingMandatoryAxis(axis(0))))
        );
        // Observed, not inferred: nothing is staged and nothing is committed.
        assert!(transport.staged().is_none());
        assert_eq!(transport.committed_count(), 0);
    }

    #[test]
    fn a_second_stage_while_one_is_pending_is_refused() {
        let mut transport = transport_with(&[3]);
        transport.receive_epoch().expect("scripted");
        let _token = transport.stage(command_frame(Epoch::new(3))).expect("stages");
        assert_eq!(
            transport.stage(command_frame(Epoch::new(3))),
            Err(TransportFault::StagePending)
        );
    }

    // --- commit --------------------------------------------------------------

    #[test]
    fn a_valid_commit_emits_exactly_the_staged_frame() {
        let mut transport = transport_with(&[3]);
        transport.receive_epoch().expect("scripted");
        let staged = command_frame(Epoch::new(3));
        let token = transport.stage(staged).expect("stages");
        transport.commit_at(token, Epoch::new(4)).expect("successor apply epoch commits");
        assert_eq!(transport.committed_count(), 1);
        assert_eq!(transport.committed(0), Some(&staged));
        assert!(transport.staged().is_none());
    }

    #[test]
    fn a_commit_naming_a_different_epoch_than_staged_is_refused_and_emits_nothing() {
        let mut transport = transport_with(&[3]);
        transport.receive_epoch().expect("scripted");
        let token = transport.stage(command_frame(Epoch::new(3))).expect("stages");
        assert_eq!(transport.commit_at(token, Epoch::new(5)), Err(TransportFault::EpochMismatch));
        assert_eq!(transport.committed_count(), 0);
        // Fail closed: the failed commit discarded the staged frame rather
        // than leaving it armed with no live token.
        assert!(transport.staged().is_none());
    }

    #[test]
    fn a_late_commit_fails_closed_and_the_command_is_never_retagged() {
        let mut transport = transport_with(&[3, 4]);
        transport.receive_epoch().expect("scripted");
        let token = transport.stage(command_frame(Epoch::new(3))).expect("stages");
        // Epoch 4 arrives before the commit lands: the apply point has passed.
        transport.receive_epoch().expect("scripted");
        assert_eq!(transport.commit_at(token, Epoch::new(4)), Err(TransportFault::LateCommit));
        // Observed, not inferred: no output happened, and a later cycle emits
        // only its own frame — the missed command never reappears.
        assert_eq!(transport.committed_count(), 0);
        let next = command_frame(Epoch::new(4));
        let token = transport.stage(next).expect("stages");
        transport.commit_at(token, Epoch::new(5)).expect("commits");
        assert_eq!(transport.committed_count(), 1);
        assert_eq!(transport.committed(0), Some(&next));
    }

    #[test]
    fn a_commit_before_any_delivered_epoch_fails_closed() {
        let mut transport = transport_with(&[]);
        let token = transport.stage(command_frame(Epoch::new(0))).expect("stages");
        assert_eq!(transport.commit_at(token, Epoch::new(1)), Err(TransportFault::LateCommit));
        assert_eq!(transport.committed_count(), 0);
    }

    // --- determinism ---------------------------------------------------------

    #[test]
    fn the_same_script_produces_the_same_run_every_time() {
        let run = || {
            let mut transport = transport_with(&[3, 4]);
            let mut delivered = Vec::new();
            let mut committed = Vec::new();
            for _ in 0..2 {
                let frame = transport.receive_epoch().expect("scripted");
                delivered.push(frame);
                let command = command_frame(frame.epoch);
                let token = transport.stage(command).expect("stages");
                transport
                    .commit_at(token, frame.epoch.next())
                    .expect("successor apply epoch commits");
                committed.push(command);
            }
            (delivered, committed, transport.committed_count())
        };
        assert_eq!(run(), run());
    }
}
