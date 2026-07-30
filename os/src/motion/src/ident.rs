//! Typed identity and time for the motion boundary (`TEST-P1-08-01-A` §1–2).
//!
//! Axis, feedback, group and epoch identity are distinct types on purpose:
//! raw integer indices must not cross the public motion boundary, and the
//! type system — not a runtime check the RT cycle would have to pay for —
//! is what stops a feedback channel being used as an axis.

/// Compile-time bound on controlled drive axes in one motion group.
pub const MAX_AXES: usize = 16;

/// Compile-time bound on position/velocity feedback channels in one group.
pub const MAX_FEEDBACK: usize = 32;

/// Compile-time bound on end effectors in one motion group. One for the
/// Hexapod's probe-carrying disc; a small headroom for machines that carry
/// more than one tool point, still a compile-time bound like every capacity
/// in this crate.
pub const MAX_EFFECTORS: usize = 4;

/// A constructor was handed an index outside its compile-time bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityError {
    /// The axis index was not below [`MAX_AXES`].
    AxisOutOfRange(u8),
    /// The feedback index was not below [`MAX_FEEDBACK`].
    FeedbackOutOfRange(u8),
    /// The effector index was not below [`MAX_EFFECTORS`].
    EffectorOutOfRange(u8),
}

/// Bounded identity of one motion group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionGroupId(u16);

impl MotionGroupId {
    /// Names a motion group. Group identity is opaque: it is compared, never
    /// arithmetic'd, and never used as an index.
    #[must_use]
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }
}

/// Identity of one controlled drive axis, always below [`MAX_AXES`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxisId(u8);

impl AxisId {
    /// Validates and wraps an axis index; out-of-range values are refused,
    /// never clamped.
    pub const fn new(index: u8) -> Result<Self, IdentityError> {
        if (index as usize) < MAX_AXES {
            Ok(Self(index))
        } else {
            Err(IdentityError::AxisOutOfRange(index))
        }
    }

    /// The validated slot index — the only sanctioned path back to a number.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Identity of one feedback channel, always below [`MAX_FEEDBACK`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FeedbackId(u8);

impl FeedbackId {
    /// Validates and wraps a feedback index; out-of-range values are refused,
    /// never clamped.
    pub const fn new(index: u8) -> Result<Self, IdentityError> {
        if (index as usize) < MAX_FEEDBACK {
            Ok(Self(index))
        } else {
            Err(IdentityError::FeedbackOutOfRange(index))
        }
    }

    /// The validated slot index — the only sanctioned path back to a number.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// Identity of one end effector, always below [`MAX_EFFECTORS`].
///
/// Added by `STORY-P1-08-02`: a probe or tool point is a first-class feedback
/// owner, never an alias of an axis (`R4` in the de-risking contract).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectorId(u8);

impl EffectorId {
    /// Validates and wraps an effector index; out-of-range values are
    /// refused, never clamped.
    pub const fn new(index: u8) -> Result<Self, IdentityError> {
        if (index as usize) < MAX_EFFECTORS {
            Ok(Self(index))
        } else {
            Err(IdentityError::EffectorOutOfRange(index))
        }
    }

    /// The validated index — the only sanctioned path back to a number.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

/// How one epoch relates to the previously accepted one.
///
/// The cyclic discipline accepts exactly the successor; everything else is a
/// rejection, and *which* rejection matters (`TEST-P1-08-01-A` §4c/§4d).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EpochStep {
    /// Exactly one step after the previous epoch, wrap included.
    Successor,
    /// The same epoch again.
    Repeated,
    /// Anything else — skipped ahead or fallen behind.
    Other,
}

/// An ordered, wrap-aware epoch counter.
///
/// Wrap is an explicit protocol event: the successor of the maximum value is
/// zero, decided by [`Epoch::step_from`], never by comparing raw magnitudes —
/// magnitude comparison is exactly how an old frame would appear current at
/// the wrap edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Epoch(u32);

impl Epoch {
    /// Wraps a raw protocol counter value.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// The raw counter value, for evidence records only — never for ordering.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// The next epoch, wrapping at the numeric bound.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }

