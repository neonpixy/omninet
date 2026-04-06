use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use x::{CrdtOperation, Value, VectorClock};

/// A CRDT operation on a Digit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DigitOperation {
    pub id: Uuid,
    pub digit_id: Uuid,
    #[serde(rename = "type")]
    pub operation_type: OperationType,
    pub payload: OperationPayload,
    pub author: String,
    pub timestamp: DateTime<Utc>,
    pub vector: VectorClock,
    /// Cryptographic signature (Crown Schnorr) for multiplayer verification.
    /// Local-only ops may be unsigned. Remote ops received over Globe MUST be signed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

/// The kind of CRDT operation being performed on a digit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OperationType {
    /// A new digit is being created.
    Insert,
    /// An existing digit's content or property is changing.
    Update,
    /// A digit is being tombstoned or permanently removed.
    Delete,
    /// A digit is being reparented in the tree.
    Move,
    /// A digit's spatial transform (position, size, rotation) is changing.
    Transform,
    /// Character-level text edits within a field (sequence CRDT operations).
    TextEdit,
}

/// The data carried by a CRDT operation, varying by operation type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OperationPayload {
    /// Payload for inserting a new digit into the tree.
    Insert {
        digit_json: serde_json::Value,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_id: Option<Uuid>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
    },
    /// Payload for updating a single field on an existing digit.
    Update {
        field: String,
        old_value: Value,
        new_value: Value,
    },
    /// Payload for deleting (tombstoning) a digit.
    Delete {
        #[serde(default)]
        tombstone: bool,
    },
    /// Payload for moving a digit to a different parent or position.
    Move {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        from_parent: Option<Uuid>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to_parent: Option<Uuid>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        to_index: Option<usize>,
    },
    /// Payload for changing a digit's spatial transform on a canvas.
    Transform {
        x: f64,
        y: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        width: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        height: Option<f64>,
        #[serde(default)]
        rotation: f64,
        #[serde(default)]
        z_index: i32,
    },
    /// Character-level text edits within a specific field on a digit.
    /// Used for real-time collaborative editing via the sequence CRDT.
    TextEdit {
        /// Which text field on the digit (e.g., "text", "code", "items[0]").
        field: String,
        /// Serialized sequence CRDT operations (Vec<SequenceOp> as JSON).
        /// Stored as serde_json::Value for decoupling from X crate's SequenceOp type.
        ops: Vec<serde_json::Value>,
    },
    /// Forward-compatibility fallback. Old clients that encounter a new operation
    /// type (e.g., future types) will deserialize it as Unknown and skip it gracefully
    /// instead of failing to deserialize.
    #[serde(other)]
    Unknown,
}

impl DigitOperation {
    /// Creates an Insert operation for adding a new digit.
    pub fn insert(
        digit_json: serde_json::Value,
        parent_id: Option<Uuid>,
        author: String,
        vector: VectorClock,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            digit_id: Uuid::new_v4(),
            operation_type: OperationType::Insert,
            payload: OperationPayload::Insert {
                digit_json,
                parent_id,
                index: None,
            },
            author,
            timestamp: Utc::now(),
            vector,
            signature: None,
        }
    }

    /// Creates an Update operation for changing a field on an existing digit.
    pub fn update(
        digit_id: Uuid,
        field: String,
        old_value: Value,
        new_value: Value,
        author: String,
        vector: VectorClock,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            digit_id,
            operation_type: OperationType::Update,
            payload: OperationPayload::Update {
                field,
                old_value,
                new_value,
            },
            author,
            timestamp: Utc::now(),
            vector,
            signature: None,
        }
    }

    /// Creates a Delete operation for removing or tombstoning a digit.
    pub fn delete(digit_id: Uuid, tombstone: bool, author: String, vector: VectorClock) -> Self {
        Self {
            id: Uuid::new_v4(),
            digit_id,
            operation_type: OperationType::Delete,
            payload: OperationPayload::Delete { tombstone },
            author,
            timestamp: Utc::now(),
            vector,
            signature: None,
        }
    }
}

