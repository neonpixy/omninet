//! Cross-device `.idea` sync via Globe relay events.
//!
//! When a user edits an `.idea` document on one device, the CRDT operations
//! are published as Globe events so other devices can apply them. This module
//! defines the payload and filter types for that sync protocol.
//!
//! # Event Kind
//!
//! `KIND_IDEA_OPS` (9010) — in the Ideas range (9000-9999), adjacent to
//! the chunk manifest kind at 9000.
//!
//! # Design
//!
//! - Operations are carried as `Vec<serde_json::Value>` to keep Globe
//!   decoupled from the CRDT types in X and the digit types in Ideas.
//! - Vector clocks are similarly opaque — Globe transports them but
//!   doesn't interpret the clock state.
//! - `IdeaSyncFilter` helps subscribers request only the operations they
//!   need (for a specific `.idea`, from a specific author, after a
//!   specific vector clock state).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Globe event kind for `.idea` CRDT operation sync.
///
/// In the Ideas range (9000-9999). `CHUNK_MANIFEST` is 9000;
/// idea operations are 9010.
pub const KIND_IDEA_OPS: u32 = 9010;

/// Payload for publishing CRDT operations as a Globe event.
///
/// The sender serializes their CRDT operations and current vector clock
/// into this struct, which is then published as a Globe event with
/// kind `KIND_IDEA_OPS`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeaSyncPayload {
    /// The `.idea` document these operations apply to.
    pub idea_id: Uuid,
    /// Serialized CRDT operations (DigitOperations from the Ideas/X layer).
    pub ops: Vec<serde_json::Value>,
    /// Vector clock state of the sender at time of publish.
    ///
    /// Opaque to Globe — the CRDT layer uses this for causal ordering
    /// and conflict resolution.
    pub vector_clock: serde_json::Value,
}

impl IdeaSyncPayload {
    /// Create a new sync payload.
    pub fn new(
        idea_id: Uuid,
        ops: Vec<serde_json::Value>,
        vector_clock: serde_json::Value,
    ) -> Self {
        Self {
            idea_id,
            ops,
            vector_clock,
        }
    }

    /// Whether this payload contains any operations.
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

/// Filter for subscribing to `.idea` changes.
///
/// Used to build `OmniFilter` subscriptions that target a specific
/// document and optionally a specific author or clock state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeaSyncFilter {
    /// The `.idea` document to subscribe to.
    pub idea_id: Uuid,
    /// Only receive ops from this author (Crown ID).
    pub author: String,
    /// Only receive ops after this vector clock state.
    ///
    /// If `None`, receive all available operations (full sync).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since_clock: Option<serde_json::Value>,
}

impl IdeaSyncFilter {
    /// Create a filter for all operations on an `.idea` from a specific author.
    pub fn new(idea_id: Uuid, author: impl Into<String>) -> Self {
        Self {
            idea_id,
            author: author.into(),
            since_clock: None,
        }
    }

    /// Builder: only receive operations after a specific vector clock state.
    #[must_use]
    pub fn since(mut self, clock: serde_json::Value) -> Self {
        self.since_clock = Some(clock);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_idea_ops_in_ideas_range() {
        assert!((9000..10000).contains(&KIND_IDEA_OPS));
        // Must not collide with chunk manifest.
        assert_ne!(KIND_IDEA_OPS, 9000);
    }

    #[test]
    fn payload_new_and_is_empty() {
        let payload = IdeaSyncPayload::new(
            Uuid::new_v4(),
            vec![],
            serde_json::json!({}),
        );
        assert!(payload.is_empty());
    }

    #[test]
    fn payload_with_ops_not_empty() {
        let payload = IdeaSyncPayload::new(
            Uuid::new_v4(),
            vec![serde_json::json!({"type": "insert", "position": 0})],
            serde_json::json!({"alice": 1}),
        );
        assert!(!payload.is_empty());
    }

    #[test]
    fn payload_serde_round_trip() {
        let iid = Uuid::new_v4();
        let payload = IdeaSyncPayload::new(
            iid,
            vec![
                serde_json::json!({"type": "insert", "digit_id": "d1"}),
                serde_json::json!({"type": "delete", "digit_id": "d2"}),
            ],
            serde_json::json!({"alice": 3, "bob": 1}),
        );

        let json = serde_json::to_string(&payload).unwrap();
        let loaded: IdeaSyncPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.idea_id, iid);
        assert_eq!(loaded.ops.len(), 2);
        assert_eq!(loaded.vector_clock["alice"], 3);
    }

    #[test]
    fn filter_new_no_clock() {
        let iid = Uuid::new_v4();
        let filter = IdeaSyncFilter::new(iid, "cpub_alice");
        assert_eq!(filter.idea_id, iid);
        assert_eq!(filter.author, "cpub_alice");
        assert!(filter.since_clock.is_none());
    }

    #[test]
    fn filter_with_since_clock() {
        let filter = IdeaSyncFilter::new(Uuid::new_v4(), "cpub_bob")
            .since(serde_json::json!({"bob": 5}));
        assert!(filter.since_clock.is_some());
        assert_eq!(filter.since_clock.as_ref().unwrap()["bob"], 5);
    }

    #[test]
    fn filter_serde_round_trip() {
        let iid = Uuid::new_v4();
        let filter = IdeaSyncFilter::new(iid, "cpub_alice")
            .since(serde_json::json!({"alice": 10}));

        let json = serde_json::to_string(&filter).unwrap();
        let loaded: IdeaSyncFilter = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.idea_id, iid);
        assert_eq!(loaded.author, "cpub_alice");
        assert!(loaded.since_clock.is_some());
    }

    #[test]
    fn filter_serde_missing_since_clock_defaults_to_none() {
        let iid = Uuid::new_v4();
        let json = format!(
            r#"{{"idea_id":"{}","author":"cpub_test"}}"#,
            iid
        );
        let loaded: IdeaSyncFilter = serde_json::from_str(&json).unwrap();
        assert!(loaded.since_clock.is_none());
    }
}
