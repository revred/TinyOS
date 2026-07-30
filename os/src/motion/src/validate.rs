//! Whole-epoch validation (`TEST-P1-08-01-A` §3–5): one accepted or rejected
//! group epoch, never a partially-accepted frame, with a distinct typed
//! reason for every rejection arm. Frames are fixed-layout hostile input from
//! a compromisable transport (`BND-03`, `PD-12`); both validators are pure
//! functions, so a rejection cannot change state by construction.
//!
//! Check order is deliberate: identity of the *frame* (group) first, then
//! the epoch discipline, then completeness, then per-sample identity, then
//! quality — nothing later in the chain is believed until everything earlier
//! held, so a wrong-group frame can never consume the epoch baseline and a
//! stale sample can never mask a missing one.

use crate::command::ActuationFrame;
use crate::feedback::{FeedbackFrame, FeedbackQuality};
use crate::ident::{AxisId, Epoch, EpochStep, FeedbackId, MAX_AXES, MAX_FEEDBACK};
use crate::profile::GroupProfile;

/// Proof that one feedback frame passed whole-epoch validation. Holding one
/// is the only sanctioned way an epoch becomes the new order baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptedEpoch {
    epoch: Epoch,
}

impl AcceptedEpoch {
    /// The accepted epoch — the caller's next order baseline.
    #[must_use]
    pub const fn epoch(self) -> Epoch {
        self.epoch
    }
}

/// Why a feedback epoch was rejected whole. One variant per disposition arm
/// in the delivery contract's fault table (§5) — never a boolean.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackRejection {
    /// The frame's group is not the profile's group.
    WrongGroup,
    /// The frame re-presents the already-accepted epoch.
    RepeatedEpoch,
    /// The frame's epoch is neither the successor nor a repeat.
    OutOfOrderEpoch,
    /// A mandatory channel did not report.
    MissingMandatoryFeedback(FeedbackId),
    /// A mandatory channel's sample identity disagrees with the profile.
    IdentityMismatch(FeedbackId),
    /// A mandatory channel reported with a non-valid quality.
    InvalidQuality {
        /// The offending channel.
        feedback: FeedbackId,
        /// The quality it reported.
        quality: FeedbackQuality,
    },
}

/// Validates one feedback frame whole against the profile and the epoch
/// baseline. `last_accepted` is `None` only before the first accepted epoch
/// of a session; after that the frame must be the exact successor.
pub fn validate_feedback(
    profile: &GroupProfile,
    last_accepted: Option<Epoch>,
    frame: &FeedbackFrame<MAX_FEEDBACK>,
) -> Result<AcceptedEpoch, FeedbackRejection> {
    if frame.group != profile.group() {
        return Err(FeedbackRejection::WrongGroup);
    }
    if let Some(previous) = last_accepted {
        match frame.epoch.step_from(previous) {
            EpochStep::Successor => {}
            EpochStep::Repeated => return Err(FeedbackRejection::RepeatedEpoch),
            EpochStep::Other => return Err(FeedbackRejection::OutOfOrderEpoch),
        }
    }
    for index in 0..MAX_FEEDBACK as u8 {
        let id = match FeedbackId::new(index) {
            Ok(id) => id,
            Err(_) => unreachable!("loop bound is MAX_FEEDBACK"),
        };
        if !profile.mandatory_feedback().contains(id) {
            continue;
        }
        if !frame.valid_mask.contains(id) {
            return Err(FeedbackRejection::MissingMandatoryFeedback(id));
        }
        let sample = &frame.samples[id.index()];
        let Some(owner) = profile.binding(id) else {
            // A mandatory bit always has a binding: `require_feedback` sets
            // both together. Defensive arm, typed as identity failure.
            return Err(FeedbackRejection::IdentityMismatch(id));
        };
        // Owner equality is total: wrong axis, wrong effector, wrong role or
        // wrong ownership kind entirely (the `R4` cast) all reject here.
        if sample.feedback_id != id || sample.owner != *owner {
            return Err(FeedbackRejection::IdentityMismatch(id));
        }
        if sample.quality != FeedbackQuality::Valid {
            return Err(FeedbackRejection::InvalidQuality {
                feedback: id,
                quality: sample.quality,
            });
        }
    }
    Ok(AcceptedEpoch { epoch: frame.epoch })
}

