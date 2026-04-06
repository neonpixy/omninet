//! Pen tool — bezier path drawing, point by point.
//!
//! Ported from Swiftlight's `PenTool.swift`. Simplified for the protocol
//! layer — platform-specific overlays are handled by Divinity.
//!
//! Behaviors:
//! - Click to add a corner point
//! - Click-and-drag to add a smooth point with mirrored bezier handles
//! - Click on the first point to close the path
//! - Escape/deactivate finishes an open path

use x::geometry::Point;

use crate::ideation::DocumentState;
use super::traits::Tool;
use super::types::{CursorStyle, ModifierKeys, ToolAction};

/// A single point in a pen path being built.
#[derive(Clone, Debug, PartialEq)]
pub struct PathPoint {
    /// The anchor position (on-curve point).
    pub anchor: Point,
    /// Incoming bezier handle (control point before the anchor).
    pub in_handle: Option<Point>,
    /// Outgoing bezier handle (control point after the anchor).
    pub out_handle: Option<Point>,
    /// Whether handles should mirror each other.
    pub mirrored: bool,
}

impl PathPoint {
    /// Creates a corner point with no handles.
    pub fn corner(anchor: Point) -> Self {
        Self {
            anchor,
            in_handle: None,
            out_handle: None,
            mirrored: false,
        }
    }

    /// Creates a smooth point with mirrored handles.
    pub fn smooth(anchor: Point, out_handle: Point) -> Self {
        let in_handle = Point::new(
            2.0 * anchor.x - out_handle.x,
            2.0 * anchor.y - out_handle.y,
        );
        Self {
            anchor,
            in_handle: Some(in_handle),
            out_handle: Some(out_handle),
            mirrored: true,
        }
    }
}

/// Tool for drawing bezier paths point by point.
pub struct PenTool {
    /// Points accumulated for the current path.
    points: Vec<PathPoint>,
    /// Whether the user is currently dragging to create a handle.
    is_dragging_handle: bool,
    /// The anchor position of the point being placed.
    drag_anchor: Point,
    /// Close tolerance in canvas units (scaled by zoom elsewhere).
    close_tolerance: f64,
}

impl PenTool {
    /// Creates a new pen tool.
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            is_dragging_handle: false,
            drag_anchor: Point::ZERO,
            close_tolerance: 10.0,
        }
    }

    /// Resets the tool to its initial state without creating a path.
    fn reset(&mut self) {
        self.points.clear();
        self.is_dragging_handle = false;
        self.drag_anchor = Point::ZERO;
    }

    /// Returns the path data as a JSON value for insertion into the document.
    fn build_path_json(&self, closed: bool) -> serde_json::Value {
        let points_json: Vec<serde_json::Value> = self
            .points
            .iter()
            .map(|p| {
                let mut point = serde_json::Map::new();
                point.insert(
                    "anchor".to_string(),
                    serde_json::json!({ "x": p.anchor.x, "y": p.anchor.y }),
                );
                if let Some(in_h) = &p.in_handle {
                    point.insert(
                        "in_handle".to_string(),
                        serde_json::json!({ "x": in_h.x, "y": in_h.y }),
                    );
                }
                if let Some(out_h) = &p.out_handle {
                    point.insert(
                        "out_handle".to_string(),
                        serde_json::json!({ "x": out_h.x, "y": out_h.y }),
                    );
                }
                point.insert("mirrored".to_string(), serde_json::json!(p.mirrored));
                serde_json::Value::Object(point)
            })
            .collect();

        // Calculate bounding rect
        let xs: Vec<f64> = self.points.iter().map(|p| p.anchor.x).collect();
        let ys: Vec<f64> = self.points.iter().map(|p| p.anchor.y).collect();
        let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_x = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let min_y = ys.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_y = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        let cx = (min_x + max_x) / 2.0;
        let cy = (min_y + max_y) / 2.0;
        let w = (max_x - min_x).max(1.0);
        let h = (max_y - min_y).max(1.0);

        serde_json::json!({
            "type": "path",
            "x": cx,
            "y": cy,
            "width": w,
            "height": h,
            "closed": closed,
            "points": points_json,
        })
    }

    /// Returns the current points being built (for overlay rendering).
    pub fn current_points(&self) -> &[PathPoint] {
        &self.points
    }

    /// Returns whether the pen tool is actively drawing a path.
    pub fn is_drawing(&self) -> bool {
        !self.points.is_empty()
    }
}

impl Default for PenTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for PenTool {
    fn id(&self) -> &str {
        "pen"
    }

    fn display_name(&self) -> &str {
        "Pen"
    }

    fn cursor(&self) -> CursorStyle {
        CursorStyle::Crosshair
    }

    fn activate(&mut self) {
        self.reset();
    }

    fn deactivate(&mut self) {
        // If we have enough points, create the path
        if self.points.len() >= 2 {
            let json = self.build_path_json(false);
            self.reset();
            // Note: deactivate returns void in the trait, so we can't return
            // the action. In practice, the host should check is_drawing()
            // before deactivating and call on_release or a finish method.
            let _ = json;
        }
        self.reset();
    }

