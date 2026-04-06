//! The core Tool trait — the contract every canvas tool implements.
//!
//! Ported from Swiftlight's `Tool` protocol, adapted for Rust's trait
//! system with `Send + Sync` bounds for cross-thread safety.
//!
//! Tools are stateful objects that respond to press/drag/release/hover
//! events in canvas coordinates. They return [`ToolAction`] values
//! describing what the host should do — tools never directly mutate
//! shared document state.

use x::geometry::Point;

use crate::ideation::DocumentState;
use super::types::{CursorStyle, ModifierKeys, ToolAction};

/// A canvas tool that responds to pointer events.
///
/// Tools are registered with a [`ToolRegistry`](super::registry::ToolRegistry)
/// and activated one at a time. The active tool receives all pointer events
/// in canvas coordinates.
///
/// # Design Principles
///
/// - **Tools are stateful** — they own their drag state, preview geometry, etc.
/// - **Tools are declarative** — they return [`ToolAction`] values, never
///   directly mutating the document. The host applies actions.
/// - **Tools are extensible** — programs register custom tools via the registry
///   (e.g., Abacus cell selection tool, Podium slide reorder tool).
///
/// # Examples
///
/// ```rust,ignore
/// struct MyTool;
///
/// impl Tool for MyTool {
///     fn id(&self) -> &str { "my-tool" }
///     fn display_name(&self) -> &str { "My Tool" }
///     fn cursor(&self) -> CursorStyle { CursorStyle::Crosshair }
///
///     fn on_press(&mut self, point: Point, _modifiers: ModifierKeys, _state: &DocumentState) -> ToolAction {
///         ToolAction::Select(vec![])
///     }
/// }
/// ```
pub trait Tool: Send + Sync {
    /// Unique identifier for this tool (e.g. "select", "pen", "shape-rectangle").
    fn id(&self) -> &str;

    /// Human-readable name shown in UI (e.g. "Select", "Pen", "Rectangle").
    fn display_name(&self) -> &str;

    /// The cursor to display when this tool is active and idle.
    fn cursor(&self) -> CursorStyle;

    /// Called when the tool becomes active (selected in the toolbar).
    ///
    /// Use this to reset internal state, show relevant UI, etc.
    fn activate(&mut self) {
        // Default no-op.
    }

    /// Called when the tool becomes inactive (another tool selected).
    ///
    /// Use this to clean up previews, commit pending paths, etc.
    fn deactivate(&mut self) {
        // Default no-op.
    }

    /// Called when the pointer presses down on the canvas.
    ///
    /// `point` is in canvas coordinates. `modifiers` captures shift/alt/cmd.
    /// `state` is a read-only view of the document for hit-testing.
    fn on_press(
        &mut self,
        point: Point,
        modifiers: ModifierKeys,
        state: &DocumentState,
    ) -> ToolAction;

    /// Called while the pointer is dragged (held down and moving).
    fn on_drag(
        &mut self,
        point: Point,
        modifiers: ModifierKeys,
        state: &DocumentState,
    ) -> ToolAction {
        let _ = (point, modifiers, state);
        ToolAction::None
    }

    /// Called when the pointer is released.
    fn on_release(
        &mut self,
        point: Point,
        modifiers: ModifierKeys,
        state: &DocumentState,
    ) -> ToolAction {
        let _ = (point, modifiers, state);
        ToolAction::None
    }

    /// Called when the pointer moves without a button pressed (hover).
    ///
    /// Returns an optional cursor override. `None` means keep the tool's
    /// default cursor.
    fn on_hover(
        &self,
        point: Point,
        state: &DocumentState,
    ) -> Option<CursorStyle> {
        let _ = (point, state);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal tool implementation for testing the trait.
    struct StubTool;

    impl Tool for StubTool {
        fn id(&self) -> &str {
            "stub"
        }
        fn display_name(&self) -> &str {
            "Stub"
        }
        fn cursor(&self) -> CursorStyle {
            CursorStyle::Default
        }
        fn on_press(
            &mut self,
            _point: Point,
            _modifiers: ModifierKeys,
            _state: &DocumentState,
        ) -> ToolAction {
            ToolAction::None
        }
    }

    #[test]
    fn stub_tool_identity() {
        let t = StubTool;
        assert_eq!(t.id(), "stub");
        assert_eq!(t.display_name(), "Stub");
        assert_eq!(t.cursor(), CursorStyle::Default);
    }

    #[test]
    fn default_methods_return_none() {
        let mut t = StubTool;
        let state = DocumentState::new("test");
        let mods = ModifierKeys::default();
        let p = Point::ZERO;

        assert!(t.on_drag(p, mods, &state).is_none());
        assert!(t.on_release(p, mods, &state).is_none());
        assert!(t.on_hover(p, &state).is_none());
    }

    #[test]
    fn tool_is_object_safe() {
        // Ensure Tool can be used as dyn Tool.
        let t: Box<dyn Tool> = Box::new(StubTool);
        assert_eq!(t.id(), "stub");
    }
}
