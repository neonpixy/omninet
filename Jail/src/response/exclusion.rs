//! Protective exclusion — the last resort, never permanent.
//!
//! From Constellation Art. 7 §12: "No enforcement action shall... create
//! permanent castes of excluded persons or communities."
//!
//! From Constellation Art. 7 §10: "Where violators demonstrate genuine
//! compliance, make appropriate reparations, and commit to future adherence,
//! communities shall provide pathways back to full participation."
//!
//! Every exclusion has a mandatory review schedule and a restoration path.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A protective exclusion — separation for safety, not punishment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProtectiveExclusion {
    /// Unique exclusion identifier.
    pub id: Uuid,
    /// The excluded person.
    pub excluded_pubkey: String,
    /// Why exclusion was necessary.
    pub reason: String,
    /// Who initiated the exclusion.
    pub initiated_by: String,
    /// Communities that have implemented the exclusion.
    pub communities: Vec<String>,
    /// When the exclusion started.
    pub started_at: DateTime<Utc>,
    /// Mandatory review schedule (no permanent castes).
    pub review_schedule: ReviewSchedule,
    /// Path back to participation (Art. 7 §10).
    pub restoration_path: Option<RestorationPath>,
    /// When the exclusion was lifted (if ever).
    pub lifted_at: Option<DateTime<Utc>>,
    /// History of reviews.
    pub reviews: Vec<ExclusionReview>,
}

/// Mandatory review schedule — ensures exclusions are periodically re-evaluated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReviewSchedule {
    /// Days between reviews.
    pub review_interval_days: u64,
    /// When the next review is due.
    pub next_review: DateTime<Utc>,
    /// How many reviews have been completed.
    pub reviews_completed: usize,
}

impl ReviewSchedule {
    /// Create a new review schedule.
    pub fn new(interval_days: u64) -> Self {
        Self {
            review_interval_days: interval_days,
            next_review: Utc::now() + Duration::days(interval_days as i64),
            reviews_completed: 0,
        }
    }

    /// Whether a review is overdue.
    pub fn is_overdue(&self) -> bool {
        Utc::now() > self.next_review
    }

    /// Advance to the next review period.
    pub fn advance(&mut self) {
        self.reviews_completed += 1;
        self.next_review = Utc::now() + Duration::days(self.review_interval_days as i64);
    }
}

/// A path back to full participation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RestorationPath {
    /// Conditions that must be met for restoration.
    pub conditions: Vec<String>,
    /// Progress toward meeting conditions.
    pub progress: Vec<RestorationProgress>,
    /// Optional mentor to support the person's return.
    pub mentor_pubkey: Option<String>,
}

/// Progress on a restoration condition.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RestorationProgress {
    /// Which condition this tracks.
    pub condition_index: usize,
    /// Description of progress made.
    pub description: String,
    /// When this progress was recorded.
    pub recorded_at: DateTime<Utc>,
    /// Whether this condition is fully met.
    pub condition_met: bool,
}

/// A review of an active exclusion.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExclusionReview {
    /// Who conducted the review.
    pub reviewer_pubkey: String,
    /// When the review occurred.
    pub reviewed_at: DateTime<Utc>,
    /// Decision from the review.
    pub decision: ExclusionDecision,
    /// Reasoning for the decision.
    pub reasoning: String,
    /// When the next review is scheduled.
    pub next_review: Option<DateTime<Utc>>,
}

/// Decision from reviewing an exclusion.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ExclusionDecision {
    /// Maintain the exclusion as-is.
    Maintain,
    /// Modify the terms (e.g., reduce scope).
    Modify,
    /// Lift the exclusion — person may return.
    Lift,
}

impl std::fmt::Display for ExclusionDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Maintain => write!(f, "maintain"),
            Self::Modify => write!(f, "modify"),
            Self::Lift => write!(f, "lift"),
        }
    }
}

impl ProtectiveExclusion {
    /// Create a new protective exclusion with mandatory review schedule.
    pub fn new(
        excluded_pubkey: impl Into<String>,
        reason: impl Into<String>,
        initiated_by: impl Into<String>,
        communities: Vec<String>,
        review_interval_days: u64,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            excluded_pubkey: excluded_pubkey.into(),
            reason: reason.into(),
            initiated_by: initiated_by.into(),
            communities,
            started_at: Utc::now(),
            review_schedule: ReviewSchedule::new(review_interval_days),
            restoration_path: None,
            lifted_at: None,
            reviews: Vec::new(),
        }
    }

    /// Set a restoration path.
    pub fn with_restoration_path(mut self, path: RestorationPath) -> Self {
        self.restoration_path = Some(path);
        self
    }

    /// Record a review of this exclusion.
    pub fn record_review(&mut self, review: ExclusionReview) {
        if review.decision == ExclusionDecision::Lift {
            self.lifted_at = Some(Utc::now());
        }
        self.review_schedule.advance();
        self.reviews.push(review);
    }

    /// Whether the exclusion is still active.
    pub fn is_active(&self) -> bool {
        self.lifted_at.is_none()
    }

    /// Whether a review is overdue.
    pub fn is_review_overdue(&self) -> bool {
        self.is_active() && self.review_schedule.is_overdue()
    }

    /// Whether all restoration conditions have been met.
    pub fn restoration_conditions_met(&self) -> bool {
        self.restoration_path.as_ref().is_some_and(|path| {
            if path.conditions.is_empty() {
                return false;
            }
            path.conditions.iter().enumerate().all(|(i, _)| {
                path.progress
                    .iter()
                    .any(|p| p.condition_index == i && p.condition_met)
            })
        })
    }
}

