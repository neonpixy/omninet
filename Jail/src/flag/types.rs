//! Accountability flags — concerns raised about a person's behavior.
//!
//! Flags are the primary input to the accountability system. Anyone can raise
//! a flag; rate limiting and anti-weaponization prevent abuse. Flags are reviewed
//! by community processes, not algorithms.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An accountability flag raised about a person.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccountabilityFlag {
    /// Unique flag identifier.
    pub id: Uuid,
    /// Pubkey of the person raising the flag.
    pub flagger_pubkey: String,
    /// Pubkey of the person being flagged.
    pub flagged_pubkey: String,
    /// Type of concern.
    pub category: FlagCategory,
    /// Urgency level.
    pub severity: FlagSeverity,
    /// Description of the concern.
    pub description: String,
    /// Additional context (evidence, witnesses, related events).
    pub context: Option<FlagContext>,
    /// Community context in which the flag was raised.
    pub community_id: Option<String>,
    /// Current review status.
    pub status: FlagReviewStatus,
    /// When the flag was raised.
    pub raised_at: DateTime<Utc>,
    /// Cryptographic signature.
    pub signature: String,
}

impl AccountabilityFlag {
    /// Raise a new accountability flag.
    pub fn raise(
        flagger: impl Into<String>,
        flagged: impl Into<String>,
        category: FlagCategory,
        severity: FlagSeverity,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            flagger_pubkey: flagger.into(),
            flagged_pubkey: flagged.into(),
            category,
            severity,
            description: description.into(),
            context: None,
            community_id: None,
            status: FlagReviewStatus::Pending,
            raised_at: Utc::now(),
            signature: String::new(),
        }
    }

    /// Set the community context.
    pub fn with_community(mut self, community_id: impl Into<String>) -> Self {
        self.community_id = Some(community_id.into());
        self
    }

    /// Add additional context.
    pub fn with_context(mut self, context: FlagContext) -> Self {
        self.context = Some(context);
        self
    }

    /// Set the cryptographic signature.
    pub fn with_signature(mut self, signature: impl Into<String>) -> Self {
        self.signature = signature.into();
        self
    }

    /// Whether this flag is in a terminal review state.
    pub fn is_resolved(&self) -> bool {
        matches!(
            self.status,
            FlagReviewStatus::Upheld | FlagReviewStatus::Dismissed
        )
    }
}

/// Category of concern.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FlagCategory {
    /// Sexual or exploitative behavior.
    PredatoryBehavior,
    /// Impersonation or false identity.
    IdentityFraud,
    /// Repeated unwanted contact.
    Harassment,
    /// Inappropriate conduct.
    Inappropriate,
    /// Unexplained behavior pattern.
    SuspiciousActivity,
    /// Concern involving minors.
    MinorSafety,
    /// Unclassified.
    Other,
}

impl std::fmt::Display for FlagCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PredatoryBehavior => write!(f, "predatory_behavior"),
            Self::IdentityFraud => write!(f, "identity_fraud"),
            Self::Harassment => write!(f, "harassment"),
            Self::Inappropriate => write!(f, "inappropriate"),
            Self::SuspiciousActivity => write!(f, "suspicious_activity"),
            Self::MinorSafety => write!(f, "minor_safety"),
            Self::Other => write!(f, "other"),
        }
    }
}

/// Severity level of a flag.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FlagSeverity {
    /// Minor issue.
    Low,
    /// Notable concern.
    Medium,
    /// Significant problem.
    High,
    /// Immediate danger.
    Critical,
}

impl std::fmt::Display for FlagSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Additional context for a flag.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FlagContext {
    /// Link to a related event or incident.
    pub related_event_id: Option<String>,
    /// Content-addressed evidence hashes.
    pub evidence_hashes: Vec<String>,
    /// Number of witnesses.
    pub witness_count: u32,
    /// Community where the incident occurred.
    pub related_community_id: Option<String>,
}

