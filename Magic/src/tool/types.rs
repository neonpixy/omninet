//! Shared types for the canvas tool system.
//!
//! Defines cursor styles, tool actions, shape kinds, modifier key state,
//! and drag state — all the vocabulary tools use to communicate.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use x::geometry::{Point, Rect, Vector2};

/// Cursor style hint passed to the platform layer.
///
/// Tools declare which cursor they want; the platform (Divinity) maps
/// these to native cursors (NSCursor on macOS, CSS cursor on web, etc.).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[derive(Default)]
pub enum CursorStyle {
    /// Platform default arrow.
    #[default]
    Default,
    /// Crosshair for precise placement.
    Crosshair,
    /// Open hand (ready to grab).
    Grab,
    /// Closed hand (actively grabbing).
    Grabbing,
    /// Four-arrow move cursor.
    Move,
    /// Pointing hand (clickable target).
    Pointer,
    /// I-beam for text editing.
    Text,
    /// Magnifying glass with plus.
    ZoomIn,
    /// Magnifying glass with minus.
    ZoomOut,
}


/// Shape variant for the parameterized [`ShapeTool`](super::shape::ShapeTool).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShapeKind {
    /// Axis-aligned rectangle.
    Rectangle,
    /// Oval/circle shape.
    Ellipse,
    /// Straight line between two points.
    Line,
}

impl ShapeKind {
    /// Human-readable display name.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Rectangle => "Rectangle",
            Self::Ellipse => "Ellipse",
            Self::Line => "Line",
        }
    }

    /// Tool identifier suffix (e.g. "shape-rectangle").
    pub fn tool_id(self) -> &'static str {
        match self {
            Self::Rectangle => "shape-rectangle",
            Self::Ellipse => "shape-ellipse",
            Self::Line => "shape-line",
        }
    }
}

/// Result of a tool interaction method.
///
/// Tools return this from `on_press`, `on_drag`, and `on_release` to tell
/// the host what happened without the tool directly mutating shared state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToolAction {
    /// Nothing happened.
    None,
    /// Select these digits (replaces current selection).
    Select(Vec<Uuid>),
    /// Clear selection.
    Deselect,
    /// Insert a new digit (serialized as JSON; host deserializes into Digit).
    Insert(serde_json::Value),
    /// Move digits by a delta.
    Move {
        ids: Vec<Uuid>,
        delta: Vector2,
    },
    /// Resize a digit to a new bounding rect.
    Resize {
        id: Uuid,
        new_rect: Rect,
    },
    /// Request the host to set a particular cursor.
    SetCursor(CursorStyle),
    /// Pan the canvas viewport by a screen-space delta.
    Pan(Vector2),
    /// Zoom the canvas by a factor around a center point.
    Zoom {
        factor: f64,
        center: Point,
    },
    /// Begin text editing for a digit.
    BeginTextEdit(Uuid),
    /// Tool-specific action identified by a string key.
    Custom(String),
    /// Multiple actions to apply in sequence.
    Batch(Vec<ToolAction>),
}

impl ToolAction {
    /// Returns `true` if this is `ToolAction::None`.
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// Modifier key state at the time of an input event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModifierKeys {
    /// Shift key held (constrain, multi-select).
    pub shift: bool,
    /// Alt/Option key held (duplicate, toggle).
    pub alt: bool,
    /// Command/Ctrl key held (system shortcuts).
    pub command: bool,
}

/// Accumulated drag state for a press-drag-release gesture.
///
/// Created on press, updated on drag, consumed on release.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DragState {
    /// Canvas-space position where the drag began.
    pub start: Point,
    /// Current canvas-space position.
    pub current: Point,
    /// Modifier keys at the start of the drag.
    pub modifiers: ModifierKeys,
}

impl DragState {
    /// Creates a new drag state at the given starting point.
    pub fn new(start: Point, modifiers: ModifierKeys) -> Self {
        Self {
            start,
            current: start,
            modifiers,
        }
    }

