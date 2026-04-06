use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::immutable::ImmutableFoundation;

/// A proposed change to the Covenant — threshold-triggered, not periodic.
///
/// From Covenant Continuum Art. 4: "This Covenant shall be held as a complete and
/// standing body of law. The presumption of the Continuum shall be noninterference.
/// No amendment shall occur unless specific conditions are met."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Amendment {
    pub id: Uuid,
    pub trigger: AmendmentTrigger,
    pub title: String,
    pub description: String,
    pub proposed_changes: Vec<ProposedChange>,
    pub status: AmendmentStatus,
    pub proposer: String,
    /// The approval ratio needed (0.60 to 0.75, based on trigger type).
    pub required_threshold: f64,
    /// The current approval ratio among eligible participants.
    pub current_support: f64,
    pub supporters: Vec<String>,
    pub objectors: Vec<String>,
    pub proposed_at: DateTime<Utc>,
    pub enacted_at: Option<DateTime<Utc>>,
    /// Freeform key-value metadata about the amendment.
    pub context: HashMap<String, String>,
}

impl Amendment {
    /// Create a new amendment proposal. The description is checked against immutable foundations
    /// at creation time -- amendments that would weaken the Core or Commons are rejected immediately.
    pub fn new(
        trigger: AmendmentTrigger,
        title: impl Into<String>,
        description: impl Into<String>,
        proposer: impl Into<String>,
    ) -> Result<Self, crate::PolityError> {
        let title = title.into();
        let description = description.into();

        // Check against immutable foundations before even creating
        if ImmutableFoundation::would_violate(&description) {
            return Err(crate::PolityError::AmendmentContradictsFoundation(
                title.clone(),
            ));
        }

        Ok(Self {
            id: Uuid::new_v4(),
            trigger,
            title,
            description,
            proposed_changes: Vec::new(),
            status: AmendmentStatus::Proposed,
            proposer: proposer.into(),
            required_threshold: trigger.default_threshold(),
            current_support: 0.0,
            supporters: Vec::new(),
            objectors: Vec::new(),
            proposed_at: Utc::now(),
            enacted_at: None,
            context: HashMap::new(),
        })
    }

    /// Attach a specific proposed change to this amendment.
    pub fn with_change(mut self, change: ProposedChange) -> Self {
        self.proposed_changes.push(change);
        self
    }

    /// Override the default threshold for this amendment (clamped to 0.5..1.0).
    pub fn with_threshold(mut self, threshold: f64) -> Self {
        self.required_threshold = threshold.clamp(0.5, 1.0);
        self
    }

    /// Advance the amendment to deliberation phase.
    pub fn begin_deliberation(&mut self) -> Result<(), crate::PolityError> {
        if self.status != AmendmentStatus::Proposed {
            return Err(crate::PolityError::InvalidAmendmentTransition {
                current: format!("{:?}", self.status),
                target: "Deliberating".into(),
            });
        }
        self.status = AmendmentStatus::Deliberating;
        Ok(())
    }

    /// Advance to ratification phase.
    pub fn begin_ratification(&mut self) -> Result<(), crate::PolityError> {
        if self.status != AmendmentStatus::Deliberating {
            return Err(crate::PolityError::InvalidAmendmentTransition {
                current: format!("{:?}", self.status),
                target: "Ratifying".into(),
            });
        }
        self.status = AmendmentStatus::Ratifying;
        Ok(())
    }

    /// Record support from a participant.
    pub fn add_support(&mut self, supporter: impl Into<String>) {
        let s = supporter.into();
        if !self.supporters.contains(&s) {
            self.supporters.push(s);
        }
    }

    /// Record objection from a participant.
    pub fn add_objection(&mut self, objector: impl Into<String>) {
        let o = objector.into();
        if !self.objectors.contains(&o) {
            self.objectors.push(o);
        }
    }

    /// Update the support ratio (caller computes from eligible participants).
    pub fn update_support(&mut self, ratio: f64) {
        self.current_support = ratio.clamp(0.0, 1.0);
    }

