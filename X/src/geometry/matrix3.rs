//! 3x3 affine transformation matrix for 2D graphics.
//!
//! Ported from Swiftlight's Matrix3.swift to idiomatic Rust.
//! Column-major storage matching Metal/CoreGraphics conventions.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Mul;

use super::point::Point;
use super::vector2::Vector2;

/// A 3x3 matrix for 2D affine transformations.
///
/// Column-major storage:
/// ```text
/// | m00  m01  m02 |   | scaleX  shearY  0 |
/// | m10  m11  m12 | = | shearX  scaleY  0 |
/// | m20  m21  m22 |   | transX  transY  1 |
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Matrix3 {
    pub m00: f64,
    pub m01: f64,
    pub m02: f64,
    pub m10: f64,
    pub m11: f64,
    pub m12: f64,
    pub m20: f64,
    pub m21: f64,
    pub m22: f64,
}

impl Matrix3 {
    // -- Construction --

    /// Creates a matrix from 9 elements in row-major order for readability.
    #[inline]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        m00: f64,
        m01: f64,
        m02: f64,
        m10: f64,
        m11: f64,
        m12: f64,
        m20: f64,
        m21: f64,
        m22: f64,
    ) -> Self {
        Self {
            m00,
            m01,
            m02,
            m10,
            m11,
            m12,
            m20,
            m21,
            m22,
        }
    }

    // -- Static constructors --

    /// The identity matrix (no transformation).
    pub fn identity() -> Self {
        Self::new(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0)
    }

    /// Creates a translation matrix.
    pub fn translation(x: f64, y: f64) -> Self {
        Self::new(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, x, y, 1.0)
    }

    /// Creates a uniform scaling matrix.
    pub fn scale_uniform(s: f64) -> Self {
        Self::new(s, 0.0, 0.0, 0.0, s, 0.0, 0.0, 0.0, 1.0)
    }

    /// Creates a non-uniform scaling matrix.
    pub fn scale(x: f64, y: f64) -> Self {
        Self::new(x, 0.0, 0.0, 0.0, y, 0.0, 0.0, 0.0, 1.0)
    }

    /// Creates a rotation matrix (angle in radians, counter-clockwise).
    pub fn rotation(angle: f64) -> Self {
        let c = angle.cos();
        let s = angle.sin();
        Self::new(c, s, 0.0, -s, c, 0.0, 0.0, 0.0, 1.0)
    }

    /// Creates a rotation matrix around a specific point.
    pub fn rotation_around(angle: f64, center: Point) -> Self {
        let t1 = Self::translation(center.x, center.y);
        let r = Self::rotation(angle);
        let t2 = Self::translation(-center.x, -center.y);
        t1 * r * t2
    }

    /// Creates a shear matrix.
    pub fn shear(x: f64, y: f64) -> Self {
        Self::new(1.0, y, 0.0, x, 1.0, 0.0, 0.0, 0.0, 1.0)
    }

    // -- Properties --

    /// The translation component.
    #[inline]
    pub fn translation_component(&self) -> Vector2 {
        Vector2::new(self.m20, self.m21)
    }

    /// The x-scale component (approximate, ignores shear).
    #[inline]
    pub fn scale_x(&self) -> f64 {
        (self.m00 * self.m00 + self.m01 * self.m01).sqrt()
    }

    /// The y-scale component (approximate, ignores shear).
    #[inline]
    pub fn scale_y(&self) -> f64 {
        (self.m10 * self.m10 + self.m11 * self.m11).sqrt()
    }

    /// The rotation angle in radians (approximate, ignores shear).
    #[inline]
    pub fn rotation_angle(&self) -> f64 {
        self.m01.atan2(self.m00)
    }

    /// The determinant of the matrix.
    #[inline]
    pub fn determinant(&self) -> f64 {
        self.m00 * (self.m11 * self.m22 - self.m12 * self.m21)
            - self.m01 * (self.m10 * self.m22 - self.m12 * self.m20)
            + self.m02 * (self.m10 * self.m21 - self.m11 * self.m20)
    }

    /// Whether the matrix is invertible.
    #[inline]
    pub fn is_invertible(&self) -> bool {
        self.determinant().abs() > f64::EPSILON
    }

    /// Whether this is the identity matrix.
    #[inline]
    pub fn is_identity(&self) -> bool {
        *self == Self::identity()
    }

    /// The inverse of the matrix, or `None` if not invertible.
    pub fn inverse(&self) -> Option<Self> {
        let det = self.determinant();
        if det.abs() <= f64::EPSILON {
            return None;
        }

        let inv_det = 1.0 / det;
        Some(Self::new(
            (self.m11 * self.m22 - self.m12 * self.m21) * inv_det,
            (self.m02 * self.m21 - self.m01 * self.m22) * inv_det,
            (self.m01 * self.m12 - self.m02 * self.m11) * inv_det,
            (self.m12 * self.m20 - self.m10 * self.m22) * inv_det,
            (self.m00 * self.m22 - self.m02 * self.m20) * inv_det,
            (self.m02 * self.m10 - self.m00 * self.m12) * inv_det,
            (self.m10 * self.m21 - self.m11 * self.m20) * inv_det,
            (self.m01 * self.m20 - self.m00 * self.m21) * inv_det,
            (self.m00 * self.m11 - self.m01 * self.m10) * inv_det,
        ))
    }

    /// The transpose of the matrix.
    pub fn transposed(&self) -> Self {
        Self::new(
            self.m00, self.m10, self.m20, self.m01, self.m11, self.m21, self.m02, self.m12,
            self.m22,
        )
    }

    // -- Transform methods --

    /// Transforms a point by this matrix (applies translation).
    pub fn transform_point(&self, point: Point) -> Point {
        let w = self.m02 * point.x + self.m12 * point.y + self.m22;
        if w.abs() <= f64::EPSILON {
            return Point::ZERO;
        }
        Point::new(
            (self.m00 * point.x + self.m10 * point.y + self.m20) / w,
            (self.m01 * point.x + self.m11 * point.y + self.m21) / w,
        )
    }

    /// Transforms a direction by this matrix (ignores translation).
    pub fn transform_direction(&self, direction: Vector2) -> Vector2 {
        Vector2::new(
            self.m00 * direction.x + self.m10 * direction.y,
            self.m01 * direction.x + self.m11 * direction.y,
        )
    }

    /// Transforms multiple points by this matrix.
    pub fn transform_points(&self, points: &[Point]) -> Vec<Point> {
        points.iter().map(|p| self.transform_point(*p)).collect()
    }

    /// Returns whether this matrix is approximately equal to another.
    pub fn is_approximately_equal(&self, other: &Self, tolerance: f64) -> bool {
        (self.m00 - other.m00).abs() < tolerance
            && (self.m01 - other.m01).abs() < tolerance
            && (self.m02 - other.m02).abs() < tolerance
            && (self.m10 - other.m10).abs() < tolerance
            && (self.m11 - other.m11).abs() < tolerance
            && (self.m12 - other.m12).abs() < tolerance
            && (self.m20 - other.m20).abs() < tolerance
            && (self.m21 - other.m21).abs() < tolerance
            && (self.m22 - other.m22).abs() < tolerance
    }

    // -- Mutation methods --

    /// Translates this matrix in place.
    pub fn translate(&mut self, x: f64, y: f64) {
        *self = *self * Self::translation(x, y);
    }

    /// Scales this matrix in place.
    pub fn scale_by(&mut self, x: f64, y: f64) {
        *self = *self * Self::scale(x, y);
    }

    /// Rotates this matrix in place.
    pub fn rotate_by(&mut self, angle: f64) {
        *self = *self * Self::rotation(angle);
    }
}

