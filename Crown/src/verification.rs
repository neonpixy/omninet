use std::fmt;

use serde::{Deserialize, Serialize};

/// Identity verification level (0–4).
///
/// Levels gate trust depth and economic eligibility (UBI).
/// `is_verified()` returns true for level >= 1.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VerificationLevel(u8);

impl VerificationLevel {
    /// No verification.
    pub const NONE: Self = Self(0);
    /// Basic verification — qualifies as "verified" for UBI.
    pub const BASIC: Self = Self(1);
    /// Standard verification.
    pub const STANDARD: Self = Self(2);
    /// Enhanced verification.
    pub const ENHANCED: Self = Self(3);
    /// Fully verified.
    pub const VERIFIED: Self = Self(4);

    /// Create a new level, clamped to 0–4.
    pub fn new(level: u8) -> Self {
        Self(level.min(4))
    }

    /// The numeric level (0–4).
    pub fn level(&self) -> u8 {
        self.0
    }

    /// Whether this identity is considered verified (level >= 1).
    pub fn is_verified(&self) -> bool {
        self.0 >= 1
    }
}

impl Default for VerificationLevel {
    fn default() -> Self {
        Self::NONE
    }
}

impl fmt::Display for VerificationLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            0 => write!(f, "none"),
            1 => write!(f, "basic"),
            2 => write!(f, "standard"),
            3 => write!(f, "enhanced"),
            4 => write!(f, "verified"),
            _ => write!(f, "unknown({})", self.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verification_levels() {
        assert_eq!(VerificationLevel::NONE.level(), 0);
        assert_eq!(VerificationLevel::BASIC.level(), 1);
        assert_eq!(VerificationLevel::STANDARD.level(), 2);
        assert_eq!(VerificationLevel::ENHANCED.level(), 3);
        assert_eq!(VerificationLevel::VERIFIED.level(), 4);

        assert!(!VerificationLevel::NONE.is_verified());
        assert!(VerificationLevel::BASIC.is_verified());
        assert!(VerificationLevel::VERIFIED.is_verified());
    }

    #[test]
    fn clamped_to_max() {
        assert_eq!(VerificationLevel::new(10).level(), 4);
        assert_eq!(VerificationLevel::new(255).level(), 4);
        assert_eq!(VerificationLevel::new(0).level(), 0);
        assert_eq!(VerificationLevel::new(3).level(), 3);
    }

    #[test]
    fn ordering() {
        assert!(VerificationLevel::NONE < VerificationLevel::BASIC);
        assert!(VerificationLevel::BASIC < VerificationLevel::STANDARD);
        assert!(VerificationLevel::STANDARD < VerificationLevel::ENHANCED);
        assert!(VerificationLevel::ENHANCED < VerificationLevel::VERIFIED);
    }
}
