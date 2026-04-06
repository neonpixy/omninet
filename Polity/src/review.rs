use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::breach::{Breach, BreachSeverity, ViolationType};
use crate::protections::{ActionDescription, ProhibitionType, ProtectionsRegistry};
use crate::rights::{RightCategory, RightsRegistry};

/// A constitutional review — checks whether an action complies with the Covenant.
///
/// This is Polity's active function: every action can be passed through review
/// to determine whether it honors Dignity, Sovereignty, and Consent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstitutionalReview {
    pub id: Uuid,
    pub action_description: String,
    pub actor: String,
    pub result: ReviewResult,
    pub reviewed_at: DateTime<Utc>,
    /// How many rights were in the registry at the time of review.
    pub rights_checked: usize,
    /// How many protections were in the registry at the time of review.
    pub protections_checked: usize,
}

/// The outcome of a constitutional review.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReviewResult {
    /// Action is permitted under the Covenant
    Permitted,
    /// Action violates one or more provisions
    Breach(Vec<ReviewViolation>),
    /// Action requires consent that hasn't been obtained
    NeedsConsent(Vec<ConsentRequirement>),
}

impl ReviewResult {
    /// Whether the reviewed action is permitted under the Covenant.
    pub fn is_permitted(&self) -> bool {
        matches!(self, ReviewResult::Permitted)
    }

    /// Whether the reviewed action constitutes a breach.
    pub fn is_breach(&self) -> bool {
        matches!(self, ReviewResult::Breach(_))
    }

    /// The specific violations found, or an empty slice if the action was permitted.
    pub fn violations(&self) -> &[ReviewViolation] {
        match self {
            ReviewResult::Breach(v) => v,
            _ => &[],
        }
    }

    /// Consent requirements that must be satisfied before the action can proceed.
    pub fn consent_requirements(&self) -> &[ConsentRequirement] {
        match self {
            ReviewResult::NeedsConsent(c) => c,
            _ => &[],
        }
    }
}

/// A specific violation found during review.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewViolation {
    pub right_category: Option<RightCategory>,
    pub prohibition_type: Option<ProhibitionType>,
    pub description: String,
    pub severity: BreachSeverity,
}

/// A consent requirement identified during review — who needs to consent, for what, and why.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsentRequirement {
    /// Who must grant consent.
    pub from: String,
    /// What scope of consent is needed.
    pub scope: String,
    /// Why this consent is required.
    pub reason: String,
}

/// Performs constitutional reviews against the Covenant's registries.
pub struct ConstitutionalReviewer<'a> {
    rights: &'a RightsRegistry,
    protections: &'a ProtectionsRegistry,
}

impl<'a> ConstitutionalReviewer<'a> {
    /// Create a reviewer backed by the given rights and protections registries.
    pub fn new(rights: &'a RightsRegistry, protections: &'a ProtectionsRegistry) -> Self {
        Self {
            rights,
            protections,
        }
    }

    /// Review an action against the Covenant.
    pub fn review(&self, action: &ActionDescription) -> ConstitutionalReview {
        let mut violations = Vec::new();

        // Check protections
        let violated = self.protections.check_violation(action);
        for protection in &violated {
            violations.push(ReviewViolation {
                right_category: None,
                prohibition_type: Some(protection.prohibition_type),
                description: format!(
                    "Violates {}: {}",
                    protection.name, protection.description
                ),
                severity: if protection.is_absolute {
                    BreachSeverity::Grave
                } else {
                    BreachSeverity::Significant
                },
            });
        }

        let result = if violations.is_empty() {
            ReviewResult::Permitted
        } else {
            ReviewResult::Breach(violations)
        };

        ConstitutionalReview {
            id: Uuid::new_v4(),
            action_description: action.description.clone(),
            actor: action.actor.clone(),
            result,
            reviewed_at: Utc::now(),
            rights_checked: self.rights.len(),
            protections_checked: self.protections.len(),
        }
    }

    /// Quick check: does an action violate any absolute prohibition?
    pub fn is_absolutely_prohibited(&self, action: &ActionDescription) -> bool {
        let violated = self.protections.check_violation(action);
        violated.iter().any(|p| p.is_absolute)
    }

