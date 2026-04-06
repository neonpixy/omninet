//! Shape tool — draw rectangles, ellipses, and lines by dragging.
//!
//! Parameterized by [`ShapeKind`], so one implementation covers all shape
//! types. Ported from Swiftlight's `RectangleTool`, `EllipseTool`, and
//! `LineTool`.

use x::geometry::Point;

use crate::ideation::DocumentState;
use super::traits::Tool;
use super::types::{CursorStyle, DragState, ModifierKeys, ShapeKind, ToolAction};

/// A drawing tool for basic shapes, parameterized by [`ShapeKind`].
///
/// Behaviors:
/// - **Drag** — draws the shape from start to end point
/// - **Shift+drag** — constrains to square (Rectangle/Ellipse) or 15-degree
///   angle increments (Line)
/// - **Release** — inserts the shape as a new digit if large enough
pub struct ShapeTool {
    kind: ShapeKind,
    drag: Option<DragState>,
    /// Minimum dimension (width or height) to create a shape.
    min_dimension: f64,
}

impl ShapeTool {
    /// Creates a new shape tool for the given kind.
    pub fn new(kind: ShapeKind) -> Self {
        Self {
            kind,
            drag: None,
            min_dimension: 1.0,
        }
    }

    /// Returns the current shape kind.
    pub fn kind(&self) -> ShapeKind {
        self.kind
    }

    /// Returns the current drag state, if any (for overlay rendering).
    pub fn current_drag(&self) -> Option<&DragState> {
        self.drag.as_ref()
    }

    /// Build the shape JSON for insertion.
    fn build_shape_json(&self, drag: &DragState) -> serde_json::Value {
        match self.kind {
            ShapeKind::Rectangle | ShapeKind::Ellipse => {
                let rect = if drag.modifiers.shift {
                    drag.constrained_rect()
                } else {
                    drag.rect()
                };
                let type_name = match self.kind {
                    ShapeKind::Rectangle => "rectangle",
                    ShapeKind::Ellipse => "ellipse",
                    _ => unreachable!(),
                };
                serde_json::json!({
                    "type": type_name,
                    "x": rect.x,
                    "y": rect.y,
                    "width": rect.width,
                    "height": rect.height,
                })
            }
            ShapeKind::Line => {
                let end = if drag.modifiers.shift {
                    constrain_angle_15(drag.start, drag.current)
                } else {
                    drag.current
                };
                serde_json::json!({
                    "type": "line",
                    "x1": drag.start.x,
                    "y1": drag.start.y,
                    "x2": end.x,
                    "y2": end.y,
                })
            }
        }
    }
}

/// Constrains a point to 15-degree angle increments from the start point.
fn constrain_angle_15(start: Point, end: Point) -> Point {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let length = (dx * dx + dy * dy).sqrt();

    if length < f64::EPSILON {
        return start;
    }

    let angle = dy.atan2(dx);
    let snap_angle =
        (angle / (std::f64::consts::PI / 12.0)).round() * (std::f64::consts::PI / 12.0);

    Point::new(
        start.x + length * snap_angle.cos(),
        start.y + length * snap_angle.sin(),
    )
}

impl Tool for ShapeTool {
    fn id(&self) -> &str {
        self.kind.tool_id()
    }

    fn display_name(&self) -> &str {
        self.kind.display_name()
    }

    fn cursor(&self) -> CursorStyle {
        CursorStyle::Crosshair
    }

    fn activate(&mut self) {
        self.drag = None;
    }

    fn deactivate(&mut self) {
        self.drag = None;
    }

    fn on_press(
        &mut self,
        point: Point,
        modifiers: ModifierKeys,
        _state: &DocumentState,
    ) -> ToolAction {
        self.drag = Some(DragState::new(point, modifiers));
        ToolAction::None
    }

    fn on_drag(
        &mut self,
        point: Point,
        modifiers: ModifierKeys,
        _state: &DocumentState,
    ) -> ToolAction {
        if let Some(ref mut drag) = self.drag {
            drag.current = point;
            drag.modifiers = modifiers;
        }
        // The host renders a preview based on current_drag().
        ToolAction::None
    }

