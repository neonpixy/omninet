//! Appeals — challenging accountability decisions.
//!
//! From Constellation Art. 7 §9: "Breach declarations and enforcement actions
//! shall be subject to appeal through intercommunity assemblies and review processes."
//!
//! Every person has the right to appeal (AccusedRights). Appeals can result in
//! the original decision being upheld, reversed, modified, or remanded for new review.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::JailError;

/// An appeal of an accountability decision.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Appeal {
    /// Unique appeal identifier.
    pub id: Uuid,
    /// The flag being appealed.
    pub flag_id: Uuid,
    /// Who is filing the appeal.
    pub appellant_pubkey: String,
    /// Grounds for the appeal.
    pub grounds: AppealGround,
    /// The appellant's statement.
    pub statement: String,
    /// Current status.
    pub status: AppealStatus,
    /// Outcome (once decided).
    pub outcome: Option<AppealOutcome>,
    /// When the appeal was filed.
    pub filed_at: DateTime<Utc>,
}

/// Grounds for filing an appeal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AppealGround {
    /// New evidence has come to light.
    NewEvidence,
    /// The original process had procedural errors.
    ProceduralError,
    /// The response was disproportionate to the offense.
    DisproportionateResponse,
    /// There are mitigating circumstances not previously considered.
    MitigatingCircumstances,
    /// The flag was filed in bad faith.
    BadFaithFlag,
    /// The context was misunderstood.
    ContextMisunderstood,
}

impl std::fmt::Display for AppealGround {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NewEvidence => write!(f, "new_evidence"),
            Self::ProceduralError => write!(f, "procedural_error"),
            Self::DisproportionateResponse => write!(f, "disproportionate_response"),
            Self::MitigatingCircumstances => write!(f, "mitigating_circumstances"),
            Self::BadFaithFlag => write!(f, "bad_faith_flag"),
            Self::ContextMisunderstood => write!(f, "context_misunderstood"),
        }
    }
}

/// Status of an appeal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AppealStatus {
    /// Appeal has been filed.
    Filed,
    /// Under active review.
    UnderReview,
    /// Decision has been made.
    Decided,
    /// Appellant withdrew the appeal.
    Withdrawn,
}

impl AppealStatus {
    /// Whether this status is final (Decided or Withdrawn) — no further transitions allowed.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Decided | Self::Withdrawn)
    }
}

impl std::fmt::Display for AppealStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Filed => write!(f, "filed"),
            Self::UnderReview => write!(f, "under_review"),
            Self::Decided => write!(f, "decided"),
            Self::Withdrawn => write!(f, "withdrawn"),
        }
    }
}

/// Outcome of a decided appeal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppealOutcome {
    /// The decision.
    pub decision: AppealDecision,
    /// Reasoning for the decision.
    pub reasoning: String,
    /// Who decided.
    pub decided_by: String,
    /// When decided.
    pub decided_at: DateTime<Utc>,
}

/// Possible appeal decisions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AppealDecision {
    /// Original decision stands.
    Upheld,
    /// Original decision reversed — flag dismissed.
    Reversed,
    /// Original decision modified — e.g., reduced severity.
    Modified,
    /// Sent back for a new review with new instructions.
    Remanded,
}

impl std::fmt::Display for AppealDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Upheld => write!(f, "upheld"),
            Self::Reversed => write!(f, "reversed"),
            Self::Modified => write!(f, "modified"),
            Self::Remanded => write!(f, "remanded"),
        }
    }
}