    /// Convert a review breach into a formal Breach record.
    pub fn to_breach(&self, review: &ConstitutionalReview) -> Option<Breach> {
        match &review.result {
            ReviewResult::Breach(violations) => {
                let max_severity = violations
                    .iter()
                    .map(|v| v.severity)
                    .max()
                    .unwrap_or(BreachSeverity::Minor);

                let affected_rights: Vec<_> = violations
                    .iter()
                    .filter_map(|v| v.right_category)
                    .collect();

                let violated_prohibitions: Vec<_> = violations
                    .iter()
                    .filter_map(|v| v.prohibition_type)
                    .collect();

                Some(
                    Breach::new(
                        ViolationType::ProtectionBreach,
                        max_severity,
                        &review.action_description,
                        &review.actor,
                    )
                    .with_rights(affected_rights)
                    .with_prohibitions(violated_prohibitions),
                )
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protections::ProtectionsRegistry;
    use crate::rights::RightsRegistry;

    fn make_reviewer() -> (RightsRegistry, ProtectionsRegistry) {
        (RightsRegistry::default(), ProtectionsRegistry::default())
    }

    #[test]
    fn clean_action_passes_review() {
        let (rights, protections) = make_reviewer();
        let reviewer = ConstitutionalReviewer::new(&rights, &protections);

        let action = ActionDescription {
            description: "Community votes on shared garden layout".into(),
            actor: "garden_collective".into(),
            violates: vec![],
        };

        let review = reviewer.review(&action);
        assert!(review.result.is_permitted());
        assert_eq!(review.rights_checked, 12);
        assert_eq!(review.protections_checked, 8);
    }

    #[test]
    fn surveillance_action_fails_review() {
        let (rights, protections) = make_reviewer();
        let reviewer = ConstitutionalReviewer::new(&rights, &protections);

        let action = ActionDescription {
            description: "Track user browsing without consent".into(),
            actor: "ad_platform".into(),
            violates: vec![ProhibitionType::Surveillance],
        };

        let review = reviewer.review(&action);
        assert!(review.result.is_breach());
        assert_eq!(review.result.violations().len(), 1);
        assert_eq!(
            review.result.violations()[0].prohibition_type,
            Some(ProhibitionType::Surveillance)
        );
    }

    #[test]
    fn multiple_violations_detected() {
        let (rights, protections) = make_reviewer();
        let reviewer = ConstitutionalReviewer::new(&rights, &protections);

        let action = ActionDescription {
            description: "Forced labor camp with surveillance".into(),
            actor: "authoritarian_state".into(),
            violates: vec![
                ProhibitionType::Domination,
                ProhibitionType::Surveillance,
                ProhibitionType::Exploitation,
                ProhibitionType::Cruelty,
            ],
        };

        let review = reviewer.review(&action);
        assert!(review.result.is_breach());
        assert_eq!(review.result.violations().len(), 4);
    }

    #[test]
    fn is_absolutely_prohibited_check() {
        let (rights, protections) = make_reviewer();
        let reviewer = ConstitutionalReviewer::new(&rights, &protections);

        let action = ActionDescription {
            description: "Torture for information".into(),
            actor: "security_agency".into(),
            violates: vec![ProhibitionType::Cruelty],
        };

        assert!(reviewer.is_absolutely_prohibited(&action));
    }

    #[test]
    fn review_to_breach_conversion() {
        let (rights, protections) = make_reviewer();
        let reviewer = ConstitutionalReviewer::new(&rights, &protections);

        let action = ActionDescription {
            description: "Enclose commons for private profit".into(),
            actor: "corporation".into(),
            violates: vec![ProhibitionType::Exploitation],
        };

        let review = reviewer.review(&action);
        let breach = reviewer.to_breach(&review).unwrap();
        assert_eq!(breach.actor, "corporation");
        assert!(breach.violated_prohibitions.contains(&ProhibitionType::Exploitation));
    }

    #[test]
    fn clean_review_produces_no_breach() {
        let (rights, protections) = make_reviewer();
        let reviewer = ConstitutionalReviewer::new(&rights, &protections);

        let action = ActionDescription {
            description: "Plant a tree".into(),
            actor: "gardener".into(),
            violates: vec![],
        };

        let review = reviewer.review(&action);
        assert!(reviewer.to_breach(&review).is_none());
    }

    #[test]
    fn review_serialization_roundtrip() {
        let (rights, protections) = make_reviewer();
        let reviewer = ConstitutionalReviewer::new(&rights, &protections);

        let action = ActionDescription {
            description: "Algorithmic discrimination".into(),
            actor: "ai_platform".into(),
            violates: vec![ProhibitionType::Discrimination],
        };

        let review = reviewer.review(&action);
        let json = serde_json::to_string(&review).unwrap();
        let restored: ConstitutionalReview = serde_json::from_str(&json).unwrap();
        assert_eq!(review.action_description, restored.action_description);
        assert_eq!(review.result.is_breach(), restored.result.is_breach());
    }
}
