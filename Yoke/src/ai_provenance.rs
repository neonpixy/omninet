//! # AI Transparency in Yoke (R6C)
//!
//! Attribution tracking for AI-assisted actions and content authorship.
//! When Advisor performs an action (via SkillCall), the resulting
//! `ActivityRecord` in Yoke includes `AdvisorAttribution`. When an .idea
//! file is published to Globe, its `ProvenanceScore` (R4B) includes
//! AI attribution data.
//!
//! Any viewer can see: "This design was 78% human-created, 22% AI-assisted."
//!
//! # Not Punitive
//!
//! AI assistance is not penalized. The goal is honest provenance, not
//! gatekeeping. A 100% AI-assisted .idea file is valid. The viewer just
//! knows what they're looking at.
//!
//! # Covenant Alignment
//!
//! **Dignity** — honest attribution respects both human and AI contributors.
//! **Sovereignty** — you choose how much AI helps. The record just shows what happened.
//! **Consent** — transparent provenance lets viewers make informed choices.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── AdvisorAttribution ───────────────────────────────────────────────

/// Attribution data for a single AI-assisted action.
///
/// Attached to an `ActivityRecord` when the action was performed
/// or assisted by Advisor.
///
/// # Example
///
/// ```
/// use yoke::ai_provenance::AdvisorAttribution;
///
/// let attr = AdvisorAttribution::new("design_suggestion", "local-llama-7b")
///     .accepted()
///     .with_confidence(0.85);
/// assert!(attr.was_accepted);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdvisorAttribution {
    /// What the Advisor did: "design_suggestion", "text_edit",
    /// "governance_vote", "data_analysis", etc.
    pub action_type: String,
    /// Whether the human accepted the Advisor's output.
    pub was_accepted: bool,
    /// Whether the human modified the Advisor's output after accepting.
    pub was_modified: bool,
    /// Which AI provider performed the action.
    pub provider_id: String,
    /// How confident the Advisor was in its output (0.0 to 1.0).
    pub confidence: f64,
    /// When this attribution was recorded.
    pub recorded_at: DateTime<Utc>,
}

impl AdvisorAttribution {
    /// Create a new attribution record.
    pub fn new(action_type: impl Into<String>, provider_id: impl Into<String>) -> Self {
        Self {
            action_type: action_type.into(),
            was_accepted: false,
            was_modified: false,
            provider_id: provider_id.into(),
            confidence: 0.0,
            recorded_at: Utc::now(),
        }
    }

    /// Mark as accepted by the human.
    pub fn accepted(mut self) -> Self {
        self.was_accepted = true;
        self
    }

    /// Mark as modified by the human after acceptance.
    pub fn modified(mut self) -> Self {
        self.was_modified = true;
        self.was_accepted = true; // Modified implies accepted first.
        self
    }

    /// Set the confidence level.
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Whether this was a collaborative action (accepted and modified).
    pub fn is_collaborative(&self) -> bool {
        self.was_accepted && self.was_modified
    }

    /// Whether this was purely AI-generated (accepted, not modified).
    pub fn is_pure_ai(&self) -> bool {
        self.was_accepted && !self.was_modified
    }

    /// Whether this was rejected by the human.
    pub fn is_rejected(&self) -> bool {
        !self.was_accepted
    }
}

// ── AuthorshipSource ─────────────────────────────────────────────────

/// Who created or last modified a piece of content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AuthorshipSource {
    /// Created/modified entirely by a human. Contains their pubkey.
    Human(String),
    /// Created/modified entirely by Advisor. Contains (pubkey of human sponsor, provider_id).
    Advisor(String, String),
    /// Human initiated, Advisor modified (or vice versa).
    /// Contains (human pubkey, provider_id).
    Collaborative(String, String),
}

impl AuthorshipSource {
    /// Whether this source involves AI.
    pub fn involves_ai(&self) -> bool {
        matches!(
            self,
            AuthorshipSource::Advisor(_, _) | AuthorshipSource::Collaborative(_, _)
        )
    }

    /// Whether this source is purely human.
    pub fn is_human(&self) -> bool {
        matches!(self, AuthorshipSource::Human(_))
    }

