//! Select tool — click to select, drag to move, shift-click for multi-select,
//! drag on empty space for selection rectangle (marquee).
//!
//! Ported from Swiftlight's `SelectTool.swift`, simplified for the Rust
//! protocol layer. Platform-specific concerns (cursor management, overlay
//! rendering) are handled by Divinity.

use uuid::Uuid;

use x::geometry::{Point, Rect};

use crate::ideation::DocumentState;
use super::traits::Tool;
use super::types::{CursorStyle, DragState, ModifierKeys, ToolAction};

/// Internal state machine for the select tool.
#[derive(Debug, Clone, PartialEq)]
enum Mode {
    /// Idle — no interaction in progress.
    Idle,
    /// Clicked on a digit — may become a move or stay as select.
    Clicked { target_id: Uuid },
    /// Dragging selected digits to move them.
    Moving { ids: Vec<Uuid> },
    /// Marquee selection — dragging on empty space.
    Marquee,
}

/// The primary selection tool.
///
/// Behaviors:
/// - **Click on digit** — select it (replacing existing selection)
/// - **Shift+click on digit** — toggle selection (multi-select)
/// - **Drag on digit** — move selected digits
/// - **Drag on empty space** — marquee (rubber-band) selection
pub struct SelectTool {
    mode: Mode,
    drag: Option<DragState>,
    /// Threshold (canvas units) before a click becomes a drag.
    drag_threshold: f64,
}

impl SelectTool {
    /// Creates a new select tool.
    pub fn new() -> Self {
        Self {
            mode: Mode::Idle,
            drag: None,
            drag_threshold: 3.0,
        }
    }

    /// Hit-test: find the first digit whose bounds contain the point.
    ///
    /// Skips tombstoned (deleted) digits.
    fn hit_test(point: Point, state: &DocumentState) -> Option<Uuid> {
        for digit in state.digits() {
            if digit.tombstone {
                continue;
            }
            if let Some(bounds) = Self::digit_bounds(digit, state) {
                if bounds.contains_point(point) {
                    return Some(digit.id());
                }
            }
        }
        None
    }

    /// Extract bounds for a digit from its properties.
    fn digit_bounds(
        digit: &ideas::Digit,
        _state: &DocumentState,
    ) -> Option<Rect> {
        let props = &digit.properties;
        let x = props.get("x").and_then(|v| v.as_double())?;
        let y = props.get("y").and_then(|v| v.as_double())?;
        let w = props
            .get("width")
            .and_then(|v| v.as_double())
            .unwrap_or(100.0);
        let h = props
            .get("height")
            .and_then(|v| v.as_double())
            .unwrap_or(40.0);
        Some(Rect::new(x, y, w, h))
    }
}

impl Default for SelectTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for SelectTool {
    fn id(&self) -> &str {
        "select"
    }

    fn display_name(&self) -> &str {
        "Select"
    }

    fn cursor(&self) -> CursorStyle {
        CursorStyle::Default
    }

    fn activate(&mut self) {
        self.mode = Mode::Idle;
        self.drag = None;
    }

    fn deactivate(&mut self) {
        self.mode = Mode::Idle;
        self.drag = None;
    }

    fn on_press(
        &mut self,
        point: Point,
        modifiers: ModifierKeys,
        state: &DocumentState,
    ) -> ToolAction {
        self.drag = Some(DragState::new(point, modifiers));

        if let Some(target_id) = Self::hit_test(point, state) {
            // Shift-click: toggle selection
            if modifiers.shift {
                if state.selection.is_selected(target_id) {
                    // Will deselect on release (so shift+drag still moves)
                    self.mode = Mode::Clicked { target_id };
                    return ToolAction::None;
                }
                self.mode = Mode::Clicked { target_id };
                // Add to selection
                let mut ids: Vec<Uuid> =
                    state.selection.selected_digit_ids.iter().copied().collect();
                ids.push(target_id);
                return ToolAction::Select(ids);
            }

            // Click on an already-selected digit: may become a move
            if state.selection.is_selected(target_id) {
                self.mode = Mode::Clicked { target_id };
                return ToolAction::None;
            }

            // Click on an unselected digit: select it
            self.mode = Mode::Clicked { target_id };
            ToolAction::Select(vec![target_id])
        } else {
            // Click on empty space: begin marquee
            self.mode = Mode::Marquee;
            if modifiers.shift {
                // Shift-click on empty: keep existing selection
                ToolAction::None
            } else {
                ToolAction::Deselect
            }
        }
    }

