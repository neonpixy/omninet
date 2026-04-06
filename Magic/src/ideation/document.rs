use std::collections::HashMap;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use ideas::crdt::{DigitOperation, OperationPayload, OperationType};
use ideas::Digit;
use x::{CrdtEngine, CrdtOperation, Value, VectorClock};

use crate::error::MagicError;
use super::selection::SelectionState;
use super::type_registry::DigitTypeRegistry;

/// Layout mode for the document canvas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[derive(Default)]
pub enum LayoutMode {
    /// Vertical flow, top to bottom (like a word processor).
    #[default]
    Vertical,
    /// Horizontal flow, left to right (like a timeline).
    Horizontal,
    /// Grid-snapped placement.
    Grid,
    /// Free-form canvas with absolute positioning.
    Freeform,
    /// Ordered slide sequence for presentations (Podium).
    /// Sequence logic is in `SlideSequenceState`.
    SlideSequence,
}


/// Document layout configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DocumentLayout {
    pub mode: LayoutMode,
    pub page_width: f64,
    pub page_height: f64,
    pub margin_top: f64,
    pub margin_bottom: f64,
    pub margin_left: f64,
    pub margin_right: f64,
    pub grid_snap: Option<f64>,
}

impl Default for DocumentLayout {
    fn default() -> Self {
        Self {
            mode: LayoutMode::Vertical,
            page_width: 595.0,  // A4
            page_height: 842.0,
            margin_top: 40.0,
            margin_bottom: 40.0,
            margin_left: 40.0,
            margin_right: 40.0,
            grid_snap: None,
        }
    }
}

/// The single source of truth for document state.
///
/// All content mutations go through `apply()`. Selection is transient (NOT CRDT).
pub struct DocumentState {
    digits: HashMap<Uuid, Digit>,
    root_digit_id: Option<Uuid>,
    pub selection: SelectionState,
    pub layout: DocumentLayout,
    vector: VectorClock,
    type_registry: DigitTypeRegistry,
    current_author: String,
    crdt_engine: CrdtEngine,
}

impl DocumentState {
    /// Create a new, empty document state for the given author.
    pub fn new(author: impl Into<String>) -> Self {
        Self {
            digits: HashMap::new(),
            root_digit_id: None,
            selection: SelectionState::none(),
            layout: DocumentLayout::default(),
            vector: VectorClock::new(),
            type_registry: DigitTypeRegistry::with_core_types(),
            current_author: author.into(),
            crdt_engine: CrdtEngine::new(),
        }
    }

    /// Replace the digit type registry for this document.
    pub fn with_registry(mut self, registry: DigitTypeRegistry) -> Self {
        self.type_registry = registry;
        self
    }

    /// Set the document layout configuration (page size, margins, mode).
    pub fn with_layout(mut self, layout: DocumentLayout) -> Self {
        self.layout = layout;
        self
    }

    // --- Read access ---

    /// Look up a digit by its ID.
    pub fn digit(&self, id: Uuid) -> Option<&Digit> {
        self.digits.get(&id)
    }

    /// Iterate over all digits in the document.
    pub fn digits(&self) -> impl Iterator<Item = &Digit> {
        self.digits.values()
    }

    /// Number of digits in the document.
    pub fn digit_count(&self) -> usize {
        self.digits.len()
    }

    /// The ID of the root digit, if one has been set.
    pub fn root_digit_id(&self) -> Option<Uuid> {
        self.root_digit_id
    }

    /// The root digit itself, if one exists.
    pub fn root_digit(&self) -> Option<&Digit> {
        self.root_digit_id.and_then(|id| self.digits.get(&id))
    }

    /// Get the children of a digit, ordered by the parent's children list.
    pub fn children_of(&self, parent_id: Uuid) -> Vec<&Digit> {
        self.digits
            .get(&parent_id)
            .and_then(|d| d.children.as_ref())
            .map(|ids| ids.iter().filter_map(|id| self.digits.get(id)).collect())
            .unwrap_or_default()
    }

    /// The current vector clock for CRDT ordering.
    pub fn vector(&self) -> &VectorClock {
        &self.vector
    }

    /// The current author identifier for operations.
    pub fn author(&self) -> &str {
        &self.current_author
    }

    /// Access the digit type registry (read-only).
    pub fn type_registry(&self) -> &DigitTypeRegistry {
        &self.type_registry
    }

    /// Access the digit type registry (mutable, for registering custom types).
    pub fn type_registry_mut(&mut self) -> &mut DigitTypeRegistry {
        &mut self.type_registry
    }

