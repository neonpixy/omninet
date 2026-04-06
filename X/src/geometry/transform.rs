//! User-friendly wrapper around Matrix3 for common 2D transform operations.

use serde::{Deserialize, Serialize};
use std::fmt;

use super::matrix3::Matrix3;
use super::point::Point;
use super::rect::Rect;

/// A user-friendly 2D affine transform backed by a Matrix3.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Transform {
    matrix: Matrix3,
}

impl Transform {
    /// Creates the identity transform (no change).
    pub fn identity() -> Self {
        Self {
            matrix: Matrix3::identity(),
        }
    }

    /// Creates a transform from an existing matrix.
    pub fn from_matrix(matrix: Matrix3) -> Self {
        Self { matrix }
    }

    /// Creates a translation transform.
    pub fn translate(x: f64, y: f64) -> Self {
        Self {
            matrix: Matrix3::translation(x, y),
        }
    }

    /// Creates a rotation transform (angle in radians).
    pub fn rotate(angle: f64) -> Self {
        Self {
            matrix: Matrix3::rotation(angle),
        }
    }

    /// Creates a uniform scale transform.
    pub fn scale(factor: f64) -> Self {
        Self {
            matrix: Matrix3::scale_uniform(factor),
        }
    }

    /// Creates a non-uniform scale transform.
    pub fn scale_xy(x: f64, y: f64) -> Self {
        Self {
            matrix: Matrix3::scale(x, y),
        }
    }

    /// Concatenates this transform with another (applies `other` after `self`).
    pub fn concatenate(self, other: Transform) -> Self {
        Self {
            matrix: other.matrix * self.matrix,
        }
    }

    /// Returns the inverse transform, or `None` if not invertible.
    pub fn inverse(self) -> Option<Self> {
        self.matrix.inverse().map(|m| Self { matrix: m })
    }

    /// Applies this transform to a point.
    pub fn apply_to_point(self, point: Point) -> Point {
        self.matrix.transform_point(point)
    }

    /// Applies this transform to a rectangle (transforms all 4 corners, returns bounding box).
    pub fn apply_to_rect(self, rect: Rect) -> Rect {
        let corners = [
            rect.top_left(),
            rect.top_right(),
            rect.bottom_right(),
            rect.bottom_left(),
        ];

        let transformed: Vec<Point> = corners
            .iter()
            .map(|c| self.matrix.transform_point(*c))
            .collect();

        Rect::bounding_box_points(&transformed).unwrap_or(Rect::ZERO)
    }

    /// Returns a reference to the underlying matrix.
    pub fn matrix(&self) -> &Matrix3 {
        &self.matrix
    }
}

impl fmt::Display for Transform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Transform({})", self.matrix)
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
        let t = Transform::identity();
        let p = t.apply_to_point(Point::new(3.0, 4.0));
        assert!((p.x - 3.0).abs() < EPSILON);
        assert!((p.y - 4.0).abs() < EPSILON);
    }

    #[test]
    fn test_translate() {
        let t = Transform::translate(5.0, 10.0);
        let p = t.apply_to_point(Point::ZERO);
        assert!((p.x - 5.0).abs() < EPSILON);
        assert!((p.y - 10.0).abs() < EPSILON);
    }

    #[test]
    fn test_rotate() {
        let t = Transform::rotate(PI / 2.0);
        let p = t.apply_to_point(Point::new(1.0, 0.0));
        assert!(p.x.abs() < EPSILON);
        assert!((p.y - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_scale() {
        let t = Transform::scale(2.0);
        let p = t.apply_to_point(Point::new(3.0, 4.0));
        assert!((p.x - 6.0).abs() < EPSILON);
        assert!((p.y - 8.0).abs() < EPSILON);
    }

    #[test]
    fn test_scale_xy() {
        let t = Transform::scale_xy(2.0, 3.0);
        let p = t.apply_to_point(Point::new(1.0, 1.0));
        assert!((p.x - 2.0).abs() < EPSILON);
        assert!((p.y - 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_concatenate() {
        let t1 = Transform::translate(5.0, 0.0);
        let t2 = Transform::scale(2.0);
        // t1 then t2: point (1,0) -> (6,0) -> (12,0)
        let combined = t1.concatenate(t2);
        let p = combined.apply_to_point(Point::new(1.0, 0.0));
        assert!((p.x - 12.0).abs() < EPSILON);
        assert!(p.y.abs() < EPSILON);
    }

    #[test]
    fn test_inverse() {
        let t = Transform::translate(5.0, 10.0);
        let inv = t.inverse().unwrap();
        let p = inv.apply_to_point(Point::new(5.0, 10.0));
        assert!(p.x.abs() < EPSILON);
        assert!(p.y.abs() < EPSILON);
    }

    #[test]
    fn test_apply_to_rect() {
        let t = Transform::translate(10.0, 20.0);
        let r = Rect::new(0.0, 0.0, 4.0, 6.0);
        let transformed = t.apply_to_rect(r);
        assert!((transformed.x - 10.0).abs() < EPSILON);
        assert!((transformed.y - 20.0).abs() < EPSILON);
        assert!((transformed.width - 4.0).abs() < EPSILON);
        assert!((transformed.height - 6.0).abs() < EPSILON);
    }

    #[test]
    fn test_apply_to_rect_with_rotation() {
        // A 2x2 rect centered at origin, rotated 45 degrees, should produce a bounding box
        // wider than the original.
        let t = Transform::rotate(PI / 4.0);
        let r = Rect::new(0.0, 0.0, 2.0, 2.0);
        let transformed = t.apply_to_rect(r);
        // The diagonal of a 2x2 square is 2*sqrt(2) ≈ 2.828
        let expected_side = 2.0_f64.sqrt() * 2.0;
        assert!((transformed.width - expected_side).abs() < EPSILON);
        assert!((transformed.height - expected_side).abs() < EPSILON);
    }

    #[test]
    fn test_matrix_accessor() {
        let t = Transform::translate(5.0, 10.0);
        let m = t.matrix();
        assert!((m.m20 - 5.0).abs() < EPSILON);
        assert!((m.m21 - 10.0).abs() < EPSILON);
    }

    #[test]
    fn test_from_matrix() {
        let m = Matrix3::scale(3.0, 4.0);
        let t = Transform::from_matrix(m);
        let p = t.apply_to_point(Point::new(1.0, 1.0));
        assert!((p.x - 3.0).abs() < EPSILON);
        assert!((p.y - 4.0).abs() < EPSILON);
    }

    #[test]
    fn test_serde_roundtrip() {
        let t = Transform::translate(5.0, 10.0);
        let json = serde_json::to_string(&t).unwrap();
        let t2: Transform = serde_json::from_str(&json).unwrap();
        assert_eq!(t, t2);
    }
}