    fn on_drag(
        &mut self,
        point: Point,
        modifiers: ModifierKeys,
        state: &DocumentState,
    ) -> ToolAction {
        if let Some(ref mut drag) = self.drag {
            drag.current = point;
            drag.modifiers = modifiers;
        }

        let drag = match &self.drag {
            Some(d) => d,
            None => return ToolAction::None,
        };

        // Haven't exceeded threshold yet
        if drag.distance() < self.drag_threshold {
            return ToolAction::None;
        }

        match &self.mode {
            Mode::Clicked { target_id } => {
                // Transition to moving
                let ids: Vec<Uuid> = if state.selection.is_selected(*target_id) {
                    state.selection.selected_digit_ids.iter().copied().collect()
                } else {
                    vec![*target_id]
                };
                self.mode = Mode::Moving { ids: ids.clone() };
                let delta = drag.delta();
                ToolAction::Move { ids, delta }
            }
            Mode::Moving { ids } => {
                let delta = drag.delta();
                ToolAction::Move {
                    ids: ids.clone(),
                    delta,
                }
            }
            Mode::Marquee => {
                // Find all digits intersecting the marquee rect
                let marquee = drag.rect();
                let mut hit_ids = Vec::new();
                for digit in state.digits() {
                    if let Some(bounds) = Self::digit_bounds(digit, state) {
                        if marquee.intersects(bounds) {
                            hit_ids.push(digit.id());
                        }
                    }
                }
                ToolAction::Select(hit_ids)
            }
            Mode::Idle => ToolAction::None,
        }
    }

    fn on_release(
        &mut self,
        _point: Point,
        modifiers: ModifierKeys,
        state: &DocumentState,
    ) -> ToolAction {
        let result = match &self.mode {
            Mode::Clicked { target_id } if modifiers.shift => {
                // Shift+click on already-selected: toggle off
                if state.selection.is_selected(*target_id) {
                    let ids: Vec<Uuid> = state
                        .selection
                        .selected_digit_ids
                        .iter()
                        .copied()
                        .filter(|id| id != target_id)
                        .collect();
                    if ids.is_empty() {
                        ToolAction::Deselect
                    } else {
                        ToolAction::Select(ids)
                    }
                } else {
                    ToolAction::None
                }
            }
            _ => ToolAction::None,
        };

        self.mode = Mode::Idle;
        self.drag = None;
        result
    }

    fn on_hover(
        &self,
        point: Point,
        state: &DocumentState,
    ) -> Option<CursorStyle> {
        if Self::hit_test(point, state).is_some() {
            Some(CursorStyle::Move)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x::Value;

    /// Helper that creates a state with one digit and returns both.
    fn make_state_with_digit_returning_id(x: f64, y: f64) -> (DocumentState, Uuid) {
        let mut state = DocumentState::new("test");
        let mut digit = ideas::Digit::new(
            "rectangle".to_string(),
            x::Value::Null,
            "test".to_string(),
        )
        .unwrap();
        digit.properties.insert("x".to_string(), Value::Double(x));
        digit.properties.insert("y".to_string(), Value::Double(y));
        digit.properties.insert("width".to_string(), Value::Double(100.0));
        digit.properties.insert("height".to_string(), Value::Double(50.0));
        let id = digit.id();
        state.insert_digit(digit, None).unwrap();
        (state, id)
    }

    #[test]
    fn click_on_digit_selects() {
        let mut tool = SelectTool::new();
        let (state, id) = make_state_with_digit_returning_id(50.0, 25.0);

        let action = tool.on_press(Point::new(50.0, 25.0), ModifierKeys::default(), &state);
        match action {
            ToolAction::Select(ids) => assert_eq!(ids, vec![id]),
            other => panic!("expected Select, got {:?}", other),
        }
    }

    #[test]
    fn click_on_empty_deselects() {
        let mut tool = SelectTool::new();
        let state = DocumentState::new("test");

        let action = tool.on_press(Point::new(500.0, 500.0), ModifierKeys::default(), &state);
        assert_eq!(action, ToolAction::Deselect);
    }

    #[test]
    fn shift_click_empty_keeps_selection() {
        let mut tool = SelectTool::new();
        let state = DocumentState::new("test");
        let mods = ModifierKeys {
            shift: true,
            ..Default::default()
        };

        let action = tool.on_press(Point::new(500.0, 500.0), mods, &state);
        assert!(action.is_none());
    }

    #[test]
    fn tool_identity() {
        let tool = SelectTool::new();
        assert_eq!(tool.id(), "select");
        assert_eq!(tool.display_name(), "Select");
        assert_eq!(tool.cursor(), CursorStyle::Default);
    }

    #[test]
    fn hover_over_digit_shows_move_cursor() {
        let tool = SelectTool::new();
        let (state, _id) = make_state_with_digit_returning_id(50.0, 25.0);

        let cursor = tool.on_hover(Point::new(50.0, 25.0), &state);
        assert_eq!(cursor, Some(CursorStyle::Move));
    }

    #[test]
    fn hover_on_empty_returns_none() {
        let tool = SelectTool::new();
        let state = DocumentState::new("test");

        let cursor = tool.on_hover(Point::new(500.0, 500.0), &state);
        assert_eq!(cursor, None);
    }
}
