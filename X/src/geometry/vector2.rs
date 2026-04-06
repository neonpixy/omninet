//! 2D vector representing direction and magnitude.
//!
//! Ported from Swiftlight's Vector2.swift to idiomatic Rust.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// A 2D vector representing direction and magnitude.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Vector2 {
    pub x: f64,
    pub y: f64,
}

impl Vector2 {
    // -- Constants --

    /// The zero vector (0, 0).
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// The one vector (1, 1).
    pub const ONE: Self = Self { x: 1.0, y: 1.0 };

    /// Up direction (0, -1) — screen coordinates.
    pub const UP: Self = Self { x: 0.0, y: -1.0 };

    /// Down direction (0, 1) — screen coordinates.
    pub const DOWN: Self = Self { x: 0.0, y: 1.0 };

    /// Left direction (-1, 0).
    pub const LEFT: Self = Self { x: -1.0, y: 0.0 };

    /// Right direction (1, 0).
    pub const RIGHT: Self = Self { x: 1.0, y: 0.0 };

    // -- Construction --

    /// Creates a new vector.
    #[inline]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Creates a vector from an angle (radians) and magnitude.
    #[inline]
    pub fn from_angle(angle: f64, magnitude: f64) -> Self {
        Self {
            x: angle.cos() * magnitude,
            y: angle.sin() * magnitude,
        }
    }

    // -- Properties --

    /// The length (magnitude) of the vector.
    #[inline]
    pub fn magnitude(self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }

    /// The squared length of the vector (faster than `magnitude`).
    #[inline]
    pub fn magnitude_squared(self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    /// Returns a unit vector pointing in the same direction, or `ZERO` if degenerate.
    #[inline]
    pub fn normalized(self) -> Self {
        let mag = self.magnitude();
        if mag < f64::EPSILON {
            return Self::ZERO;
        }
        Self {
            x: self.x / mag,
            y: self.y / mag,
        }
    }

    /// The angle in radians from the positive x-axis.
    #[inline]
    pub fn angle(self) -> f64 {
        self.y.atan2(self.x)
    }

    /// A perpendicular vector (rotated 90 degrees counter-clockwise).
    #[inline]
    pub fn perpendicular(self) -> Self {
        Self {
            x: -self.y,
            y: self.x,
        }
    }

    /// Whether this is effectively a zero vector.
    #[inline]
    pub fn is_zero(self) -> bool {
        self.magnitude_squared() < f64::EPSILON
    }

    // -- Methods --

    /// Returns this vector rotated by the given angle in radians.
    #[inline]
    pub fn rotated(self, angle: f64) -> Self {
        let c = angle.cos();
        let s = angle.sin();
        Self {
            x: self.x * c - self.y * s,
            y: self.x * s + self.y * c,
        }
    }

    /// Returns the dot product with another vector.
    #[inline]
    pub fn dot(self, other: Self) -> f64 {
        self.x * other.x + self.y * other.y
    }

    /// Returns the cross product (z-component of 3D cross product).
    #[inline]
    pub fn cross(self, other: Self) -> f64 {
        self.x * other.y - self.y * other.x
    }

    /// Returns the distance to another vector.
    #[inline]
    pub fn distance(self, other: Self) -> f64 {
        (self - other).magnitude()
    }

    /// Returns the squared distance to another vector (faster than `distance`).
    #[inline]
    pub fn distance_squared(self, other: Self) -> f64 {
        (self - other).magnitude_squared()
    }

    /// Returns the angle to another vector in radians.
    #[inline]
    pub fn angle_to(self, other: Self) -> f64 {
        (other.y - self.y).atan2(other.x - self.x)
    }

    /// Returns the projection of this vector onto another.
    #[inline]
    pub fn projected(self, onto: Self) -> Self {
        let dot = self.dot(onto);
        let mag_sq = onto.magnitude_squared();
        if mag_sq < f64::EPSILON {
            return Self::ZERO;
        }
        onto * (dot / mag_sq)
    }

    /// Returns the reflection of this vector off a surface with the given normal.
    #[inline]
    pub fn reflected(self, normal: Self) -> Self {
        self - normal * (2.0 * self.dot(normal))
    }

    /// Linearly interpolates between this vector and another.
    #[inline]
    pub fn lerp(self, other: Self, t: f64) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
        }
    }

    /// Returns whether this vector is approximately equal to another.
    #[inline]
    pub fn is_approximately_equal(self, other: Self, tolerance: f64) -> bool {
        (self.x - other.x).abs() < tolerance && (self.y - other.y).abs() < tolerance
    }
}

