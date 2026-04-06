//! Center-origin rectangle with 9-anchor-point system.
//!
//! Ported from Swiftlight's Rect.swift with the addition of the Anchor system.

use serde::{Deserialize, Serialize};
use std::fmt;

use super::point::Point;
use super::size::Size;
use super::vector2::Vector2;

/// One of 9 anchor points on a rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Anchor {
    /// Top-left corner.
    TopLeft,
    /// Center of the top edge.
    TopCenter,
    /// Top-right corner.
    TopRight,
    /// Center of the left edge.
    MiddleLeft,
    /// The center point.
    Center,
    /// Center of the right edge.
    MiddleRight,
    /// Bottom-left corner.
    BottomLeft,
    /// Center of the bottom edge.
    BottomCenter,
    /// Bottom-right corner.
    BottomRight,
}

/// A rectangle defined by its center point and size.
///
/// Internal storage is center-based: `x` and `y` are the center coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    /// Center X position.
    pub x: f64,
    /// Center Y position.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

impl Rect {
    // -- Constants --

    /// A zero rect at the origin.
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        width: 0.0,
        height: 0.0,
    };

    // -- Constructors --

    /// Creates a rectangle from center position and size (default, center-origin).
    #[inline]
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Creates a rectangle from center point and size.
    #[inline]
    pub const fn from_center_size(center: Point, size: Size) -> Self {
        Self {
            x: center.x,
            y: center.y,
            width: size.width,
            height: size.height,
        }
    }

    /// Creates a rectangle from any of the 9 anchor points.
    ///
    /// The `x` and `y` values represent the position of the specified anchor point.
    /// Internally converts to center storage.
    pub fn from_anchor(anchor: Anchor, x: f64, y: f64, width: f64, height: f64) -> Self {
        let half_w = width / 2.0;
        let half_h = height / 2.0;

        let (cx, cy) = match anchor {
            Anchor::TopLeft => (x + half_w, y + half_h),
            Anchor::TopCenter => (x, y + half_h),
            Anchor::TopRight => (x - half_w, y + half_h),
            Anchor::MiddleLeft => (x + half_w, y),
            Anchor::Center => (x, y),
            Anchor::MiddleRight => (x - half_w, y),
            Anchor::BottomLeft => (x + half_w, y - half_h),
            Anchor::BottomCenter => (x, y - half_h),
            Anchor::BottomRight => (x - half_w, y - half_h),
        };

        Self {
            x: cx,
            y: cy,
            width,
            height,
        }
    }

    /// Creates a rectangle from two opposite corners.
    pub fn from_corners(p1: Point, p2: Point) -> Self {
        Self {
            x: (p1.x + p2.x) / 2.0,
            y: (p1.y + p2.y) / 2.0,
            width: (p2.x - p1.x).abs(),
            height: (p2.y - p1.y).abs(),
        }
    }

    // -- Edge properties --

    /// The minimum X (left edge).
    #[inline]
    pub fn min_x(self) -> f64 {
        self.x - self.width / 2.0
    }

    /// The maximum X (right edge).
    #[inline]
    pub fn max_x(self) -> f64 {
        self.x + self.width / 2.0
    }

    /// The minimum Y (top edge).
    #[inline]
    pub fn min_y(self) -> f64 {
        self.y - self.height / 2.0
    }

    /// The maximum Y (bottom edge).
    #[inline]
    pub fn max_y(self) -> f64 {
        self.y + self.height / 2.0
    }

    // -- Center and size --

    /// The center point.
    #[inline]
    pub fn center(self) -> Point {
        Point::new(self.x, self.y)
    }

    /// The size.
    #[inline]
    pub fn size(self) -> Size {
        Size::new(self.width, self.height)
    }

    // -- Corner properties --

    /// Top-left corner.
    #[inline]
    pub fn top_left(self) -> Point {
        Point::new(self.min_x(), self.min_y())
    }

    /// Top-right corner.
    #[inline]
    pub fn top_right(self) -> Point {
        Point::new(self.max_x(), self.min_y())
    }

    /// Bottom-left corner.
    #[inline]
    pub fn bottom_left(self) -> Point {
        Point::new(self.min_x(), self.max_y())
    }

    /// Bottom-right corner.
    #[inline]
    pub fn bottom_right(self) -> Point {
        Point::new(self.max_x(), self.max_y())
    }

    // -- Edge midpoints --

    /// Midpoint of top edge.
    #[inline]
    pub fn top_center(self) -> Point {
        Point::new(self.x, self.min_y())
    }

    /// Midpoint of bottom edge.
    #[inline]
    pub fn bottom_center(self) -> Point {
        Point::new(self.x, self.max_y())
    }

    /// Midpoint of left edge.
    #[inline]
    pub fn middle_left(self) -> Point {
        Point::new(self.min_x(), self.y)
    }

    /// Midpoint of right edge.
    #[inline]
    pub fn middle_right(self) -> Point {
        Point::new(self.max_x(), self.y)
    }

    // -- Computed properties --

    /// The area.
    #[inline]
    pub fn area(self) -> f64 {
        self.width * self.height
    }

    /// The perimeter.
    #[inline]
    pub fn perimeter(self) -> f64 {
        2.0 * (self.width + self.height)
    }

    /// The aspect ratio (width / height). Returns 0 if height is zero.
    #[inline]
    pub fn aspect_ratio(self) -> f64 {
        if self.height.abs() < f64::EPSILON {
            return 0.0;
        }
        self.width / self.height
    }

    /// The diagonal length.
    #[inline]
    pub fn diagonal(self) -> f64 {
        (self.width * self.width + self.height * self.height).sqrt()
    }

    /// Whether the rectangle has zero or negative area.
    #[inline]
    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    // -- Anchor point access --

    /// Returns the position of any of the 9 anchor points.
    pub fn anchor_point(self, anchor: Anchor) -> Point {
        let half_w = self.width / 2.0;
        let half_h = self.height / 2.0;

        match anchor {
            Anchor::TopLeft => Point::new(self.x - half_w, self.y - half_h),
            Anchor::TopCenter => Point::new(self.x, self.y - half_h),
            Anchor::TopRight => Point::new(self.x + half_w, self.y - half_h),
            Anchor::MiddleLeft => Point::new(self.x - half_w, self.y),
            Anchor::Center => Point::new(self.x, self.y),
            Anchor::MiddleRight => Point::new(self.x + half_w, self.y),
            Anchor::BottomLeft => Point::new(self.x - half_w, self.y + half_h),
            Anchor::BottomCenter => Point::new(self.x, self.y + half_h),
            Anchor::BottomRight => Point::new(self.x + half_w, self.y + half_h),
        }
    }

    // -- Hit testing --

    /// Returns whether this rectangle contains a point.
    #[inline]
    pub fn contains_point(self, point: Point) -> bool {
        point.x >= self.min_x()
            && point.x <= self.max_x()
            && point.y >= self.min_y()
            && point.y <= self.max_y()
    }

    /// Returns whether this rectangle fully contains another.
    #[inline]
    pub fn contains_rect(self, other: Rect) -> bool {
        other.min_x() >= self.min_x()
            && other.max_x() <= self.max_x()
            && other.min_y() >= self.min_y()
            && other.max_y() <= self.max_y()
    }

    /// Returns whether this rectangle intersects another.
    #[inline]
    pub fn intersects(self, other: Rect) -> bool {
        self.min_x() < other.max_x()
            && self.max_x() > other.min_x()
            && self.min_y() < other.max_y()
            && self.max_y() > other.min_y()
    }

    /// Returns the intersection of this rectangle with another, or `None` if disjoint.
    pub fn intersection(self, other: Rect) -> Option<Rect> {
        let new_min_x = self.min_x().max(other.min_x());
        let new_max_x = self.max_x().min(other.max_x());
        let new_min_y = self.min_y().max(other.min_y());
        let new_max_y = self.max_y().min(other.max_y());

        if new_min_x >= new_max_x || new_min_y >= new_max_y {
            return None;
        }

        Some(Rect {
            x: (new_min_x + new_max_x) / 2.0,
            y: (new_min_y + new_max_y) / 2.0,
            width: new_max_x - new_min_x,
            height: new_max_y - new_min_y,
        })
    }

    /// Returns the union (bounding box) of this rectangle with another.
    pub fn union(self, other: Rect) -> Rect {
        let new_min_x = self.min_x().min(other.min_x());
        let new_max_x = self.max_x().max(other.max_x());
        let new_min_y = self.min_y().min(other.min_y());
        let new_max_y = self.max_y().max(other.max_y());

        Rect {
            x: (new_min_x + new_max_x) / 2.0,
            y: (new_min_y + new_max_y) / 2.0,
            width: new_max_x - new_min_x,
            height: new_max_y - new_min_y,
        }
    }

    // -- Transformations --

    /// Returns a rectangle with edges inset by `amount` on all sides.
    #[inline]
    pub fn inset(self, amount: f64) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            width: self.width - 2.0 * amount,
            height: self.height - 2.0 * amount,
        }
    }

    /// Returns a rectangle with edges inset by separate horizontal/vertical amounts.
    #[inline]
    pub fn inset_xy(self, horizontal: f64, vertical: f64) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            width: self.width - 2.0 * horizontal,
            height: self.height - 2.0 * vertical,
        }
    }

    /// Returns a rectangle expanded outward by `amount` on all sides.
    #[inline]
    pub fn expanded(self, amount: f64) -> Rect {
        self.inset(-amount)
    }

    /// Returns a rectangle with position offset by the given amounts.
    #[inline]
    pub fn offset(self, dx: f64, dy: f64) -> Rect {
        Rect {
            x: self.x + dx,
            y: self.y + dy,
            width: self.width,
            height: self.height,
        }
    }

    /// Returns a rectangle with position offset by a vector.
    #[inline]
    pub fn offset_by(self, vector: Vector2) -> Rect {
        self.offset(vector.x, vector.y)
    }

    /// Returns a rectangle scaled by a uniform factor around its center.
    #[inline]
    pub fn scaled(self, factor: f64) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            width: self.width * factor,
            height: self.height * factor,
        }
    }

    /// Linearly interpolates between this rectangle and another.
    #[inline]
    pub fn lerp(self, other: Rect, t: f64) -> Rect {
        Rect {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
            width: self.width + (other.width - self.width) * t,
            height: self.height + (other.height - self.height) * t,
        }
    }

    /// Returns whether this rectangle is approximately equal to another.
    #[inline]
    pub fn is_approximately_equal(self, other: Rect, tolerance: f64) -> bool {
        (self.x - other.x).abs() < tolerance
            && (self.y - other.y).abs() < tolerance
            && (self.width - other.width).abs() < tolerance
            && (self.height - other.height).abs() < tolerance
    }

    // -- Static constructors --

    /// Creates a bounding box from a slice of points. Returns `None` if empty.
    pub fn bounding_box_points(points: &[Point]) -> Option<Rect> {
        let first = points.first()?;
        let mut min_x = first.x;
        let mut max_x = first.x;
        let mut min_y = first.y;
        let mut max_y = first.y;

        for p in points.iter().skip(1) {
            min_x = min_x.min(p.x);
            max_x = max_x.max(p.x);
            min_y = min_y.min(p.y);
            max_y = max_y.max(p.y);
        }

        Some(Rect {
            x: (min_x + max_x) / 2.0,
            y: (min_y + max_y) / 2.0,
            width: max_x - min_x,
            height: max_y - min_y,
        })
    }

    /// Creates a bounding box from a slice of rects. Returns `None` if empty.
    pub fn bounding_box_rects(rects: &[Rect]) -> Option<Rect> {
        let first = rects.first()?;
        let result = rects.iter().skip(1).fold(*first, |acc, r| acc.union(*r));
        Some(result)
    }
}

impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Rect(center: ({}, {}), size: ({}, {}))",
            self.x, self.y, self.width, self.height
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
    fn test_new_center_origin() {
        let r = Rect::new(5.0, 10.0, 20.0, 30.0);
        assert!((r.x - 5.0).abs() < EPSILON);
        assert!((r.y - 10.0).abs() < EPSILON);
        assert!((r.min_x() - -5.0).abs() < EPSILON);
        assert!((r.max_x() - 15.0).abs() < EPSILON);
        assert!((r.min_y() - -5.0).abs() < EPSILON);
        assert!((r.max_y() - 25.0).abs() < EPSILON);
    }

    #[test]
    fn test_from_anchor_center() {
        let r = Rect::from_anchor(Anchor::Center, 5.0, 10.0, 20.0, 30.0);
        assert!((r.x - 5.0).abs() < EPSILON);
        assert!((r.y - 10.0).abs() < EPSILON);
    }

    #[test]
    fn test_from_anchor_top_left() {
        let r = Rect::from_anchor(Anchor::TopLeft, 0.0, 0.0, 100.0, 80.0);
        assert!((r.x - 50.0).abs() < EPSILON);
        assert!((r.y - 40.0).abs() < EPSILON);
        assert!((r.min_x() - 0.0).abs() < EPSILON);
        assert!((r.min_y() - 0.0).abs() < EPSILON);
        assert!((r.max_x() - 100.0).abs() < EPSILON);
        assert!((r.max_y() - 80.0).abs() < EPSILON);
    }

    #[test]
    fn test_from_anchor_bottom_right() {
        let r = Rect::from_anchor(Anchor::BottomRight, 100.0, 80.0, 100.0, 80.0);
        assert!((r.x - 50.0).abs() < EPSILON);
        assert!((r.y - 40.0).abs() < EPSILON);
        assert!((r.min_x() - 0.0).abs() < EPSILON);
        assert!((r.min_y() - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_from_anchor_top_right() {
        let r = Rect::from_anchor(Anchor::TopRight, 100.0, 0.0, 100.0, 80.0);
        assert!((r.x - 50.0).abs() < EPSILON);
        assert!((r.y - 40.0).abs() < EPSILON);
    }

    #[test]
    fn test_from_anchor_bottom_left() {
        let r = Rect::from_anchor(Anchor::BottomLeft, 0.0, 80.0, 100.0, 80.0);
        assert!((r.x - 50.0).abs() < EPSILON);
        assert!((r.y - 40.0).abs() < EPSILON);
    }

    #[test]
    fn test_from_anchor_middle_left() {
        let r = Rect::from_anchor(Anchor::MiddleLeft, 0.0, 40.0, 100.0, 80.0);
        assert!((r.x - 50.0).abs() < EPSILON);
        assert!((r.y - 40.0).abs() < EPSILON);
    }

    #[test]
    fn test_from_anchor_middle_right() {
        let r = Rect::from_anchor(Anchor::MiddleRight, 100.0, 40.0, 100.0, 80.0);
        assert!((r.x - 50.0).abs() < EPSILON);
        assert!((r.y - 40.0).abs() < EPSILON);
    }

    #[test]
    fn test_from_anchor_top_center() {
        let r = Rect::from_anchor(Anchor::TopCenter, 50.0, 0.0, 100.0, 80.0);
        assert!((r.x - 50.0).abs() < EPSILON);
        assert!((r.y - 40.0).abs() < EPSILON);
    }

    #[test]
    fn test_from_anchor_bottom_center() {
        let r = Rect::from_anchor(Anchor::BottomCenter, 50.0, 80.0, 100.0, 80.0);
        assert!((r.x - 50.0).abs() < EPSILON);
        assert!((r.y - 40.0).abs() < EPSILON);
    }

    #[test]
    fn test_from_corners() {
        let r = Rect::from_corners(Point::new(0.0, 0.0), Point::new(10.0, 20.0));
        assert!((r.x - 5.0).abs() < EPSILON);
        assert!((r.y - 10.0).abs() < EPSILON);
        assert!((r.width - 10.0).abs() < EPSILON);
        assert!((r.height - 20.0).abs() < EPSILON);
    }

    #[test]
    fn test_center_and_size() {
        let r = Rect::new(5.0, 10.0, 20.0, 30.0);
        assert_eq!(r.center(), Point::new(5.0, 10.0));
        assert_eq!(r.size(), Size::new(20.0, 30.0));
    }

    #[test]
    fn test_corners() {
        let r = Rect::new(0.0, 0.0, 10.0, 8.0);
        assert!(r.top_left().is_approximately_equal(Point::new(-5.0, -4.0), EPSILON));
        assert!(r.top_right().is_approximately_equal(Point::new(5.0, -4.0), EPSILON));
        assert!(r.bottom_left().is_approximately_equal(Point::new(-5.0, 4.0), EPSILON));
        assert!(r.bottom_right().is_approximately_equal(Point::new(5.0, 4.0), EPSILON));
    }

    #[test]
    fn test_edge_midpoints() {
        let r = Rect::new(0.0, 0.0, 10.0, 8.0);
        assert!(r.top_center().is_approximately_equal(Point::new(0.0, -4.0), EPSILON));
        assert!(r.bottom_center().is_approximately_equal(Point::new(0.0, 4.0), EPSILON));
        assert!(r.middle_left().is_approximately_equal(Point::new(-5.0, 0.0), EPSILON));
        assert!(r.middle_right().is_approximately_equal(Point::new(5.0, 0.0), EPSILON));
    }

    #[test]
    fn test_computed_properties() {
        let r = Rect::new(0.0, 0.0, 10.0, 5.0);
        assert!((r.area() - 50.0).abs() < EPSILON);
        assert!((r.perimeter() - 30.0).abs() < EPSILON);
        assert!((r.aspect_ratio() - 2.0).abs() < EPSILON);
        assert!(!r.is_empty());
    }

    #[test]
    fn test_is_empty() {
        assert!(Rect::ZERO.is_empty());
        assert!(Rect::new(0.0, 0.0, -1.0, 5.0).is_empty());
        assert!(!Rect::new(0.0, 0.0, 1.0, 1.0).is_empty());
    }

    #[test]
    fn test_diagonal() {
        let r = Rect::new(0.0, 0.0, 3.0, 4.0);
        assert!((r.diagonal() - 5.0).abs() < EPSILON);
    }

    #[test]
    fn test_anchor_point() {
        let r = Rect::new(5.0, 10.0, 20.0, 30.0);
        assert!(r.anchor_point(Anchor::Center).is_approximately_equal(Point::new(5.0, 10.0), EPSILON));
        assert!(r.anchor_point(Anchor::TopLeft).is_approximately_equal(Point::new(-5.0, -5.0), EPSILON));
        assert!(r.anchor_point(Anchor::BottomRight).is_approximately_equal(Point::new(15.0, 25.0), EPSILON));
    }

    #[test]
    fn test_anchor_roundtrip() {
        // Creating from an anchor and then reading that anchor back should give the original position.
        let anchors = [
            Anchor::TopLeft, Anchor::TopCenter, Anchor::TopRight,
            Anchor::MiddleLeft, Anchor::Center, Anchor::MiddleRight,
            Anchor::BottomLeft, Anchor::BottomCenter, Anchor::BottomRight,
        ];
        for anchor in &anchors {
            let r = Rect::from_anchor(*anchor, 42.0, 77.0, 100.0, 60.0);
            let p = r.anchor_point(*anchor);
            assert!(
                p.is_approximately_equal(Point::new(42.0, 77.0), EPSILON),
                "Anchor {:?} roundtrip failed: got {:?}",
                anchor,
                p
            );
        }
    }

    #[test]
    fn test_contains_point() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(r.contains_point(Point::new(0.0, 0.0)));
        assert!(r.contains_point(Point::new(5.0, 5.0)));
        assert!(r.contains_point(Point::new(-5.0, -5.0)));
        assert!(!r.contains_point(Point::new(6.0, 0.0)));
    }

    #[test]
    fn test_contains_rect() {
        let outer = Rect::new(0.0, 0.0, 20.0, 20.0);
        let inner = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(outer.contains_rect(inner));
        assert!(!inner.contains_rect(outer));
    }

    #[test]
    fn test_intersects() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(4.0, 0.0, 10.0, 10.0);
        assert!(a.intersects(b));

        let c = Rect::new(20.0, 0.0, 10.0, 10.0);
        assert!(!a.intersects(c));
    }

    #[test]
    fn test_intersection() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(3.0, 0.0, 10.0, 10.0);
        let i = a.intersection(b).unwrap();
        // a: -5..5, b: -2..8 -> intersection: -2..5 = width 7, center 1.5
        assert!((i.width - 7.0).abs() < EPSILON);
        assert!((i.x - 1.5).abs() < EPSILON);

        let c = Rect::new(20.0, 0.0, 10.0, 10.0);
        assert!(a.intersection(c).is_none());
    }

    #[test]
    fn test_union() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(10.0, 0.0, 10.0, 10.0);
        let u = a.union(b);
        // a: -5..5, b: 5..15 -> union: -5..15 = width 20, center 5
        assert!((u.width - 20.0).abs() < EPSILON);
        assert!((u.x - 5.0).abs() < EPSILON);
    }

    #[test]
    fn test_inset() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        let i = r.inset(2.0);
        assert!((i.width - 6.0).abs() < EPSILON);
        assert!((i.height - 6.0).abs() < EPSILON);
        assert!((i.x - 0.0).abs() < EPSILON);
    }

    #[test]
    fn test_inset_xy() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        let i = r.inset_xy(1.0, 2.0);
        assert!((i.width - 8.0).abs() < EPSILON);
        assert!((i.height - 6.0).abs() < EPSILON);
    }

    #[test]
    fn test_expanded() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        let e = r.expanded(2.0);
        assert!((e.width - 14.0).abs() < EPSILON);
        assert!((e.height - 14.0).abs() < EPSILON);
    }

    #[test]
    fn test_offset() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        let o = r.offset(5.0, 3.0);
        assert!((o.x - 5.0).abs() < EPSILON);
        assert!((o.y - 3.0).abs() < EPSILON);
        assert!((o.width - 10.0).abs() < EPSILON);
    }

    #[test]
    fn test_offset_by() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        let o = r.offset_by(Vector2::new(5.0, 3.0));
        assert!((o.x - 5.0).abs() < EPSILON);
        assert!((o.y - 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_scaled() {
        let r = Rect::new(5.0, 5.0, 10.0, 10.0);
        let s = r.scaled(2.0);
        assert!((s.x - 5.0).abs() < EPSILON);
        assert!((s.width - 20.0).abs() < EPSILON);
    }

    #[test]
    fn test_lerp() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        let b = Rect::new(10.0, 20.0, 30.0, 40.0);
        let mid = a.lerp(b, 0.5);
        assert!((mid.x - 5.0).abs() < EPSILON);
        assert!((mid.y - 10.0).abs() < EPSILON);
        assert!((mid.width - 20.0).abs() < EPSILON);
        assert!((mid.height - 25.0).abs() < EPSILON);
    }

    #[test]
    fn test_is_approximately_equal() {
        let a = Rect::new(1.0, 2.0, 3.0, 4.0);
        let b = Rect::new(1.00005, 2.00005, 3.00005, 4.00005);
        assert!(a.is_approximately_equal(b, 0.0001));
        assert!(!a.is_approximately_equal(Rect::new(1.1, 2.0, 3.0, 4.0), 0.0001));
    }

    #[test]
    fn test_bounding_box_points() {
        let points = vec![
            Point::new(0.0, 0.0),
            Point::new(10.0, 5.0),
            Point::new(-3.0, 8.0),
        ];
        let bb = Rect::bounding_box_points(&points).unwrap();
        assert!((bb.min_x() - -3.0).abs() < EPSILON);
        assert!((bb.max_x() - 10.0).abs() < EPSILON);
        assert!((bb.min_y() - 0.0).abs() < EPSILON);
        assert!((bb.max_y() - 8.0).abs() < EPSILON);
    }

    #[test]
    fn test_bounding_box_points_empty() {
        assert!(Rect::bounding_box_points(&[]).is_none());
    }

    #[test]
    fn test_bounding_box_rects() {
        let rects = vec![
            Rect::new(0.0, 0.0, 10.0, 10.0),
            Rect::new(20.0, 0.0, 10.0, 10.0),
        ];
        let bb = Rect::bounding_box_rects(&rects).unwrap();
        assert!((bb.min_x() - -5.0).abs() < EPSILON);
        assert!((bb.max_x() - 25.0).abs() < EPSILON);
    }

    #[test]
    fn test_bounding_box_rects_empty() {
        assert!(Rect::bounding_box_rects(&[]).is_none());
    }

    #[test]
    fn test_display() {
        let r = Rect::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(format!("{r}"), "Rect(center: (1, 2), size: (3, 4))");
    }

    #[test]
    fn test_serde_roundtrip() {
        let r = Rect::new(1.0, 2.0, 3.0, 4.0);
        let json = serde_json::to_string(&r).unwrap();
        let r2: Rect = serde_json::from_str(&json).unwrap();
        assert_eq!(r, r2);
    }

    #[test]
    fn test_anchor_serde_roundtrip() {
        let a = Anchor::TopLeft;
        let json = serde_json::to_string(&a).unwrap();
        let a2: Anchor = serde_json::from_str(&json).unwrap();
        assert_eq!(a, a2);
    }
}