    fn on_release(
        &mut self,
        point: Point,
        modifiers: ModifierKeys,
        _state: &DocumentState,
    ) -> ToolAction {
        let drag = match self.drag.take() {
            Some(mut d) => {
                d.current = point;
                d.modifiers = modifiers;
                d
            }
            None => return ToolAction::None,
        };

        match self.kind {
            ShapeKind::Rectangle | ShapeKind::Ellipse => {
                let rect = if drag.modifiers.shift {
                    drag.constrained_rect()
                } else {
                    drag.rect()
                };
                if rect.width > self.min_dimension && rect.height > self.min_dimension {
                    let json = self.build_shape_json(&drag);
                    ToolAction::Insert(json)
                } else {
                    ToolAction::None
                }
            }
            ShapeKind::Line => {
                if drag.distance() > self.min_dimension {
                    let json = self.build_shape_json(&drag);
                    ToolAction::Insert(json)
                } else {
                    ToolAction::None
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_identity_rectangle() {
        let tool = ShapeTool::new(ShapeKind::Rectangle);
        assert_eq!(tool.id(), "shape-rectangle");
        assert_eq!(tool.display_name(), "Rectangle");
        assert_eq!(tool.cursor(), CursorStyle::Crosshair);
    }

    #[test]
    fn tool_identity_ellipse() {
        let tool = ShapeTool::new(ShapeKind::Ellipse);
        assert_eq!(tool.id(), "shape-ellipse");
        assert_eq!(tool.display_name(), "Ellipse");
    }

    #[test]
    fn tool_identity_line() {
        let tool = ShapeTool::new(ShapeKind::Line);
        assert_eq!(tool.id(), "shape-line");
        assert_eq!(tool.display_name(), "Line");
    }

    #[test]
    fn drag_creates_rectangle() {
        let mut tool = ShapeTool::new(ShapeKind::Rectangle);
        let state = DocumentState::new("test");
        let mods = ModifierKeys::default();

        tool.on_press(Point::new(0.0, 0.0), mods, &state);
        tool.on_drag(Point::new(100.0, 50.0), mods, &state);
        let action = tool.on_release(Point::new(100.0, 50.0), mods, &state);

        match action {
            ToolAction::Insert(json) => {
                assert_eq!(json["type"], "rectangle");
                assert!((json["width"].as_f64().unwrap() - 100.0).abs() < 1e-10);
                assert!((json["height"].as_f64().unwrap() - 50.0).abs() < 1e-10);
            }
            other => panic!("expected Insert, got {:?}", other),
        }
    }

    #[test]
    fn drag_creates_ellipse() {
        let mut tool = ShapeTool::new(ShapeKind::Ellipse);
        let state = DocumentState::new("test");
        let mods = ModifierKeys::default();

        tool.on_press(Point::new(0.0, 0.0), mods, &state);
        let action = tool.on_release(Point::new(80.0, 60.0), mods, &state);

        match action {
            ToolAction::Insert(json) => {
                assert_eq!(json["type"], "ellipse");
            }
            other => panic!("expected Insert, got {:?}", other),
        }
    }

    #[test]
    fn drag_creates_line() {
        let mut tool = ShapeTool::new(ShapeKind::Line);
        let state = DocumentState::new("test");
        let mods = ModifierKeys::default();

        tool.on_press(Point::new(10.0, 10.0), mods, &state);
        let action = tool.on_release(Point::new(100.0, 100.0), mods, &state);

        match action {
            ToolAction::Insert(json) => {
                assert_eq!(json["type"], "line");
                assert!((json["x1"].as_f64().unwrap() - 10.0).abs() < 1e-10);
                assert!((json["y1"].as_f64().unwrap() - 10.0).abs() < 1e-10);
            }
            other => panic!("expected Insert, got {:?}", other),
        }
    }

    #[test]
    fn tiny_drag_does_not_create_shape() {
        let mut tool = ShapeTool::new(ShapeKind::Rectangle);
        let state = DocumentState::new("test");
        let mods = ModifierKeys::default();

        tool.on_press(Point::new(0.0, 0.0), mods, &state);
        let action = tool.on_release(Point::new(0.5, 0.5), mods, &state);

        assert!(action.is_none());
    }

    #[test]
    fn shift_constrains_to_square() {
        let mut tool = ShapeTool::new(ShapeKind::Rectangle);
        let state = DocumentState::new("test");
        let mods = ModifierKeys {
            shift: true,
            ..Default::default()
        };

        tool.on_press(Point::new(0.0, 0.0), mods, &state);
        let action = tool.on_release(Point::new(100.0, 50.0), mods, &state);

        match action {
            ToolAction::Insert(json) => {
                let w = json["width"].as_f64().unwrap();
                let h = json["height"].as_f64().unwrap();
                assert!((w - h).abs() < 1e-10, "expected square, got {}x{}", w, h);
            }
            other => panic!("expected Insert, got {:?}", other),
        }
    }

    #[test]
    fn constrain_angle_snaps() {
        // Exactly 45 degrees should stay 45
        let result = constrain_angle_15(Point::new(0.0, 0.0), Point::new(10.0, 10.0));
        let expected_angle = std::f64::consts::PI / 4.0; // 45 degrees
        let actual_angle = (result.y - 0.0).atan2(result.x - 0.0);
        assert!(
            (actual_angle - expected_angle).abs() < 0.01,
            "expected ~45deg, got {}",
            actual_angle.to_degrees()
        );
    }

    #[test]
    fn kind_accessor() {
        let tool = ShapeTool::new(ShapeKind::Ellipse);
        assert_eq!(tool.kind(), ShapeKind::Ellipse);
    }
}
