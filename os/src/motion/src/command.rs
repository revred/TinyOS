//! The command half of the motion boundary: per-axis commands, the
//! whole-group axis mask, and the fixed-capacity actuation frame that is the
//! *only* output shape active control may produce (`ADR 0010`) — there is no
//! per-axis write path anywhere in this crate.
//!
//! Fields are public for the same reason `feedback`'s are: a frame is data
//! crossing a boundary, and [`crate::validate::validate_actuation`] plus the
//! transport's stage/commit discipline are the gates — an actuation frame
//! with an incomplete mandatory mask never stages, and a staged frame never
//! partially commits.

use crate::ident::{AxisId, Epoch, MotionGroupId};
use crate::units::{Position, Torque, Velocity};

/// The CiA-402-shaped cyclic mode a command addresses. Position, velocity
/// and torque fields act as target, limit or feed-forward according to the
/// selected mode; their presence does not imply Ti64 owns every inner loop
/// (delivery contract §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandMode {
    /// The axis is a group member but not actively commanded this epoch.
    Inactive,
    /// Cyclic Synchronous Position — the first supported drive profile.
    CyclicSynchronousPosition,
    /// Cyclic Synchronous Velocity.
    CyclicSynchronousVelocity,
    /// Cyclic Synchronous Torque.
    CyclicSynchronousTorque,
}

/// Per-command envelope the drive-side adapter enforces against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandLimits {
    /// Velocity magnitude bound for this command.
    pub max_velocity: Velocity,
    /// Torque magnitude bound for this command.
    pub max_torque: Torque,
}

/// One axis's share of one group command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxisCommand {
    /// Which axis this commands.
    pub axis_id: AxisId,
    /// The cyclic mode the targets are interpreted under.
    pub mode: CommandMode,
    /// Target position (or feed-forward, per mode).
    pub target_position: Position,
    /// Target velocity (or feed-forward, per mode).
    pub target_velocity: Velocity,
    /// Target torque (or limit, per mode).
    pub target_torque: Torque,
    /// The envelope this command must stay inside.
    pub limits: CommandLimits,
}

impl AxisCommand {
    /// The inert placeholder occupying a slot no command has been placed
    /// into: mode [`CommandMode::Inactive`], zero targets, zero limits — a
    /// slot that could never drive an axis even if misread.
    #[must_use]
    pub const fn inert() -> Self {
        Self {
            axis_id: match AxisId::new(0) {
                Ok(id) => id,
                // Index 0 is inside every bound; the arm exists because
                // `Result::unwrap` is not const.
                Err(_) => panic!("axis index 0 is always in range"),
            },
            mode: CommandMode::Inactive,
            target_position: Position::new(0),
            target_velocity: Velocity::new(0),
            target_torque: Torque::new(0),
            limits: CommandLimits { max_velocity: Velocity::new(0), max_torque: Torque::new(0) },
        }
    }
}

/// Which axes of one actuation frame carry a placed command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxisMask(u16);

impl AxisMask {
    /// No axes.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// This mask plus one axis.
    #[must_use]
    pub const fn with(self, id: AxisId) -> Self {
        Self(self.0 | 1 << id.index())
    }

    /// Whether one axis is present.
    #[must_use]
    pub const fn contains(self, id: AxisId) -> bool {
        self.0 & 1 << id.index() != 0
    }

    /// Whether every axis of `other` is present in `self`.
    #[must_use]
    pub const fn contains_all(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether no axis is present.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// One atomic, time-tagged group command — the output of control.
///
/// `apply_epoch` is fixed to the successor of `based_on` at construction
/// (the deliberate one-epoch-delayed law, delivery contract §3) and a
/// command is never relabelled for a later epoch after missing its commit
/// point — the transport enforces that, and `validate_actuation` refuses a
/// frame whose epochs disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActuationFrame<const N: usize> {
    /// The group this command addresses.
    pub group: MotionGroupId,
    /// The feedback epoch this command was calculated from.
    pub based_on: Epoch,
    /// The epoch every selected drive latches this command at.
    pub apply_epoch: Epoch,
    /// Which axes carry a placed command.
    pub valid_mask: AxisMask,
    /// Per-axis commands, indexed by axis id; unplaced slots hold
    /// [`AxisCommand::inert`].
    pub commands: [AxisCommand; N],
}

impl<const N: usize> ActuationFrame<N> {
    /// An empty frame calculated from `based_on`, applying on its successor.
    #[must_use]
    pub const fn new(group: MotionGroupId, based_on: Epoch) -> Self {
        Self {
            group,
            based_on,
            apply_epoch: based_on.next(),
            valid_mask: AxisMask::empty(),
            commands: [AxisCommand::inert(); N],
        }
    }

    /// Records one axis's command in its slot and marks it present.
    pub fn place(&mut self, command: AxisCommand) {
        self.valid_mask = self.valid_mask.with(command.axis_id);
        self.commands[command.axis_id.index()] = command;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ident::{AxisId, Epoch, MotionGroupId, MAX_AXES};
    use crate::units::{Position, Torque, Velocity};

    fn axis(index: u8) -> AxisId {
        AxisId::new(index).expect("test index in range")
    }

    #[test]
    fn an_empty_axis_mask_contains_nothing() {
        let mask = AxisMask::empty();
        for index in 0..MAX_AXES as u8 {
            assert!(!mask.contains(axis(index)));
        }
        assert!(mask.is_empty());
    }

    #[test]
    fn an_axis_mask_contains_exactly_what_was_added() {
        let mask = AxisMask::empty().with(axis(0)).with(axis(15));
        assert!(mask.contains(axis(0)));
        assert!(mask.contains(axis(15)));
        assert!(!mask.contains(axis(7)));
    }

    #[test]
    fn axis_contains_all_is_subset_not_equality() {
        let superset = AxisMask::empty().with(axis(4)).with(axis(5));
        let subset = AxisMask::empty().with(axis(5));
        assert!(superset.contains_all(subset));
        assert!(!subset.contains_all(superset));
    }

    #[test]
    fn a_new_actuation_frame_applies_on_the_successor_epoch() {
        let frame = ActuationFrame::<MAX_AXES>::new(MotionGroupId::new(2), Epoch::new(u32::MAX));
        assert_eq!(frame.based_on, Epoch::new(u32::MAX));
        assert_eq!(frame.apply_epoch, Epoch::new(0));
        assert!(frame.valid_mask.is_empty());
    }

    #[test]
    fn placing_a_command_sets_its_slot_and_its_mask_bit() {
        let mut frame = ActuationFrame::<MAX_AXES>::new(MotionGroupId::new(2), Epoch::new(3));
        let command = AxisCommand {
            axis_id: axis(11),
            mode: CommandMode::CyclicSynchronousPosition,
            target_position: Position::new(500),
            target_velocity: Velocity::new(20),
            target_torque: Torque::new(0),
            limits: CommandLimits { max_velocity: Velocity::new(100), max_torque: Torque::new(50) },
        };
        frame.place(command);
        assert!(frame.valid_mask.contains(axis(11)));
        assert_eq!(frame.commands[11], command);
    }
}
