//! The feedback half of the motion boundary: per-channel samples, the
//! whole-group validity mask, and the fixed-capacity feedback frame that is
//! the *only* input shape coupled control accepts (`ADR 0010`).
//!
//! Fields are public because a frame is *data crossing a trust boundary*,
//! not an object with invariants of its own: it arrives from a compromisable
//! transport, and [`crate::validate::validate_feedback`] is the single gate
//! that decides whether the whole epoch is believed. Nothing downstream of
//! that gate consumes an unvalidated frame.

use crate::ident::{AxisId, Epoch, FeedbackId, MotionGroupId, MotionTime};
use crate::units::{Position, Velocity};

/// What a feedback channel measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackRole {
    /// Motor-side position (encoder on the motor shaft).
    MotorPosition,
    /// Load-side position (linear scale or second encoder on the load).
    LoadPosition,
    /// A velocity channel.
    Velocity,
    /// An auxiliary channel the profile carries but control does not couple on.
    Auxiliary,
}

/// The transport's judgement of one sample, distinguished because every
/// non-valid state has its own disposition in the fault table (delivery
/// contract §5) and collapsing them to a boolean would erase the policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackQuality {
    /// Sampled this epoch, from the right device, in range.
    Valid,
    /// A previous epoch's value re-presented.
    Stale,
    /// No value arrived.
    Missing,
    /// The value jumped in a way the device flags as implausible.
    Discontinuous,
    /// The reporting device declared a fault.
    DeviceFault,
    /// The transport could not vouch for the bytes.
    TransportInvalid,
    /// The sample's claimed identity disagrees with the configuration.
    IdentityMismatch,
}

/// One feedback channel's contribution to one epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedbackSample {
    /// Which channel this is.
    pub feedback_id: FeedbackId,
    /// Which axis the channel reports for.
    pub axis_id: AxisId,
    /// What the channel measures.
    pub role: FeedbackRole,
    /// Measured position.
    pub position: Position,
    /// Measured velocity.
    pub velocity: Velocity,
    /// The transport's judgement of this sample.
    pub quality: FeedbackQuality,
}

impl FeedbackSample {
    /// The inert placeholder occupying a slot no channel has reported into:
    /// quality [`FeedbackQuality::Missing`], so an unfilled slot can never
    /// read as data.
    #[must_use]
    pub const fn absent() -> Self {
        Self {
            feedback_id: match FeedbackId::new(0) {
                Ok(id) => id,
                // Index 0 is inside every bound; the arm exists because
                // `Result::unwrap` is not const.
                Err(_) => panic!("feedback index 0 is always in range"),
            },
            axis_id: match AxisId::new(0) {
                Ok(id) => id,
                Err(_) => panic!("axis index 0 is always in range"),
            },
            role: FeedbackRole::Auxiliary,
            position: Position::new(0),
            velocity: Velocity::new(0),
            quality: FeedbackQuality::Missing,
        }
    }
}

/// Which feedback channels of one frame carry a reported sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedbackMask(u32);

impl FeedbackMask {
    /// No channels.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// This mask plus one channel. Builder-style so profile and test setup
    /// stay expression-shaped.
    #[must_use]
    pub const fn with(self, id: FeedbackId) -> Self {
        Self(self.0 | 1 << id.index())
    }

    /// Whether one channel is present.
    #[must_use]
    pub const fn contains(self, id: FeedbackId) -> bool {
        self.0 & 1 << id.index() != 0
    }

    /// Whether every channel of `other` is present in `self`.
    #[must_use]
    pub const fn contains_all(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether no channel is present.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// One coherent feedback epoch for a whole motion group — the input to
/// control. The frame, not an individual sample, is the unit of acceptance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedbackFrame<const N: usize> {
    /// The group this epoch belongs to.
    pub group: MotionGroupId,
    /// The epoch every sample in this frame was taken at.
    pub epoch: Epoch,
    /// When the group sampled, in the common time domain.
    pub sampled_at: MotionTime,
    /// Which channels report in this frame.
    pub valid_mask: FeedbackMask,
    /// Per-channel samples, indexed by feedback id; unreported slots hold
    /// [`FeedbackSample::absent`].
    pub samples: [FeedbackSample; N],
}

impl<const N: usize> FeedbackFrame<N> {
    /// An empty frame for one epoch: no channels report yet.
    #[must_use]
    pub const fn new(group: MotionGroupId, epoch: Epoch, sampled_at: MotionTime) -> Self {
        Self {
            group,
            epoch,
            sampled_at,
            valid_mask: FeedbackMask::empty(),
            samples: [FeedbackSample::absent(); N],
        }
    }

    /// Records one channel's sample in its slot and marks it present.
    pub fn place(&mut self, sample: FeedbackSample) {
        self.valid_mask = self.valid_mask.with(sample.feedback_id);
        self.samples[sample.feedback_id.index()] = sample;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ident::{AxisId, Epoch, FeedbackId, MotionGroupId, MotionTime, MAX_FEEDBACK};
    use crate::units::{Position, Velocity};

    fn feedback(index: u8) -> FeedbackId {
        FeedbackId::new(index).expect("test index in range")
    }

    #[test]
    fn an_empty_mask_contains_nothing() {
        let mask = FeedbackMask::empty();
        for index in 0..MAX_FEEDBACK as u8 {
            assert!(!mask.contains(feedback(index)));
        }
        assert!(mask.is_empty());
    }

    #[test]
    fn a_mask_contains_exactly_what_was_added() {
        let mask = FeedbackMask::empty().with(feedback(0)).with(feedback(31));
        assert!(mask.contains(feedback(0)));
        assert!(mask.contains(feedback(31)));
        assert!(!mask.contains(feedback(15)));
        assert!(!mask.is_empty());
    }

    #[test]
    fn contains_all_is_subset_not_equality() {
        let superset = FeedbackMask::empty().with(feedback(1)).with(feedback(2)).with(feedback(3));
        let subset = FeedbackMask::empty().with(feedback(1)).with(feedback(3));
        assert!(superset.contains_all(subset));
        assert!(!subset.contains_all(superset));
    }

    #[test]
    fn an_absent_sample_reports_missing_quality() {
        assert_eq!(FeedbackSample::absent().quality, FeedbackQuality::Missing);
    }

    #[test]
    fn placing_a_sample_sets_its_slot_and_its_mask_bit() {
        let group = MotionGroupId::new(1);
        let mut frame =
            FeedbackFrame::<MAX_FEEDBACK>::new(group, Epoch::new(5), MotionTime::new(1_000));
        assert!(frame.valid_mask.is_empty());

        let id = feedback(9);
        let sample = FeedbackSample {
            feedback_id: id,
            axis_id: AxisId::new(2).expect("in range"),
            role: FeedbackRole::LoadPosition,
            position: Position::new(77),
            velocity: Velocity::new(-3),
            quality: FeedbackQuality::Valid,
        };
        frame.place(sample);
        assert!(frame.valid_mask.contains(id));
        assert_eq!(frame.samples[9], sample);
    }
}
