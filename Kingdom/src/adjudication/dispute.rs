use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A formal dispute between parties — restorative, not punitive.
///
/// From Constellation Art. 5 §3: "All governance bodies shall maintain open and
/// accessible procedures for receiving, reviewing, and responding to public challenge."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Dispute {
    pub id: Uuid,
    pub complainant: String,
    pub respondent: String,
    pub dispute_type: DisputeType,
    pub context: DisputeContext,
    pub description: String,
    pub evidence: Vec<EvidenceItem>,
    pub status: DisputeStatus,
    pub response: Option<DisputeResponse>,
    pub filed_at: DateTime<Utc>,
}

impl Dispute {
    pub fn new(
        complainant: impl Into<String>,
        respondent: impl Into<String>,
        dispute_type: DisputeType,
        context: DisputeContext,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            complainant: complainant.into(),
            respondent: respondent.into(),
            dispute_type,
            context,
            description: description.into(),
            evidence: Vec::new(),
            status: DisputeStatus::Filed,
            response: None,
            filed_at: Utc::now(),
        }
    }

    pub fn with_evidence(mut self, evidence: Vec<EvidenceItem>) -> Self {
        self.evidence = evidence;
        self
    }

    /// Record the respondent's answer. Only valid from ResponseRequired status.
    pub fn add_response(&mut self, response: DisputeResponse) -> Result<(), crate::KingdomError> {
        if self.status != DisputeStatus::ResponseRequired {
            return Err(crate::KingdomError::InvalidDisputeTransition {
                current: format!("{:?}", self.status),
                target: "ResponseRequired".into(),
            });
        }
        self.response = Some(response);
        self.status = DisputeStatus::UnderReview;
        Ok(())
    }

    /// Transition from Filed to ResponseRequired, asking the respondent to answer.
    pub fn require_response(&mut self) -> Result<(), crate::KingdomError> {
        if self.status != DisputeStatus::Filed {
            return Err(crate::KingdomError::InvalidDisputeTransition {
                current: format!("{:?}", self.status),
                target: "ResponseRequired".into(),
            });
        }
        self.status = DisputeStatus::ResponseRequired;
        Ok(())
    }

    /// Move the dispute from UnderReview to HearingScheduled.
    pub fn advance_to_hearing(&mut self) -> Result<(), crate::KingdomError> {
        if self.status != DisputeStatus::UnderReview {
            return Err(crate::KingdomError::InvalidDisputeTransition {
                current: format!("{:?}", self.status),
                target: "HearingScheduled".into(),
            });
        }
        self.status = DisputeStatus::HearingScheduled;
        Ok(())
    }

    /// Move the dispute to AwaitingResolution after hearing or review.
    pub fn advance_to_resolution(&mut self) -> Result<(), crate::KingdomError> {
        if !matches!(
            self.status,
            DisputeStatus::HearingScheduled | DisputeStatus::UnderReview
        ) {
            return Err(crate::KingdomError::InvalidDisputeTransition {
                current: format!("{:?}", self.status),
                target: "AwaitingResolution".into(),
            });
        }
        self.status = DisputeStatus::AwaitingResolution;
        Ok(())
    }

    /// Mark the dispute as resolved after a resolution has been rendered.
    pub fn resolve(&mut self) -> Result<(), crate::KingdomError> {
        if self.status != DisputeStatus::AwaitingResolution {
            return Err(crate::KingdomError::InvalidDisputeTransition {
                current: format!("{:?}", self.status),
                target: "Resolved".into(),
            });
        }
        self.status = DisputeStatus::Resolved;
        Ok(())
    }

    /// Dismiss the dispute without a hearing (no merit).
    pub fn dismiss(&mut self) {
        self.status = DisputeStatus::Dismissed;
    }

    /// Withdraw the dispute (complainant's choice).
    pub fn withdraw(&mut self) {
        self.status = DisputeStatus::Withdrawn;
    }

    /// Whether the dispute is still in progress (not resolved, dismissed, or withdrawn).
    pub fn is_active(&self) -> bool {
        !matches!(
            self.status,
            DisputeStatus::Resolved | DisputeStatus::Dismissed | DisputeStatus::Withdrawn
        )
    }
}

/// Category of dispute.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DisputeType {
    /// Failure to honor an agreement or compact.
    ContractBreach,
    /// Violation of community charter rules.
    RuleViolation,
    /// Physical, emotional, or digital harm.
    Harm,
    /// Dispute over resource allocation or access.
    Resource,
    /// Dispute about governance decisions or processes.
    Governance,
    /// Conflict between individuals.
    Interpersonal,
    /// Disputes not fitting other categories.
    Other,
}

