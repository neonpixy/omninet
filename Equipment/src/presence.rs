//! Cursor and presence data types for collaborative editing.
//!
//! These types model the real-time state of collaborators within a shared
//! `.idea` document. They flow over Equipment's Communicator channels
//! (`collaboration.cursor` and `collaboration.presence`) and are serialized
//! to JSON for transmission over Globe relay events.
//!
//! # Design
//!
//! - `CursorPosition` tracks where a collaborator's cursor (and optional
//!   selection) sits within a specific digit and field.
//! - `PresenceInfo` describes a collaborator: who they are, what color
//!   they've been assigned, and whether they're actively editing.
//! - `CollaborationSession` groups all participants editing the same
//!   `.idea` document.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A collaborator's cursor position within a document.
///
/// Points to a specific character offset in a specific field of a specific
/// digit. An optional `selection_end` extends the cursor into a selection
/// range.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorPosition {
    /// Which digit the cursor is in.
    pub digit_id: Uuid,
    /// Which text field within the digit (e.g., `"text"`, `"code"`, `"items[0]"`).
    pub field: String,
    /// Character offset from the start of the field.
    pub offset: usize,
    /// End of selection range (if text is selected). `None` = cursor only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selection_end: Option<usize>,
}

impl CursorPosition {
    /// Create a cursor at a position with no selection.
    pub fn new(digit_id: Uuid, field: impl Into<String>, offset: usize) -> Self {
        Self {
            digit_id,
            field: field.into(),
            offset,
            selection_end: None,
        }
    }

    /// Create a cursor with a selection range.
    ///
    /// `offset` is the anchor and `end` is the active end of the selection.
    pub fn with_selection(
        digit_id: Uuid,
        field: impl Into<String>,
        offset: usize,
        end: usize,
    ) -> Self {
        Self {
            digit_id,
            field: field.into(),
            offset,
            selection_end: Some(end),
        }
    }

    /// Whether the cursor represents a selection (not just a caret).
    pub fn has_selection(&self) -> bool {
        self.selection_end.is_some()
    }

    /// Length of the selection in characters, or 0 if no selection.
    pub fn selection_len(&self) -> usize {
        match self.selection_end {
            Some(end) if end > self.offset => end - self.offset,
            Some(end) => self.offset - end,
            None => 0,
        }
    }
}

/// Information about a collaborator's presence in a shared document.
///
/// Each collaborator gets a unique color for their cursor/selection
/// highlights. Presence is ephemeral — it's not persisted in Vault.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresenceInfo {
    /// The collaborator's Crown public key.
    pub crown_id: String,
    /// Display name for UI rendering.
    pub display_name: String,
    /// Assigned color for cursor/selection highlighting (hex, e.g., `"#e74c3c"`).
    pub color: String,
    /// Current cursor position (`None` if not actively editing).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<CursorPosition>,
    /// When this presence was last updated.
    pub last_seen: DateTime<Utc>,
    /// Whether the collaborator is actively typing.
    #[serde(default)]
    pub is_active: bool,
}

impl PresenceInfo {
    /// Create a new presence entry with no cursor and inactive state.
    pub fn new(
        crown_id: impl Into<String>,
        display_name: impl Into<String>,
        color: impl Into<String>,
    ) -> Self {
        Self {
            crown_id: crown_id.into(),
            display_name: display_name.into(),
            color: color.into(),
            cursor: None,
            last_seen: Utc::now(),
            is_active: false,
        }
    }
}

/// A collaboration session for a shared `.idea` document.
///
/// Tracks all participants currently editing the same document.
/// Sessions are ephemeral — they exist while at least one collaborator
/// is present and are cleaned up when all leave.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollaborationSession {
    /// Unique identifier for this collaboration session.
    pub session_id: Uuid,
    /// The `.idea` document being collaboratively edited.
    pub idea_id: Uuid,
    /// All participants currently in the session.
    pub participants: Vec<PresenceInfo>,
    /// When this session started.
    pub started_at: DateTime<Utc>,
}

impl CollaborationSession {
    /// Create a new empty collaboration session for an `.idea` document.
    pub fn new(idea_id: Uuid) -> Self {
        Self {
            session_id: Uuid::new_v4(),
            idea_id,
            participants: Vec::new(),
            started_at: Utc::now(),
        }
    }

