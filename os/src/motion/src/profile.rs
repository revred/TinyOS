//! The declared shape of one motion group: which feedback channels are
//! mandatory, which axis each one reports for, and which axes an active
//! command frame must cover. The profile is configuration decided at
//! admission time — validation compares hostile frames *against* it; nothing
//! in the cyclic path ever mutates it.
//!
//! If a selected drive exposes only a locally fused feedback result, that
//! limitation is recorded here by simply *not* declaring the second channel —
//! the profile must never represent fused feedback as raw channels (delivery
//! contract §2).

use crate::command::AxisMask;
use crate::feedback::{FeedbackMask, FeedbackRole};
use crate::ident::{AxisId, FeedbackId, MotionGroupId, MAX_FEEDBACK};

/// Which axis and role one declared feedback channel reports for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedbackBinding {
    /// The axis the channel belongs to.
    pub axis: AxisId,
    /// What the channel measures.
    pub role: FeedbackRole,
}

/// The declared shape of one motion group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroupProfile {
    group: MotionGroupId,
    bindings: [Option<FeedbackBinding>; MAX_FEEDBACK],
    mandatory_feedback: FeedbackMask,
    mandatory_axes: AxisMask,
}

impl GroupProfile {
    /// A profile requiring nothing — bindings are declared one by one at
    /// admission time.
    #[must_use]
    pub const fn new(group: MotionGroupId) -> Self {
        Self {
            group,
            bindings: [None; MAX_FEEDBACK],
            mandatory_feedback: FeedbackMask::empty(),
            mandatory_axes: AxisMask::empty(),
        }
    }

    /// The group this profile describes.
    #[must_use]
    pub const fn group(&self) -> MotionGroupId {
        self.group
    }

    /// Declares one mandatory feedback channel and the identity it must
    /// report with. A frame missing this bit — or reporting it with a
    /// different axis or role — is invalid for active control.
    pub fn require_feedback(&mut self, id: FeedbackId, axis: AxisId, role: FeedbackRole) {
        self.bindings[id.index()] = Some(FeedbackBinding { axis, role });
        self.mandatory_feedback = self.mandatory_feedback.with(id);
    }

    /// Declares one axis an active-control command frame must cover.
    pub fn require_axis(&mut self, axis: AxisId) {
        self.mandatory_axes = self.mandatory_axes.with(axis);
    }

    /// Every channel a frame must carry to be valid for active control.
    #[must_use]
    pub const fn mandatory_feedback(&self) -> FeedbackMask {
        self.mandatory_feedback
    }

    /// Every axis an active-control command frame must cover.
    #[must_use]
    pub const fn mandatory_axes(&self) -> AxisMask {
        self.mandatory_axes
    }

    /// The declared identity of one channel, if the profile declared it.
    #[must_use]
    pub fn binding(&self, id: FeedbackId) -> Option<&FeedbackBinding> {
        self.bindings[id.index()].as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::AxisMask;
    use crate::feedback::{FeedbackMask, FeedbackRole};
    use crate::ident::{AxisId, FeedbackId, MotionGroupId};

    fn axis(index: u8) -> AxisId {
        AxisId::new(index).expect("test index in range")
    }

    fn feedback(index: u8) -> FeedbackId {
        FeedbackId::new(index).expect("test index in range")
    }

    #[test]
    fn a_new_profile_requires_nothing() {
        let profile = GroupProfile::new(MotionGroupId::new(1));
        assert_eq!(profile.group(), MotionGroupId::new(1));
        assert_eq!(profile.mandatory_feedback(), FeedbackMask::empty());
        assert_eq!(profile.mandatory_axes(), AxisMask::empty());
    }

    #[test]
    fn requiring_feedback_records_its_binding_and_its_mask_bit() {
        let mut profile = GroupProfile::new(MotionGroupId::new(1));
        profile.require_feedback(feedback(4), axis(2), FeedbackRole::MotorPosition);

        assert!(profile.mandatory_feedback().contains(feedback(4)));
        let binding = profile.binding(feedback(4)).expect("binding was declared");
        assert_eq!(binding.axis, axis(2));
        assert_eq!(binding.role, FeedbackRole::MotorPosition);
        assert!(profile.binding(feedback(5)).is_none());
    }

    #[test]
    fn requiring_an_axis_records_its_mask_bit() {
        let mut profile = GroupProfile::new(MotionGroupId::new(1));
        profile.require_axis(axis(0));
        profile.require_axis(axis(3));
        assert!(profile.mandatory_axes().contains(axis(0)));
        assert!(profile.mandatory_axes().contains(axis(3)));
        assert!(!profile.mandatory_axes().contains(axis(1)));
    }
}