    /// Get the human pubkey associated with this source.
    pub fn human_pubkey(&self) -> &str {
        match self {
            AuthorshipSource::Human(pk) => pk,
            AuthorshipSource::Advisor(pk, _) => pk,
            AuthorshipSource::Collaborative(pk, _) => pk,
        }
    }

    /// Get the provider ID if AI was involved.
    pub fn provider_id(&self) -> Option<&str> {
        match self {
            AuthorshipSource::Human(_) => None,
            AuthorshipSource::Advisor(_, pid) => Some(pid),
            AuthorshipSource::Collaborative(_, pid) => Some(pid),
        }
    }
}

impl std::fmt::Display for AuthorshipSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthorshipSource::Human(pk) => write!(f, "Human({pk})"),
            AuthorshipSource::Advisor(pk, pid) => write!(f, "Advisor({pk}, {pid})"),
            AuthorshipSource::Collaborative(pk, pid) => {
                write!(f, "Collaborative({pk}, {pid})")
            }
        }
    }
}

// ── AuthorshipEntry ──────────────────────────────────────────────────

/// Authorship record for a single Digit within an .idea file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuthorshipEntry {
    /// The Digit's unique identifier.
    pub digit_id: Uuid,
    /// Who originally created this Digit.
    pub created_by: AuthorshipSource,
    /// Who last modified this Digit.
    pub last_modified_by: AuthorshipSource,
    /// When the Digit was last modified.
    pub last_modified_at: DateTime<Utc>,
}

impl AuthorshipEntry {
    /// Create a new authorship entry.
    pub fn new(digit_id: Uuid, created_by: AuthorshipSource) -> Self {
        Self {
            digit_id,
            last_modified_by: created_by.clone(),
            created_by,
            last_modified_at: Utc::now(),
        }
    }

    /// Record a modification.
    pub fn modify(&mut self, modified_by: AuthorshipSource) {
        self.last_modified_by = modified_by;
        self.last_modified_at = Utc::now();
    }

    /// Whether AI was involved in creating or modifying this Digit.
    pub fn has_ai_involvement(&self) -> bool {
        self.created_by.involves_ai() || self.last_modified_by.involves_ai()
    }
}

// ── IdeaAuthorship ───────────────────────────────────────────────────

/// Authorship summary for an entire .idea file.
///
/// Aggregates per-Digit authorship into a percentage breakdown.
/// Viewers see: "This design was 78% human-created, 22% AI-assisted."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IdeaAuthorship {
    /// The .idea file's unique identifier.
    pub idea_id: Uuid,
    /// Total number of human-only actions (create + modify).
    pub human_actions: usize,
    /// Total number of AI-involved actions (create + modify).
    pub advisor_actions: usize,
    /// Percentage of actions involving AI (0.0 to 100.0).
    pub advisor_percentage: f64,
    /// Per-Digit authorship breakdown.
    pub breakdown: Vec<AuthorshipEntry>,
}

impl IdeaAuthorship {
    /// Create a new empty authorship record for an .idea.
    pub fn new(idea_id: Uuid) -> Self {
        Self {
            idea_id,
            human_actions: 0,
            advisor_actions: 0,
            advisor_percentage: 0.0,
            breakdown: Vec::new(),
        }
    }

    /// Add a Digit creation event.
    pub fn record_creation(&mut self, digit_id: Uuid, source: AuthorshipSource) {
        if source.involves_ai() {
            self.advisor_actions += 1;
        } else {
            self.human_actions += 1;
        }
        self.breakdown.push(AuthorshipEntry::new(digit_id, source));
        self.recompute_percentage();
    }

    /// Record a Digit modification event.
    pub fn record_modification(&mut self, digit_id: Uuid, source: AuthorshipSource) {
        if source.involves_ai() {
            self.advisor_actions += 1;
        } else {
            self.human_actions += 1;
        }

        // Update existing entry or create new one.
        if let Some(entry) = self.breakdown.iter_mut().find(|e| e.digit_id == digit_id) {
            entry.modify(source);
        } else {
            self.breakdown
                .push(AuthorshipEntry::new(digit_id, source));
        }
        self.recompute_percentage();
    }

