use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use ideas::crdt::{DigitOperation, OperationPayload, OperationType};
use ideas::Digit;
use x::Value;

use crate::error::MagicError;
use crate::ideation::DocumentState;

/// The 5 built-in document actions.
///
/// Each variant carries its data and can produce a DigitOperation via `execute()`.
/// `inverse()` captures the pre-existing state needed for undo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Action {
    InsertDigit {
        digit: Digit,
        parent_id: Option<Uuid>,
        index: Option<usize>,
    },
    UpdateDigit {
        digit_id: Uuid,
        field: String,
        old_value: Value,
        new_value: Value,
    },
    DeleteDigit {
        digit_id: Uuid,
        /// Snapshot of the digit before deletion (for undo).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        snapshot: Option<Digit>,
        /// Which parent held this digit (for undo).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_id: Option<Uuid>,
    },
    MoveDigit {
        digit_id: Uuid,
        from_parent: Option<Uuid>,
        to_parent: Option<Uuid>,
        to_index: Option<usize>,
    },
    TransformDigit {
        digit_id: Uuid,
        x: f64,
        y: f64,
        width: Option<f64>,
        height: Option<f64>,
        rotation: f64,
        z_index: i32,
        /// Previous transform values (for undo).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        old_x: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        old_y: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        old_width: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        old_height: Option<f64>,
        #[serde(default)]
        old_rotation: f64,
        #[serde(default)]
        old_z_index: i32,
    },
}

impl Action {
    // --- Convenience constructors ---

    /// Create an insert action for a new digit, optionally under a parent.
    pub fn insert(digit: Digit, parent_id: Option<Uuid>) -> Self {
        Self::InsertDigit {
            digit,
            parent_id,
            index: None,
        }
    }

    /// Create an update action that changes a single field on a digit.
    pub fn update(
        digit_id: Uuid,
        field: impl Into<String>,
        old_value: Value,
        new_value: Value,
    ) -> Self {
        Self::UpdateDigit {
            digit_id,
            field: field.into(),
            old_value,
            new_value,
        }
    }

    /// Create a delete action for a digit (tombstone soft-delete).
    pub fn delete(digit_id: Uuid) -> Self {
        Self::DeleteDigit {
            digit_id,
            snapshot: None,
            parent_id: None,
        }
    }

    /// Create a move action that reparents a digit from one container to another.
    pub fn move_digit(
        digit_id: Uuid,
        from_parent: Option<Uuid>,
        to_parent: Option<Uuid>,
        to_index: Option<usize>,
    ) -> Self {
        Self::MoveDigit {
            digit_id,
            from_parent,
            to_parent,
            to_index,
        }
    }

    /// Create a transform action that repositions and optionally resizes a digit.
    pub fn transform(
        digit_id: Uuid,
        x: f64,
        y: f64,
        width: Option<f64>,
        height: Option<f64>,
    ) -> Self {
        Self::TransformDigit {
            digit_id,
            x,
            y,
            width,
            height,
            rotation: 0.0,
            z_index: 0,
            old_x: None,
            old_y: None,
            old_width: None,
            old_height: None,
            old_rotation: 0.0,
            old_z_index: 0,
        }
    }