    /// Add a participant to the session.
    ///
    /// If a participant with the same `crown_id` already exists, they are
    /// replaced (reconnection scenario).
    pub fn add_participant(&mut self, info: PresenceInfo) {
        self.remove_participant(&info.crown_id);
        self.participants.push(info);
    }

    /// Remove a participant by their Crown ID. Returns `true` if removed.
    pub fn remove_participant(&mut self, crown_id: &str) -> bool {
        let before = self.participants.len();
        self.participants.retain(|p| p.crown_id != crown_id);
        self.participants.len() < before
    }

    /// Update the cursor for a participant. Returns `false` if the
    /// participant is not in the session.
    pub fn update_cursor(&mut self, crown_id: &str, cursor: CursorPosition) -> bool {
        if let Some(p) = self.participants.iter_mut().find(|p| p.crown_id == crown_id) {
            p.cursor = Some(cursor);
            p.last_seen = Utc::now();
            p.is_active = true;
            true
        } else {
            false
        }
    }

    /// Look up a participant by Crown ID.
    pub fn participant(&self, crown_id: &str) -> Option<&PresenceInfo> {
        self.participants.iter().find(|p| p.crown_id == crown_id)
    }

    /// Whether the session is empty (no participants).
    pub fn is_empty(&self) -> bool {
        self.participants.is_empty()
    }

    /// Number of participants.
    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_digit_id() -> Uuid {
        Uuid::new_v4()
    }

    fn test_idea_id() -> Uuid {
        Uuid::new_v4()
    }

    // -- CursorPosition tests --

    #[test]
    fn cursor_new_no_selection() {
        let did = test_digit_id();
        let cursor = CursorPosition::new(did, "text", 42);
        assert_eq!(cursor.digit_id, did);
        assert_eq!(cursor.field, "text");
        assert_eq!(cursor.offset, 42);
        assert!(!cursor.has_selection());
        assert_eq!(cursor.selection_len(), 0);
    }

    #[test]
    fn cursor_with_selection() {
        let did = test_digit_id();
        let cursor = CursorPosition::with_selection(did, "code", 10, 25);
        assert!(cursor.has_selection());
        assert_eq!(cursor.selection_end, Some(25));
        assert_eq!(cursor.selection_len(), 15);
    }

    #[test]
    fn cursor_backward_selection() {
        let did = test_digit_id();
        let cursor = CursorPosition::with_selection(did, "text", 25, 10);
        assert!(cursor.has_selection());
        assert_eq!(cursor.selection_len(), 15);
    }

    #[test]
    fn cursor_zero_length_selection() {
        let did = test_digit_id();
        let cursor = CursorPosition::with_selection(did, "text", 5, 5);
        assert!(cursor.has_selection());
        assert_eq!(cursor.selection_len(), 0);
    }

    #[test]
    fn cursor_serde_round_trip() {
        let did = test_digit_id();
        let cursor = CursorPosition::with_selection(did, "items[0]", 3, 8);
        let json = serde_json::to_string(&cursor).unwrap();
        let loaded: CursorPosition = serde_json::from_str(&json).unwrap();
        assert_eq!(cursor, loaded);
    }

    #[test]
    fn cursor_serde_no_selection_omits_field() {
        let did = test_digit_id();
        let cursor = CursorPosition::new(did, "text", 0);
        let json = serde_json::to_string(&cursor).unwrap();
        assert!(!json.contains("selection_end"));
    }

