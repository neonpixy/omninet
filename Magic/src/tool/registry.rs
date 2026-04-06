//! Tool registry — manages registered tools and the active selection.
//!
//! Ported from Swiftlight's `ToolManager`. The registry is extensible:
//! programs register program-specific tools at runtime (e.g., Abacus cell
//! selection tool, Podium slide reorder tool).

use super::traits::Tool;

/// Manages registered tools and the currently active tool.
///
/// # Examples
///
/// ```rust,ignore
/// let mut registry = ToolRegistry::new();
/// registry.register(Box::new(SelectTool::new()));
/// registry.register(Box::new(HandTool::new()));
/// registry.select("select");
/// assert_eq!(registry.active().unwrap().id(), "select");
/// ```
pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
    active_index: Option<usize>,
}

impl ToolRegistry {
    /// Creates an empty registry with no tools registered.
    pub fn new() -> Self {
        Self {
            tools: Vec::new(),
            active_index: None,
        }
    }

    /// Registers a tool. If a tool with the same ID already exists, it is
    /// replaced.
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        // Replace if already registered
        if let Some(idx) = self.tools.iter().position(|t| t.id() == tool.id()) {
            self.tools[idx] = tool;
        } else {
            self.tools.push(tool);
        }
    }

    /// Activates the tool with the given ID.
    ///
    /// Deactivates the current tool first. Returns `true` if the tool was
    /// found and activated.
    pub fn select(&mut self, id: &str) -> bool {
        // Deactivate current
        if let Some(idx) = self.active_index {
            self.tools[idx].deactivate();
        }

        // Find and activate new
        if let Some(idx) = self.tools.iter().position(|t| t.id() == id) {
            self.tools[idx].activate();
            self.active_index = Some(idx);
            true
        } else {
            self.active_index = None;
            false
        }
    }

    /// Returns a reference to the active tool, or `None` if no tool is active.
    pub fn active(&self) -> Option<&dyn Tool> {
        self.active_index
            .and_then(|idx| self.tools.get(idx))
            .map(|t| t.as_ref())
    }

    /// Returns a mutable reference to the active tool, or `None` if no tool
    /// is active.
    pub fn active_mut(&mut self) -> Option<&mut dyn Tool> {
        let idx = self.active_index?;
        Some(self.tools.get_mut(idx)?.as_mut())
    }

    /// Returns the IDs of all registered tools, in registration order.
    pub fn list(&self) -> Vec<&str> {
        self.tools.iter().map(|t| t.id()).collect()
    }

    /// Returns the number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Returns `true` if no tools are registered.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Returns a reference to a tool by ID, or `None` if not found.
    pub fn get(&self, id: &str) -> Option<&dyn Tool> {
        self.tools
            .iter()
            .find(|t| t.id() == id)
            .map(|t| t.as_ref())
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ToolRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolRegistry")
            .field("tool_count", &self.tools.len())
            .field("active_id", &self.active().map(|t| t.id()))
            .field("tool_ids", &self.list())
            .finish()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::types::{CursorStyle, ModifierKeys, ToolAction};
    use crate::ideation::DocumentState;
    use x::geometry::Point;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    struct TestTool {
        tool_id: &'static str,
        activated: Arc<AtomicBool>,
        deactivated: Arc<AtomicBool>,
    }

    impl TestTool {
        fn new(id: &'static str) -> Self {
            Self {
                tool_id: id,
                activated: Arc::new(AtomicBool::new(false)),
                deactivated: Arc::new(AtomicBool::new(false)),
            }
        }
    }

    impl Tool for TestTool {
        fn id(&self) -> &str {
            self.tool_id
        }
        fn display_name(&self) -> &str {
            self.tool_id
        }
        fn cursor(&self) -> CursorStyle {
            CursorStyle::Default
        }
        fn activate(&mut self) {
            self.activated.store(true, Ordering::SeqCst);
        }
        fn deactivate(&mut self) {
            self.deactivated.store(true, Ordering::SeqCst);
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
    fn empty_registry() {
        let reg = ToolRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        assert!(reg.active().is_none());
        assert!(reg.list().is_empty());
    }

    #[test]
    fn register_and_list() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(TestTool::new("alpha")));
        reg.register(Box::new(TestTool::new("beta")));
        assert_eq!(reg.len(), 2);
        assert_eq!(reg.list(), vec!["alpha", "beta"]);
    }

    #[test]
    fn select_activates_tool() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(TestTool::new("select")));
        assert!(reg.select("select"));
        assert_eq!(reg.active().unwrap().id(), "select");
    }

    #[test]
    fn select_nonexistent_returns_false() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(TestTool::new("select")));
        assert!(!reg.select("nonexistent"));
        assert!(reg.active().is_none());
    }

    #[test]
    fn select_deactivates_previous() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(TestTool::new("alpha")));
        reg.register(Box::new(TestTool::new("beta")));

        reg.select("alpha");
        reg.select("beta");

        // Alpha should have been deactivated
        let alpha = reg.get("alpha").unwrap();
        assert_eq!(alpha.id(), "alpha");
        // Beta should be active
        assert_eq!(reg.active().unwrap().id(), "beta");
    }

    #[test]
    fn register_replaces_duplicate_id() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(TestTool::new("dupe")));
        reg.register(Box::new(TestTool::new("dupe")));
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn get_returns_tool() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(TestTool::new("find-me")));
        assert!(reg.get("find-me").is_some());
        assert!(reg.get("not-here").is_none());
    }

    #[test]
    fn active_mut_allows_mutation() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(TestTool::new("mutable")));
        reg.select("mutable");
        let tool = reg.active_mut().unwrap();
        assert_eq!(tool.id(), "mutable");
    }

    #[test]
    fn debug_impl() {
        let mut reg = ToolRegistry::new();
        reg.register(Box::new(TestTool::new("test")));
        let debug = format!("{:?}", reg);
        assert!(debug.contains("ToolRegistry"));
        assert!(debug.contains("test"));
    }
}
