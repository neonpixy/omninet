use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

/// Text selection within a single digit (cursor or range).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextSelection {
    pub digit_id: Uuid,
    pub start: usize,
    pub end: usize,
}

impl TextSelection {
    /// Create a text selection spanning from `start` to `end` within a digit.
    pub fn new(digit_id: Uuid, start: usize, end: usize) -> Self {
        Self {
            digit_id,
            start,
            end,
        }
    }

    /// Cursor (zero-width selection).
    pub fn cursor(digit_id: Uuid, position: usize) -> Self {
        Self::new(digit_id, position, position)
    }

    /// The number of characters in this selection.
    pub fn length(&self) -> usize {
        self.end.abs_diff(self.start)
    }

    /// Whether this is a zero-width cursor (start equals end).
    pub fn is_collapsed(&self) -> bool {
        self.start == self.end
    }

    /// Ensure start <= end.
    pub fn normalized(&self) -> Self {
        if self.start <= self.end {
            self.clone()
        } else {
            Self {
                digit_id: self.digit_id,
                start: self.end,
                end: self.start,
            }
        }
    }
}

/// Transient UI selection state. NOT part of CRDT — local only.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SelectionState {
    pub selected_digit_ids: HashSet<Uuid>,
    pub focused_digit_id: Option<Uuid>,
    pub text_selection: Option<TextSelection>,
}

impl SelectionState {
    /// Empty selection -- nothing is selected.
    pub fn none() -> Self {
        Self::default()
    }

    /// Select a single digit and focus it.
    pub fn single(digit_id: Uuid) -> Self {
        let mut ids = HashSet::new();
        ids.insert(digit_id);
        Self {
            selected_digit_ids: ids,
            focused_digit_id: Some(digit_id),
            text_selection: None,
        }
    }

    /// Select multiple digits without a focused element.
    pub fn multiple(ids: impl IntoIterator<Item = Uuid>) -> Self {
        Self {
            selected_digit_ids: ids.into_iter().collect(),
            focused_digit_id: None,
            text_selection: None,
        }
    }

    /// Set the focused digit within this selection.
    pub fn with_focus(mut self, digit_id: Uuid) -> Self {
        self.focused_digit_id = Some(digit_id);
        self
    }

    /// Attach a text selection (cursor or range) to this selection state.
    pub fn with_text_selection(mut self, sel: TextSelection) -> Self {
        self.text_selection = Some(sel);
        self
    }

    /// Whether no digits are selected.
    pub fn is_empty(&self) -> bool {
        self.selected_digit_ids.is_empty()
    }

    /// Whether a specific digit is part of the current selection.
    pub fn is_selected(&self, digit_id: Uuid) -> bool {
        self.selected_digit_ids.contains(&digit_id)
    }

    /// Number of selected digits.
    pub fn count(&self) -> usize {
        self.selected_digit_ids.len()
    }

    /// Add a digit to the selection.
    pub fn select(&mut self, digit_id: Uuid) {
        self.selected_digit_ids.insert(digit_id);
    }

    /// Remove a digit from the selection.
    pub fn deselect(&mut self, digit_id: Uuid) {
        self.selected_digit_ids.remove(&digit_id);
    }

    /// Clear all selection state -- digits, focus, and text selection.
    pub fn clear(&mut self) {
        self.selected_digit_ids.clear();
        self.focused_digit_id = None;
        self.text_selection = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_selection() {
        let s = SelectionState::none();
        assert!(s.is_empty());
        assert_eq!(s.count(), 0);
        assert!(s.focused_digit_id.is_none());
    }

    #[test]
    fn single_select() {
        let id = Uuid::new_v4();
        let s = SelectionState::single(id);
        assert!(s.is_selected(id));
        assert_eq!(s.count(), 1);
        assert_eq!(s.focused_digit_id, Some(id));
    }

    #[test]
    fn multi_select() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let s = SelectionState::multiple([a, b]);
        assert!(s.is_selected(a));
        assert!(s.is_selected(b));
        assert_eq!(s.count(), 2);
    }

    #[test]
    fn select_deselect() {
        let id = Uuid::new_v4();
        let mut s = SelectionState::none();
        s.select(id);
        assert!(s.is_selected(id));
        s.deselect(id);
        assert!(!s.is_selected(id));
    }

    #[test]
    fn focus() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let s = SelectionState::multiple([a, b]).with_focus(a);
        assert_eq!(s.focused_digit_id, Some(a));
    }

    #[test]
    fn text_selection_normalized() {
        let id = Uuid::new_v4();
        let sel = TextSelection::new(id, 10, 5);
        let norm = sel.normalized();
        assert_eq!(norm.start, 5);
        assert_eq!(norm.end, 10);
        assert_eq!(norm.length(), 5);
    }

    #[test]
    fn text_selection_collapsed_cursor() {
        let id = Uuid::new_v4();
        let sel = TextSelection::cursor(id, 7);
        assert!(sel.is_collapsed());
        assert_eq!(sel.length(), 0);
    }

    #[test]
    fn clear_resets_everything() {
        let id = Uuid::new_v4();
        let mut s = SelectionState::single(id)
            .with_text_selection(TextSelection::cursor(id, 0));
        s.clear();
        assert!(s.is_empty());
        assert!(s.focused_digit_id.is_none());
        assert!(s.text_selection.is_none());
    }
}