    /// The vector from start to current position.
    pub fn delta(&self) -> Vector2 {
        self.current - self.start
    }

    /// The bounding rectangle formed by the start and current positions.
    pub fn rect(&self) -> Rect {
        Rect::from_corners(self.start, self.current)
    }

    /// The bounding rectangle constrained to a square (shift-drag).
    pub fn constrained_rect(&self) -> Rect {
        let dx = (self.current.x - self.start.x).abs();
        let dy = (self.current.y - self.start.y).abs();
        let side = dx.max(dy);
        let sign_x: f64 = if self.current.x >= self.start.x {
            1.0
        } else {
            -1.0
        };
        let sign_y: f64 = if self.current.y >= self.start.y {
            1.0
        } else {
            -1.0
        };
        Rect::from_corners(
            self.start,
            Point::new(
                self.start.x + side * sign_x,
                self.start.y + side * sign_y,
            ),
        )
    }

    /// The total drag distance in canvas units.
    pub fn distance(&self) -> f64 {
        self.start.distance(self.current)
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_style_default() {
        assert_eq!(CursorStyle::default(), CursorStyle::Default);
    }

    #[test]
    fn shape_kind_display_name() {
        assert_eq!(ShapeKind::Rectangle.display_name(), "Rectangle");
        assert_eq!(ShapeKind::Ellipse.display_name(), "Ellipse");
        assert_eq!(ShapeKind::Line.display_name(), "Line");
    }

    #[test]
    fn shape_kind_tool_id() {
        assert_eq!(ShapeKind::Rectangle.tool_id(), "shape-rectangle");
    }

    #[test]
    fn tool_action_is_none() {
        assert!(ToolAction::None.is_none());
        assert!(!ToolAction::Deselect.is_none());
    }

    #[test]
    fn drag_state_delta() {
        let ds = DragState {
            start: Point::new(10.0, 20.0),
            current: Point::new(30.0, 50.0),
            modifiers: ModifierKeys::default(),
        };
        let d = ds.delta();
        assert!((d.x - 20.0).abs() < 1e-10);
        assert!((d.y - 30.0).abs() < 1e-10);
    }

    #[test]
    fn drag_state_rect() {
        let ds = DragState {
            start: Point::new(0.0, 0.0),
            current: Point::new(10.0, 20.0),
            modifiers: ModifierKeys::default(),
        };
        let r = ds.rect();
        assert!((r.width - 10.0).abs() < 1e-10);
        assert!((r.height - 20.0).abs() < 1e-10);
    }

    #[test]
    fn drag_state_constrained_rect() {
        let ds = DragState {
            start: Point::new(0.0, 0.0),
            current: Point::new(10.0, 20.0),
            modifiers: ModifierKeys::default(),
        };
        let r = ds.constrained_rect();
        // Should be a square with side = max(10, 20) = 20
        assert!((r.width - 20.0).abs() < 1e-10);
        assert!((r.height - 20.0).abs() < 1e-10);
    }

    #[test]
    fn drag_state_constrained_rect_negative() {
        let ds = DragState {
            start: Point::new(10.0, 10.0),
            current: Point::new(0.0, 5.0),
            modifiers: ModifierKeys::default(),
        };
        let r = ds.constrained_rect();
        // dx=10, dy=5, side=10. Sign: x negative, y negative
        assert!((r.width - 10.0).abs() < 1e-10);
        assert!((r.height - 10.0).abs() < 1e-10);
    }

    #[test]
    fn drag_state_distance() {
        let ds = DragState {
            start: Point::new(0.0, 0.0),
            current: Point::new(3.0, 4.0),
            modifiers: ModifierKeys::default(),
        };
        assert!((ds.distance() - 5.0).abs() < 1e-10);
    }

    #[test]
    fn modifier_keys_default() {
        let m = ModifierKeys::default();
        assert!(!m.shift);
        assert!(!m.alt);
        assert!(!m.command);
    }
}