/// Why an actuation frame was refused staging, whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StagingRejection {
    /// The frame's group is not the profile's group.
    WrongGroup,
    /// The frame's apply epoch is not the successor of its basis epoch — a
    /// command may never be relabelled past its intended commit point.
    NonSuccessorApplyEpoch,
    /// A mandatory axis carries no command.
    MissingMandatoryAxis(AxisId),
    /// A masked slot's command identity disagrees with its axis.
    CommandIdentityMismatch(AxisId),
}

/// Validates one actuation frame whole against the profile: an
/// active-control commit with an incomplete mandatory mask is forbidden, and
/// a frame that fails here must stage nothing at all.
pub fn validate_actuation(
    profile: &GroupProfile,
    frame: &ActuationFrame<MAX_AXES>,
) -> Result<(), StagingRejection> {
    if frame.group != profile.group() {
        return Err(StagingRejection::WrongGroup);
    }
    if frame.apply_epoch.step_from(frame.based_on) != EpochStep::Successor {
        return Err(StagingRejection::NonSuccessorApplyEpoch);
    }
    for index in 0..MAX_AXES as u8 {
        let axis = match AxisId::new(index) {
            Ok(axis) => axis,
            Err(_) => unreachable!("loop bound is MAX_AXES"),
        };
        if !profile.mandatory_axes().contains(axis) {
            continue;
        }
        if !frame.valid_mask.contains(axis) {
            return Err(StagingRejection::MissingMandatoryAxis(axis));
        }
        if frame.commands[axis.index()].axis_id != axis {
            return Err(StagingRejection::CommandIdentityMismatch(axis));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{ActuationFrame, AxisCommand, CommandLimits, CommandMode};
    use crate::feedback::{
        AxisFeedbackRole, EffectorFeedbackRole, FeedbackFrame, FeedbackOwner, FeedbackQuality,
        FeedbackSample, GroupFeedbackRole,
    };
    use crate::ident::{
        AxisId, EffectorId, Epoch, FeedbackId, MotionGroupId, MotionTime, MAX_AXES, MAX_FEEDBACK,
    };
    use crate::profile::GroupProfile;
    use crate::units::{Position, Torque, Velocity};

    const GROUP: MotionGroupId = MotionGroupId::new(1);

    fn axis(index: u8) -> AxisId {
        AxisId::new(index).expect("test index in range")
    }

    fn feedback(index: u8) -> FeedbackId {
        FeedbackId::new(index).expect("test index in range")
    }

    fn axis_owner(axis_index: u8, role: AxisFeedbackRole) -> FeedbackOwner {
        FeedbackOwner::Axis { axis: axis(axis_index), role }
    }

    /// Two axes, one motor-side and one load-side channel each — the smallest
    /// profile shaped like the delivery contract's first demonstration.
    fn two_axis_profile() -> GroupProfile {
        let mut profile = GroupProfile::new(GROUP);
        profile.require_feedback(feedback(0), axis_owner(0, AxisFeedbackRole::MotorPosition));
        profile.require_feedback(feedback(1), axis_owner(0, AxisFeedbackRole::LoadPosition));
        profile.require_feedback(feedback(2), axis_owner(1, AxisFeedbackRole::MotorPosition));
        profile.require_feedback(feedback(3), axis_owner(1, AxisFeedbackRole::LoadPosition));
        profile.require_axis(axis(0));
        profile.require_axis(axis(1));
        profile
    }

    /// The Hexapod worked-case shape: three drive axes with motor- and
    /// load-side channels each, one end-effector probe-deflection channel, one
    /// group metrology channel — all mandatory, one epoch (`R4`).
    fn hexapod_profile() -> GroupProfile {
        let mut profile = GroupProfile::new(GROUP);
        for arm in 0..3u8 {
            profile.require_feedback(
                feedback(arm * 2),
                axis_owner(arm, AxisFeedbackRole::MotorPosition),
            );
            profile.require_feedback(
                feedback(arm * 2 + 1),
                axis_owner(arm, AxisFeedbackRole::LoadPosition),
            );
            profile.require_axis(axis(arm));
        }
        profile.require_feedback(
            feedback(6),
            FeedbackOwner::EndEffector {
                effector: EffectorId::new(0).expect("in range"),
                role: EffectorFeedbackRole::ProbeDeflection,
            },
        );
        profile.require_feedback(
            feedback(7),
            FeedbackOwner::Group { role: GroupFeedbackRole::Metrology },
        );
        profile
    }

    fn valid_sample(profile: &GroupProfile, id: FeedbackId) -> FeedbackSample {
        let owner = *profile.binding(id).expect("mandatory binding declared");
        FeedbackSample {
            feedback_id: id,
            owner,
            position: Position::new(10),
            velocity: Velocity::new(1),
            quality: FeedbackQuality::Valid,
        }
    }

    fn complete_frame(profile: &GroupProfile, epoch: Epoch) -> FeedbackFrame<MAX_FEEDBACK> {
        let mut frame = FeedbackFrame::new(GROUP, epoch, MotionTime::new(1_000));
        for index in 0..MAX_FEEDBACK as u8 {
            let id = feedback(index);
            if profile.mandatory_feedback().contains(id) {
                frame.place(valid_sample(profile, id));
            }
        }
        frame
    }

    fn complete_actuation(based_on: Epoch) -> ActuationFrame<MAX_AXES> {
        let mut frame = ActuationFrame::new(GROUP, based_on);
        for index in [0u8, 1] {
            frame.place(AxisCommand {
                axis_id: axis(index),
                mode: CommandMode::CyclicSynchronousPosition,
                target_position: Position::new(100),
                target_velocity: Velocity::new(5),
                target_torque: Torque::new(0),
                limits: CommandLimits {
                    max_velocity: Velocity::new(50),
                    max_torque: Torque::new(20),
                },
            });
        }
        frame
    }

    // --- feedback acceptance -------------------------------------------------

    #[test]
    fn a_complete_successor_epoch_is_accepted() {
        let profile = two_axis_profile();
        let frame = complete_frame(&profile, Epoch::new(8));
        let accepted = validate_feedback(&profile, Some(Epoch::new(7)), &frame)
            .expect("complete successor epoch is valid");
        assert_eq!(accepted.epoch(), Epoch::new(8));
    }

    #[test]
    fn the_first_epoch_needs_no_predecessor() {
        let profile = two_axis_profile();
        let frame = complete_frame(&profile, Epoch::new(999));
        assert!(validate_feedback(&profile, None, &frame).is_ok());
    }

    #[test]
    fn acceptance_holds_at_the_wrap_edge() {
        let profile = two_axis_profile();
        let frame = complete_frame(&profile, Epoch::new(0));
        assert!(validate_feedback(&profile, Some(Epoch::new(u32::MAX)), &frame).is_ok());
    }

    // --- feedback rejection arms --------------------------------------------

    #[test]
    fn a_missing_mandatory_bit_rejects_the_whole_epoch() {
        let profile = two_axis_profile();
        let mut frame = FeedbackFrame::new(GROUP, Epoch::new(8), MotionTime::new(1_000));
        // Only three of the four mandatory channels report.
        for index in [0u8, 1, 3] {
            frame.place(valid_sample(&profile, feedback(index)));
        }
        assert_eq!(
            validate_feedback(&profile, Some(Epoch::new(7)), &frame),
            Err(FeedbackRejection::MissingMandatoryFeedback(feedback(2)))
        );
    }

    #[test]
    fn every_non_valid_quality_on_a_mandatory_bit_rejects_the_epoch() {
        let profile = two_axis_profile();
        for quality in [
            FeedbackQuality::Stale,
            FeedbackQuality::Missing,
            FeedbackQuality::Discontinuous,
            FeedbackQuality::DeviceFault,
            FeedbackQuality::TransportInvalid,
            FeedbackQuality::IdentityMismatch,
        ] {
            let mut frame = complete_frame(&profile, Epoch::new(8));
            let mut sample = valid_sample(&profile, feedback(1));
            sample.quality = quality;
            frame.place(sample);
            assert_eq!(
                validate_feedback(&profile, Some(Epoch::new(7)), &frame),
                Err(FeedbackRejection::InvalidQuality { feedback: feedback(1), quality }),
                "quality {quality:?} must reject the whole epoch"
            );
        }
    }

    #[test]
    fn a_repeated_epoch_is_rejected_as_repeated_not_out_of_order() {
        let profile = two_axis_profile();
        let frame = complete_frame(&profile, Epoch::new(7));
        assert_eq!(
            validate_feedback(&profile, Some(Epoch::new(7)), &frame),
            Err(FeedbackRejection::RepeatedEpoch)
        );
    }

    #[test]
    fn a_skipped_or_past_epoch_is_rejected_as_out_of_order() {
        let profile = two_axis_profile();
        let skipped = complete_frame(&profile, Epoch::new(9));
        assert_eq!(
            validate_feedback(&profile, Some(Epoch::new(7)), &skipped),
            Err(FeedbackRejection::OutOfOrderEpoch)
        );
        let past = complete_frame(&profile, Epoch::new(6));
        assert_eq!(
            validate_feedback(&profile, Some(Epoch::new(7)), &past),
            Err(FeedbackRejection::OutOfOrderEpoch)
        );
    }

    #[test]
    fn a_wrong_group_is_rejected_before_anything_else_is_believed() {
        let profile = two_axis_profile();
        let mut frame = complete_frame(&profile, Epoch::new(8));
        frame.group = MotionGroupId::new(2);
        assert_eq!(
            validate_feedback(&profile, Some(Epoch::new(7)), &frame),
            Err(FeedbackRejection::WrongGroup)
        );
    }

    #[test]
    fn a_sample_reporting_for_the_wrong_axis_is_an_identity_mismatch() {
        let profile = two_axis_profile();
        let mut frame = complete_frame(&profile, Epoch::new(8));
        let mut sample = valid_sample(&profile, feedback(2));
        // Profile binds feedback 2 to axis 1; the frame claims axis 0.
        sample.owner = axis_owner(0, AxisFeedbackRole::MotorPosition);
        frame.place(sample);
        assert_eq!(
            validate_feedback(&profile, Some(Epoch::new(7)), &frame),
            Err(FeedbackRejection::IdentityMismatch(feedback(2)))
        );
    }

    // --- typed ownership: the R4 kill rule as a positive control -------------

    #[test]
    fn a_probe_channel_cast_into_an_axis_rejects_the_whole_epoch() {
        let profile = hexapod_profile();
        let mut frame = complete_frame(&profile, Epoch::new(8));
        // The forbidden cast: probe data presented as axis-owned feedback.
        let mut sample = valid_sample(&profile, feedback(6));
        sample.owner = axis_owner(0, AxisFeedbackRole::LoadPosition);
        frame.place(sample);
        assert_eq!(
            validate_feedback(&profile, Some(Epoch::new(7)), &frame),
            Err(FeedbackRejection::IdentityMismatch(feedback(6)))
        );
    }

    #[test]
    fn a_group_channel_cast_into_an_axis_rejects_the_whole_epoch() {
        let profile = hexapod_profile();
        let mut frame = complete_frame(&profile, Epoch::new(8));
        let mut sample = valid_sample(&profile, feedback(7));
        sample.owner = axis_owner(2, AxisFeedbackRole::Velocity);
        frame.place(sample);
        assert_eq!(
            validate_feedback(&profile, Some(Epoch::new(7)), &frame),
            Err(FeedbackRejection::IdentityMismatch(feedback(7)))
        );
    }

    #[test]
    fn a_wrong_effector_or_wrong_group_role_is_an_identity_mismatch() {
        let profile = hexapod_profile();
        // Right ownership kind, wrong effector identity.
        let mut frame = complete_frame(&profile, Epoch::new(8));
        let mut sample = valid_sample(&profile, feedback(6));
        sample.owner = FeedbackOwner::EndEffector {
            effector: EffectorId::new(1).expect("in range"),
            role: EffectorFeedbackRole::ProbeDeflection,
        };
        frame.place(sample);
        assert_eq!(
            validate_feedback(&profile, Some(Epoch::new(7)), &frame),
            Err(FeedbackRejection::IdentityMismatch(feedback(6)))
        );
        // Right ownership kind, wrong group role.
        let mut frame = complete_frame(&profile, Epoch::new(8));
        let mut sample = valid_sample(&profile, feedback(7));
        sample.owner = FeedbackOwner::Group { role: GroupFeedbackRole::Environment };
        frame.place(sample);
        assert_eq!(
            validate_feedback(&profile, Some(Epoch::new(7)), &frame),
            Err(FeedbackRejection::IdentityMismatch(feedback(7)))
        );
    }

    // --- the Hexapod sensor set shares one epoch -----------------------------

    #[test]
    fn the_full_hexapod_sensor_set_is_accepted_in_one_epoch() {
        let profile = hexapod_profile();
        let frame = complete_frame(&profile, Epoch::new(8));
        let accepted = validate_feedback(&profile, Some(Epoch::new(7)), &frame)
            .expect("axes, probe and metrology validate as one epoch");
        assert_eq!(accepted.epoch(), Epoch::new(8));
    }

    #[test]
    fn a_missing_probe_bit_rejects_the_epoch_like_a_missing_axis_channel() {
        let profile = hexapod_profile();
        let mut frame = FeedbackFrame::new(GROUP, Epoch::new(8), MotionTime::new(1_000));
        for index in 0..6u8 {
            frame.place(valid_sample(&profile, feedback(index)));
        }
        frame.place(valid_sample(&profile, feedback(7)));
        // Everything reports except the probe: the epoch must not validate.
        assert_eq!(
            validate_feedback(&profile, Some(Epoch::new(7)), &frame),
            Err(FeedbackRejection::MissingMandatoryFeedback(feedback(6)))
        );
    }

    #[test]
    fn a_stale_probe_sample_rejects_the_epoch() {
        let profile = hexapod_profile();
        let mut frame = complete_frame(&profile, Epoch::new(8));
        let mut sample = valid_sample(&profile, feedback(6));
        sample.quality = FeedbackQuality::Stale;
        frame.place(sample);
        assert_eq!(
            validate_feedback(&profile, Some(Epoch::new(7)), &frame),
            Err(FeedbackRejection::InvalidQuality {
                feedback: feedback(6),
                quality: FeedbackQuality::Stale
            })
        );
    }

    #[test]
    fn a_slot_carrying_a_foreign_feedback_id_is_an_identity_mismatch() {
        let profile = two_axis_profile();
        let mut frame = complete_frame(&profile, Epoch::new(8));
        // Mask bit 3 is set but the slot's sample claims to be channel 0.
        frame.samples[3].feedback_id = feedback(0);
        assert_eq!(
            validate_feedback(&profile, Some(Epoch::new(7)), &frame),
            Err(FeedbackRejection::IdentityMismatch(feedback(3)))
        );
    }

    #[test]
    fn a_rejection_leaves_the_order_baseline_untouched() {
        let profile = two_axis_profile();
        let last = Some(Epoch::new(7));
        let bad = complete_frame(&profile, Epoch::new(9));
        assert!(validate_feedback(&profile, last, &bad).is_err());
        // The same baseline still accepts the true successor: rejecting the
        // bad frame changed nothing.
        let good = complete_frame(&profile, Epoch::new(8));
        assert!(validate_feedback(&profile, last, &good).is_ok());
    }

    // --- actuation staging validation ---------------------------------------

    #[test]
    fn a_complete_actuation_frame_passes() {
        let profile = two_axis_profile();
        assert_eq!(validate_actuation(&profile, &complete_actuation(Epoch::new(8))), Ok(()));
    }

    #[test]
    fn a_missing_mandatory_axis_refuses_the_whole_frame() {
        let profile = two_axis_profile();
        let mut frame = ActuationFrame::new(GROUP, Epoch::new(8));
        let complete = complete_actuation(Epoch::new(8));
        frame.place(complete.commands[0]); // axis 1 absent
        assert_eq!(
            validate_actuation(&profile, &frame),
            Err(StagingRejection::MissingMandatoryAxis(axis(1)))
        );
    }

    #[test]
    fn a_masked_slot_carrying_a_foreign_axis_is_an_identity_mismatch() {
        let profile = two_axis_profile();
        let mut frame = complete_actuation(Epoch::new(8));
        frame.commands[1].axis_id = axis(0);
        assert_eq!(
            validate_actuation(&profile, &frame),
            Err(StagingRejection::CommandIdentityMismatch(axis(1)))
        );
    }

    #[test]
    fn an_apply_epoch_that_is_not_the_successor_is_refused() {
        let profile = two_axis_profile();
        let mut frame = complete_actuation(Epoch::new(8));
        frame.apply_epoch = Epoch::new(8); // must be 9
        assert_eq!(
            validate_actuation(&profile, &frame),
            Err(StagingRejection::NonSuccessorApplyEpoch)
        );
    }

    #[test]
    fn a_wrong_group_actuation_frame_is_refused() {
        let profile = two_axis_profile();
        let mut frame = complete_actuation(Epoch::new(8));
        frame.group = MotionGroupId::new(9);
        assert_eq!(validate_actuation(&profile, &frame), Err(StagingRejection::WrongGroup));
    }
}
