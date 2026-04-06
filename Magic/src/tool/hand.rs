//! Hand tool — pan the canvas by dragging.
//!
//! Ported from Swiftlight's `HandTool.swift`. The simplest tool: press
//! to grab, drag to pan, release to stop. Returns `Pan` actions that
//! the host applies to the viewport.

use x::geometry::Point;

use crate::ideation::DocumentState;
use super::traits::Tool;
use super::types::{CursorStyle, ModifierKeys, ToolAction};

/// Tool for panning the canvas by dragging.
///
/// Behaviors:
/// - **Press** — switches to grabbing cursor
/// - **Drag** — emits `Pan` actions with the delta from last position
/// - **Release** — switches back to open-hand cursor
pub struct HandTool {
    is_dragging: bool,
    last_point: Point,
}

impl HandTool {
    /// Creates a new hand tool.
    pub fn new() -> Self {
        Self {
            is_dragging: false,
            last_point: Point::ZERO,
        }
    }
}

impl Default for HandTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for HandTool {
    fn id(&self) -> &str {
        "hand"
    }

    fn display_name(&self) -> &str {
        "Hand"
    }

    fn cursor(&self) -> CursorStyle {
        if self.is_dragging {
            CursorStyle::Grabbing
        } else {
            CursorStyle::Grab
        }
    }

    fn activate(&mut self) {
        self.is_dragging = false;
    }

    fn deactivate(&mut self) {
        self.is_dragging = false;
    }

    fn on_press(
        &mut self,
        point: Point,
        _modifiers: ModifierKeys,
        _state: &DocumentState,
    ) -> ToolAction {
        self.is_dragging = true;
        self.last_point = point;
        ToolAction::SetCursor(CursorStyle::Grabbing)
    }

    fn on_drag(
        &mut self,
        point: Point,
        _modifiers: ModifierKeys,
        _state: &DocumentState,
    ) -> ToolAction {
        if !self.is_dragging {
            return ToolAction::None;
        }

        let delta = point - self.last_point;
        self.last_point = point;

        ToolAction::Pan(delta)
    }

    fn on_release(
        &mut self,
        _point: Point,
        _modifiers: ModifierKeys,
        _state: &DocumentState,
    ) -> ToolAction {
        self.is_dragging = false;
        ToolAction::SetCursor(CursorStyle::Grab)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_identity() {
        let tool = HandTool::new();
        assert_eq!(tool.id(), "hand");
        assert_eq!(tool.display_name(), "Hand");
        assert_eq!(tool.cursor(), CursorStyle::Grab);
    }

    #[test]
    fn press_sets_grabbing_cursor() {
        let mut tool = HandTool::new();
        let state = DocumentState::new("test");
        let mods = ModifierKeys::default();

        let action = tool.on_press(Point::new(0.0, 0.0), mods, &state);
        assert_eq!(action, ToolAction::SetCursor(CursorStyle::Grabbing));
        assert_eq!(tool.cursor(), CursorStyle::Grabbing);
    }

    #[test]
    fn drag_emits_pan_delta() {
        let mut tool = HandTool::new();
        let state = DocumentState::new("test");
        let mods = ModifierKeys::default();

        tool.on_press(Point::new(100.0, 200.0), mods, &state);
        let action = tool.on_drag(Point::new(110.0, 215.0), mods, &state);

        match action {
            ToolAction::Pan(delta) => {
                assert!((delta.x - 10.0).abs() < 1e-10);
                assert!((delta.y - 15.0).abs() < 1e-10);
            }
            other => panic!("expected Pan, got {:?}", other),
        }
    }

    #[test]
    fn release_restores_grab_cursor() {
        let mut tool = HandTool::new();
        let state = DocumentState::new("test");
        let mods = ModifierKeys::default();

        tool.on_press(Point::new(0.0, 0.0), mods, &state);
        let action = tool.on_release(Point::new(0.0, 0.0), mods, &state);
        assert_eq!(action, ToolAction::SetCursor(CursorStyle::Grab));
        assert_eq!(tool.cursor(), CursorStyle::Grab);
    }

    #[test]
    fn drag_without_press_does_nothing() {
        let mut tool = HandTool::new();
        let state = DocumentState::new("test");
        let mods = ModifierKeys::default();

        let action = tool.on_drag(Point::new(10.0, 10.0), mods, &state);
        assert!(action.is_none());
    }
}
