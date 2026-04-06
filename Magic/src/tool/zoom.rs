//! Zoom tool — click to zoom in, alt+click to zoom out.
//!
//! Ported from Swiftlight's `ZoomTool.swift`. The tool emits `Zoom`
//! actions that the host applies to the viewport.

use x::geometry::Point;

use crate::ideation::DocumentState;
use super::traits::Tool;
use super::types::{CursorStyle, ModifierKeys, ToolAction};

/// Tool for zooming the canvas.
///
/// Behaviors:
/// - **Click** — zoom in by 1.5x at the click point
/// - **Alt+click** — zoom out by 1/1.5x at the click point
/// - **Hover** — shows ZoomIn or ZoomOut cursor based on alt key
pub struct ZoomTool {
    /// Current alt-key state for cursor display.
    alt_held: bool,
    /// Zoom-in factor per click.
    zoom_in_factor: f64,
}

impl ZoomTool {
    /// Creates a new zoom tool.
    pub fn new() -> Self {
        Self {
            alt_held: false,
            zoom_in_factor: 1.5,
        }
    }
}

impl Default for ZoomTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ZoomTool {
    fn id(&self) -> &str {
        "zoom"
    }

    fn display_name(&self) -> &str {
        "Zoom"
    }

    fn cursor(&self) -> CursorStyle {
        if self.alt_held {
            CursorStyle::ZoomOut
        } else {
            CursorStyle::ZoomIn
        }
    }

    fn activate(&mut self) {
        self.alt_held = false;
    }

    fn deactivate(&mut self) {
        self.alt_held = false;
    }

    fn on_press(
        &mut self,
        point: Point,
        modifiers: ModifierKeys,
        _state: &DocumentState,
    ) -> ToolAction {
        self.alt_held = modifiers.alt;

        let factor = if modifiers.alt {
            1.0 / self.zoom_in_factor
        } else {
            self.zoom_in_factor
        };

        ToolAction::Zoom {
            factor,
            center: point,
        }
    }

    fn on_hover(
        &self,
        _point: Point,
        _state: &DocumentState,
    ) -> Option<CursorStyle> {
        // The cursor depends on alt key state, which is tracked via on_press
        // modifiers. The host can also update cursor based on platform key
        // events.
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_identity() {
        let tool = ZoomTool::new();
        assert_eq!(tool.id(), "zoom");
        assert_eq!(tool.display_name(), "Zoom");
        assert_eq!(tool.cursor(), CursorStyle::ZoomIn);
    }

    #[test]
    fn click_zooms_in() {
        let mut tool = ZoomTool::new();
        let state = DocumentState::new("test");
        let mods = ModifierKeys::default();

        let action = tool.on_press(Point::new(100.0, 100.0), mods, &state);
        match action {
            ToolAction::Zoom { factor, center } => {
                assert!((factor - 1.5).abs() < 1e-10);
                assert_eq!(center, Point::new(100.0, 100.0));
            }
            other => panic!("expected Zoom, got {:?}", other),
        }
    }

    #[test]
    fn alt_click_zooms_out() {
        let mut tool = ZoomTool::new();
        let state = DocumentState::new("test");
        let mods = ModifierKeys {
            alt: true,
            ..Default::default()
        };

        let action = tool.on_press(Point::new(50.0, 50.0), mods, &state);
        match action {
            ToolAction::Zoom { factor, center } => {
                assert!((factor - 1.0 / 1.5).abs() < 1e-10);
                assert_eq!(center, Point::new(50.0, 50.0));
            }
            other => panic!("expected Zoom, got {:?}", other),
        }
    }

    #[test]
    fn alt_changes_cursor() {
        let mut tool = ZoomTool::new();
        let state = DocumentState::new("test");

        // Default: zoom in
        assert_eq!(tool.cursor(), CursorStyle::ZoomIn);

        // After alt+press: zoom out
        let mods = ModifierKeys {
            alt: true,
            ..Default::default()
        };
        tool.on_press(Point::ZERO, mods, &state);
        assert_eq!(tool.cursor(), CursorStyle::ZoomOut);

        // After normal press: zoom in again
        tool.on_press(Point::ZERO, ModifierKeys::default(), &state);
        assert_eq!(tool.cursor(), CursorStyle::ZoomIn);
    }
}