// ---------------------------------------------------------------------------
// Operators
// ---------------------------------------------------------------------------

impl Mul for Matrix3 {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self::new(
            self.m00 * rhs.m00 + self.m10 * rhs.m01 + self.m20 * rhs.m02,
            self.m01 * rhs.m00 + self.m11 * rhs.m01 + self.m21 * rhs.m02,
            self.m02 * rhs.m00 + self.m12 * rhs.m01 + self.m22 * rhs.m02,
            self.m00 * rhs.m10 + self.m10 * rhs.m11 + self.m20 * rhs.m12,
            self.m01 * rhs.m10 + self.m11 * rhs.m11 + self.m21 * rhs.m12,
            self.m02 * rhs.m10 + self.m12 * rhs.m11 + self.m22 * rhs.m12,
            self.m00 * rhs.m20 + self.m10 * rhs.m21 + self.m20 * rhs.m22,
            self.m01 * rhs.m20 + self.m11 * rhs.m21 + self.m21 * rhs.m22,
            self.m02 * rhs.m20 + self.m12 * rhs.m21 + self.m22 * rhs.m22,
        )
    }
}

impl Mul<Vector2> for Matrix3 {
    type Output = Vector2;
    /// Shorthand for `transform_direction`.
    fn mul(self, rhs: Vector2) -> Vector2 {
        self.transform_direction(rhs)
    }
}