    /// Capture the current state needed for undo, then execute.
    ///
    /// Returns `(operation, inverse_action)`. The inverse can be used to undo.
    pub fn execute(
        &self,
        state: &mut DocumentState,
    ) -> Result<(DigitOperation, Action), MagicError> {
        let author = state.author().to_string();
        let mut vector = state.vector().clone();
        vector.increment(&author);

        match self {
            Action::InsertDigit {
                digit,
                parent_id,
                index,
            } => {
                let digit_json = serde_json::to_value(digit)
                    .map_err(|e| MagicError::Serialization(e.to_string()))?;
                let op = DigitOperation {
                    id: Uuid::new_v4(),
                    digit_id: digit.id(),
                    operation_type: OperationType::Insert,
                    payload: OperationPayload::Insert {
                        digit_json,
                        parent_id: *parent_id,
                        index: *index,
                    },
                    author,
                    timestamp: Utc::now(),
                    vector,
                    signature: None,
                };
                state.apply(&op)?;
                let inverse = Action::DeleteDigit {
                    digit_id: digit.id(),
                    snapshot: Some(digit.clone()),
                    parent_id: *parent_id,
                };
                Ok((op, inverse))
            }
            Action::UpdateDigit {
                digit_id,
                field,
                old_value,
                new_value,
            } => {
                if state.digit(*digit_id).is_none() {
                    return Err(MagicError::DigitNotFound(*digit_id));
                }
                let op = DigitOperation {
                    id: Uuid::new_v4(),
                    digit_id: *digit_id,
                    operation_type: OperationType::Update,
                    payload: OperationPayload::Update {
                        field: field.clone(),
                        old_value: old_value.clone(),
                        new_value: new_value.clone(),
                    },
                    author,
                    timestamp: Utc::now(),
                    vector,
                    signature: None,
                };
                state.apply(&op)?;
                let inverse = Action::UpdateDigit {
                    digit_id: *digit_id,
                    field: field.clone(),
                    old_value: new_value.clone(),
                    new_value: old_value.clone(),
                };
                Ok((op, inverse))
            }
            Action::DeleteDigit { digit_id, .. } => {
                let snapshot = state
                    .digit(*digit_id)
                    .ok_or(MagicError::DigitNotFound(*digit_id))?
                    .clone();
                // Find parent
                let parent_id = Self::find_parent(state, *digit_id);
                let op = DigitOperation::delete(
                    *digit_id,
                    true,
                    author,
                    vector,
                );
                state.apply(&op)?;
                let inverse = Action::InsertDigit {
                    digit: snapshot,
                    parent_id,
                    index: None,
                };
                Ok((op, inverse))
            }
            Action::MoveDigit {
                digit_id,
                from_parent,
                to_parent,
                to_index,
            } => {
                if state.digit(*digit_id).is_none() {
                    return Err(MagicError::DigitNotFound(*digit_id));
                }
                let op = DigitOperation {
                    id: Uuid::new_v4(),
                    digit_id: *digit_id,
                    operation_type: OperationType::Move,
                    payload: OperationPayload::Move {
                        from_parent: *from_parent,
                        to_parent: *to_parent,
                        to_index: *to_index,
                    },
                    author,
                    timestamp: Utc::now(),
                    vector,
                    signature: None,
                };
                state.apply(&op)?;
                let inverse = Action::MoveDigit {
                    digit_id: *digit_id,
                    from_parent: *to_parent,
                    to_parent: *from_parent,
                    to_index: None,
                };
                Ok((op, inverse))
            }
            Action::TransformDigit {
                digit_id,
                x,
                y,
                width,
                height,
                rotation,
                z_index,
                ..
            } => {
                let digit = state
                    .digit(*digit_id)
                    .ok_or(MagicError::DigitNotFound(*digit_id))?;
                // Capture old values for inverse
                let old_x = digit
                    .properties
                    .get("x")
                    .and_then(|v| v.as_double());
                let old_y = digit
                    .properties
                    .get("y")
                    .and_then(|v| v.as_double());
                let old_width = digit
                    .properties
                    .get("width")
                    .and_then(|v| v.as_double());
                let old_height = digit
                    .properties
                    .get("height")
                    .and_then(|v| v.as_double());
                let old_rotation = digit
                    .properties
                    .get("rotation")
                    .and_then(|v| v.as_double())
                    .unwrap_or(0.0);
                let old_z_index = digit
                    .properties
                    .get("z_index")
                    .and_then(|v| v.as_int())
                    .unwrap_or(0) as i32;

                let op = DigitOperation {
                    id: Uuid::new_v4(),
                    digit_id: *digit_id,
                    operation_type: OperationType::Transform,
                    payload: OperationPayload::Transform {
                        x: *x,
                        y: *y,
                        width: *width,
                        height: *height,
                        rotation: *rotation,
                        z_index: *z_index,
                    },
                    author,
                    timestamp: Utc::now(),
                    vector,
                    signature: None,
                };
                state.apply(&op)?;
                let inverse = Action::TransformDigit {
                    digit_id: *digit_id,
                    x: old_x.unwrap_or(0.0),
                    y: old_y.unwrap_or(0.0),
                    width: old_width,
                    height: old_height,
                    rotation: old_rotation,
                    z_index: old_z_index,
                    old_x: Some(*x),
                    old_y: Some(*y),
                    old_width: *width,
                    old_height: *height,
                    old_rotation: *rotation,
                    old_z_index: *z_index,
                };
                Ok((op, inverse))
            }
        }
    }