    #[test]
    fn cursor_serde_missing_selection_defaults_to_none() {
        let did = test_digit_id();
        let json = format!(
            r#"{{"digit_id":"{}","field":"text","offset":10}}"#,
            did
        );
        let loaded: CursorPosition = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.selection_end, None);
    }

    // -- PresenceInfo tests --

    #[test]
    fn presence_new_defaults() {
        let info = PresenceInfo::new("cpub_alice", "Alice", "#e74c3c");
        assert_eq!(info.crown_id, "cpub_alice");
        assert_eq!(info.display_name, "Alice");
        assert_eq!(info.color, "#e74c3c");
        assert!(info.cursor.is_none());
        assert!(!info.is_active);
    }

    #[test]
    fn presence_serde_round_trip() {
        let mut info = PresenceInfo::new("cpub_bob", "Bob", "#3498db");
        info.cursor = Some(CursorPosition::new(test_digit_id(), "text", 5));
        info.is_active = true;

        let json = serde_json::to_string(&info).unwrap();
        let loaded: PresenceInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.crown_id, "cpub_bob");
        assert_eq!(loaded.display_name, "Bob");
        assert!(loaded.cursor.is_some());
        assert!(loaded.is_active);
    }

    #[test]
    fn presence_serde_backward_compat_without_optional_fields() {
        let json = r##"{
            "crown_id": "cpub_old",
            "display_name": "Old Client",
            "color": "#2ecc71",
            "last_seen": "2025-01-01T00:00:00Z"
        }"##;
        let loaded: PresenceInfo = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.crown_id, "cpub_old");
        assert!(loaded.cursor.is_none());
        assert!(!loaded.is_active);
    }

    // -- CollaborationSession tests --

    #[test]
    fn session_new_is_empty() {
        let iid = test_idea_id();
        let session = CollaborationSession::new(iid);
        assert_eq!(session.idea_id, iid);
        assert!(session.is_empty());
        assert_eq!(session.participant_count(), 0);
    }

    #[test]
    fn session_add_and_lookup_participant() {
        let mut session = CollaborationSession::new(test_idea_id());
        let info = PresenceInfo::new("cpub_alice", "Alice", "#e74c3c");
        session.add_participant(info);

        assert_eq!(session.participant_count(), 1);
        let found = session.participant("cpub_alice").unwrap();
        assert_eq!(found.display_name, "Alice");
    }

    #[test]
    fn session_add_replaces_existing_participant() {
        let mut session = CollaborationSession::new(test_idea_id());
        session.add_participant(PresenceInfo::new("cpub_alice", "Alice v1", "#e74c3c"));
        session.add_participant(PresenceInfo::new("cpub_alice", "Alice v2", "#2ecc71"));

        assert_eq!(session.participant_count(), 1);
        let found = session.participant("cpub_alice").unwrap();
        assert_eq!(found.display_name, "Alice v2");
        assert_eq!(found.color, "#2ecc71");
    }

    #[test]
    fn session_remove_participant() {
        let mut session = CollaborationSession::new(test_idea_id());
        session.add_participant(PresenceInfo::new("cpub_alice", "Alice", "#e74c3c"));
        session.add_participant(PresenceInfo::new("cpub_bob", "Bob", "#3498db"));

        assert!(session.remove_participant("cpub_alice"));
        assert_eq!(session.participant_count(), 1);
        assert!(session.participant("cpub_alice").is_none());
        assert!(session.participant("cpub_bob").is_some());
    }

    #[test]
    fn session_remove_nonexistent_returns_false() {
        let mut session = CollaborationSession::new(test_idea_id());
        assert!(!session.remove_participant("cpub_ghost"));
    }

    #[test]
    fn session_update_cursor() {
        let did = test_digit_id();
        let mut session = CollaborationSession::new(test_idea_id());
        session.add_participant(PresenceInfo::new("cpub_alice", "Alice", "#e74c3c"));

        let cursor = CursorPosition::new(did, "text", 42);
        assert!(session.update_cursor("cpub_alice", cursor));

        let alice = session.participant("cpub_alice").unwrap();
        assert!(alice.is_active);
        assert_eq!(alice.cursor.as_ref().unwrap().offset, 42);
    }

    #[test]
    fn session_update_cursor_unknown_participant_returns_false() {
        let did = test_digit_id();
        let mut session = CollaborationSession::new(test_idea_id());
        let cursor = CursorPosition::new(did, "text", 0);
        assert!(!session.update_cursor("cpub_ghost", cursor));
    }

    #[test]
    fn session_serde_round_trip() {
        let mut session = CollaborationSession::new(test_idea_id());
        session.add_participant(PresenceInfo::new("cpub_alice", "Alice", "#e74c3c"));
        session.add_participant(PresenceInfo::new("cpub_bob", "Bob", "#3498db"));

        let json = serde_json::to_string(&session).unwrap();
        let loaded: CollaborationSession = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.session_id, session.session_id);
        assert_eq!(loaded.idea_id, session.idea_id);
        assert_eq!(loaded.participant_count(), 2);
    }
}
