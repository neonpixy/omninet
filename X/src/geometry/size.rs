//! 2D size representing width and height.
//!
//! Ported from Swiftlight's Size.swift to idiomatic Rust.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

/// A 2D size representing width and height.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Size {
    pub width: f64,
    pub height: f64,
}

impl Size {
    // -- Constants --

    /// A zero size.
    pub const ZERO: Self = Self {
        width: 0.0,
        height: 0.0,
    };

    /// A unit size (1x1).
    pub const ONE: Self = Self {
        width: 1.0,
        height: 1.0,
    };

    // -- Construction --

    /// Creates a new size.
    #[inline]
    pub const fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }

    /// Creates a square size with equal width and height.
    #[inline]
    pub const fn square(side: f64) -> Self {
        Self {
            width: side,
            height: side,
        }
    }

    // -- Properties --

    /// The area (width * height).
    #[inline]
    pub fn area(self) -> f64 {
        self.width * self.height
    }

    /// The aspect ratio (width / height). Returns 0 if height is zero.
    #[inline]
    pub fn aspect_ratio(self) -> f64 {
        if self.height.abs() < f64::EPSILON {
            return 0.0;
        }
        self.width / self.height
    }

    /// Whether this is a zero or negative size.
    #[inline]
    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    /// Whether width equals height (within floating-point tolerance).
    #[inline]
    pub fn is_square(self) -> bool {
        (self.width - self.height).abs() < f64::EPSILON
    }

    /// Whether width is greater than height.
    #[inline]
    pub fn is_landscape(self) -> bool {
        self.width > self.height
    }

    /// Whether height is greater than width.
    #[inline]
    pub fn is_portrait(self) -> bool {
        self.height > self.width
    }

    /// The diagonal length.
    #[inline]
    pub fn diagonal(self) -> f64 {
        (self.width * self.width + self.height * self.height).sqrt()
    }

    // -- Methods --

    /// Returns a size scaled by a uniform factor.
    #[inline]
    pub fn scaled(self, factor: f64) -> Self {
        Self {
            width: self.width * factor,
            height: self.height * factor,
        }
    }

    /// Returns a size with dimensions clamped between min and max sizes.
    #[inline]
    pub fn clamped(self, min: Self, max: Self) -> Self {
        Self {
            width: self.width.clamp(min.width, max.width),
            height: self.height.clamp(min.height, max.height),
        }
    }

    /// Returns this size fit within a container, preserving aspect ratio.
    #[inline]
    pub fn fitting(self, within: Self) -> Self {
        if self.is_empty() {
            return Self::ZERO;
        }
        let width_ratio = within.width / self.width;
        let height_ratio = within.height / self.height;
        let scale = width_ratio.min(height_ratio);
        Self {
            width: self.width * scale,
            height: self.height * scale,
        }
    }

    /// Returns this size expanded to fill a target, preserving aspect ratio.
    #[inline]
    pub fn filling(self, target: Self) -> Self {
        if self.is_empty() {
            return Self::ZERO;
        }
        let width_ratio = target.width / self.width;
        let height_ratio = target.height / self.height;
        let scale = width_ratio.max(height_ratio);
        Self {
            width: self.width * scale,
            height: self.height * scale,
        }
    }

    /// Linearly interpolates between this size and another.
    #[inline]
    pub fn lerp(self, other: Self, t: f64) -> Self {
        Self {
            width: self.width + (other.width - self.width) * t,
            height: self.height + (other.height - self.height) * t,
        }
    }

    /// Returns whether this size is approximately equal to another.
    #[inline]
    pub fn is_approximately_equal(self, other: Self, tolerance: f64) -> bool {
        (self.width - other.width).abs() < tolerance
            && (self.height - other.height).abs() < tolerance
    }
}

// ---------------------------------------------------------------------------
// Operators
// ---------------------------------------------------------------------------

impl Add for Size {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            width: self.width + rhs.width,
            height: self.height + rhs.height,
        }
    }
}

impl Sub for Size {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            width: self.width - rhs.width,
            height: self.height - rhs.height,
        }
    }
}

impl Mul<f64> for Size {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: f64) -> Self {
        Self {
            width: self.width * rhs,
            height: self.height * rhs,
        }
    }
}

impl Div<f64> for Size {
    type Output = Self;
    #[inline]
    fn div(self, rhs: f64) -> Self {
        Self {
            width: self.width / rhs,
            height: self.height / rhs,
        }
    }
}

impl AddAssign for Size {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.width += rhs.width;
        self.height += rhs.height;
    }
}

impl SubAssign for Size {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.width -= rhs.width;
        self.height -= rhs.height;
    }
}

impl MulAssign<f64> for Size {
    #[inline]
    fn mul_assign(&mut self, rhs: f64) {
        self.width *= rhs;
        self.height *= rhs;
    }
}

impl DivAssign<f64> for Size {
    #[inline]
    fn div_assign(&mut self, rhs: f64) {
        self.width /= rhs;
        self.height /= rhs;
    }
}