    fn find_parent(state: &DocumentState, child_id: Uuid) -> Option<Uuid> {
        for digit in state.digits() {
            if let Some(children) = &digit.children {
                if children.contains(&child_id) {
                    return Some(digit.id());
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_text(text: &str) -> Digit {
        Digit::new("text".into(), Value::from(text), "cpub1test".into()).unwrap()
    }

    fn make_state() -> DocumentState {
        DocumentState::new("cpub1test")
    }

    #[test]
    fn insert_execute_and_inverse() {
        let mut state = make_state();
        let digit = make_text("hello");
        let id = digit.id();
        let action = Action::insert(digit, None);
        let (_, inverse) = action.execute(&mut state).unwrap();
        assert_eq!(state.digit_count(), 1);
        // Inverse is a delete
        assert!(matches!(inverse, Action::DeleteDigit { digit_id, .. } if digit_id == id));
    }

    #[test]
    fn update_execute_and_inverse() {
        let mut state = make_state();
        let digit = make_text("old");
        let id = digit.id();
        Action::insert(digit, None).execute(&mut state).unwrap();

        let action = Action::update(id, "content", Value::from("old"), Value::from("new"));
        let (_, inverse) = action.execute(&mut state).unwrap();
        assert_eq!(state.digit(id).unwrap().content, Value::from("new"));
        // Inverse swaps old/new
        if let Action::UpdateDigit {
            old_value,
            new_value,
            ..
        } = inverse
        {
            assert_eq!(old_value, Value::from("new"));
            assert_eq!(new_value, Value::from("old"));
        } else {
            panic!("expected UpdateDigit inverse");
        }
    }

    #[test]
    fn delete_execute_and_inverse() {
        let mut state = make_state();
        let digit = make_text("gone");
        let id = digit.id();
        Action::insert(digit, None).execute(&mut state).unwrap();

        let action = Action::delete(id);
        let (_, inverse) = action.execute(&mut state).unwrap();
        assert!(state.digit(id).unwrap().is_deleted());
        // Inverse is an insert with the snapshot
        assert!(matches!(inverse, Action::InsertDigit { .. }));
    }

    #[test]
    fn move_execute_produces_operation() {
        let mut state = make_state();
        let container = Digit::new("container".into(), Value::Null, "cpub1test".into()).unwrap();
        let cid = container.id();
        Action::insert(container, None)
            .execute(&mut state)
            .unwrap();
        let child = make_text("child");
        let child_id = child.id();
        Action::insert(child, Some(cid))
            .execute(&mut state)
            .unwrap();

        // Create a second container
        let c2 = Digit::new("container".into(), Value::Null, "cpub1test".into()).unwrap();
        let c2id = c2.id();
        Action::insert(c2, None).execute(&mut state).unwrap();

        let action = Action::move_digit(child_id, Some(cid), Some(c2id), None);
        let (_, inverse) = action.execute(&mut state).unwrap();
        // Child should be under c2
        let children = state.children_of(c2id);
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].id(), child_id);
        // Inverse moves back
        if let Action::MoveDigit {
            from_parent,
            to_parent,
            ..
        } = inverse
        {
            assert_eq!(from_parent, Some(c2id));
            assert_eq!(to_parent, Some(cid));
        }
    }

    #[test]
    fn transform_execute_captures_old_values() {
        let mut state = make_state();
        let digit = make_text("box");
        let id = digit.id();
        Action::insert(digit, None).execute(&mut state).unwrap();

        let action = Action::TransformDigit {
            digit_id: id,
            x: 100.0,
            y: 200.0,
            width: Some(50.0),
            height: Some(30.0),
            rotation: 45.0,
            z_index: 1,
            old_x: None,
            old_y: None,
            old_width: None,
            old_height: None,
            old_rotation: 0.0,
            old_z_index: 0,
        };
        let (_, inverse) = action.execute(&mut state).unwrap();
        // Check properties were set
        let d = state.digit(id).unwrap();
        assert_eq!(d.properties.get("x"), Some(&Value::Double(100.0)));
        assert_eq!(d.properties.get("y"), Some(&Value::Double(200.0)));
        // Inverse captures the current (new) values as old_*
        if let Action::TransformDigit {
            old_x, old_y, ..
        } = inverse
        {
            assert_eq!(old_x, Some(100.0));
            assert_eq!(old_y, Some(200.0));
        }
    }

    #[test]
    fn inverse_of_insert_is_delete() {
        let mut state = make_state();
        let digit = make_text("x");
        let id = digit.id();
        let (_, inverse) = Action::insert(digit, None).execute(&mut state).unwrap();
        assert!(matches!(inverse, Action::DeleteDigit { digit_id, .. } if digit_id == id));
    }

    #[test]
    fn inverse_of_delete_is_insert() {
        let mut state = make_state();
        let digit = make_text("x");
        let id = digit.id();
        Action::insert(digit, None).execute(&mut state).unwrap();
        let (_, inverse) = Action::delete(id).execute(&mut state).unwrap();
        assert!(matches!(inverse, Action::InsertDigit { .. }));
    }

    #[test]
    fn inverse_of_update_swaps_values() {
        let mut state = make_state();
        let digit = make_text("a");
        let id = digit.id();
        Action::insert(digit, None).execute(&mut state).unwrap();
        let (_, inverse) = Action::update(id, "content", Value::from("a"), Value::from("b"))
            .execute(&mut state)
            .unwrap();
        if let Action::UpdateDigit {
            old_value,
            new_value,
            ..
        } = inverse
        {
            assert_eq!(old_value, Value::from("b"));
            assert_eq!(new_value, Value::from("a"));
        }
    }

    #[test]
    fn execute_on_missing_digit_fails() {
        let mut state = make_state();
        let action = Action::update(Uuid::new_v4(), "content", Value::Null, Value::from("x"));
        assert!(action.execute(&mut state).is_err());
    }

    #[test]
    fn action_serde_roundtrip() {
        let digit = make_text("hello");
        let action = Action::insert(digit, None);
        let json = serde_json::to_string(&action).unwrap();
        let decoded: Action = serde_json::from_str(&json).unwrap();
        assert_eq!(action, decoded);
    }
}
