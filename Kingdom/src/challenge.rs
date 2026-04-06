use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A public challenge to a governance action or structure.
///
/// From Constellation Art. 5 §1: "Every person and every community shall hold
/// the inalienable right to challenge any structure of governance that claims
/// authority over their lives."
///
/// From Constellation Art. 5 §6: "All challenges shall be brought in good faith
/// to address genuine breaches, not to harass, exhaust, or silence legitimate actors."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Challenge {
    pub id: Uuid,
    pub challenger: String,
    pub challenge_type: ChallengeType,
    pub target: ChallengeTarget,
    pub grounds: String,
    pub evidence: Vec<String>,
    pub co_signers: Vec<String>,
    pub status: ChallengeStatus,
    pub response: Option<ChallengeResponse>,
    pub filed_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

impl Challenge {
    pub fn new(
        challenger: impl Into<String>,
        challenge_type: ChallengeType,
        target: ChallengeTarget,
        grounds: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            challenger: challenger.into(),
            challenge_type,
            target,
            grounds: grounds.into(),
            evidence: Vec::new(),
            co_signers: Vec::new(),
            status: ChallengeStatus::Filed,
            response: None,
            filed_at: Utc::now(),
            resolved_at: None,
        }
    }

    pub fn with_evidence(mut self, evidence: Vec<String>) -> Self {
        self.evidence = evidence;
        self
    }

    /// Add a co-signer to strengthen the challenge. Duplicates and self-signing are ignored.
    pub fn add_co_signer(&mut self, pubkey: impl Into<String>) {
        let pubkey = pubkey.into();
        if !self.co_signers.contains(&pubkey) && pubkey != self.challenger {
            self.co_signers.push(pubkey);
        }
    }

    /// Submit a response to the challenge. Moves the challenge to UnderReview.
    pub fn respond(&mut self, response: ChallengeResponse) -> Result<(), crate::KingdomError> {
        if self.status != ChallengeStatus::Filed && self.status != ChallengeStatus::UnderReview {
            return Err(crate::KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Responded".into(),
            });
        }
        self.status = ChallengeStatus::UnderReview;
        self.response = Some(response);
        Ok(())
    }

    /// Uphold the challenge as valid. Only valid from UnderReview.
    pub fn uphold(&mut self) -> Result<(), crate::KingdomError> {
        if self.status != ChallengeStatus::UnderReview {
            return Err(crate::KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Upheld".into(),
            });
        }
        self.status = ChallengeStatus::Upheld;
        self.resolved_at = Some(Utc::now());
        Ok(())
    }

    /// Dismiss the challenge as without merit.
    pub fn dismiss(&mut self) -> Result<(), crate::KingdomError> {
        if self.status != ChallengeStatus::UnderReview && self.status != ChallengeStatus::Filed {
            return Err(crate::KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Dismissed".into(),
            });
        }
        self.status = ChallengeStatus::Dismissed;
        self.resolved_at = Some(Utc::now());
        Ok(())
    }

    /// Whether the challenge is still in progress (Filed or UnderReview).
    pub fn is_active(&self) -> bool {
        matches!(
            self.status,
            ChallengeStatus::Filed | ChallengeStatus::UnderReview
        )
    }
}

/// What is being challenged.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChallengeTarget {
    /// A specific proposal or decision.
    Proposal(Uuid),
    /// A governance structure itself.
    GovernanceStructure(Uuid),
    /// A delegate's actions.
    Delegate(String),
    /// A community's charter compliance.
    CharterCompliance(Uuid),
    /// A consortium's operations.
    ConsortiumAction(Uuid),
}

/// Kind of challenge.
///
/// From Constellation Art. 5 §2: "A governance body shall be subject to challenge
/// where it enacts or enables harm, becomes unaccountable, refuses participation,
/// betrays the Core, violates the Commons, or sustains domination."
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ChallengeType {
    /// Governance body is unaccountable.
    Accountability,
    /// Decision violates the Covenant Core.
    CoreViolation,
    /// Decision violates the Commons.
    CommonsViolation,
    /// Process was unfair or exclusionary.
    ProceduralFairness,
    /// Power has become concentrated.
    PowerConcentration,
    /// Mandate exceeded by delegate.
    MandateExceeded,
    /// Charter terms violated.
    CharterViolation,
}

