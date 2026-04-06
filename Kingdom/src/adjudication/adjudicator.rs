use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::dispute::DisputeType;

/// A person qualified to adjudicate disputes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Adjudicator {
    pub id: Uuid,
    pub pubkey: String,
    pub display_name: Option<String>,
    pub jurisdictions: Vec<AdjudicatorJurisdiction>,
    pub specializations: Vec<DisputeType>,
    pub qualifications: Vec<Qualification>,
    pub availability: AdjudicatorAvailability,
    pub record: AdjudicatorRecord,
    pub status: AdjudicatorStatus,
    pub certified_at: DateTime<Utc>,
}

impl Adjudicator {
    /// Create a new adjudicator with no specializations or jurisdictions (generalist).
    pub fn new(pubkey: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            pubkey: pubkey.into(),
            display_name: None,
            jurisdictions: Vec::new(),
            specializations: Vec::new(),
            qualifications: Vec::new(),
            availability: AdjudicatorAvailability::Available,
            record: AdjudicatorRecord::new(),
            status: AdjudicatorStatus::Active,
            certified_at: Utc::now(),
        }
    }

    /// Whether this adjudicator is active and has capacity for new cases.
    pub fn is_available(&self) -> bool {
        self.status == AdjudicatorStatus::Active
            && self.availability == AdjudicatorAvailability::Available
    }

    /// Whether this adjudicator can handle a given dispute type (generalists can handle all).
    pub fn can_handle(&self, dispute_type: &DisputeType) -> bool {
        self.specializations.is_empty() || self.specializations.contains(dispute_type)
    }

    /// Whether this adjudicator has jurisdiction in a given context (empty = universal).
    pub fn has_jurisdiction(&self, context: &AdjudicatorJurisdiction) -> bool {
        self.jurisdictions.is_empty()
            || self.jurisdictions.contains(context)
            || self.jurisdictions.contains(&AdjudicatorJurisdiction::General)
    }
}

/// Where an adjudicator can serve.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AdjudicatorJurisdiction {
    /// Can serve in disputes within a specific union.
    Union(Uuid),
    /// Can serve in disputes within a specific community.
    Community(Uuid),
    /// Can serve in disputes within a specific consortium.
    Consortium(Uuid),
    /// Can serve in any jurisdiction.
    General,
}

/// A qualification or training credential.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Qualification {
    pub id: Uuid,
    pub qualification_type: QualificationType,
    pub granted_by: Option<String>,
    pub obtained_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub description: Option<String>,
}

impl Qualification {
    /// Whether this qualification is still valid (not expired).
    pub fn is_current(&self) -> bool {
        self.expires_at
            .is_none_or(|exp| Utc::now() <= exp)
    }
}

/// Types of adjudicator qualification.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum QualificationType {
    /// Completed basic adjudication training.
    BasicTraining,
    /// Completed advanced adjudication training.
    AdvancedTraining,
    /// Certified as a mediator.
    MediationCertified,
    /// Certified as an arbitrator.
    ArbitrationCertified,
    /// Trained in restorative justice practices.
    RestorativeJustice,
    /// Recognized community elder with lived experience.
    CommunityElder,
    /// Recommended by peers in the community.
    PeerRecommended,
}

/// Adjudicator's track record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdjudicatorRecord {
    pub cases_handled: u32,
    pub satisfactory_resolutions: u32,
    pub cases_appealed: u32,
    pub appeals_upheld: u32,
    pub average_resolution_days: f64,
    pub feedback_score: Option<f64>,
    pub feedback_count: u32,
}

impl AdjudicatorRecord {
    pub fn new() -> Self {
        Self {
            cases_handled: 0,
            satisfactory_resolutions: 0,
            cases_appealed: 0,
            appeals_upheld: 0,
            average_resolution_days: 0.0,
            feedback_score: None,
            feedback_count: 0,
        }
    }

    /// Fraction of cases that were resolved satisfactorily.
    pub fn satisfaction_rate(&self) -> f64 {
        if self.cases_handled == 0 {
            return 0.0;
        }
        f64::from(self.satisfactory_resolutions) / f64::from(self.cases_handled)
    }

    /// Fraction of cases that were appealed.
    pub fn appeal_rate(&self) -> f64 {
        if self.cases_handled == 0 {
            return 0.0;
        }
        f64::from(self.cases_appealed) / f64::from(self.cases_handled)
    }

    /// Record a completed case, updating satisfaction rate and average resolution time.
    pub fn record_case(&mut self, satisfactory: bool, resolution_days: f64) {
        let total_days = self.average_resolution_days * f64::from(self.cases_handled) + resolution_days;
        self.cases_handled += 1;
        if satisfactory {
            self.satisfactory_resolutions += 1;
        }
        self.average_resolution_days = total_days / f64::from(self.cases_handled);
    }