    /// Classifies this epoch against the previously accepted one.
    #[must_use]
    pub const fn step_from(self, previous: Self) -> EpochStep {
        if self.0 == previous.0.wrapping_add(1) {
            EpochStep::Successor
        } else if self.0 == previous.0 {
            EpochStep::Repeated
        } else {
            EpochStep::Other
        }
    }
}

/// A value in the motion group's common time domain, in transport-defined
/// ticks. The tick's physical meaning belongs to the transport profile; this
/// crate only carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionTime(u64);

impl MotionTime {
    /// Wraps a raw tick count.
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// The raw tick count.
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_id_accepts_every_index_below_the_bound() {
        for index in 0..MAX_AXES as u8 {
            let axis = AxisId::new(index).expect("index below MAX_AXES is valid");
            assert_eq!(axis.index(), index as usize);
        }
    }

    #[test]
    fn axis_id_refuses_the_bound_and_above() {
        assert_eq!(AxisId::new(MAX_AXES as u8), Err(IdentityError::AxisOutOfRange(16)));
        assert_eq!(AxisId::new(200), Err(IdentityError::AxisOutOfRange(200)));
    }

    #[test]
    fn feedback_id_accepts_every_index_below_the_bound() {
        for index in 0..MAX_FEEDBACK as u8 {
            let feedback = FeedbackId::new(index).expect("index below MAX_FEEDBACK is valid");
            assert_eq!(feedback.index(), index as usize);
        }
    }

    #[test]
    fn feedback_id_refuses_the_bound_and_above() {
        assert_eq!(FeedbackId::new(MAX_FEEDBACK as u8), Err(IdentityError::FeedbackOutOfRange(32)));
        assert_eq!(FeedbackId::new(255), Err(IdentityError::FeedbackOutOfRange(255)));
    }

    #[test]
    fn effector_id_accepts_every_index_below_the_bound() {
        for index in 0..MAX_EFFECTORS as u8 {
            let effector = EffectorId::new(index).expect("index below MAX_EFFECTORS is valid");
            assert_eq!(effector.index(), index as usize);
        }
    }

    #[test]
    fn effector_id_refuses_the_bound_and_above() {
        assert_eq!(EffectorId::new(MAX_EFFECTORS as u8), Err(IdentityError::EffectorOutOfRange(4)));
        assert_eq!(EffectorId::new(255), Err(IdentityError::EffectorOutOfRange(255)));
    }

    #[test]
    fn epoch_successor_wraps_to_zero() {
        assert_eq!(Epoch::new(u32::MAX).next(), Epoch::new(0));
        assert_eq!(Epoch::new(41).next(), Epoch::new(42));
    }

    #[test]
    fn epoch_step_distinguishes_successor_repeated_and_other() {
        let previous = Epoch::new(7);
        assert_eq!(Epoch::new(8).step_from(previous), EpochStep::Successor);
        assert_eq!(Epoch::new(7).step_from(previous), EpochStep::Repeated);
        assert_eq!(Epoch::new(9).step_from(previous), EpochStep::Other);
        assert_eq!(Epoch::new(6).step_from(previous), EpochStep::Other);
    }

    #[test]
    fn epoch_wrap_cannot_make_an_old_frame_current() {
        // At the wrap edge the successor of MAX is 0 — and nothing else is.
        let at_wrap = Epoch::new(u32::MAX);
        assert_eq!(Epoch::new(0).step_from(at_wrap), EpochStep::Successor);
        assert_eq!(Epoch::new(u32::MAX).step_from(at_wrap), EpochStep::Repeated);
        // An ancient pre-wrap epoch is Other, never current.
        assert_eq!(Epoch::new(u32::MAX - 1).step_from(at_wrap), EpochStep::Other);
    }

    #[test]
    fn group_identity_is_comparable_not_interchangeable() {
        assert_eq!(MotionGroupId::new(3), MotionGroupId::new(3));
        assert_ne!(MotionGroupId::new(3), MotionGroupId::new(4));
    }

    #[test]
    fn motion_time_holds_its_raw_tick_value() {
        assert_eq!(MotionTime::new(123_456).raw(), 123_456);
    }
}