impl Appeal {
    /// File a new appeal.
    pub fn file(
        flag_id: Uuid,
        appellant: impl Into<String>,
        grounds: AppealGround,
        statement: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            flag_id,
            appellant_pubkey: appellant.into(),
            grounds,
            statement: statement.into(),
            status: AppealStatus::Filed,
            outcome: None,
            filed_at: Utc::now(),
        }
    }

    /// Move to under review.
    pub fn begin_review(&mut self) -> Result<(), JailError> {
        if self.status.is_terminal() {
            return Err(JailError::AppealAlreadyResolved(self.id.to_string()));
        }
        self.status = AppealStatus::UnderReview;
        Ok(())
    }

    /// Decide the appeal.
    pub fn decide(&mut self, outcome: AppealOutcome) -> Result<(), JailError> {
        if self.status.is_terminal() {
            return Err(JailError::AppealAlreadyResolved(self.id.to_string()));
        }
        self.status = AppealStatus::Decided;
        self.outcome = Some(outcome);
        Ok(())
    }

    /// Withdraw the appeal.
    pub fn withdraw(&mut self) -> Result<(), JailError> {
        if self.status.is_terminal() {
            return Err(JailError::AppealTerminalState(self.id.to_string()));
        }
        self.status = AppealStatus::Withdrawn;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_appeal() {
        let flag_id = Uuid::new_v4();
        let appeal = Appeal::file(flag_id, "bob", AppealGround::NewEvidence, "I have new proof");
        assert_eq!(appeal.flag_id, flag_id);
        assert_eq!(appeal.appellant_pubkey, "bob");
        assert_eq!(appeal.grounds, AppealGround::NewEvidence);
        assert_eq!(appeal.status, AppealStatus::Filed);
        assert!(appeal.outcome.is_none());
    }

    #[test]
    fn appeal_lifecycle_to_reversal() {
        let mut appeal = Appeal::file(
            Uuid::new_v4(),
            "bob",
            AppealGround::BadFaithFlag,
            "The flag was retaliatory",
        );

        appeal.begin_review().unwrap();
        assert_eq!(appeal.status, AppealStatus::UnderReview);

        appeal
            .decide(AppealOutcome {
                decision: AppealDecision::Reversed,
                reasoning: "Evidence shows bad faith".into(),
                decided_by: "reviewer".into(),
                decided_at: Utc::now(),
            })
            .unwrap();
        assert_eq!(appeal.status, AppealStatus::Decided);
        assert_eq!(
            appeal.outcome.as_ref().unwrap().decision,
            AppealDecision::Reversed
        );
    }

    #[test]
    fn appeal_withdrawal() {
        let mut appeal = Appeal::file(
            Uuid::new_v4(),
            "bob",
            AppealGround::ContextMisunderstood,
            "Actually, never mind",
        );
        appeal.withdraw().unwrap();
        assert_eq!(appeal.status, AppealStatus::Withdrawn);
    }

    #[test]
    fn cannot_decide_terminal_appeal() {
        let mut appeal = Appeal::file(Uuid::new_v4(), "bob", AppealGround::NewEvidence, "test");
        appeal.withdraw().unwrap();

        let result = appeal.decide(AppealOutcome {
            decision: AppealDecision::Upheld,
            reasoning: "test".into(),
            decided_by: "reviewer".into(),
            decided_at: Utc::now(),
        });
        assert!(result.is_err());
    }

    #[test]
    fn appeal_ground_display() {
        assert_eq!(AppealGround::NewEvidence.to_string(), "new_evidence");
        assert_eq!(AppealGround::BadFaithFlag.to_string(), "bad_faith_flag");
    }

    #[test]
    fn appeal_decision_display() {
        assert_eq!(AppealDecision::Reversed.to_string(), "reversed");
        assert_eq!(AppealDecision::Remanded.to_string(), "remanded");
    }

    #[test]
    fn terminal_states() {
        assert!(AppealStatus::Decided.is_terminal());
        assert!(AppealStatus::Withdrawn.is_terminal());
        assert!(!AppealStatus::Filed.is_terminal());
        assert!(!AppealStatus::UnderReview.is_terminal());
    }

    #[test]
    fn appeal_serialization_roundtrip() {
        let appeal = Appeal::file(Uuid::new_v4(), "bob", AppealGround::NewEvidence, "test");
        let json = serde_json::to_string(&appeal).unwrap();
        let deserialized: Appeal = serde_json::from_str(&json).unwrap();
        assert_eq!(appeal, deserialized);
    }
}