impl fmt::Display for Size {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Size({}, {})", self.width, self.height)
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
    fn test_constants() {
        assert_eq!(Size::ZERO, Size::new(0.0, 0.0));
        assert_eq!(Size::ONE, Size::new(1.0, 1.0));
    }

    #[test]
    fn test_square() {
        let s = Size::square(5.0);
        assert_eq!(s, Size::new(5.0, 5.0));
    }

    #[test]
    fn test_area() {
        assert!((Size::new(3.0, 4.0).area() - 12.0).abs() < EPSILON);
    }

    #[test]
    fn test_aspect_ratio() {
        assert!((Size::new(16.0, 9.0).aspect_ratio() - 16.0 / 9.0).abs() < EPSILON);
        assert!((Size::new(1.0, 0.0).aspect_ratio() - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_is_empty() {
        assert!(Size::ZERO.is_empty());
        assert!(Size::new(-1.0, 5.0).is_empty());
        assert!(!Size::ONE.is_empty());
    }

    #[test]
    fn test_is_square() {
        assert!(Size::square(5.0).is_square());
        assert!(!Size::new(3.0, 4.0).is_square());
    }

    #[test]
    fn test_is_landscape_portrait() {
        assert!(Size::new(16.0, 9.0).is_landscape());
        assert!(!Size::new(16.0, 9.0).is_portrait());
        assert!(Size::new(9.0, 16.0).is_portrait());
        assert!(!Size::new(9.0, 16.0).is_landscape());
    }

    #[test]
    fn test_diagonal() {
        assert!((Size::new(3.0, 4.0).diagonal() - 5.0).abs() < EPSILON);
    }

    #[test]
    fn test_scaled() {
        let s = Size::new(3.0, 4.0).scaled(2.0);
        assert_eq!(s, Size::new(6.0, 8.0));
    }

    #[test]
    fn test_clamped() {
        let s = Size::new(100.0, 50.0);
        let c = s.clamped(Size::new(10.0, 10.0), Size::new(80.0, 80.0));
        assert_eq!(c, Size::new(80.0, 50.0));
    }

    #[test]
    fn test_fitting() {
        let s = Size::new(200.0, 100.0);
        let container = Size::new(100.0, 100.0);
        let fitted = s.fitting(container);
        assert!((fitted.width - 100.0).abs() < EPSILON);
        assert!((fitted.height - 50.0).abs() < EPSILON);
    }

    #[test]
    fn test_fitting_empty() {
        let fitted = Size::ZERO.fitting(Size::new(100.0, 100.0));
        assert_eq!(fitted, Size::ZERO);
    }

    #[test]
    fn test_filling() {
        let s = Size::new(200.0, 100.0);
        let target = Size::new(100.0, 100.0);
        let filled = s.filling(target);
        assert!((filled.width - 200.0).abs() < EPSILON);
        assert!((filled.height - 100.0).abs() < EPSILON);
    }

    #[test]
    fn test_lerp() {
        let a = Size::new(0.0, 0.0);
        let b = Size::new(10.0, 20.0);
        let mid = a.lerp(b, 0.5);
        assert!((mid.width - 5.0).abs() < EPSILON);
        assert!((mid.height - 10.0).abs() < EPSILON);
    }

    #[test]
    fn test_is_approximately_equal() {
        let a = Size::new(1.0, 2.0);
        let b = Size::new(1.00005, 2.00005);
        assert!(a.is_approximately_equal(b, 0.0001));
        assert!(!a.is_approximately_equal(Size::new(1.1, 2.0), 0.0001));
    }

    #[test]
    fn test_operators() {
        let a = Size::new(1.0, 2.0);
        let b = Size::new(3.0, 4.0);

        assert_eq!(a + b, Size::new(4.0, 6.0));
        assert_eq!(b - a, Size::new(2.0, 2.0));
        assert_eq!(a * 3.0, Size::new(3.0, 6.0));
        assert_eq!(Size::new(6.0, 8.0) / 2.0, Size::new(3.0, 4.0));
    }

    #[test]
    fn test_assign_operators() {
        let mut s = Size::new(1.0, 2.0);
        s += Size::new(3.0, 4.0);
        assert_eq!(s, Size::new(4.0, 6.0));

        s -= Size::new(1.0, 1.0);
        assert_eq!(s, Size::new(3.0, 5.0));

        s *= 2.0;
        assert_eq!(s, Size::new(6.0, 10.0));

        s /= 2.0;
        assert_eq!(s, Size::new(3.0, 5.0));
    }

    #[test]
    fn test_display() {
        let s = Size::new(1.5, 2.5);
        assert_eq!(format!("{s}"), "Size(1.5, 2.5)");
    }

    #[test]
    fn test_serde_roundtrip() {
        let s = Size::new(3.0, 4.0);
        let json = serde_json::to_string(&s).unwrap();
        let s2: Size = serde_json::from_str(&json).unwrap();
        assert_eq!(s, s2);
    }
}