    // --- Mutation (CRDT-managed) ---

    /// Apply a DigitOperation. Returns Ok(true) if applied, Ok(false) if duplicate.
    pub fn apply(&mut self, operation: &DigitOperation) -> Result<bool, MagicError> {
        if self.crdt_engine.has_applied(operation.id()) {
            return Ok(false);
        }
        self.apply_operation(operation)?;
        self.vector.merge(operation.vector());
        self.crdt_engine.mark_applied(operation.id());
        Ok(true)
    }

    /// Insert a digit, returning the generated operation.
    pub fn insert_digit(
        &mut self,
        digit: Digit,
        parent_id: Option<Uuid>,
    ) -> Result<DigitOperation, MagicError> {
        let digit_json = serde_json::to_value(&digit).map_err(|e| {
            MagicError::Serialization(e.to_string())
        })?;
        self.vector.increment(&self.current_author);
        let op = DigitOperation {
            id: Uuid::new_v4(),
            digit_id: digit.id(),
            operation_type: OperationType::Insert,
            payload: OperationPayload::Insert {
                digit_json,
                parent_id,
                index: None,
            },
            author: self.current_author.clone(),
            timestamp: Utc::now(),
            vector: self.vector.clone(),
            signature: None,
        };
        self.apply(&op)?;
        Ok(op)
    }

    /// Update a digit field, returning the generated operation.
    pub fn update_digit(
        &mut self,
        digit_id: Uuid,
        field: String,
        old_value: Value,
        new_value: Value,
    ) -> Result<DigitOperation, MagicError> {
        if !self.digits.contains_key(&digit_id) {
            return Err(MagicError::DigitNotFound(digit_id));
        }
        self.vector.increment(&self.current_author);
        let op = DigitOperation::update(
            digit_id,
            field,
            old_value,
            new_value,
            self.current_author.clone(),
            self.vector.clone(),
        );
        self.apply(&op)?;
        Ok(op)
    }

    /// Delete a digit, returning the generated operation.
    pub fn delete_digit(&mut self, digit_id: Uuid) -> Result<DigitOperation, MagicError> {
        if !self.digits.contains_key(&digit_id) {
            return Err(MagicError::DigitNotFound(digit_id));
        }
        self.vector.increment(&self.current_author);
        let op = DigitOperation::delete(
            digit_id,
            true, // tombstone (soft delete)
            self.current_author.clone(),
            self.vector.clone(),
        );
        self.apply(&op)?;
        Ok(op)
    }

    /// Load a set of digits wholesale (for deserialization).
    pub fn load_digits(&mut self, digits: Vec<Digit>, root_id: Option<Uuid>) {
        self.digits.clear();
        for d in digits {
            self.digits.insert(d.id(), d);
        }
        self.root_digit_id = root_id;
    }

    // --- Internal operation application ---

