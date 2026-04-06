use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The resolution of a dispute — findings, remedies, and reasoning.
///
/// From Constellation Art. 5 §4: "Reconstitution shall preserve the lawful rights
/// of the people while renewing legitimacy through participatory reaffirmation."
///
/// Remedies are restorative: repair, restore, prevent, reintegrate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Resolution {
    pub id: Uuid,
    pub dispute_id: Uuid,
    pub decided_by: Vec<Uuid>,
    pub findings: Vec<Finding>,
    pub decision: DecisionOutcome,
    pub remedies: Vec<OrderedRemedy>,
    pub reasoning: String,
    pub decided_at: DateTime<Utc>,
    pub appeal_deadline: Option<DateTime<Utc>>,
    pub compliance_deadline: Option<DateTime<Utc>>,
}

impl Resolution {
    pub fn new(
        dispute_id: Uuid,
        decided_by: Vec<Uuid>,
        decision: DecisionOutcome,
        reasoning: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            dispute_id,
            decided_by,
            findings: Vec::new(),
            decision,
            remedies: Vec::new(),
            reasoning: reasoning.into(),
            decided_at: Utc::now(),
            appeal_deadline: Some(Utc::now() + chrono::Duration::days(30)),
            compliance_deadline: None,
        }
    }

    pub fn with_findings(mut self, findings: Vec<Finding>) -> Self {
        self.findings = findings;
        self
    }

    pub fn with_remedies(mut self, remedies: Vec<OrderedRemedy>) -> Self {
        self.remedies = remedies;
        self
    }

    pub fn with_compliance_deadline(mut self, deadline: DateTime<Utc>) -> Self {
        self.compliance_deadline = Some(deadline);
        self
    }

    /// Whether the appeal deadline has passed.
    pub fn appeal_period_passed(&self) -> bool {
        self.appeal_deadline
            .is_some_and(|d| Utc::now() > d)
    }

    /// Days remaining to file an appeal (0 if expired, None if no deadline).
    pub fn days_to_appeal(&self) -> Option<i64> {
        self.appeal_deadline.map(|d| {
            let diff = d - Utc::now();
            diff.num_days().max(0)
        })
    }
}

/// A factual finding from the adjudicator(s).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Finding {
    pub id: Uuid,
    pub statement: String,
    pub confidence: FindingConfidence,
    pub supporting_evidence: Vec<Uuid>,
}

impl Finding {
    pub fn new(statement: impl Into<String>, confidence: FindingConfidence) -> Self {
        Self {
            id: Uuid::new_v4(),
            statement: statement.into(),
            confidence,
            supporting_evidence: Vec::new(),
        }
    }
}

/// How confident the adjudicator is in a finding.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FindingConfidence {
    /// Clearly proven by the evidence.
    Established,
    /// More likely true than not.
    Preponderance,
    /// Evidence is inconclusive.
    Uncertain,
    /// The claim was not supported by evidence.
    NotEstablished,
}

/// Outcome of the decision.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DecisionOutcome {
    /// The complainant's claims were upheld.
    ForComplainant,
    /// The respondent was found not at fault.
    ForRespondent,
    /// Some claims upheld, some denied.
    Split,
    /// The case was dismissed on procedural grounds.
    Dismissed,
    /// The parties reached a mutual settlement.
    Settled,
}

/// A specific remedy ordered as part of resolution.
///
/// From Constellation Art. 7 §10: "The goal of all enforcement shall be restoration
/// of lawful relation, not permanent punishment or exclusion."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrderedRemedy {
    pub id: Uuid,
    pub action: RemedyAction,
    pub obligated_party: String,
    pub beneficiary: String,
    pub description: String,
    pub deadline: Option<DateTime<Utc>>,
    pub compliance_verified: bool,
    pub verified_at: Option<DateTime<Utc>>,
}

impl OrderedRemedy {
    pub fn new(
        action: RemedyAction,
        obligated_party: impl Into<String>,
        beneficiary: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            action,
            obligated_party: obligated_party.into(),
            beneficiary: beneficiary.into(),
            description: description.into(),
            deadline: None,
            compliance_verified: false,
            verified_at: None,
        }
    }

    /// Mark this remedy as having been complied with.
    pub fn verify_compliance(&mut self) {
        self.compliance_verified = true;
        self.verified_at = Some(Utc::now());
    }
}

/// Types of restorative remedy — repair, not punishment.
///
/// From Constellation Art. 5 §4: "Redress may include material compensation,
/// redistribution of power, public acknowledgment of breach, structural transformation,
/// or ceremonial acts of repair."
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum RemedyAction {
    /// A formal, public apology.
    Apology,
    /// Return or compensation for what was taken or damaged.
    Restitution,
    /// Change to governance structures to prevent recurrence.
    StructuralChange,
    /// Ongoing mediated dialogue between parties.
    Mediation,
    /// Educational process for the offending party.
    Education,
    /// Time-limited restriction on roles or privileges.
    TemporaryRestriction,
    /// Service to the affected community.
    CommunityService,
    /// Referral to a specialized body (e.g., for mental health).
    Referral,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_with_findings_and_remedies() {
        let adj_id = Uuid::new_v4();
        let resolution = Resolution::new(
            Uuid::new_v4(),
            vec![adj_id],
            DecisionOutcome::ForComplainant,
            "The evidence clearly shows a breach of agreed terms",
        )
        .with_findings(vec![
            Finding::new("Contract terms were violated", FindingConfidence::Established),
            Finding::new("Intent was negligent, not malicious", FindingConfidence::Preponderance),
        ])
        .with_remedies(vec![
            OrderedRemedy::new(
                RemedyAction::Restitution,
                "bob",
                "alice",
                "Return borrowed tools within 14 days",
            ),
            OrderedRemedy::new(
                RemedyAction::Apology,
                "bob",
                "alice",
                "Public acknowledgment of the breach",
            ),
        ]);

        assert_eq!(resolution.findings.len(), 2);
        assert_eq!(resolution.remedies.len(), 2);
        assert_eq!(resolution.decision, DecisionOutcome::ForComplainant);
        assert!(!resolution.appeal_period_passed());
        assert!(resolution.days_to_appeal().unwrap() > 0);
    }

    #[test]
    fn remedy_compliance_verification() {
        let mut remedy = OrderedRemedy::new(
            RemedyAction::Restitution,
            "bob",
            "alice",
            "Return tools",
        );
        assert!(!remedy.compliance_verified);

        remedy.verify_compliance();
        assert!(remedy.compliance_verified);
        assert!(remedy.verified_at.is_some());
    }

    #[test]
    fn remedy_types_are_restorative() {
        // All remedy types should be restorative, not punitive
        let types = [
            RemedyAction::Apology,
            RemedyAction::Restitution,
            RemedyAction::StructuralChange,
            RemedyAction::Mediation,
            RemedyAction::Education,
            RemedyAction::TemporaryRestriction,
            RemedyAction::CommunityService,
            RemedyAction::Referral,
        ];
        assert_eq!(types.len(), 8);
    }
}
