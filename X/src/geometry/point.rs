//! 2D point representing a location in space.
//!
//! Ported from Swiftlight's Point.swift to idiomatic Rust.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};

use super::vector2::Vector2;

/// A 2D point representing a location in space.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    // -- Constants --

    /// The origin point (0, 0).
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    // -- Construction --

    /// Creates a new point.
    #[inline]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Converts this point to a Vector2.
    #[inline]
    pub const fn to_vector(self) -> Vector2 {
        Vector2 { x: self.x, y: self.y }
    }

    /// Creates a point from a Vector2.
    #[inline]
    pub const fn from_vector(v: Vector2) -> Self {
        Self { x: v.x, y: v.y }
    }

    // -- Methods --

    /// Returns the distance to another point.
    #[inline]
    pub fn distance(self, other: Self) -> f64 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Returns the squared distance to another point (faster than `distance`).
    #[inline]
    pub fn distance_squared(self, other: Self) -> f64 {
        let dx = other.x - self.x;
        let dy = other.y - self.y;
        dx * dx + dy * dy
    }

    /// Returns the midpoint between this point and another.
    #[inline]
    pub fn midpoint(self, other: Self) -> Self {
        Self {
            x: (self.x + other.x) / 2.0,
            y: (self.y + other.y) / 2.0,
        }
    }

    /// Returns a point offset by the given amounts.
    #[inline]
    pub fn offset(self, dx: f64, dy: f64) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
        }
    }

    /// Returns a point offset by a vector.
    #[inline]
    pub fn offset_by(self, vector: Vector2) -> Self {
        Self {
            x: self.x + vector.x,
            y: self.y + vector.y,
        }
    }

    /// Linearly interpolates between this point and another.
    #[inline]
    pub fn lerp(self, other: Self, t: f64) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
        }
    }

    /// Returns whether this point is approximately equal to another.
    #[inline]
    pub fn is_approximately_equal(self, other: Self, tolerance: f64) -> bool {
        (self.x - other.x).abs() < tolerance && (self.y - other.y).abs() < tolerance
    }
}

// ---------------------------------------------------------------------------
// Operators: Point + Vector2 -> Point, Point - Vector2 -> Point, Point - Point -> Vector2
// ---------------------------------------------------------------------------

impl Add<Vector2> for Point {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Vector2) -> Self {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub<Vector2> for Point {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Vector2) -> Self {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Sub for Point {
    type Output = Vector2;
    #[inline]
    fn sub(self, rhs: Self) -> Vector2 {
        Vector2 {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl AddAssign<Vector2> for Point {
    #[inline]
    fn add_assign(&mut self, rhs: Vector2) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl SubAssign<Vector2> for Point {
    #[inline]
    fn sub_assign(&mut self, rhs: Vector2) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Point({}, {})", self.x, self.y)
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
        assert_eq!(Point::ZERO, Point::new(0.0, 0.0));
    }

    #[test]
    fn test_to_vector() {
        let p = Point::new(3.0, 4.0);
        let v = p.to_vector();
        assert_eq!(v, Vector2::new(3.0, 4.0));
    }

    #[test]
    fn test_from_vector() {
        let v = Vector2::new(3.0, 4.0);
        let p = Point::from_vector(v);
        assert_eq!(p, Point::new(3.0, 4.0));
    }

    #[test]
    fn test_distance() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(3.0, 4.0);
        assert!((a.distance(b) - 5.0).abs() < EPSILON);
        assert!((a.distance_squared(b) - 25.0).abs() < EPSILON);
    }

    #[test]
    fn test_midpoint() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(10.0, 20.0);
        let m = a.midpoint(b);
        assert!((m.x - 5.0).abs() < EPSILON);
        assert!((m.y - 10.0).abs() < EPSILON);
    }

    #[test]
    fn test_offset() {
        let p = Point::new(1.0, 2.0);
        let o = p.offset(3.0, 4.0);
        assert_eq!(o, Point::new(4.0, 6.0));
    }

    #[test]
    fn test_offset_by() {
        let p = Point::new(1.0, 2.0);
        let v = Vector2::new(3.0, 4.0);
        let o = p.offset_by(v);
        assert_eq!(o, Point::new(4.0, 6.0));
    }

    #[test]
    fn test_lerp() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(10.0, 20.0);
        let mid = a.lerp(b, 0.5);
        assert!((mid.x - 5.0).abs() < EPSILON);
        assert!((mid.y - 10.0).abs() < EPSILON);
    }

    #[test]
    fn test_is_approximately_equal() {
        let a = Point::new(1.0, 2.0);
        let b = Point::new(1.00005, 2.00005);
        assert!(a.is_approximately_equal(b, 0.0001));
        assert!(!a.is_approximately_equal(Point::new(1.1, 2.0), 0.0001));
    }

    #[test]
    fn test_point_plus_vector() {
        let p = Point::new(1.0, 2.0);
        let v = Vector2::new(3.0, 4.0);
        let result = p + v;
        assert_eq!(result, Point::new(4.0, 6.0));
    }

    #[test]
    fn test_point_minus_vector() {
        let p = Point::new(5.0, 7.0);
        let v = Vector2::new(3.0, 4.0);
        let result = p - v;
        assert_eq!(result, Point::new(2.0, 3.0));
    }

    #[test]
    fn test_point_minus_point() {
        let a = Point::new(5.0, 7.0);
        let b = Point::new(3.0, 4.0);
        let v: Vector2 = a - b;
        assert_eq!(v, Vector2::new(2.0, 3.0));
    }

    #[test]
    fn test_assign_operators() {
        let mut p = Point::new(1.0, 2.0);
        p += Vector2::new(3.0, 4.0);
        assert_eq!(p, Point::new(4.0, 6.0));

        p -= Vector2::new(1.0, 1.0);
        assert_eq!(p, Point::new(3.0, 5.0));
    }

    #[test]
    fn test_display() {
        let p = Point::new(1.5, 2.5);
        assert_eq!(format!("{p}"), "Point(1.5, 2.5)");
    }

    #[test]
    fn test_serde_roundtrip() {
        let p = Point::new(1.0, 2.0);
        let json = serde_json::to_string(&p).unwrap();
        let p2: Point = serde_json::from_str(&json).unwrap();
        assert_eq!(p, p2);
    }
}