impl Mul<Point> for Matrix3 {
    type Output = Point;
    /// Shorthand for `transform_point`.
    fn mul(self, rhs: Point) -> Point {
        self.transform_point(rhs)
    }
}

impl fmt::Display for Matrix3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Matrix3(\n  [{}, {}, {}],\n  [{}, {}, {}],\n  [{}, {}, {}]\n)",
            self.m00, self.m01, self.m02, self.m10, self.m11, self.m12, self.m20, self.m21,
            self.m22,
        )
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    const EPSILON: f64 = 1e-9;

    #[test]
    fn test_identity() {
        let id = Matrix3::identity();
        assert!(id.is_identity());
        assert!((id.determinant() - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_translation() {
        let m = Matrix3::translation(5.0, 10.0);
        let p = m.transform_point(Point::ZERO);
        assert!((p.x - 5.0).abs() < EPSILON);
        assert!((p.y - 10.0).abs() < EPSILON);
    }

    #[test]
    fn test_translation_component() {
        let m = Matrix3::translation(5.0, 10.0);
        let t = m.translation_component();
        assert!((t.x - 5.0).abs() < EPSILON);
        assert!((t.y - 10.0).abs() < EPSILON);
    }

    #[test]
    fn test_scale_uniform() {
        let m = Matrix3::scale_uniform(2.0);
        let p = m.transform_point(Point::new(3.0, 4.0));
        assert!((p.x - 6.0).abs() < EPSILON);
        assert!((p.y - 8.0).abs() < EPSILON);
    }

    #[test]
    fn test_scale_non_uniform() {
        let m = Matrix3::scale(2.0, 3.0);
        let p = m.transform_point(Point::new(4.0, 5.0));
        assert!((p.x - 8.0).abs() < EPSILON);
        assert!((p.y - 15.0).abs() < EPSILON);
    }

    #[test]
    fn test_scale_properties() {
        let m = Matrix3::scale(2.0, 3.0);
        assert!((m.scale_x() - 2.0).abs() < EPSILON);
        assert!((m.scale_y() - 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_rotation() {
        let m = Matrix3::rotation(PI / 2.0);
        let p = m.transform_point(Point::new(1.0, 0.0));
        assert!(p.x.abs() < EPSILON);
        assert!((p.y - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_rotation_angle() {
        let m = Matrix3::rotation(PI / 4.0);
        assert!((m.rotation_angle() - PI / 4.0).abs() < EPSILON);
    }

    #[test]
    fn test_rotation_around() {
        let center = Point::new(5.0, 5.0);
        let m = Matrix3::rotation_around(PI / 2.0, center);
        // Point at (6,5) is 1 unit right of center → should go to (5,6) after 90° CCW
        let p = m.transform_point(Point::new(6.0, 5.0));
        assert!((p.x - 5.0).abs() < EPSILON);
        assert!((p.y - 6.0).abs() < EPSILON);
    }

    #[test]
    fn test_shear() {
        let m = Matrix3::shear(1.0, 0.0);
        let p = m.transform_point(Point::new(0.0, 1.0));
        assert!((p.x - 1.0).abs() < EPSILON);
        assert!((p.y - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_determinant() {
        let id = Matrix3::identity();
        assert!((id.determinant() - 1.0).abs() < EPSILON);

        let s = Matrix3::scale(2.0, 3.0);
        assert!((s.determinant() - 6.0).abs() < EPSILON);
    }

    #[test]
    fn test_is_invertible() {
        assert!(Matrix3::identity().is_invertible());
        // Zero matrix is not invertible
        let zero = Matrix3::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        assert!(!zero.is_invertible());
    }

    #[test]
    fn test_inverse() {
        let m = Matrix3::translation(5.0, 10.0) * Matrix3::scale(2.0, 3.0);
        let inv = m.inverse().unwrap();
        let product = m * inv;
        assert!(product.is_approximately_equal(&Matrix3::identity(), EPSILON));
    }

    #[test]
    fn test_inverse_none() {
        let zero = Matrix3::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        assert!(zero.inverse().is_none());
    }

    #[test]
    fn test_transposed() {
        let m = Matrix3::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0);
        let t = m.transposed();
        assert!((t.m00 - 1.0).abs() < EPSILON);
        assert!((t.m01 - 4.0).abs() < EPSILON);
        assert!((t.m02 - 7.0).abs() < EPSILON);
        assert!((t.m10 - 2.0).abs() < EPSILON);
        assert!((t.m11 - 5.0).abs() < EPSILON);
        assert!((t.m12 - 8.0).abs() < EPSILON);
        assert!((t.m20 - 3.0).abs() < EPSILON);
        assert!((t.m21 - 6.0).abs() < EPSILON);
        assert!((t.m22 - 9.0).abs() < EPSILON);
    }

    #[test]
    fn test_transform_direction_ignores_translation() {
        let m = Matrix3::translation(100.0, 200.0);
        let d = m.transform_direction(Vector2::new(1.0, 0.0));
        assert!((d.x - 1.0).abs() < EPSILON);
        assert!(d.y.abs() < EPSILON);
    }

    #[test]
    fn test_transform_points() {
        let m = Matrix3::translation(1.0, 2.0);
        let points = vec![Point::new(0.0, 0.0), Point::new(1.0, 1.0)];
        let transformed = m.transform_points(&points);
        assert!((transformed[0].x - 1.0).abs() < EPSILON);
        assert!((transformed[0].y - 2.0).abs() < EPSILON);
        assert!((transformed[1].x - 2.0).abs() < EPSILON);
        assert!((transformed[1].y - 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_is_approximately_equal() {
        let a = Matrix3::identity();
        let mut b = Matrix3::identity();
        b.m00 += 1e-11;
        assert!(a.is_approximately_equal(&b, 1e-10));
    }

    #[test]
    fn test_mutating_translate() {
        let mut m = Matrix3::identity();
        m.translate(5.0, 10.0);
        let p = m.transform_point(Point::ZERO);
        assert!((p.x - 5.0).abs() < EPSILON);
        assert!((p.y - 10.0).abs() < EPSILON);
    }

    #[test]
    fn test_mutating_scale() {
        let mut m = Matrix3::identity();
        m.scale_by(2.0, 3.0);
        let p = m.transform_point(Point::new(1.0, 1.0));
        assert!((p.x - 2.0).abs() < EPSILON);
        assert!((p.y - 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_mutating_rotate() {
        let mut m = Matrix3::identity();
        m.rotate_by(PI / 2.0);
        let p = m.transform_point(Point::new(1.0, 0.0));
        assert!(p.x.abs() < EPSILON);
        assert!((p.y - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_mul_matrix() {
        let t = Matrix3::translation(5.0, 10.0);
        let s = Matrix3::scale(2.0, 2.0);
        // Scale then translate: point (1,1) -> (2,2) -> (7,12)
        let m = t * s;
        let p = m.transform_point(Point::new(1.0, 1.0));
        assert!((p.x - 7.0).abs() < EPSILON);
        assert!((p.y - 12.0).abs() < EPSILON);
    }

    #[test]
    fn test_mul_vector2() {
        let m = Matrix3::scale(2.0, 3.0);
        let v = m * Vector2::new(1.0, 1.0);
        assert!((v.x - 2.0).abs() < EPSILON);
        assert!((v.y - 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_mul_point() {
        let m = Matrix3::translation(5.0, 10.0);
        let p = m * Point::new(1.0, 2.0);
        assert!((p.x - 6.0).abs() < EPSILON);
        assert!((p.y - 12.0).abs() < EPSILON);
    }

    #[test]
    fn test_serde_roundtrip() {
        let m = Matrix3::rotation(0.5) * Matrix3::translation(3.0, 4.0);
        let json = serde_json::to_string(&m).unwrap();
        let m2: Matrix3 = serde_json::from_str(&json).unwrap();
        assert!(m.is_approximately_equal(&m2, EPSILON));
    }
}