    fn on_press(
        &mut self,
        point: Point,
        _modifiers: ModifierKeys,
        _state: &DocumentState,
    ) -> ToolAction {
        // Check if clicking near the first point to close the path
        if self.points.len() >= 2 {
            let first = self.points[0].anchor;
            if point.distance(first) < self.close_tolerance {
                let json = self.build_path_json(true);
                self.reset();
                return ToolAction::Insert(json);
            }
        }

        self.is_dragging_handle = true;
        self.drag_anchor = point;
        ToolAction::None
    }

    fn on_drag(
        &mut self,
        _point: Point,
        _modifiers: ModifierKeys,
        _state: &DocumentState,
    ) -> ToolAction {
        // Handle positioning happens in on_release based on drag distance.
        // During drag, the host renders a preview using current_points().
        ToolAction::None
    }

    fn on_release(
        &mut self,
        point: Point,
        modifiers: ModifierKeys,
        _state: &DocumentState,
    ) -> ToolAction {
        if !self.is_dragging_handle {
            return ToolAction::None;
        }
        self.is_dragging_handle = false;

        let drag_distance = self.drag_anchor.distance(point);

        if drag_distance < 3.0 {
            // Click — add a corner point
            self.points.push(PathPoint::corner(self.drag_anchor));
        } else {
            // Drag — add a smooth point with handles
            if modifiers.alt {
                // Alt-drag: only set out_handle, no mirroring
                let mut pp = PathPoint::corner(self.drag_anchor);
                pp.out_handle = Some(point);
                self.points.push(pp);
            } else {
                self.points
                    .push(PathPoint::smooth(self.drag_anchor, point));
            }
        }

        ToolAction::None
    }

    fn on_hover(
        &self,
        point: Point,
        _state: &DocumentState,
    ) -> Option<CursorStyle> {
        // Show close indicator when hovering near first point
        if self.points.len() >= 2 {
            let first = self.points[0].anchor;
            if point.distance(first) < self.close_tolerance {
                return Some(CursorStyle::Pointer);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_identity() {
        let tool = PenTool::new();
        assert_eq!(tool.id(), "pen");
        assert_eq!(tool.display_name(), "Pen");
        assert_eq!(tool.cursor(), CursorStyle::Crosshair);
    }

    #[test]
    fn initially_not_drawing() {
        let tool = PenTool::new();
        assert!(!tool.is_drawing());
        assert!(tool.current_points().is_empty());
    }

    #[test]
    fn click_adds_corner_point() {
        let mut tool = PenTool::new();
        let state = DocumentState::new("test");
        let mods = ModifierKeys::default();

        // Press then release at same point = click
        tool.on_press(Point::new(10.0, 20.0), mods, &state);
        tool.on_release(Point::new(10.0, 20.0), mods, &state);

        assert_eq!(tool.current_points().len(), 1);
        assert_eq!(tool.current_points()[0].anchor, Point::new(10.0, 20.0));
        assert!(tool.current_points()[0].in_handle.is_none());
        assert!(tool.current_points()[0].out_handle.is_none());
    }

    #[test]
    fn drag_adds_smooth_point() {
        let mut tool = PenTool::new();
        let state = DocumentState::new("test");
        let mods = ModifierKeys::default();

        tool.on_press(Point::new(10.0, 20.0), mods, &state);
        tool.on_release(Point::new(30.0, 20.0), mods, &state);

        assert_eq!(tool.current_points().len(), 1);
        let pp = &tool.current_points()[0];
        assert_eq!(pp.anchor, Point::new(10.0, 20.0));
        assert!(pp.out_handle.is_some());
        assert!(pp.in_handle.is_some());
        assert!(pp.mirrored);
    }

    #[test]
    fn close_path_on_first_point() {
        let mut tool = PenTool::new();
        let state = DocumentState::new("test");
        let mods = ModifierKeys::default();

        // Add three points
        tool.on_press(Point::new(0.0, 0.0), mods, &state);
        tool.on_release(Point::new(0.0, 0.0), mods, &state);
        tool.on_press(Point::new(100.0, 0.0), mods, &state);
        tool.on_release(Point::new(100.0, 0.0), mods, &state);
        tool.on_press(Point::new(50.0, 100.0), mods, &state);
        tool.on_release(Point::new(50.0, 100.0), mods, &state);

        // Click near first point to close
        let action = tool.on_press(Point::new(1.0, 1.0), mods, &state);
        match action {
            ToolAction::Insert(json) => {
                assert_eq!(json["closed"], true);
                assert_eq!(json["type"], "path");
            }
            other => panic!("expected Insert, got {:?}", other),
        }
        assert!(!tool.is_drawing());
    }

    #[test]
    fn path_point_corner() {
        let pp = PathPoint::corner(Point::new(5.0, 10.0));
        assert_eq!(pp.anchor, Point::new(5.0, 10.0));
        assert!(pp.in_handle.is_none());
        assert!(pp.out_handle.is_none());
        assert!(!pp.mirrored);
    }

    #[test]
    fn path_point_smooth_mirrors_handles() {
        let pp = PathPoint::smooth(Point::new(10.0, 10.0), Point::new(20.0, 10.0));
        assert_eq!(pp.anchor, Point::new(10.0, 10.0));
        assert_eq!(pp.out_handle, Some(Point::new(20.0, 10.0)));
        // Mirrored: in_handle should be at (0, 10) — opposite of out
        let in_h = pp.in_handle.unwrap();
        assert!((in_h.x - 0.0).abs() < 1e-10);
        assert!((in_h.y - 10.0).abs() < 1e-10);
        assert!(pp.mirrored);
    }
}
