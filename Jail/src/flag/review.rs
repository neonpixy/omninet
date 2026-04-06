//! Flag review — community-driven accountability decisions.
//!
//! Flags are reviewed by community processes, not algorithms. The review
//! produces an outcome and optional community action.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A review of an accountability flag.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlagReview {
    /// Unique review identifier.
    pub id: Uuid,
    /// The flag being reviewed.
    pub flag_id: Uuid,
    /// Who conducted the review.
    pub reviewer_pubkey: String,
    /// Outcome of the review.
    pub outcome: ReviewOutcome,
    /// Action taken by the community (if any).
    pub action: Option<CommunityAction>,
    /// When the review was completed.
    pub reviewed_at: DateTime<Utc>,
    /// Reviewer's notes.
    pub notes: Option<String>,
}

impl FlagReview {
    /// Create a new flag review.
    pub fn new(
        flag_id: Uuid,
        reviewer: impl Into<String>,
        outcome: ReviewOutcome,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            flag_id,
            reviewer_pubkey: reviewer.into(),
            outcome,
            action: None,
            reviewed_at: Utc::now(),
            notes: None,
        }
    }

    /// Set the community action.
    pub fn with_action(mut self, action: CommunityAction) -> Self {
        self.action = Some(action);
        self
    }

    /// Add reviewer notes.
    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }
}

/// Outcome of reviewing a flag.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ReviewOutcome {
    /// Flag is upheld — the concern is valid.
    Upheld,
    /// Flag is dismissed — the concern was not substantiated.
    Dismissed,
    /// More evidence needed before deciding.
    NeedsMoreEvidence,
    /// Escalated to a higher body or external authority.
    Escalated,
}

impl std::fmt::Display for ReviewOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Upheld => write!(f, "upheld"),
            Self::Dismissed => write!(f, "dismissed"),
            Self::NeedsMoreEvidence => write!(f, "needs_more_evidence"),
            Self::Escalated => write!(f, "escalated"),
        }
    }
}

/// Community action in response to an upheld flag.
///
/// These are restorative, not punitive. "Education" is always the first option;
/// escalation happens only when lesser measures prove insufficient (Art. 7 §3).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CommunityAction {
    /// Educational dialogue — resources and pathways to compliance.
    Education,
    /// Formal warning — acknowledgment of the concern.
    Warning,
    /// Restricted access to certain community functions.
    RestrictedAccess,
    /// Must complete re-verification of identity.
    RequireReVerification,
    /// Escalated to external/real-world authorities.
    EscalateExternally,
}

impl CommunityAction {
    /// Human-readable description of what this action entails.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Education => "Educational dialogue with resources and compliance pathways",
            Self::Warning => "Formal acknowledgment of the concern on record",
            Self::RestrictedAccess => "Restricted access to certain community functions",
            Self::RequireReVerification => "Identity re-verification required",
            Self::EscalateExternally => "Escalated to external real-world authorities",
        }
    }
}

impl std::fmt::Display for CommunityAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Education => write!(f, "education"),
            Self::Warning => write!(f, "warning"),
            Self::RestrictedAccess => write!(f, "restricted_access"),
            Self::RequireReVerification => write!(f, "require_reverification"),
            Self::EscalateExternally => write!(f, "escalate_externally"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_review() {
        let flag_id = Uuid::new_v4();
        let review = FlagReview::new(flag_id, "reviewer", ReviewOutcome::Upheld)
            .with_action(CommunityAction::Warning)
            .with_notes("Corroborated by 2 witnesses");
        assert_eq!(review.flag_id, flag_id);
        assert_eq!(review.outcome, ReviewOutcome::Upheld);
        assert_eq!(review.action, Some(CommunityAction::Warning));
        assert!(review.notes.unwrap().contains("witnesses"));
    }

    #[test]
    fn review_without_action() {
        let review = FlagReview::new(Uuid::new_v4(), "reviewer", ReviewOutcome::Dismissed);
        assert!(review.action.is_none());
    }

    #[test]
    fn outcome_display() {
        assert_eq!(ReviewOutcome::Upheld.to_string(), "upheld");
        assert_eq!(ReviewOutcome::NeedsMoreEvidence.to_string(), "needs_more_evidence");
    }

    #[test]
    fn action_display() {
        assert_eq!(CommunityAction::Education.to_string(), "education");
        assert_eq!(CommunityAction::EscalateExternally.to_string(), "escalate_externally");
    }

    #[test]
    fn action_descriptions_non_empty() {
        for action in [
            CommunityAction::Education,
            CommunityAction::Warning,
            CommunityAction::RestrictedAccess,
            CommunityAction::RequireReVerification,
            CommunityAction::EscalateExternally,
        ] {
            assert!(!action.description().is_empty());
        }
    }

    #[test]
    fn review_serialization_roundtrip() {
        let review = FlagReview::new(Uuid::new_v4(), "reviewer", ReviewOutcome::Upheld)
            .with_action(CommunityAction::Education);
        let json = serde_json::to_string(&review).unwrap();
        let deserialized: FlagReview = serde_json::from_str(&json).unwrap();
        assert_eq!(review, deserialized);
    }
}