    /// Attempt to enact the amendment.
    pub fn enact(&mut self) -> Result<(), crate::PolityError> {
        if self.status != AmendmentStatus::Ratifying {
            return Err(crate::PolityError::InvalidAmendmentTransition {
                current: format!("{:?}", self.status),
                target: "Enacted".into(),
            });
        }
        if self.current_support < self.required_threshold {
            return Err(crate::PolityError::ThresholdNotMet {
                required: self.required_threshold,
                actual: self.current_support,
            });
        }
        self.status = AmendmentStatus::Enacted;
        self.enacted_at = Some(Utc::now());
        Ok(())
    }

    /// Reject the amendment.
    pub fn reject(&mut self) -> Result<(), crate::PolityError> {
        if matches!(self.status, AmendmentStatus::Enacted | AmendmentStatus::Null) {
            return Err(crate::PolityError::InvalidAmendmentTransition {
                current: format!("{:?}", self.status),
                target: "Rejected".into(),
            });
        }
        self.status = AmendmentStatus::Rejected;
        Ok(())
    }

    /// Nullify an amendment that contradicts immutable foundations. No transition guard -- this
    /// is an administrative override that can happen at any stage.
    pub fn nullify(&mut self) {
        self.status = AmendmentStatus::Null;
    }
}

/// What triggered the amendment process.
/// From Continuum Art. 4 Section 2: these are the ONLY valid triggers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AmendmentTrigger {
    /// A contradiction or ambiguity within the Covenant text
    Contradiction,
    /// A breach revealed or sustained
    PersistentBreach,
    /// Fundamental transformation in material/relational/technological reality
    MaterialTransformation,
    /// Public invocation by the People declaring the Covenant has failed a need
    PublicInvocation,
}

impl AmendmentTrigger {
    /// Default threshold required for each trigger type.
    /// Higher triggers require broader consensus.
    pub fn default_threshold(&self) -> f64 {
        match self {
            AmendmentTrigger::Contradiction => 0.60,
            AmendmentTrigger::PersistentBreach => 0.60,
            AmendmentTrigger::MaterialTransformation => 0.67,
            AmendmentTrigger::PublicInvocation => 0.75,
        }
    }
}

/// A specific change proposed by an amendment — what to change, from what, to what, and why.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProposedChange {
    /// The article or section being changed (e.g., "Core Art. 5 Section 2").
    pub target: String,
    /// The current text being replaced, if known.
    pub current_text: Option<String>,
    /// The new text being proposed.
    pub proposed_text: String,
    /// Why this change is needed.
    pub rationale: String,
}