    /// Record feedback from a party, updating the running average score.
    pub fn record_feedback(&mut self, score: f64) {
        let total = self.feedback_score.unwrap_or(0.0) * f64::from(self.feedback_count) + score;
        self.feedback_count += 1;
        self.feedback_score = Some(total / f64::from(self.feedback_count));
    }
}

impl Default for AdjudicatorRecord {
    fn default() -> Self {
        Self::new()
    }
}

/// Adjudicator availability.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AdjudicatorAvailability {
    /// Ready to accept new cases.
    Available,
    /// Handling maximum concurrent cases.
    AtCapacity,
    /// Briefly unavailable (conflict, short absence).
    TemporarilyUnavailable,
    /// Extended leave from adjudication duties.
    OnLeave,
}

/// Adjudicator status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AdjudicatorStatus {
    /// Eligible and active as an adjudicator.
    Active,
    /// Temporarily suspended from adjudication.
    Suspended,
    /// Certification revoked permanently.
    Revoked,
    /// Voluntarily stepped down from adjudication.
    Retired,
}

/// Assignment of an adjudicator to a dispute.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdjudicatorAssignment {
    pub id: Uuid,
    pub dispute_id: Uuid,
    pub adjudicator_id: Uuid,
    pub role: AdjudicatorRole,
    pub assigned_at: DateTime<Utc>,
    pub accepted: Option<bool>,
    pub responded_at: Option<DateTime<Utc>>,
    pub decline_reason: Option<String>,
}

impl AdjudicatorAssignment {
    pub fn new(dispute_id: Uuid, adjudicator_id: Uuid, role: AdjudicatorRole) -> Self {
        Self {
            id: Uuid::new_v4(),
            dispute_id,
            adjudicator_id,
            role,
            assigned_at: Utc::now(),
            accepted: None,
            responded_at: None,
            decline_reason: None,
        }
    }

    /// Accept the assignment to adjudicate this dispute.
    pub fn accept(&mut self) {
        self.accepted = Some(true);
        self.responded_at = Some(Utc::now());
    }

    /// Decline the assignment with an explanation.
    pub fn decline(&mut self, reason: impl Into<String>) {
        self.accepted = Some(false);
        self.decline_reason = Some(reason.into());
        self.responded_at = Some(Utc::now());
    }
}

/// Role an adjudicator plays in a dispute.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AdjudicatorRole {
    /// Lead adjudicator responsible for the case.
    Primary,
    /// Member of a multi-person adjudication panel.
    PanelMember,
    /// Serves as a neutral mediator between parties.
    Mediator,
    /// Provides expert knowledge on a specific topic.
    Expert,
    /// Observes the process without deciding authority.
    Observer,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjudicator_creation_and_availability() {
        let adj = Adjudicator::new("alice");
        assert!(adj.is_available());
        assert!(adj.can_handle(&DisputeType::Harm)); // no specializations = can handle all
        assert!(adj.has_jurisdiction(&AdjudicatorJurisdiction::General)); // empty = all
    }

    #[test]
    fn adjudicator_record_tracking() {
        let mut record = AdjudicatorRecord::new();
        record.record_case(true, 10.0);
        record.record_case(true, 20.0);
        record.record_case(false, 30.0);

        assert_eq!(record.cases_handled, 3);
        assert_eq!(record.satisfactory_resolutions, 2);
        assert!((record.satisfaction_rate() - 0.6667).abs() < 0.01);
        assert_eq!(record.average_resolution_days, 20.0);
    }

    #[test]
    fn adjudicator_feedback() {
        let mut record = AdjudicatorRecord::new();
        record.record_feedback(4.0);
        record.record_feedback(5.0);
        assert_eq!(record.feedback_count, 2);
        assert!((record.feedback_score.unwrap() - 4.5).abs() < 0.01);
    }

    #[test]
    fn assignment_accept_decline() {
        let mut a = AdjudicatorAssignment::new(Uuid::new_v4(), Uuid::new_v4(), AdjudicatorRole::Primary);
        assert!(a.accepted.is_none());

        a.accept();
        assert_eq!(a.accepted, Some(true));

        let mut b = AdjudicatorAssignment::new(Uuid::new_v4(), Uuid::new_v4(), AdjudicatorRole::Mediator);
        b.decline("Conflict of interest");
        assert_eq!(b.accepted, Some(false));
        assert!(b.decline_reason.is_some());
    }
}