impl FlagContext {
    pub fn new() -> Self {
        Self {
            related_event_id: None,
            evidence_hashes: Vec::new(),
            witness_count: 0,
            related_community_id: None,
        }
    }

    pub fn with_evidence(mut self, hash: impl Into<String>) -> Self {
        self.evidence_hashes.push(hash.into());
        self
    }

    pub fn with_witnesses(mut self, count: u32) -> Self {
        self.witness_count = count;
        self
    }
}

impl Default for FlagContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Review status of a flag.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FlagReviewStatus {
    /// Awaiting review.
    Pending,
    /// Currently being reviewed.
    UnderReview,
    /// Community upheld the flag.
    Upheld,
    /// Community dismissed the flag.
    Dismissed,
    /// Under appeal.
    Appealed,
}

impl std::fmt::Display for FlagReviewStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::UnderReview => write!(f, "under_review"),
            Self::Upheld => write!(f, "upheld"),
            Self::Dismissed => write!(f, "dismissed"),
            Self::Appealed => write!(f, "appealed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raise_flag() {
        let flag = AccountabilityFlag::raise(
            "alice",
            "bob",
            FlagCategory::Harassment,
            FlagSeverity::Medium,
            "Repeated unwanted messages",
        );
        assert_eq!(flag.flagger_pubkey, "alice");
        assert_eq!(flag.flagged_pubkey, "bob");
        assert_eq!(flag.category, FlagCategory::Harassment);
        assert_eq!(flag.severity, FlagSeverity::Medium);
        assert_eq!(flag.status, FlagReviewStatus::Pending);
        assert!(flag.context.is_none());
        assert!(flag.community_id.is_none());
    }

    #[test]
    fn flag_with_community() {
        let flag = AccountabilityFlag::raise(
            "alice",
            "bob",
            FlagCategory::SuspiciousActivity,
            FlagSeverity::Low,
            "test",
        )
        .with_community("community_1");
        assert_eq!(flag.community_id, Some("community_1".to_string()));
    }

    #[test]
    fn flag_with_context() {
        let context = FlagContext::new()
            .with_evidence("sha256_abc123")
            .with_witnesses(3);
        let flag = AccountabilityFlag::raise(
            "alice",
            "bob",
            FlagCategory::PredatoryBehavior,
            FlagSeverity::Critical,
            "test",
        )
        .with_context(context);
        let ctx = flag.context.unwrap();
        assert_eq!(ctx.evidence_hashes.len(), 1);
        assert_eq!(ctx.witness_count, 3);
    }

    #[test]
    fn flag_is_resolved() {
        let mut flag = AccountabilityFlag::raise(
            "a", "b", FlagCategory::Other, FlagSeverity::Low, "test",
        );
        assert!(!flag.is_resolved());

        flag.status = FlagReviewStatus::Upheld;
        assert!(flag.is_resolved());

        flag.status = FlagReviewStatus::Dismissed;
        assert!(flag.is_resolved());

        flag.status = FlagReviewStatus::Appealed;
        assert!(!flag.is_resolved());
    }

    #[test]
    fn severity_ordering() {
        assert!(FlagSeverity::Low < FlagSeverity::Medium);
        assert!(FlagSeverity::Medium < FlagSeverity::High);
        assert!(FlagSeverity::High < FlagSeverity::Critical);
    }

    #[test]
    fn category_display() {
        assert_eq!(FlagCategory::PredatoryBehavior.to_string(), "predatory_behavior");
        assert_eq!(FlagCategory::MinorSafety.to_string(), "minor_safety");
    }

    #[test]
    fn flag_serialization_roundtrip() {
        let flag = AccountabilityFlag::raise(
            "alice", "bob", FlagCategory::Harassment, FlagSeverity::High, "test",
        )
        .with_community("comm_1");
        let json = serde_json::to_string(&flag).unwrap();
        let deserialized: AccountabilityFlag = serde_json::from_str(&json).unwrap();
        assert_eq!(flag, deserialized);
    }
}