    fn apply_operation(&mut self, op: &DigitOperation) -> Result<(), MagicError> {
        match &op.payload {
            OperationPayload::Insert {
                digit_json,
                parent_id,
                index,
            } => {
                let digit: Digit = serde_json::from_value(digit_json.clone())
                    .map_err(|e| MagicError::Serialization(e.to_string()))?;
                let digit_id = digit.id();
                self.digits.insert(digit_id, digit);

                if let Some(pid) = parent_id {
                    if let Some(parent) = self.digits.get(pid) {
                        let updated = if let Some(idx) = index {
                            // Insert at specific index
                            let mut children = parent.children.clone().unwrap_or_default();
                            let clamped = (*idx).min(children.len());
                            children.insert(clamped, digit_id);
                            let mut p = parent.clone();
                            p.children = Some(children);
                            p
                        } else {
                            parent.with_child(digit_id, &op.author)
                        };
                        self.digits.insert(*pid, updated);
                    }
                } else if self.root_digit_id.is_none() {
                    self.root_digit_id = Some(digit_id);
                }
            }
            OperationPayload::Update {
                field,
                new_value,
                ..
            } => {
                let digit = self
                    .digits
                    .get(&op.digit_id)
                    .ok_or(MagicError::DigitNotFound(op.digit_id))?;

                let updated = if field == "content" {
                    digit.with_content(new_value.clone(), &op.author)
                } else {
                    digit.with_property(field.clone(), new_value.clone(), &op.author)
                };
                self.digits.insert(op.digit_id, updated);
            }
            OperationPayload::Delete { tombstone } => {
                if *tombstone {
                    if let Some(digit) = self.digits.get(&op.digit_id) {
                        let deleted = digit.deleted(&op.author);
                        self.digits.insert(op.digit_id, deleted);
                    }
                } else {
                    self.digits.remove(&op.digit_id);
                }
                // Remove from parent's children
                self.remove_from_parent(op.digit_id);
                if self.root_digit_id == Some(op.digit_id) {
                    self.root_digit_id = None;
                }
                // Clear selection if deleted
                self.selection.deselect(op.digit_id);
            }
            OperationPayload::Move {
                from_parent,
                to_parent,
                to_index,
            } => {
                // Remove from old parent
                if let Some(pid) = from_parent {
                    self.remove_child_from(*pid, op.digit_id);
                }
                // Add to new parent
                if let Some(pid) = to_parent {
                    if let Some(parent) = self.digits.get(pid) {
                        let updated = if let Some(idx) = to_index {
                            let mut children = parent.children.clone().unwrap_or_default();
                            let clamped = (*idx).min(children.len());
                            children.insert(clamped, op.digit_id);
                            let mut p = parent.clone();
                            p.children = Some(children);
                            p
                        } else {
                            parent.with_child(op.digit_id, &op.author)
                        };
                        self.digits.insert(*pid, updated);
                    }
                } else {
                    self.root_digit_id = Some(op.digit_id);
                }
            }
            OperationPayload::Transform {
                x,
                y,
                width,
                height,
                rotation,
                z_index,
            } => {
                let digit = self
                    .digits
                    .get(&op.digit_id)
                    .ok_or(MagicError::DigitNotFound(op.digit_id))?;
                let mut updated = digit
                    .with_property("x".into(), Value::Double(*x), &op.author);
                updated = updated.with_property("y".into(), Value::Double(*y), &op.author);
                if let Some(w) = width {
                    updated = updated.with_property("width".into(), Value::Double(*w), &op.author);
                }
                if let Some(h) = height {
                    updated =
                        updated.with_property("height".into(), Value::Double(*h), &op.author);
                }
                updated = updated.with_property(
                    "rotation".into(),
                    Value::Double(*rotation),
                    &op.author,
                );
                updated = updated.with_property(
                    "z_index".into(),
                    Value::Int(i64::from(*z_index)),
                    &op.author,
                );
                self.digits.insert(op.digit_id, updated);
            }
            // Character-level text edits — applied via sequence CRDT.
            // The actual text merging happens in the program layer (TypeScript editor)
            // which owns the SequenceRga instance. Here we just update the digit's
            // text property with the result.
            OperationPayload::TextEdit { field, .. } => {
                log::debug!(
                    "TextEdit on digit {} field '{}' — sequence CRDT ops applied by editor",
                    op.digit_id, field
                );
                // Verify the target digit exists.
                if !self.digits.contains_key(&op.digit_id) {
                    return Err(MagicError::DigitNotFound(op.digit_id));
                }
                // The actual character-level merge is performed by the editor's
                // SequenceRga instance. The resulting text is written back via
                // a separate Update operation on the "text" field.
            }
            // Forward-compat: skip operation types this version doesn't understand.
            OperationPayload::Unknown => {
                log::debug!("Skipping unknown operation payload for digit {}", op.digit_id);
            }
        }
        Ok(())
    }

    fn remove_from_parent(&mut self, digit_id: Uuid) {
        // Find which digit has this as a child and remove it
        let parents: Vec<Uuid> = self
            .digits
            .iter()
            .filter_map(|(pid, d)| {
                d.children
                    .as_ref()
                    .and_then(|c| if c.contains(&digit_id) { Some(*pid) } else { None })
            })
            .collect();
        for pid in parents {
            self.remove_child_from(pid, digit_id);
        }
    }