/// Where the dispute occurs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DisputeContext {
    /// Dispute within a personal union.
    Union(Uuid),
    /// Dispute within a single community.
    Community(Uuid),
    /// Dispute within a consortium (federation).
    Consortium(Uuid),
    /// Dispute between communities in the same consortium.
    InterCommunity { consortium_id: Uuid },
    /// Dispute between individuals outside any particular body.
    Interpersonal,
}

/// Lifecycle of a dispute.
///
/// State machine: Filed → ResponseRequired → UnderReview → HearingScheduled
/// → AwaitingResolution → Resolved (or Dismissed/Withdrawn at any point)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DisputeStatus {
    /// The dispute has been filed.
    Filed,
    /// The respondent has been asked to reply.
    ResponseRequired,
    /// Both sides have been heard; adjudicator is reviewing.
    UnderReview,
    /// A formal hearing has been scheduled.
    HearingScheduled,
    /// Hearing complete; awaiting the adjudicator's decision.
    AwaitingResolution,
    /// A resolution has been rendered.
    Resolved,
    /// The dispute was dismissed (no merit).
    Dismissed,
    /// The complainant withdrew the dispute.
    Withdrawn,
}

/// A respondent's answer to a dispute.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DisputeResponse {
    pub respondent: String,
    pub position: ResponsePosition,
    pub narrative: String,
    pub evidence: Vec<EvidenceItem>,
    pub counterclaims: Vec<Counterclaim>,
    pub responded_at: DateTime<Utc>,
}

impl DisputeResponse {
    pub fn new(
        respondent: impl Into<String>,
        position: ResponsePosition,
        narrative: impl Into<String>,
    ) -> Self {
        Self {
            respondent: respondent.into(),
            position,
            narrative: narrative.into(),
            evidence: Vec::new(),
            counterclaims: Vec::new(),
            responded_at: Utc::now(),
        }
    }
}

/// How the respondent answers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ResponsePosition {
    /// Admits the claim in full.
    Admit,
    /// Denies the claim entirely.
    Deny,
    /// Admits some aspects, denies others.
    Partial,
    /// Cannot respond due to lack of information.
    InsufficientInfo,
    /// Files a counterclaim against the complainant.
    Counterclaim,
}

/// A counter-complaint within a dispute.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Counterclaim {
    pub id: Uuid,
    pub description: String,
    pub evidence: Vec<EvidenceItem>,
}

/// A piece of evidence submitted in a dispute.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceItem {
    pub id: Uuid,
    pub evidence_type: EvidenceType,
    pub description: String,
    pub reference: Option<String>,
}

impl EvidenceItem {
    pub fn new(
        evidence_type: EvidenceType,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            evidence_type,
            description: description.into(),
            reference: None,
        }
    }
}

/// Type of evidence.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EvidenceType {
    /// A written document or file.
    Document,
    /// A first-person account.
    Testimony,
    /// A transaction record from Fortune or Vault.
    Transaction,
    /// A message or communication log.
    Communication,
    /// A third-party witness statement.
    Witness,
    /// Evidence not fitting other categories.
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispute_lifecycle() {
        let mut d = Dispute::new(
            "alice",
            "bob",
            DisputeType::ContractBreach,
            DisputeContext::Community(Uuid::new_v4()),
            "Failed to deliver promised goods",
        );
        assert!(d.is_active());
        assert_eq!(d.status, DisputeStatus::Filed);

        d.require_response().unwrap();
        assert_eq!(d.status, DisputeStatus::ResponseRequired);

        let response = DisputeResponse::new("bob", ResponsePosition::Partial, "Delayed, not refused");
        d.add_response(response).unwrap();
        assert_eq!(d.status, DisputeStatus::UnderReview);

        d.advance_to_hearing().unwrap();
        assert_eq!(d.status, DisputeStatus::HearingScheduled);

        d.advance_to_resolution().unwrap();
        assert_eq!(d.status, DisputeStatus::AwaitingResolution);

        d.resolve().unwrap();
        assert_eq!(d.status, DisputeStatus::Resolved);
        assert!(!d.is_active());
    }

    #[test]
    fn invalid_dispute_transitions() {
        let mut d = Dispute::new("a", "b", DisputeType::Harm, DisputeContext::Interpersonal, "x");
        // Can't go to hearing from Filed
        assert!(d.advance_to_hearing().is_err());
        // Can't resolve from Filed
        assert!(d.resolve().is_err());
    }

    #[test]
    fn dismiss_and_withdraw() {
        let mut d1 = Dispute::new("a", "b", DisputeType::Other, DisputeContext::Interpersonal, "x");
        d1.dismiss();
        assert!(!d1.is_active());

        let mut d2 = Dispute::new("a", "b", DisputeType::Other, DisputeContext::Interpersonal, "x");
        d2.withdraw();
        assert!(!d2.is_active());
    }
}
