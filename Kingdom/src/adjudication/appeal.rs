use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An appeal of a dispute resolution.
///
/// From Constellation Art. 7 §9: "Breach declarations and enforcement actions
/// shall be subject to appeal through intercommunity assemblies and review processes."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Appeal {
    pub id: Uuid,
    pub resolution_id: Uuid,
    pub dispute_id: Uuid,
    pub appellant: String,
    pub grounds: Vec<AppealGround>,
    pub argument: String,
    pub status: AppealStatus,
    pub outcome: Option<AppealOutcome>,
    pub filed_at: DateTime<Utc>,
}

impl Appeal {
    pub fn new(
        resolution_id: Uuid,
        dispute_id: Uuid,
        appellant: impl Into<String>,
        grounds: Vec<AppealGround>,
        argument: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            resolution_id,
            dispute_id,
            appellant: appellant.into(),
            grounds,
            argument: argument.into(),
            status: AppealStatus::Filed,
            outcome: None,
            filed_at: Utc::now(),
        }
    }

    /// Accept the appeal and begin review. Only valid from Filed status.
    pub fn begin_review(&mut self) -> Result<(), crate::KingdomError> {
        if self.status != AppealStatus::Filed {
            return Err(crate::KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "UnderReview".into(),
            });
        }
        self.status = AppealStatus::UnderReview;
        Ok(())
    }

    /// Render a decision on the appeal. Only valid from UnderReview status.
    pub fn decide(&mut self, outcome: AppealOutcome) -> Result<(), crate::KingdomError> {
        if self.status != AppealStatus::UnderReview {
            return Err(crate::KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Decided".into(),
            });
        }
        self.status = AppealStatus::Decided;
        self.outcome = Some(outcome);
        Ok(())
    }

    /// Dismiss the appeal without a decision.
    pub fn dismiss(&mut self) {
        self.status = AppealStatus::Dismissed;
    }

    /// Withdraw the appeal (appellant's choice).
    pub fn withdraw(&mut self) {
        self.status = AppealStatus::Withdrawn;
    }

    /// Whether the appeal is still in progress (Filed or UnderReview).
    pub fn is_pending(&self) -> bool {
        matches!(self.status, AppealStatus::Filed | AppealStatus::UnderReview)
    }
}

/// Grounds for appeal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AppealGround {
    /// The process was not followed correctly.
    ProceduralError,
    /// Evidence was improperly excluded or admitted.
    EvidenceError,
    /// The factual findings were clearly wrong.
    FactualError,
    /// The law or charter was misapplied.
    ApplicationError,
    /// The adjudicator(s) were biased.
    Bias,
    /// New evidence has emerged since the decision.
    NewEvidence,
    /// The remedy was disproportionate or inappropriate.
    RemedyError,
}

/// Lifecycle of an appeal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AppealStatus {
    /// Appeal has been filed but not yet reviewed.
    Filed,
    /// Appeal is being reviewed by the appeal body.
    UnderReview,
    /// A decision has been rendered on the appeal.
    Decided,
    /// Appeal was dismissed (no valid grounds).
    Dismissed,
    /// Appellant withdrew the appeal.
    Withdrawn,
}

/// The result of an appeal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppealOutcome {
    pub decision: AppealDecision,
    pub reasoning: String,
    pub decided_at: DateTime<Utc>,
    pub decided_by: Vec<Uuid>,
}

impl AppealOutcome {
    pub fn new(
        decision: AppealDecision,
        reasoning: impl Into<String>,
        decided_by: Vec<Uuid>,
    ) -> Self {
        Self {
            decision,
            reasoning: reasoning.into(),
            decided_at: Utc::now(),
            decided_by,
        }
    }
}

/// How an appeal is resolved.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AppealDecision {
    /// Original resolution stands.
    Affirmed,
    /// Original resolution overturned.
    Reversed,
    /// Resolution modified.
    Modified,
    /// Sent back for new hearing.
    Remanded,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appeal_lifecycle() {
        let mut appeal = Appeal::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "alice",
            vec![AppealGround::ProceduralError, AppealGround::Bias],
            "The adjudicator had a conflict of interest",
        );
        assert!(appeal.is_pending());

        appeal.begin_review().unwrap();
        assert_eq!(appeal.status, AppealStatus::UnderReview);

        let outcome = AppealOutcome::new(
            AppealDecision::Remanded,
            "Bias confirmed, new hearing required",
            vec![Uuid::new_v4()],
        );
        appeal.decide(outcome).unwrap();
        assert_eq!(appeal.status, AppealStatus::Decided);
        assert!(appeal.outcome.is_some());
        assert!(!appeal.is_pending());
    }

    #[test]
    fn appeal_dismiss_and_withdraw() {
        let mut a1 = Appeal::new(Uuid::new_v4(), Uuid::new_v4(), "x", vec![], "x");
        a1.dismiss();
        assert_eq!(a1.status, AppealStatus::Dismissed);

        let mut a2 = Appeal::new(Uuid::new_v4(), Uuid::new_v4(), "x", vec![], "x");
        a2.withdraw();
        assert_eq!(a2.status, AppealStatus::Withdrawn);
    }

    #[test]
    fn cannot_decide_without_review() {
        let mut appeal = Appeal::new(Uuid::new_v4(), Uuid::new_v4(), "x", vec![], "x");
        let outcome = AppealOutcome::new(AppealDecision::Affirmed, "x", vec![]);
        assert!(appeal.decide(outcome).is_err());
    }
}
