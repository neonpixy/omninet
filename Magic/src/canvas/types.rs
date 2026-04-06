//! Types for the canvas interaction layer — handles, guides, and snap state.

use serde::{Deserialize, Serialize};
use x::geometry::{Point, Rect};

/// Position of a drag handle on a selection bounding box.
///
/// Eight resize handles (corners + edges) plus one rotation handle
/// above the top edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HandlePosition {
    TopLeft,
    Top,
    TopRight,
    Left,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
    Rotation,
}

impl HandlePosition {
    /// Returns all 8 resize handle positions (no rotation).
    pub fn resize_handles() -> &'static [HandlePosition] {
        &[
            Self::TopLeft,
            Self::Top,
            Self::TopRight,
            Self::Left,
            Self::Right,
            Self::BottomLeft,
            Self::Bottom,
            Self::BottomRight,
        ]
    }

    /// Returns all 9 handle positions including rotation.
    pub fn all() -> &'static [HandlePosition] {
        &[
            Self::TopLeft,
            Self::Top,
            Self::TopRight,
            Self::Left,
            Self::Right,
            Self::BottomLeft,
            Self::Bottom,
            Self::BottomRight,
            Self::Rotation,
        ]
    }

    /// Whether this is a corner handle (for proportional resize).
    pub fn is_corner(self) -> bool {
        matches!(
            self,
            Self::TopLeft | Self::TopRight | Self::BottomLeft | Self::BottomRight
        )
    }
}

/// A draggable handle at a specific position on the selection bounds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DragHandle {
    /// Which handle position this represents.
    pub position: HandlePosition,
    /// The handle's bounding rect in canvas coordinates.
    pub rect: Rect,
}

/// Default handle size in screen pixels (scaled by 1/zoom).
const HANDLE_SIZE: f64 = 8.0;

/// Offset above the selection rect for the rotation handle (screen pixels).
const ROTATION_HANDLE_OFFSET: f64 = 20.0;

/// Computes drag handles for a selection bounding rect.
///
/// Handle rects are centered on the edge/corner/rotation point.
/// The `zoom` parameter ensures handles appear at a consistent
/// screen-space size regardless of zoom level.
///
/// # Examples
///
/// ```rust,ignore
/// let handles = compute_handles(selection_rect, 1.0);
/// assert_eq!(handles.len(), 9); // 8 resize + 1 rotation
/// ```
pub fn compute_handles(selection_rect: Rect, zoom: f64) -> Vec<DragHandle> {
    let safe_zoom = zoom.max(0.01);
    let handle_half = HANDLE_SIZE / (2.0 * safe_zoom);
    let handle_size = HANDLE_SIZE / safe_zoom;

    let min_x = selection_rect.min_x();
    let max_x = selection_rect.max_x();
    let min_y = selection_rect.min_y();
    let max_y = selection_rect.max_y();
    let mid_x = selection_rect.x;
    let mid_y = selection_rect.y;

    let make = |pos: HandlePosition, cx: f64, cy: f64| DragHandle {
        position: pos,
        rect: Rect::new(cx, cy, handle_size, handle_size),
    };

    let _ = handle_half; // used conceptually for handle placement

    let mut handles = vec![
        make(HandlePosition::TopLeft, min_x, min_y),
        make(HandlePosition::Top, mid_x, min_y),
        make(HandlePosition::TopRight, max_x, min_y),
        make(HandlePosition::Left, min_x, mid_y),
        make(HandlePosition::Right, max_x, mid_y),
        make(HandlePosition::BottomLeft, min_x, max_y),
        make(HandlePosition::Bottom, mid_x, max_y),
        make(HandlePosition::BottomRight, max_x, max_y),
    ];

    // Rotation handle above the top edge
    let rotation_offset = ROTATION_HANDLE_OFFSET / safe_zoom;
    handles.push(make(
        HandlePosition::Rotation,
        mid_x,
        min_y - rotation_offset,
    ));

    handles
}

/// Hit-tests a point against a set of handles.
///
/// Returns the first handle whose rect contains the point, or `None`.
pub fn hit_test_handles(point: Point, handles: &[DragHandle]) -> Option<&DragHandle> {
    handles.iter().find(|h| h.rect.contains_point(point))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_position_resize_count() {
        assert_eq!(HandlePosition::resize_handles().len(), 8);
    }

    #[test]
    fn handle_position_all_count() {
        assert_eq!(HandlePosition::all().len(), 9);
    }

    #[test]
    fn handle_position_is_corner() {
        assert!(HandlePosition::TopLeft.is_corner());
        assert!(HandlePosition::TopRight.is_corner());
        assert!(HandlePosition::BottomLeft.is_corner());
        assert!(HandlePosition::BottomRight.is_corner());
        assert!(!HandlePosition::Top.is_corner());
        assert!(!HandlePosition::Left.is_corner());
        assert!(!HandlePosition::Rotation.is_corner());
    }

    #[test]
    fn compute_handles_count() {
        let rect = Rect::new(100.0, 100.0, 200.0, 150.0);
        let handles = compute_handles(rect, 1.0);
        assert_eq!(handles.len(), 9);
    }

    #[test]
    fn compute_handles_positions() {
        let rect = Rect::new(0.0, 0.0, 100.0, 80.0);
        let handles = compute_handles(rect, 1.0);

        // TopLeft should be near (-50, -40)
        let tl = handles.iter().find(|h| h.position == HandlePosition::TopLeft).unwrap();
        assert!((tl.rect.x - (-50.0)).abs() < 1.0);
        assert!((tl.rect.y - (-40.0)).abs() < 1.0);

        // BottomRight should be near (50, 40)
        let br = handles.iter().find(|h| h.position == HandlePosition::BottomRight).unwrap();
        assert!((br.rect.x - 50.0).abs() < 1.0);
        assert!((br.rect.y - 40.0).abs() < 1.0);

        // Rotation should be above the top center
        let rot = handles.iter().find(|h| h.position == HandlePosition::Rotation).unwrap();
        assert!((rot.rect.x - 0.0).abs() < 1.0);
        assert!(rot.rect.y < -40.0); // above top edge
    }

    #[test]
    fn compute_handles_zoomed() {
        let rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        let handles_1x = compute_handles(rect, 1.0);
        let handles_2x = compute_handles(rect, 2.0);

        // At 2x zoom, handles should be smaller in canvas space
        let h1 = &handles_1x[0].rect;
        let h2 = &handles_2x[0].rect;
        assert!(h2.width < h1.width);
    }

    #[test]
    fn hit_test_handles_finds_match() {
        let rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        let handles = compute_handles(rect, 1.0);

        // Click near top-left corner (-50, -50)
        let hit = hit_test_handles(Point::new(-50.0, -50.0), &handles);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().position, HandlePosition::TopLeft);
    }

    #[test]
    fn hit_test_handles_misses() {
        let rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        let handles = compute_handles(rect, 1.0);

        // Click far away
        let hit = hit_test_handles(Point::new(500.0, 500.0), &handles);
        assert!(hit.is_none());
    }
}