    fn remove_child_from(&mut self, parent_id: Uuid, child_id: Uuid) {
        if let Some(parent) = self.digits.get(&parent_id) {
            if let Some(children) = &parent.children {
                let new_children: Vec<Uuid> =
                    children.iter().copied().filter(|c| *c != child_id).collect();
                let mut updated = parent.clone();
                updated.children = if new_children.is_empty() {
                    None
                } else {
                    Some(new_children)
                };
                self.digits.insert(parent_id, updated);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_text_digit(text: &str) -> Digit {
        Digit::new("text".into(), Value::from(text), "cpub1test".into()).unwrap()
    }

    fn make_container() -> Digit {
        Digit::new("container".into(), Value::Null, "cpub1test".into()).unwrap()
    }

    #[test]
    fn new_state_is_empty() {
        let state = DocumentState::new("cpub1test");
        assert_eq!(state.digit_count(), 0);
        assert!(state.root_digit_id().is_none());
        assert!(state.root_digit().is_none());
        assert_eq!(state.author(), "cpub1test");
    }

    #[test]
    fn insert_digit_becomes_root() {
        let mut state = DocumentState::new("cpub1test");
        let digit = make_text_digit("hello");
        let id = digit.id();
        state.insert_digit(digit, None).unwrap();
        assert_eq!(state.digit_count(), 1);
        assert_eq!(state.root_digit_id(), Some(id));
        assert_eq!(state.digit(id).unwrap().digit_type(), "text");
    }

    #[test]
    fn insert_with_parent() {
        let mut state = DocumentState::new("cpub1test");
        let container = make_container();
        let container_id = container.id();
        state.insert_digit(container, None).unwrap();

        let child = make_text_digit("inside");
        let child_id = child.id();
        state.insert_digit(child, Some(container_id)).unwrap();

        assert_eq!(state.digit_count(), 2);
        let children = state.children_of(container_id);
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].id(), child_id);
    }

    #[test]
    fn update_digit_content() {
        let mut state = DocumentState::new("cpub1test");
        let digit = make_text_digit("old");
        let id = digit.id();
        state.insert_digit(digit, None).unwrap();

        state
            .update_digit(id, "content".into(), Value::from("old"), Value::from("new"))
            .unwrap();
        assert_eq!(state.digit(id).unwrap().content, Value::from("new"));
    }

    #[test]
    fn update_digit_property() {
        let mut state = DocumentState::new("cpub1test");
        let digit = make_text_digit("hello");
        let id = digit.id();
        state.insert_digit(digit, None).unwrap();

        state
            .update_digit(
                id,
                "font".into(),
                Value::Null,
                Value::from("mono"),
            )
            .unwrap();
        assert_eq!(
            state.digit(id).unwrap().properties.get("font"),
            Some(&Value::from("mono"))
        );
    }

    #[test]
    fn delete_digit_tombstone() {
        let mut state = DocumentState::new("cpub1test");
        let digit = make_text_digit("gone");
        let id = digit.id();
        state.insert_digit(digit, None).unwrap();
        state.delete_digit(id).unwrap();
        // Tombstoned, still present but marked deleted
        assert!(state.digit(id).unwrap().is_deleted());
        assert!(state.root_digit_id().is_none()); // root cleared
    }

    #[test]
    fn apply_idempotent() {
        let mut state = DocumentState::new("cpub1test");
        let digit = make_text_digit("hello");
        let op = state.insert_digit(digit, None).unwrap();
        // Apply the same operation again
        let result = state.apply(&op).unwrap();
        assert!(!result); // duplicate, not applied
        assert_eq!(state.digit_count(), 1); // no duplicate digit
    }

    #[test]
    fn root_digit_tracking() {
        let mut state = DocumentState::new("cpub1test");
        let a = make_text_digit("a");
        let a_id = a.id();
        state.insert_digit(a, None).unwrap();
        assert_eq!(state.root_digit_id(), Some(a_id));

        // Second insert with no parent doesn't change root
        let b = make_text_digit("b");
        state.insert_digit(b, None).unwrap();
        assert_eq!(state.root_digit_id(), Some(a_id));
    }

    #[test]
    fn load_digits() {
        let mut state = DocumentState::new("cpub1test");
        let a = make_text_digit("a");
        let b = make_text_digit("b");
        let root_id = a.id();
        state.load_digits(vec![a, b], Some(root_id));
        assert_eq!(state.digit_count(), 2);
        assert_eq!(state.root_digit_id(), Some(root_id));
    }

    #[test]
    fn vector_clock_increments() {
        let mut state = DocumentState::new("cpub1test");
        let digit = make_text_digit("hello");
        state.insert_digit(digit, None).unwrap();
        // Vector should have been incremented for our author
        assert!(state.vector().count_for("test") > 0); // cpub1 prefix stripped
    }

    #[test]
    fn delete_nonexistent_fails() {
        let mut state = DocumentState::new("cpub1test");
        let result = state.delete_digit(Uuid::new_v4());
        assert!(result.is_err());
    }

    #[test]
    fn layout_default() {
        let layout = DocumentLayout::default();
        assert_eq!(layout.mode, LayoutMode::Vertical);
        assert_eq!(layout.page_width, 595.0);
    }

    #[test]
    fn layout_mode_serde() {
        let json = serde_json::to_string(&LayoutMode::Freeform).unwrap();
        let decoded: LayoutMode = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, LayoutMode::Freeform);
    }
}