impl CrdtOperation for DigitOperation {
    fn id(&self) -> Uuid {
        self.id
    }
    fn target_id(&self) -> Uuid {
        self.digit_id
    }
    fn vector(&self) -> &VectorClock {
        &self.vector
    }
    fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }
    fn author(&self) -> &str {
        &self.author
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_insert_operation() {
        let vc = VectorClock::new();
        let op = DigitOperation::insert(
            serde_json::json!({"type": "text", "content": {"string": "hello"}}),
            None,
            "cpub1alice".into(),
            vc,
        );
        assert_eq!(op.operation_type, OperationType::Insert);
        assert_eq!(op.author(), "cpub1alice");
    }

    #[test]
    fn create_update_operation() {
        let vc = VectorClock::new();
        let digit_id = Uuid::new_v4();
        let op = DigitOperation::update(
            digit_id,
            "content".into(),
            Value::from("old"),
            Value::from("new"),
            "cpub1alice".into(),
            vc,
        );
        assert_eq!(op.operation_type, OperationType::Update);
        assert_eq!(op.target_id(), digit_id);
    }

    #[test]
    fn implements_crdt_operation_trait() {
        let vc = VectorClock::new();
        let op = DigitOperation::delete(Uuid::new_v4(), true, "cpub1bob".into(), vc);

        // These are the CrdtOperation trait methods
        let _id: Uuid = op.id();
        let _target: Uuid = op.target_id();
        let _vec: &VectorClock = op.vector();
        let _ts: DateTime<Utc> = op.timestamp();
        let _author: &str = op.author();
    }

    #[test]
    fn signature_field_optional_and_backward_compat() {
        let mut vc = VectorClock::new();
        vc.increment("alice");
        let op = DigitOperation::update(
            Uuid::new_v4(),
            "text".into(),
            Value::Null,
            Value::from("hello"),
            "cpub1alice".into(),
            vc,
        );
        // No signature by default.
        assert!(op.signature.is_none());

        // Serialize without signature.
        let json = serde_json::to_string(&op).unwrap();
        assert!(!json.contains("signature"));

        // Deserialize old JSON without signature field — backward compat.
        let rt: DigitOperation = serde_json::from_str(&json).unwrap();
        assert!(rt.signature.is_none());
    }

    #[test]
    fn signature_field_round_trips() {
        let mut vc = VectorClock::new();
        vc.increment("alice");
        let mut op = DigitOperation::update(
            Uuid::new_v4(),
            "text".into(),
            Value::Null,
            Value::from("signed"),
            "cpub1alice".into(),
            vc,
        );
        op.signature = Some("deadbeef0123456789abcdef".into());

        let json = serde_json::to_string(&op).unwrap();
        let rt: DigitOperation = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.signature.as_deref(), Some("deadbeef0123456789abcdef"));
    }

    #[test]
    fn text_edit_payload_serde() {
        let payload = OperationPayload::TextEdit {
            field: "text".into(),
            ops: vec![
                serde_json::json!({"Insert": {"id": {"replica_id": "r1", "seq": 1}, "value": "a", "after": null}}),
                serde_json::json!({"Delete": {"id": {"replica_id": "r1", "seq": 0}}}),
            ],
        };
        let json = serde_json::to_string(&payload).unwrap();
        let rt: OperationPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(rt, payload);
    }

    #[test]
    fn unknown_payload_deserializes() {
        // Simulate a future operation type that old code doesn't know about
        let json = r#"{
            "kind": "futureTextEdit",
            "field": "text",
            "ops": []
        }"#;
        let payload: OperationPayload = serde_json::from_str(json).unwrap();
        assert_eq!(payload, OperationPayload::Unknown);
    }

    #[test]
    fn serde_round_trip() {
        let mut vc = VectorClock::new();
        vc.increment("alice");
        let op = DigitOperation::update(
            Uuid::new_v4(),
            "content".into(),
            Value::Null,
            Value::from("hello"),
            "cpub1alice".into(),
            vc,
        );
        let json = serde_json::to_string_pretty(&op).unwrap();
        let rt: DigitOperation = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.id, op.id);
        assert_eq!(rt.operation_type, op.operation_type);
    }
}
