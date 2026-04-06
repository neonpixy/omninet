//! Restorative remedies — healing, not punishment.
//!
//! From Constellation Art. 7 §10: "The goal of all enforcement shall be
//! restoration of lawful relation, not permanent punishment or exclusion."

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A restorative remedy assigned to address harm.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Remedy {
    /// Unique remedy identifier.
    pub id: Uuid,
    /// The person this remedy is for.
    pub target_pubkey: String,
    /// Type of remedy.
    pub remedy_type: RemedyType,
    /// Description of what this remedy entails.
    pub description: String,
    /// Who assigned this remedy.
    pub assigned_by: String,
    /// Current status.
    pub status: RemedyStatus,
    /// When the remedy was created.
    pub created_at: DateTime<Utc>,
    /// When the remedy was completed (if ever).
    pub completed_at: Option<DateTime<Utc>>,
}

/// Types of restorative remedy.
///
/// These are about healing, not punishing. Each type serves a distinct
/// purpose in the restoration process.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RemedyType {
    /// Make right what was broken — direct repair of harm.
    Repair,
    /// Return the person to their place in community — rebuild relationship.
    Restore,
    /// Change structures to prevent recurrence — systemic fix.
    Prevent,
    /// Create a pathway back to full participation — reintegration.
    Reintegrate,
}

impl RemedyType {
    /// Human-readable description of what this remedy type means in practice.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Repair => "Direct repair of harm caused",
            Self::Restore => "Rebuild relationship with community",
            Self::Prevent => "Structural changes to prevent recurrence",
            Self::Reintegrate => "Pathway back to full participation",
        }
    }
}

impl std::fmt::Display for RemedyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Repair => write!(f, "repair"),
            Self::Restore => write!(f, "restore"),
            Self::Prevent => write!(f, "prevent"),
            Self::Reintegrate => write!(f, "reintegrate"),
        }
    }
}

/// Status of a remedy.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RemedyStatus {
    /// Remedy proposed but not yet accepted.
    Proposed,
    /// Accepted by the target.
    Accepted,
    /// Work is in progress.
    InProgress,
    /// Successfully completed.
    Completed,
    /// Rejected by the target.
    Rejected,
}

impl std::fmt::Display for RemedyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Proposed => write!(f, "proposed"),
            Self::Accepted => write!(f, "accepted"),
            Self::InProgress => write!(f, "in_progress"),
            Self::Completed => write!(f, "completed"),
            Self::Rejected => write!(f, "rejected"),
        }
    }
}

impl Remedy {
    /// Create a new remedy.
    pub fn new(
        target_pubkey: impl Into<String>,
        remedy_type: RemedyType,
        description: impl Into<String>,
        assigned_by: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            target_pubkey: target_pubkey.into(),
            remedy_type,
            description: description.into(),
            assigned_by: assigned_by.into(),
            status: RemedyStatus::Proposed,
            created_at: Utc::now(),
            completed_at: None,
        }
    }

    /// Accept the remedy.
    pub fn accept(&mut self) {
        self.status = RemedyStatus::Accepted;
    }

    /// Begin working on the remedy.
    pub fn begin(&mut self) {
        self.status = RemedyStatus::InProgress;
    }

    /// Complete the remedy.
    pub fn complete(&mut self) {
        self.status = RemedyStatus::Completed;
        self.completed_at = Some(Utc::now());
    }

    /// Reject the remedy.
    pub fn reject(&mut self) {
        self.status = RemedyStatus::Rejected;
    }

    /// Whether the remedy is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.status,
            RemedyStatus::Completed | RemedyStatus::Rejected
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_remedy() {
        let remedy = Remedy::new("bob", RemedyType::Repair, "Apologize to affected party", "alice");
        assert_eq!(remedy.target_pubkey, "bob");
        assert_eq!(remedy.remedy_type, RemedyType::Repair);
        assert_eq!(remedy.status, RemedyStatus::Proposed);
        assert!(!remedy.is_terminal());
    }

    #[test]
    fn remedy_lifecycle() {
        let mut remedy = Remedy::new("bob", RemedyType::Restore, "Rebuild trust", "alice");

        remedy.accept();
        assert_eq!(remedy.status, RemedyStatus::Accepted);

        remedy.begin();
        assert_eq!(remedy.status, RemedyStatus::InProgress);

        remedy.complete();
        assert_eq!(remedy.status, RemedyStatus::Completed);
        assert!(remedy.completed_at.is_some());
        assert!(remedy.is_terminal());
    }

    #[test]
    fn remedy_rejection() {
        let mut remedy = Remedy::new("bob", RemedyType::Prevent, "Change policy", "alice");
        remedy.reject();
        assert_eq!(remedy.status, RemedyStatus::Rejected);
        assert!(remedy.is_terminal());
    }

    #[test]
    fn remedy_type_descriptions() {
        for rt in [RemedyType::Repair, RemedyType::Restore, RemedyType::Prevent, RemedyType::Reintegrate] {
            assert!(!rt.description().is_empty());
        }
    }

    #[test]
    fn remedy_serialization_roundtrip() {
        let remedy = Remedy::new("bob", RemedyType::Reintegrate, "Path back", "alice");
        let json = serde_json::to_string(&remedy).unwrap();
        let deserialized: Remedy = serde_json::from_str(&json).unwrap();
        assert_eq!(remedy, deserialized);
    }
}
