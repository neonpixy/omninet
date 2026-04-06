//! Edge insets for padding and margins.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Edge insets (top, right, bottom, left) for padding and margins.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct EdgeInsets {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl EdgeInsets {
    // -- Constants --

    /// Zero insets on all sides.
    pub const ZERO: Self = Self {
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
        left: 0.0,
    };

    // -- Construction --

    /// Creates insets with individual values for each edge.
    #[inline]
    pub const fn new(top: f64, right: f64, bottom: f64, left: f64) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Creates insets with the same value on all edges.
    #[inline]
    pub const fn uniform(value: f64) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Creates insets with symmetric horizontal and vertical values.
    #[inline]
    pub const fn symmetric(horizontal: f64, vertical: f64) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    // -- Properties --

    /// Total horizontal insets (left + right).
    #[inline]
    pub fn horizontal(self) -> f64 {
        self.left + self.right
    }

    /// Total vertical insets (top + bottom).
    #[inline]
    pub fn vertical(self) -> f64 {
        self.top + self.bottom
    }
}

impl fmt::Display for EdgeInsets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "EdgeInsets(top: {}, right: {}, bottom: {}, left: {})",
            self.top, self.right, self.bottom, self.left
        )
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-10;

    #[test]
    fn test_zero() {
        let z = EdgeInsets::ZERO;
        assert!((z.top - 0.0).abs() < EPSILON);
        assert!((z.right - 0.0).abs() < EPSILON);
        assert!((z.bottom - 0.0).abs() < EPSILON);
        assert!((z.left - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_new() {
        let e = EdgeInsets::new(1.0, 2.0, 3.0, 4.0);
        assert!((e.top - 1.0).abs() < EPSILON);
        assert!((e.right - 2.0).abs() < EPSILON);
        assert!((e.bottom - 3.0).abs() < EPSILON);
        assert!((e.left - 4.0).abs() < EPSILON);
    }

    #[test]
    fn test_uniform() {
        let e = EdgeInsets::uniform(5.0);
        assert!((e.top - 5.0).abs() < EPSILON);
        assert!((e.right - 5.0).abs() < EPSILON);
        assert!((e.bottom - 5.0).abs() < EPSILON);
        assert!((e.left - 5.0).abs() < EPSILON);
    }

    #[test]
    fn test_symmetric() {
        let e = EdgeInsets::symmetric(10.0, 20.0);
        assert!((e.top - 20.0).abs() < EPSILON);
        assert!((e.right - 10.0).abs() < EPSILON);
        assert!((e.bottom - 20.0).abs() < EPSILON);
        assert!((e.left - 10.0).abs() < EPSILON);
    }

    #[test]
    fn test_horizontal() {
        let e = EdgeInsets::new(1.0, 2.0, 3.0, 4.0);
        assert!((e.horizontal() - 6.0).abs() < EPSILON);
    }

    #[test]
    fn test_vertical() {
        let e = EdgeInsets::new(1.0, 2.0, 3.0, 4.0);
        assert!((e.vertical() - 4.0).abs() < EPSILON);
    }

    #[test]
    fn test_display() {
        let e = EdgeInsets::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(
            format!("{e}"),
            "EdgeInsets(top: 1, right: 2, bottom: 3, left: 4)"
        );
    }

    #[test]
    fn test_serde_roundtrip() {
        let e = EdgeInsets::new(1.0, 2.0, 3.0, 4.0);
        let json = serde_json::to_string(&e).unwrap();
        let e2: EdgeInsets = serde_json::from_str(&json).unwrap();
        assert_eq!(e, e2);
    }
}