/// Lifecycle of an amendment.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AmendmentStatus {
    /// Just proposed, awaiting deliberation
    Proposed,
    /// Under active deliberation
    Deliberating,
    /// Deliberation complete, ratification vote underway
    Ratifying,
    /// Ratified and enacted
    Enacted,
    /// Rejected by the people
    Rejected,
    /// Nullified — contradicts immutable foundations
    Null,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_amendment() {
        let amendment = Amendment::new(
            AmendmentTrigger::MaterialTransformation,
            "Digital Consciousness Rights",
            "Extend explicit protections to synthetic consciousness as technology evolves",
            "community_assembly_42",
        )
        .unwrap();

        assert_eq!(amendment.status, AmendmentStatus::Proposed);
        assert_eq!(amendment.required_threshold, 0.67);
        assert_eq!(amendment.current_support, 0.0);
    }

    #[test]
    fn cannot_create_amendment_violating_foundations() {
        let result = Amendment::new(
            AmendmentTrigger::PublicInvocation,
            "Emergency Surveillance Act",
            "Permit surveillance during declared emergencies",
            "fearful_committee",
        );
        assert!(matches!(
            result,
            Err(crate::PolityError::AmendmentContradictsFoundation(_))
        ));
    }

    #[test]
    fn amendment_lifecycle_happy_path() {
        let mut amendment = Amendment::new(
            AmendmentTrigger::Contradiction,
            "Clarify Digital Stewardship",
            "Add explicit guidance for digital commons stewardship",
            "tech_council",
        )
        .unwrap();

        assert_eq!(amendment.status, AmendmentStatus::Proposed);
        amendment.begin_deliberation().unwrap();
        assert_eq!(amendment.status, AmendmentStatus::Deliberating);
        amendment.begin_ratification().unwrap();
        assert_eq!(amendment.status, AmendmentStatus::Ratifying);

        amendment.add_support("alice");
        amendment.add_support("bob");
        amendment.add_support("carol");
        amendment.update_support(0.72);

        amendment.enact().unwrap();
        assert_eq!(amendment.status, AmendmentStatus::Enacted);
        assert!(amendment.enacted_at.is_some());
    }

    #[test]
    fn cannot_enact_below_threshold() {
        let mut amendment = Amendment::new(
            AmendmentTrigger::PublicInvocation,
            "New Economic Model",
            "Introduce time-banking as primary exchange",
            "economics_assembly",
        )
        .unwrap();

        amendment.begin_deliberation().unwrap();
        amendment.begin_ratification().unwrap();
        amendment.update_support(0.50); // needs 0.75

        let result = amendment.enact();
        assert!(matches!(result, Err(crate::PolityError::ThresholdNotMet { .. })));
    }

    #[test]
    fn cannot_skip_deliberation() {
        let mut amendment = Amendment::new(
            AmendmentTrigger::Contradiction,
            "Quick Fix",
            "Fix a typo",
            "editor",
        )
        .unwrap();

        let result = amendment.begin_ratification();
        assert!(matches!(
            result,
            Err(crate::PolityError::InvalidAmendmentTransition { .. })
        ));
    }

    #[test]
    fn nullify_amendment() {
        let mut amendment = Amendment::new(
            AmendmentTrigger::MaterialTransformation,
            "Restructure Rights",
            "Reorganize rights categories for clarity",
            "council",
        )
        .unwrap();

        amendment.nullify();
        assert_eq!(amendment.status, AmendmentStatus::Null);
    }

    #[test]
    fn trigger_thresholds() {
        assert_eq!(AmendmentTrigger::Contradiction.default_threshold(), 0.60);
        assert_eq!(AmendmentTrigger::PersistentBreach.default_threshold(), 0.60);
        assert_eq!(AmendmentTrigger::MaterialTransformation.default_threshold(), 0.67);
        assert_eq!(AmendmentTrigger::PublicInvocation.default_threshold(), 0.75);
    }

    #[test]
    fn amendment_serialization_roundtrip() {
        let amendment = Amendment::new(
            AmendmentTrigger::PersistentBreach,
            "Strengthen Anti-Surveillance",
            "Add explicit prohibition on algorithmic profiling",
            "privacy_council",
        )
        .unwrap()
        .with_change(ProposedChange {
            target: "Core Art. 5 Section 2".into(),
            current_text: None,
            proposed_text: "Algorithmic profiling shall be prohibited.".into(),
            rationale: "Current text does not explicitly address algorithmic harm.".into(),
        });

        let json = serde_json::to_string(&amendment).unwrap();
        let restored: Amendment = serde_json::from_str(&json).unwrap();
        assert_eq!(amendment.title, restored.title);
        assert_eq!(amendment.proposed_changes.len(), restored.proposed_changes.len());
    }

    #[test]
    fn reject_at_any_stage() {
        let mut amendment = Amendment::new(
            AmendmentTrigger::Contradiction,
            "Some Change",
            "A proposed change",
            "someone",
        )
        .unwrap();

        amendment.begin_deliberation().unwrap();
        amendment.reject().unwrap();
        assert_eq!(amendment.status, AmendmentStatus::Rejected);
    }

    #[test]
    fn cannot_reject_enacted_amendment() {
        let mut amendment = Amendment::new(
            AmendmentTrigger::Contradiction,
            "Enacted Change",
            "Already ratified",
            "council",
        )
        .unwrap();

        amendment.begin_deliberation().unwrap();
        amendment.begin_ratification().unwrap();
        amendment.update_support(0.80);
        amendment.enact().unwrap();

        let result = amendment.reject();
        assert!(matches!(
            result,
            Err(crate::PolityError::InvalidAmendmentTransition { .. })
        ));
    }
}
