//! Scalar motion quantities as newtypes, so the type system catches a
//! velocity used as a position — misuse the RT path cannot afford to catch at
//! runtime (`agent/CODING_STANDARDS.md`, style notes).
//!
//! All three quantities are raw signed counts. Their physical scaling
//! (increments per unit, gearing, direction) is decided by the drive profile
//! at commissioning time — the delivery contract deliberately does not
//! pre-commit an ABI, and this crate does not pretend to know a unit.

/// A position, in profile-scaled counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position(i64);

impl Position {
    /// Wraps a raw position count.
    #[must_use]
    pub const fn new(raw: i64) -> Self {
        Self(raw)
    }

    /// The raw count.
    #[must_use]
    pub const fn raw(self) -> i64 {
        self.0
    }
}

/// A velocity, in profile-scaled counts per cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Velocity(i64);

impl Velocity {
    /// Wraps a raw velocity count.
    #[must_use]
    pub const fn new(raw: i64) -> Self {
        Self(raw)
    }

    /// The raw count.
    #[must_use]
    pub const fn raw(self) -> i64 {
        self.0
    }
}

/// A torque, in profile-scaled counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Torque(i64);

impl Torque {
    /// Wraps a raw torque count.
    #[must_use]
    pub const fn new(raw: i64) -> Self {
        Self(raw)
    }

    /// The raw count.
    #[must_use]
    pub const fn raw(self) -> i64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantities_hold_their_raw_counts() {
        assert_eq!(Position::new(-42).raw(), -42);
        assert_eq!(Velocity::new(7).raw(), 7);
        assert_eq!(Torque::new(0).raw(), 0);
    }
}
