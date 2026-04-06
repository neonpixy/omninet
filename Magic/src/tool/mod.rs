//! Canvas tool system — traits, types, registry, and core tools.
//!
//! Ported from Swiftlight's tool architecture. Tools are stateful objects
//! registered in a [`ToolRegistry`]. The active tool receives pointer events
//! and returns [`ToolAction`] values that the host applies.
//!
//! # Core Tools
//!
//! | Tool | ID | Description |
//! |------|----|-------------|
//! | [`SelectTool`] | `select` | Click to select, drag to move, shift for multi-select, marquee |
//! | [`PenTool`] | `pen` | Bezier path drawing, point by point |
//! | [`ShapeTool`] | `shape-*` | Rectangle, ellipse, line (parameterized by [`ShapeKind`]) |
//! | [`TextTool`] | `text` | Click to create text, drag to create text box |
//! | [`HandTool`] | `hand` | Pan canvas by dragging |
//! | [`ZoomTool`] | `zoom` | Click to zoom in, alt+click to zoom out |
//!
//! # Extensibility
//!
//! Programs register custom tools via the registry:
//! ```rust,ignore
//! registry.register(Box::new(AbacusCellTool::new()));
//! registry.register(Box::new(PodiumSlideReorderTool::new()));
//! ```

mod hand;
mod pen;
mod registry;
mod select;
mod shape;
mod text;
mod traits;
mod types;
mod zoom;

// Re-export core tools
pub use hand::HandTool;
pub use pen::{PathPoint, PenTool};
pub use select::SelectTool;
pub use shape::ShapeTool;
pub use text::TextTool;
pub use zoom::ZoomTool;

// Re-export the trait
pub use traits::Tool;

// Re-export types
pub use types::{CursorStyle, DragState, ModifierKeys, ShapeKind, ToolAction};

// Re-export the registry
pub use registry::ToolRegistry;

/// Creates a [`ToolRegistry`] pre-loaded with all core tools.
///
/// This is the standard setup. Programs can add more tools afterward.
pub fn default_tool_registry() -> ToolRegistry {
    let mut reg = ToolRegistry::new();
    reg.register(Box::new(SelectTool::new()));
    reg.register(Box::new(PenTool::new()));
    reg.register(Box::new(ShapeTool::new(ShapeKind::Rectangle)));
    reg.register(Box::new(ShapeTool::new(ShapeKind::Ellipse)));
    reg.register(Box::new(ShapeTool::new(ShapeKind::Line)));
    reg.register(Box::new(TextTool::new()));
    reg.register(Box::new(HandTool::new()));
    reg.register(Box::new(ZoomTool::new()));
    reg
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_has_all_tools() {
        let reg = default_tool_registry();
        let ids = reg.list();

        assert!(ids.contains(&"select"));
        assert!(ids.contains(&"pen"));
        assert!(ids.contains(&"shape-rectangle"));
        assert!(ids.contains(&"shape-ellipse"));
        assert!(ids.contains(&"shape-line"));
        assert!(ids.contains(&"text"));
        assert!(ids.contains(&"hand"));
        assert!(ids.contains(&"zoom"));
        assert_eq!(reg.len(), 8);
    }

    #[test]
    fn default_registry_tools_are_selectable() {
        let mut reg = default_tool_registry();
        assert!(reg.select("select"));
        assert_eq!(reg.active().unwrap().id(), "select");

        assert!(reg.select("hand"));
        assert_eq!(reg.active().unwrap().id(), "hand");
    }
}