impl RestorationPath {
    /// Create a new restoration path with conditions.
    pub fn new(conditions: Vec<String>) -> Self {
        Self {
            conditions,
            progress: Vec::new(),
            mentor_pubkey: None,
        }
    }

    /// Assign a mentor.
    pub fn with_mentor(mut self, mentor: impl Into<String>) -> Self {
        self.mentor_pubkey = Some(mentor.into());
        self
    }

    /// Record progress on a condition.
    pub fn record_progress(&mut self, condition_index: usize, description: impl Into<String>, met: bool) {
        self.progress.push(RestorationProgress {
            condition_index,
            description: description.into(),
            recorded_at: Utc::now(),
            condition_met: met,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_exclusion_with_review_schedule() {
        let exclusion = ProtectiveExclusion::new(
            "bob",
            "persistent safety concern",
            "alice",
            vec!["comm_1".into(), "comm_2".into()],
            90,
        );
        assert!(exclusion.is_active());
        assert!(!exclusion.is_review_overdue());
        assert_eq!(exclusion.review_schedule.review_interval_days, 90);
        assert_eq!(exclusion.review_schedule.reviews_completed, 0);
    }

    #[test]
    fn exclusion_with_restoration_path() {
        let path = RestorationPath::new(vec![
            "Complete mediation with affected party".into(),
            "Demonstrate changed behavior for 60 days".into(),
        ])
        .with_mentor("carol");

        let exclusion = ProtectiveExclusion::new("bob", "reason", "alice", vec![], 90)
            .with_restoration_path(path);

        assert!(exclusion.restoration_path.is_some());
        let path = exclusion.restoration_path.as_ref().unwrap();
        assert_eq!(path.conditions.len(), 2);
        assert_eq!(path.mentor_pubkey, Some("carol".to_string()));
    }

    #[test]
    fn review_lifts_exclusion() {
        let mut exclusion =
            ProtectiveExclusion::new("bob", "reason", "alice", vec!["comm_1".into()], 30);
        assert!(exclusion.is_active());

        exclusion.record_review(ExclusionReview {
            reviewer_pubkey: "reviewer".into(),
            reviewed_at: Utc::now(),
            decision: ExclusionDecision::Lift,
            reasoning: "Conditions met, safe to return".into(),
            next_review: None,
        });

        assert!(!exclusion.is_active());
        assert!(exclusion.lifted_at.is_some());
        assert_eq!(exclusion.review_schedule.reviews_completed, 1);
    }

    #[test]
    fn review_maintains_exclusion() {
        let mut exclusion = ProtectiveExclusion::new("bob", "reason", "alice", vec![], 30);

        exclusion.record_review(ExclusionReview {
            reviewer_pubkey: "reviewer".into(),
            reviewed_at: Utc::now(),
            decision: ExclusionDecision::Maintain,
            reasoning: "Safety concern persists".into(),
            next_review: Some(Utc::now() + Duration::days(30)),
        });

        assert!(exclusion.is_active());
        assert_eq!(exclusion.reviews.len(), 1);
    }

    #[test]
    fn restoration_conditions_tracking() {
        let mut path = RestorationPath::new(vec!["condition_0".into(), "condition_1".into()]);

        let exclusion = ProtectiveExclusion::new("bob", "reason", "alice", vec![], 90)
            .with_restoration_path(path.clone());
        assert!(!exclusion.restoration_conditions_met());

        path.record_progress(0, "Completed mediation", true);
        path.record_progress(1, "Demonstrated change", true);

        let exclusion = ProtectiveExclusion::new("bob", "reason", "alice", vec![], 90)
            .with_restoration_path(path);
        assert!(exclusion.restoration_conditions_met());
    }

    #[test]
    fn partial_restoration_progress() {
        let mut path = RestorationPath::new(vec!["condition_0".into(), "condition_1".into()]);
        path.record_progress(0, "Completed", true);
        // condition_1 not yet met

        let exclusion = ProtectiveExclusion::new("bob", "reason", "alice", vec![], 90)
            .with_restoration_path(path);
        assert!(!exclusion.restoration_conditions_met());
    }

    #[test]
    fn exclusion_decision_display() {
        assert_eq!(ExclusionDecision::Maintain.to_string(), "maintain");
        assert_eq!(ExclusionDecision::Modify.to_string(), "modify");
        assert_eq!(ExclusionDecision::Lift.to_string(), "lift");
    }

    #[test]
    fn exclusion_serialization_roundtrip() {
        let exclusion = ProtectiveExclusion::new("bob", "reason", "alice", vec!["comm".into()], 90);
        let json = serde_json::to_string(&exclusion).unwrap();
        let deserialized: ProtectiveExclusion = serde_json::from_str(&json).unwrap();
        assert_eq!(exclusion, deserialized);
    }
}