    /// Recompute the advisor percentage from action counts.
    fn recompute_percentage(&mut self) {
        let total = self.human_actions + self.advisor_actions;
        if total == 0 {
            self.advisor_percentage = 0.0;
        } else {
            self.advisor_percentage = (self.advisor_actions as f64 / total as f64) * 100.0;
        }
    }

    /// Total number of actions tracked.
    pub fn total_actions(&self) -> usize {
        self.human_actions + self.advisor_actions
    }

    /// Human percentage (complement of advisor_percentage).
    pub fn human_percentage(&self) -> f64 {
        100.0 - self.advisor_percentage
    }

    /// Number of Digits with AI involvement.
    pub fn digits_with_ai(&self) -> usize {
        self.breakdown.iter().filter(|e| e.has_ai_involvement()).count()
    }

    /// Number of Digits created purely by humans.
    pub fn digits_purely_human(&self) -> usize {
        self.breakdown.iter().filter(|e| !e.has_ai_involvement()).count()
    }

    /// Build from a complete list of authorship entries (recalculates counts).
    pub fn from_entries(idea_id: Uuid, entries: Vec<AuthorshipEntry>) -> Self {
        let mut authorship = Self::new(idea_id);
        for entry in &entries {
            if entry.created_by.involves_ai() {
                authorship.advisor_actions += 1;
            } else {
                authorship.human_actions += 1;
            }
            // Count last_modified_by as an additional action if it differs.
            if entry.last_modified_by != entry.created_by {
                if entry.last_modified_by.involves_ai() {
                    authorship.advisor_actions += 1;
                } else {
                    authorship.human_actions += 1;
                }
            }
        }
        authorship.breakdown = entries;
        authorship.recompute_percentage();
        authorship
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // --- AdvisorAttribution ---

    #[test]
    fn attribution_new() {
        let attr = AdvisorAttribution::new("text_edit", "llama-7b");
        assert_eq!(attr.action_type, "text_edit");
        assert!(!attr.was_accepted);
        assert!(!attr.was_modified);
    }

    #[test]
    fn attribution_accepted() {
        let attr = AdvisorAttribution::new("design_suggestion", "claude-3")
            .accepted()
            .with_confidence(0.9);
        assert!(attr.was_accepted);
        assert!(!attr.was_modified);
        assert!(attr.is_pure_ai());
        assert!(!attr.is_collaborative());
    }

    #[test]
    fn attribution_modified() {
        let attr = AdvisorAttribution::new("text_edit", "local-model")
            .modified()
            .with_confidence(0.7);
        assert!(attr.was_accepted);
        assert!(attr.was_modified);
        assert!(attr.is_collaborative());
        assert!(!attr.is_pure_ai());
    }

    #[test]
    fn attribution_rejected() {
        let attr = AdvisorAttribution::new("governance_vote", "small-model");
        assert!(attr.is_rejected());
    }

    #[test]
    fn attribution_confidence_clamped() {
        let attr = AdvisorAttribution::new("test", "p").with_confidence(1.5);
        assert!((attr.confidence - 1.0).abs() < 0.001);

        let attr = AdvisorAttribution::new("test", "p").with_confidence(-0.5);
        assert!((attr.confidence - 0.0).abs() < 0.001);
    }

    #[test]
    fn attribution_serde() {
        let attr = AdvisorAttribution::new("data_analysis", "provider-1")
            .accepted()
            .with_confidence(0.85);
        let json = serde_json::to_string(&attr).unwrap();
        let restored: AdvisorAttribution = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.action_type, "data_analysis");
        assert!(restored.was_accepted);
    }

    // --- AuthorshipSource ---

    #[test]
    fn source_human() {
        let src = AuthorshipSource::Human("cpub1alice".to_string());
        assert!(src.is_human());
        assert!(!src.involves_ai());
        assert_eq!(src.human_pubkey(), "cpub1alice");
        assert!(src.provider_id().is_none());
    }

    #[test]
    fn source_advisor() {
        let src = AuthorshipSource::Advisor("cpub1alice".to_string(), "llama-7b".to_string());
        assert!(!src.is_human());
        assert!(src.involves_ai());
        assert_eq!(src.human_pubkey(), "cpub1alice");
        assert_eq!(src.provider_id(), Some("llama-7b"));
    }