/// Lifecycle of a challenge.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ChallengeStatus {
    /// Challenge has been filed and is awaiting response.
    Filed,
    /// Challenge is being reviewed after a response was submitted.
    UnderReview,
    /// The challenge was found valid and upheld.
    Upheld,
    /// The challenge was found without merit.
    Dismissed,
    /// The challenger withdrew the challenge.
    Withdrawn,
}

/// A response to a challenge.
///
/// From Constellation Art. 5 §5: "No person or community shall be punished,
/// silenced, or retaliated against for initiating or supporting lawful challenge."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChallengeResponse {
    pub responder: String,
    pub position: ResponsePosition,
    pub narrative: String,
    pub evidence: Vec<String>,
    pub proposed_remedy: Option<String>,
    pub responded_at: DateTime<Utc>,
}

impl ChallengeResponse {
    pub fn new(
        responder: impl Into<String>,
        position: ResponsePosition,
        narrative: impl Into<String>,
    ) -> Self {
        Self {
            responder: responder.into(),
            position,
            narrative: narrative.into(),
            evidence: Vec::new(),
            proposed_remedy: None,
            responded_at: Utc::now(),
        }
    }
}

/// How the challenged party responds.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ResponsePosition {
    /// Acknowledges the issue.
    Acknowledge,
    /// Denies the claim.
    Deny,
    /// Partially accepts.
    Partial,
    /// Proposes structural change in response.
    ProposesReform,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_challenge() {
        let c = Challenge::new(
            "alice",
            ChallengeType::PowerConcentration,
            ChallengeTarget::GovernanceStructure(Uuid::new_v4()),
            "Steward has served 3 consecutive terms without rotation",
        )
        .with_evidence(vec!["Term records from 2024-2026".into()]);

        assert!(c.is_active());
        assert_eq!(c.status, ChallengeStatus::Filed);
        assert_eq!(c.evidence.len(), 1);
    }

    #[test]
    fn challenge_lifecycle() {
        let mut c = Challenge::new(
            "alice",
            ChallengeType::ProceduralFairness,
            ChallengeTarget::Proposal(Uuid::new_v4()),
            "Voting period was too short for meaningful participation",
        );

        c.add_co_signer("bob");
        c.add_co_signer("charlie");
        c.add_co_signer("alice"); // self-signing ignored
        assert_eq!(c.co_signers.len(), 2);

        let response = ChallengeResponse::new(
            "steward_diana",
            ResponsePosition::Acknowledge,
            "We agree the timeline was insufficient",
        );
        c.respond(response).unwrap();
        assert_eq!(c.status, ChallengeStatus::UnderReview);

        c.uphold().unwrap();
        assert_eq!(c.status, ChallengeStatus::Upheld);
        assert!(c.resolved_at.is_some());
    }

    #[test]
    fn dismiss_challenge() {
        let mut c = Challenge::new(
            "alice",
            ChallengeType::CoreViolation,
            ChallengeTarget::Delegate("bob".into()),
            "Unsubstantiated claim",
        );
        c.dismiss().unwrap();
        assert_eq!(c.status, ChallengeStatus::Dismissed);
        assert!(!c.is_active());
    }

    #[test]
    fn cannot_uphold_without_review() {
        let mut c = Challenge::new(
            "alice",
            ChallengeType::Accountability,
            ChallengeTarget::ConsortiumAction(Uuid::new_v4()),
            "No transparency reports published",
        );
        assert!(c.uphold().is_err());
    }

    #[test]
    fn challenge_types_cover_covenant_grounds() {
        // Constellation Art. 5 §2 lists: harm, unaccountable, refuses participation,
        // betrays Core, violates Commons, sustains domination
        let types = [
            ChallengeType::Accountability,
            ChallengeType::CoreViolation,
            ChallengeType::CommonsViolation,
            ChallengeType::ProceduralFairness,
            ChallengeType::PowerConcentration,
            ChallengeType::MandateExceeded,
            ChallengeType::CharterViolation,
        ];
        assert_eq!(types.len(), 7);
    }
}
