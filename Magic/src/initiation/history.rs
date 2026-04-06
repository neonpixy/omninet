use ideas::crdt::DigitOperation;

use super::action::Action;

/// A recorded step: the operation + the inverse action for undo.
#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub operation: DigitOperation,
    pub inverse: Action,
}

/// Undo/redo stacks with max depth.
pub struct DocumentHistory {
    undo_stack: Vec<HistoryEntry>,
    redo_stack: Vec<HistoryEntry>,
    max_depth: usize,
}

impl DocumentHistory {
    pub const DEFAULT_MAX_DEPTH: usize = 100;

    /// Create a new history with the default max depth (100 entries).
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_depth: Self::DEFAULT_MAX_DEPTH,
        }
    }

    /// Create a new history with a custom max depth.
    pub fn with_max_depth(max_depth: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            max_depth,
        }
    }

    /// Push an entry to the undo stack. Clears the redo stack.
    pub fn record(&mut self, entry: HistoryEntry) {
        self.redo_stack.clear();
        self.undo_stack.push(entry);
        if self.undo_stack.len() > self.max_depth {
            self.undo_stack.remove(0);
        }
    }

    /// Pop from undo stack.
    pub fn pop_undo(&mut self) -> Option<HistoryEntry> {
        self.undo_stack.pop()
    }

    /// Push an entry to the redo stack.
    pub fn push_redo(&mut self, entry: HistoryEntry) {
        self.redo_stack.push(entry);
    }

    /// Pop from redo stack.
    pub fn pop_redo(&mut self) -> Option<HistoryEntry> {
        self.redo_stack.pop()
    }

    /// Whether there are entries available to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Whether there are entries available to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Number of entries on the undo stack.
    pub fn undo_count(&self) -> usize {
        self.undo_stack.len()
    }

    /// Number of entries on the redo stack.
    pub fn redo_count(&self) -> usize {
        self.redo_stack.len()
    }

    /// Clear both the undo and redo stacks.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }
}

impl Default for DocumentHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use ideas::Digit;
    use uuid::Uuid;
    use x::{Value, VectorClock};

    fn make_entry() -> HistoryEntry {
        let digit = Digit::new("text".into(), Value::from("x"), "cpub1test".into()).unwrap();
        let digit_json = serde_json::to_value(&digit).unwrap();
        let op = ideas::crdt::DigitOperation {
            id: Uuid::new_v4(),
            digit_id: digit.id(),
            operation_type: ideas::crdt::OperationType::Insert,
            payload: ideas::crdt::OperationPayload::Insert {
                digit_json,
                parent_id: None,
                index: None,
            },
            author: "cpub1test".into(),
            timestamp: Utc::now(),
            vector: VectorClock::new(),
            signature: None,
        };
        HistoryEntry {
            operation: op,
            inverse: Action::delete(digit.id()),
        }
    }

    #[test]
    fn empty_history() {
        let h = DocumentHistory::new();
        assert!(!h.can_undo());
        assert!(!h.can_redo());
        assert_eq!(h.undo_count(), 0);
        assert_eq!(h.redo_count(), 0);
    }

    #[test]
    fn record_then_undo() {
        let mut h = DocumentHistory::new();
        h.record(make_entry());
        assert!(h.can_undo());
        assert_eq!(h.undo_count(), 1);
        let entry = h.pop_undo().unwrap();
        assert!(matches!(entry.inverse, Action::DeleteDigit { .. }));
        assert!(!h.can_undo());
    }

    #[test]
    fn undo_then_redo() {
        let mut h = DocumentHistory::new();
        h.record(make_entry());
        let entry = h.pop_undo().unwrap();
        h.push_redo(entry);
        assert!(h.can_redo());
        assert_eq!(h.redo_count(), 1);
        let _re = h.pop_redo().unwrap();
        assert!(!h.can_redo());
    }

    #[test]
    fn record_clears_redo() {
        let mut h = DocumentHistory::new();
        h.record(make_entry());
        let entry = h.pop_undo().unwrap();
        h.push_redo(entry);
        assert!(h.can_redo());
        // New record should clear redo
        h.record(make_entry());
        assert!(!h.can_redo());
    }

    #[test]
    fn max_depth_enforced() {
        let mut h = DocumentHistory::with_max_depth(3);
        for _ in 0..5 {
            h.record(make_entry());
        }
        assert_eq!(h.undo_count(), 3);
    }

    #[test]
    fn clear_empties_both() {
        let mut h = DocumentHistory::new();
        h.record(make_entry());
        let entry = h.pop_undo().unwrap();
        h.push_redo(entry);
        h.record(make_entry());
        h.clear();
        assert!(!h.can_undo());
        assert!(!h.can_redo());
    }

    #[test]
    fn pop_undo_returns_none_when_empty() {
        let mut h = DocumentHistory::new();
        assert!(h.pop_undo().is_none());
    }

    #[test]
    fn pop_redo_returns_none_when_empty() {
        let mut h = DocumentHistory::new();
        assert!(h.pop_redo().is_none());
    }
}