// ---------------------------------------------------------------------------
// Operators
// ---------------------------------------------------------------------------

impl Add for Vector2 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Vector2 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Mul<f64> for Vector2 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: f64) -> Self {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl Mul<Vector2> for f64 {
    type Output = Vector2;
    #[inline]
    fn mul(self, rhs: Vector2) -> Vector2 {
        Vector2 {
            x: self * rhs.x,
            y: self * rhs.y,
        }
    }
}

impl Div<f64> for Vector2 {
    type Output = Self;
    #[inline]
    fn div(self, rhs: f64) -> Self {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}

impl Neg for Vector2 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self {
        Self {
            x: -self.x,
            y: -self.y,
        }
    }
}

impl AddAssign for Vector2 {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl SubAssign for Vector2 {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl MulAssign<f64> for Vector2 {
    #[inline]
    fn mul_assign(&mut self, rhs: f64) {
        self.x *= rhs;
        self.y *= rhs;
    }
}

impl DivAssign<f64> for Vector2 {
    #[inline]
    fn div_assign(&mut self, rhs: f64) {
        self.x /= rhs;
        self.y /= rhs;
    }
}

impl fmt::Display for Vector2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Vector2({}, {})", self.x, self.y)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const EPSILON: f64 = 1e-10;
    const APPROX: f64 = 1e-6;

    #[test]
    fn test_constants() {
        assert_eq!(Vector2::ZERO, Vector2::new(0.0, 0.0));
        assert_eq!(Vector2::ONE, Vector2::new(1.0, 1.0));
        assert_eq!(Vector2::UP, Vector2::new(0.0, -1.0));
        assert_eq!(Vector2::DOWN, Vector2::new(0.0, 1.0));
        assert_eq!(Vector2::LEFT, Vector2::new(-1.0, 0.0));
        assert_eq!(Vector2::RIGHT, Vector2::new(1.0, 0.0));
    }

    #[test]
    fn test_from_angle() {
        let v = Vector2::from_angle(0.0, 1.0);
        assert!((v.x - 1.0).abs() < APPROX);
        assert!(v.y.abs() < APPROX);

        let v = Vector2::from_angle(PI / 2.0, 2.0);
        assert!(v.x.abs() < APPROX);
        assert!((v.y - 2.0).abs() < APPROX);
    }