    #[test]
    fn source_collaborative() {
        let src =
            AuthorshipSource::Collaborative("cpub1bob".to_string(), "claude-3".to_string());
        assert!(!src.is_human());
        assert!(src.involves_ai());
        assert_eq!(src.human_pubkey(), "cpub1bob");
        assert_eq!(src.provider_id(), Some("claude-3"));
    }

    #[test]
    fn source_display() {
        let human = AuthorshipSource::Human("cpub1alice".to_string());
        assert!(format!("{human}").contains("Human"));

        let ai = AuthorshipSource::Advisor("cpub1alice".to_string(), "llama".to_string());
        assert!(format!("{ai}").contains("Advisor"));
    }

    #[test]
    fn source_serde() {
        let src =
            AuthorshipSource::Collaborative("cpub1alice".to_string(), "provider".to_string());
        let json = serde_json::to_string(&src).unwrap();
        let restored: AuthorshipSource = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, src);
    }

    // --- AuthorshipEntry ---

    #[test]
    fn entry_creation() {
        let id = Uuid::new_v4();
        let entry = AuthorshipEntry::new(id, AuthorshipSource::Human("cpub1alice".to_string()));
        assert_eq!(entry.digit_id, id);
        assert!(!entry.has_ai_involvement());
    }

    #[test]
    fn entry_modification() {
        let id = Uuid::new_v4();
        let mut entry =
            AuthorshipEntry::new(id, AuthorshipSource::Human("cpub1alice".to_string()));
        entry.modify(AuthorshipSource::Advisor(
            "cpub1alice".to_string(),
            "llama".to_string(),
        ));
        assert!(entry.has_ai_involvement());
        assert!(entry.last_modified_by.involves_ai());
    }

    #[test]
    fn entry_serde() {
        let entry = AuthorshipEntry::new(
            Uuid::new_v4(),
            AuthorshipSource::Advisor("cpub1bob".to_string(), "provider-1".to_string()),
        );
        let json = serde_json::to_string(&entry).unwrap();
        let restored: AuthorshipEntry = serde_json::from_str(&json).unwrap();
        assert!(restored.has_ai_involvement());
    }

    // --- IdeaAuthorship ---

    #[test]
    fn authorship_new_empty() {
        let id = Uuid::new_v4();
        let auth = IdeaAuthorship::new(id);
        assert_eq!(auth.idea_id, id);
        assert_eq!(auth.total_actions(), 0);
        assert!((auth.advisor_percentage - 0.0).abs() < 0.001);
    }

    #[test]
    fn authorship_record_creation() {
        let mut auth = IdeaAuthorship::new(Uuid::new_v4());
        auth.record_creation(
            Uuid::new_v4(),
            AuthorshipSource::Human("cpub1alice".to_string()),
        );
        auth.record_creation(
            Uuid::new_v4(),
            AuthorshipSource::Advisor("cpub1alice".to_string(), "llama".to_string()),
        );
        assert_eq!(auth.human_actions, 1);
        assert_eq!(auth.advisor_actions, 1);
        assert!((auth.advisor_percentage - 50.0).abs() < 0.001);
    }

    #[test]
    fn authorship_record_modification() {
        let mut auth = IdeaAuthorship::new(Uuid::new_v4());
        let digit_id = Uuid::new_v4();
        auth.record_creation(
            digit_id,
            AuthorshipSource::Human("cpub1alice".to_string()),
        );
        auth.record_modification(
            digit_id,
            AuthorshipSource::Collaborative("cpub1alice".to_string(), "llama".to_string()),
        );
        assert_eq!(auth.total_actions(), 2);
        assert!((auth.advisor_percentage - 50.0).abs() < 0.001);
    }

    #[test]
    fn authorship_percentage_all_human() {
        let mut auth = IdeaAuthorship::new(Uuid::new_v4());
        for _ in 0..5 {
            auth.record_creation(
                Uuid::new_v4(),
                AuthorshipSource::Human("cpub1alice".to_string()),
            );
        }
        assert!((auth.advisor_percentage - 0.0).abs() < 0.001);
        assert!((auth.human_percentage() - 100.0).abs() < 0.001);
    }

    #[test]
    fn authorship_percentage_all_ai() {
        let mut auth = IdeaAuthorship::new(Uuid::new_v4());
        for _ in 0..4 {
            auth.record_creation(
                Uuid::new_v4(),
                AuthorshipSource::Advisor("cpub1alice".to_string(), "model".to_string()),
            );
        }
        assert!((auth.advisor_percentage - 100.0).abs() < 0.001);
        assert!((auth.human_percentage() - 0.0).abs() < 0.001);
    }

    #[test]
    fn authorship_digits_with_ai() {
        let mut auth = IdeaAuthorship::new(Uuid::new_v4());
        auth.record_creation(
            Uuid::new_v4(),
            AuthorshipSource::Human("cpub1alice".to_string()),
        );
        auth.record_creation(
            Uuid::new_v4(),
            AuthorshipSource::Advisor("cpub1alice".to_string(), "model".to_string()),
        );
        auth.record_creation(
            Uuid::new_v4(),
            AuthorshipSource::Collaborative("cpub1bob".to_string(), "model".to_string()),
        );
        assert_eq!(auth.digits_with_ai(), 2);
        assert_eq!(auth.digits_purely_human(), 1);
    }

    #[test]
    fn authorship_from_entries() {
        let entries = vec![
            AuthorshipEntry::new(
                Uuid::new_v4(),
                AuthorshipSource::Human("cpub1alice".to_string()),
            ),
            AuthorshipEntry::new(
                Uuid::new_v4(),
                AuthorshipSource::Advisor("cpub1alice".to_string(), "llama".to_string()),
            ),
            AuthorshipEntry::new(
                Uuid::new_v4(),
                AuthorshipSource::Human("cpub1bob".to_string()),
            ),
        ];
        let auth = IdeaAuthorship::from_entries(Uuid::new_v4(), entries);
        assert_eq!(auth.human_actions, 2);
        assert_eq!(auth.advisor_actions, 1);
        assert!((auth.advisor_percentage - 33.333).abs() < 0.5);
    }

    #[test]
    fn authorship_from_entries_with_modifications() {
        let digit_id = Uuid::new_v4();
        let mut entry = AuthorshipEntry::new(
            digit_id,
            AuthorshipSource::Human("cpub1alice".to_string()),
        );
        entry.modify(AuthorshipSource::Advisor(
            "cpub1alice".to_string(),
            "llama".to_string(),
        ));

        let auth = IdeaAuthorship::from_entries(Uuid::new_v4(), vec![entry]);
        // 1 human creation + 1 AI modification = 50% AI.
        assert_eq!(auth.human_actions, 1);
        assert_eq!(auth.advisor_actions, 1);
        assert!((auth.advisor_percentage - 50.0).abs() < 0.001);
    }

    #[test]
    fn authorship_serde() {
        let mut auth = IdeaAuthorship::new(Uuid::new_v4());
        auth.record_creation(
            Uuid::new_v4(),
            AuthorshipSource::Human("cpub1alice".to_string()),
        );
        let json = serde_json::to_string(&auth).unwrap();
        let restored: IdeaAuthorship = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.human_actions, 1);
        assert_eq!(restored.breakdown.len(), 1);
    }

    #[test]
    fn authorship_modification_updates_existing() {
        let mut auth = IdeaAuthorship::new(Uuid::new_v4());
        let digit_id = Uuid::new_v4();
        auth.record_creation(
            digit_id,
            AuthorshipSource::Human("cpub1alice".to_string()),
        );
        auth.record_modification(
            digit_id,
            AuthorshipSource::Advisor("cpub1alice".to_string(), "model".to_string()),
        );
        // Should still have 1 entry (same digit), not 2.
        assert_eq!(auth.breakdown.len(), 1);
        assert!(auth.breakdown[0].last_modified_by.involves_ai());
    }

    #[test]
    fn authorship_modification_new_digit_adds_entry() {
        let mut auth = IdeaAuthorship::new(Uuid::new_v4());
        auth.record_modification(
            Uuid::new_v4(),
            AuthorshipSource::Human("cpub1alice".to_string()),
        );
        assert_eq!(auth.breakdown.len(), 1);
    }
}
