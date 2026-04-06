//! Text tool — click to create text, drag to create a text box.
//!
//! Ported from Swiftlight's `TextTool.swift`. Platform text editing is
//! handled by Divinity; this tool just creates the digit and signals
//! the host to begin editing.

use x::geometry::Point;

use crate::ideation::DocumentState;
use super::traits::Tool;
use super::types::{CursorStyle, DragState, ModifierKeys, ToolAction};

/// Tool for creating text elements.
///
/// Behaviors:
/// - **Click** — creates a text digit at the click point with default content,
///   then signals the host to begin text editing.
/// - **Drag** — creates a text box with the dragged dimensions.
pub struct TextTool {
    drag: Option<DragState>,
    /// Minimum drag distance to create a text box (vs click-to-place).
    click_threshold: f64,
}

impl TextTool {
    /// Creates a new text tool.
    pub fn new() -> Self {
        Self {
            drag: None,
            click_threshold: 5.0,
        }
    }

    /// Returns the current drag state, if any (for overlay rendering).
    pub fn current_drag(&self) -> Option<&DragState> {
        self.drag.as_ref()
    }
}

impl Default for TextTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for TextTool {
    fn id(&self) -> &str {
        "text"
    }

    fn display_name(&self) -> &str {
        "Text"
    }

    fn cursor(&self) -> CursorStyle {
        CursorStyle::Text
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

        let dx = (drag.current.x - drag.start.x).abs();
        let dy = (drag.current.y - drag.start.y).abs();

        if dx < self.click_threshold && dy < self.click_threshold {
            // Click — create text at click point
            let json = serde_json::json!({
                "type": "text",
                "x": drag.start.x,
                "y": drag.start.y,
                "content": "Text",
                "font_size": 16.0,
            });
            ToolAction::Insert(json)
        } else {
            // Drag — create text box with dimensions
            let rect = drag.rect();
            let font_size = (rect.height / 1.2).clamp(12.0, 72.0);

            let json = serde_json::json!({
                "type": "text",
                "x": rect.x,
                "y": rect.y,
                "width": rect.width,
                "height": rect.height,
                "content": "Text",
                "font_size": font_size,
                "auto_resize": "height",
            });
            ToolAction::Insert(json)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_identity() {
        let tool = TextTool::new();
        assert_eq!(tool.id(), "text");
        assert_eq!(tool.display_name(), "Text");
        assert_eq!(tool.cursor(), CursorStyle::Text);
    }

    #[test]
    fn click_creates_text() {
        let mut tool = TextTool::new();
        let state = DocumentState::new("test");
        let mods = ModifierKeys::default();

        tool.on_press(Point::new(100.0, 200.0), mods, &state);
        let action = tool.on_release(Point::new(101.0, 201.0), mods, &state);

        match action {
            ToolAction::Insert(json) => {
                assert_eq!(json["type"], "text");
                assert_eq!(json["content"], "Text");
                assert!((json["x"].as_f64().unwrap() - 100.0).abs() < 1e-10);
                assert!((json["y"].as_f64().unwrap() - 200.0).abs() < 1e-10);
            }
            other => panic!("expected Insert, got {:?}", other),
        }
    }

    #[test]
    fn drag_creates_text_box() {
        let mut tool = TextTool::new();
        let state = DocumentState::new("test");
        let mods = ModifierKeys::default();

        tool.on_press(Point::new(0.0, 0.0), mods, &state);
        tool.on_drag(Point::new(200.0, 100.0), mods, &state);
        let action = tool.on_release(Point::new(200.0, 100.0), mods, &state);

        match action {
            ToolAction::Insert(json) => {
                assert_eq!(json["type"], "text");
                assert!(json["width"].as_f64().unwrap() > 0.0);
                assert!(json["height"].as_f64().unwrap() > 0.0);
                assert_eq!(json["auto_resize"], "height");
            }
            other => panic!("expected Insert, got {:?}", other),
        }
    }
}