    #[test]
    fn test_magnitude() {
        assert!((Vector2::new(3.0, 4.0).magnitude() - 5.0).abs() < EPSILON);
        assert!((Vector2::new(3.0, 4.0).magnitude_squared() - 25.0).abs() < EPSILON);
        assert!((Vector2::ZERO.magnitude() - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_normalized() {
        let v = Vector2::new(3.0, 4.0).normalized();
        assert!((v.magnitude() - 1.0).abs() < APPROX);
        assert!((v.x - 0.6).abs() < APPROX);
        assert!((v.y - 0.8).abs() < APPROX);

        // Zero vector normalizes to zero
        let v = Vector2::ZERO.normalized();
        assert_eq!(v, Vector2::ZERO);
    }

    #[test]
    fn test_angle() {
        assert!((Vector2::RIGHT.angle() - 0.0).abs() < APPROX);
        assert!((Vector2::new(0.0, 1.0).angle() - PI / 2.0).abs() < APPROX);
    }

    #[test]
    fn test_perpendicular() {
        let v = Vector2::new(1.0, 0.0).perpendicular();
        assert!((v.x - 0.0).abs() < EPSILON);
        assert!((v.y - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_is_zero() {
        assert!(Vector2::ZERO.is_zero());
        assert!(!Vector2::ONE.is_zero());
    }

    #[test]
    fn test_rotated() {
        let v = Vector2::new(1.0, 0.0).rotated(PI / 2.0);
        assert!(v.x.abs() < APPROX);
        assert!((v.y - 1.0).abs() < APPROX);
    }

    #[test]
    fn test_dot() {
        assert!((Vector2::new(1.0, 0.0).dot(Vector2::new(0.0, 1.0)) - 0.0).abs() < EPSILON);
        assert!((Vector2::new(2.0, 3.0).dot(Vector2::new(4.0, 5.0)) - 23.0).abs() < EPSILON);
    }

    #[test]
    fn test_cross() {
        assert!(
            (Vector2::new(1.0, 0.0).cross(Vector2::new(0.0, 1.0)) - 1.0).abs() < EPSILON
        );
        assert!(
            (Vector2::new(0.0, 1.0).cross(Vector2::new(1.0, 0.0)) - -1.0).abs() < EPSILON
        );
    }

    #[test]
    fn test_distance() {
        let a = Vector2::new(0.0, 0.0);
        let b = Vector2::new(3.0, 4.0);
        assert!((a.distance(b) - 5.0).abs() < EPSILON);
        assert!((a.distance_squared(b) - 25.0).abs() < EPSILON);
    }

    #[test]
    fn test_angle_to() {
        let a = Vector2::ZERO;
        let b = Vector2::new(1.0, 0.0);
        assert!(a.angle_to(b).abs() < APPROX);
    }

    #[test]
    fn test_projected() {
        let v = Vector2::new(3.0, 4.0);
        let onto = Vector2::new(1.0, 0.0);
        let p = v.projected(onto);
        assert!((p.x - 3.0).abs() < APPROX);
        assert!(p.y.abs() < APPROX);

        // Projection onto zero
        let p = v.projected(Vector2::ZERO);
        assert_eq!(p, Vector2::ZERO);
    }

    #[test]
    fn test_reflected() {
        let v = Vector2::new(1.0, -1.0);
        let normal = Vector2::new(0.0, 1.0);
        let r = v.reflected(normal);
        assert!((r.x - 1.0).abs() < APPROX);
        assert!((r.y - 1.0).abs() < APPROX);
    }

    #[test]
    fn test_lerp() {
        let a = Vector2::new(0.0, 0.0);
        let b = Vector2::new(10.0, 20.0);
        let mid = a.lerp(b, 0.5);
        assert!((mid.x - 5.0).abs() < EPSILON);
        assert!((mid.y - 10.0).abs() < EPSILON);
    }

    #[test]
    fn test_is_approximately_equal() {
        let a = Vector2::new(1.0, 2.0);
        let b = Vector2::new(1.00005, 2.00005);
        assert!(a.is_approximately_equal(b, 0.0001));
        assert!(!a.is_approximately_equal(Vector2::new(1.1, 2.0), 0.0001));
    }

    #[test]
    fn test_operators() {
        let a = Vector2::new(1.0, 2.0);
        let b = Vector2::new(3.0, 4.0);

        let sum = a + b;
        assert_eq!(sum, Vector2::new(4.0, 6.0));

        let diff = b - a;
        assert_eq!(diff, Vector2::new(2.0, 2.0));

        let scaled = a * 3.0;
        assert_eq!(scaled, Vector2::new(3.0, 6.0));

        let scaled2 = 3.0 * a;
        assert_eq!(scaled2, Vector2::new(3.0, 6.0));

        let divided = Vector2::new(6.0, 8.0) / 2.0;
        assert_eq!(divided, Vector2::new(3.0, 4.0));

        let neg = -a;
        assert_eq!(neg, Vector2::new(-1.0, -2.0));
    }

    #[test]
    fn test_assign_operators() {
        let mut v = Vector2::new(1.0, 2.0);
        v += Vector2::new(3.0, 4.0);
        assert_eq!(v, Vector2::new(4.0, 6.0));

        v -= Vector2::new(1.0, 1.0);
        assert_eq!(v, Vector2::new(3.0, 5.0));

        v *= 2.0;
        assert_eq!(v, Vector2::new(6.0, 10.0));

        v /= 2.0;
        assert_eq!(v, Vector2::new(3.0, 5.0));
    }

    #[test]
    fn test_display() {
        let v = Vector2::new(1.5, 2.5);
        assert_eq!(format!("{v}"), "Vector2(1.5, 2.5)");
    }

    #[test]
    fn test_serde_roundtrip() {
        let v = Vector2::new(1.0, 2.0);
        let json = serde_json::to_string(&v).unwrap();
        let v2: Vector2 = serde_json::from_str(&json).unwrap();
        assert_eq!(v, v2);
    }
}
