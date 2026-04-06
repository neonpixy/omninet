//! Graduated response — proportional accountability escalation.
//!
//! From Constellation Art. 7 §3: "Enforcement shall proceed through graduated
//! response, escalating only when lesser measures prove insufficient."
//!
//! Education → Public Censure → Economic Disengagement → Coordinated
//! Non-Cooperation → Protective Exclusion.
//!
//! Every level is reversible. Even exclusion has a path back (Art. 7 §10).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::JailError;

/// Levels of graduated response (Art. 7 §3).
///
/// "The first response shall be educational dialogue... where this fails,
/// communities may proceed to public censure, economic disengagement, and
/// coordinated non-cooperation."
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ResponseLevel {
    /// Educational dialogue — resources and pathways to compliance.
    Education,
    /// Public acknowledgment of harm by the community.
    PublicCensure,
    /// Withdrawal of economic cooperation.
    EconomicDisengagement,
    /// Multiple communities withdraw cooperation.
    CoordinatedNonCooperation,
    /// When safety requires it — not punishment (Art. 7 §12).
    ProtectiveExclusion,
}

impl ResponseLevel {
    /// Human-readable description.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Education => "Educational dialogue with clear pathways to compliance",
            Self::PublicCensure => "Public acknowledgment of harm by the community",
            Self::EconomicDisengagement => "Withdrawal of economic cooperation",
            Self::CoordinatedNonCooperation => "Multiple communities withdraw cooperation",
            Self::ProtectiveExclusion => "Protective separation when safety requires it",
        }
    }

    /// All levels are reversible (Art. 7 §10).
    pub fn is_reversible(&self) -> bool {
        true // Every level. No exceptions. No permanent castes.
    }

    /// Numeric severity (0-4).
    pub fn severity(&self) -> u8 {
        match self {
            Self::Education => 0,
            Self::PublicCensure => 1,
            Self::EconomicDisengagement => 2,
            Self::CoordinatedNonCooperation => 3,
            Self::ProtectiveExclusion => 4,
        }
    }
}

impl std::fmt::Display for ResponseLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Education => write!(f, "education"),
            Self::PublicCensure => write!(f, "public_censure"),
            Self::EconomicDisengagement => write!(f, "economic_disengagement"),
            Self::CoordinatedNonCooperation => write!(f, "coordinated_non_cooperation"),
            Self::ProtectiveExclusion => write!(f, "protective_exclusion"),
        }
    }
}

/// A graduated response being applied to a person.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraduatedResponse {
    /// Unique response identifier.
    pub id: Uuid,
    /// The person this response applies to.
    pub target_pubkey: String,
    /// Current response level.
    pub current_level: ResponseLevel,
    /// History of level changes.
    pub history: Vec<ResponseRecord>,
    /// When the response was first initiated.
    pub started_at: DateTime<Utc>,
    /// When resolved (if ever).
    pub resolved_at: Option<DateTime<Utc>>,
}

/// A record of a response level being applied or changed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResponseRecord {
    /// The response level.
    pub level: ResponseLevel,
    /// Why this level was applied.
    pub reason: String,
    /// Who initiated this level.
    pub initiated_by: String,
    /// Community context.
    pub community_id: Option<String>,
    /// When this level started.
    pub started_at: DateTime<Utc>,
    /// When this level ended (if superseded).
    pub ended_at: Option<DateTime<Utc>>,
}

impl GraduatedResponse {
    /// Begin a graduated response at the Education level (always start at the bottom).
    pub fn begin(
        target_pubkey: impl Into<String>,
        reason: impl Into<String>,
        initiated_by: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        let record = ResponseRecord {
            level: ResponseLevel::Education,
            reason: reason.into(),
            initiated_by: initiated_by.into(),
            community_id: None,
            started_at: now,
            ended_at: None,
        };

        Self {
            id: Uuid::new_v4(),
            target_pubkey: target_pubkey.into(),
            current_level: ResponseLevel::Education,
            history: vec![record],
            started_at: now,
            resolved_at: None,
        }
    }

    /// Escalate to the next level.
    pub fn escalate(
        &mut self,
        reason: impl Into<String>,
        initiated_by: impl Into<String>,
    ) -> Result<ResponseLevel, JailError> {
        if self.resolved_at.is_some() {
            return Err(JailError::InvalidResponseTransition {
                from: self.current_level.to_string(),
                to: "escalation after resolution".into(),
            });
        }

        let next = match self.current_level {
            ResponseLevel::Education => ResponseLevel::PublicCensure,
            ResponseLevel::PublicCensure => ResponseLevel::EconomicDisengagement,
            ResponseLevel::EconomicDisengagement => ResponseLevel::CoordinatedNonCooperation,
            ResponseLevel::CoordinatedNonCooperation => ResponseLevel::ProtectiveExclusion,
            ResponseLevel::ProtectiveExclusion => {
                return Err(JailError::InvalidResponseTransition {
                    from: "protective_exclusion".into(),
                    to: "cannot escalate beyond exclusion".into(),
                });
            }
        };

        let now = Utc::now();

        // Close the current record
        if let Some(current) = self.history.last_mut() {
            current.ended_at = Some(now);
        }

        // Start new record
        self.history.push(ResponseRecord {
            level: next,
            reason: reason.into(),
            initiated_by: initiated_by.into(),
            community_id: None,
            started_at: now,
            ended_at: None,
        });

        self.current_level = next;
        Ok(next)
    }

    /// De-escalate to the previous level.
    pub fn de_escalate(
        &mut self,
        reason: impl Into<String>,
        initiated_by: impl Into<String>,
    ) -> Result<ResponseLevel, JailError> {
        if self.resolved_at.is_some() {
            return Err(JailError::InvalidResponseTransition {
                from: self.current_level.to_string(),
                to: "de-escalation after resolution".into(),
            });
        }

        let prev = match self.current_level {
            ResponseLevel::Education => {
                return Err(JailError::InvalidResponseTransition {
                    from: "education".into(),
                    to: "cannot de-escalate below education".into(),
                });
            }
            ResponseLevel::PublicCensure => ResponseLevel::Education,
            ResponseLevel::EconomicDisengagement => ResponseLevel::PublicCensure,
            ResponseLevel::CoordinatedNonCooperation => ResponseLevel::EconomicDisengagement,
            ResponseLevel::ProtectiveExclusion => ResponseLevel::CoordinatedNonCooperation,
        };

        let now = Utc::now();

        if let Some(current) = self.history.last_mut() {
            current.ended_at = Some(now);
        }

        self.history.push(ResponseRecord {
            level: prev,
            reason: reason.into(),
            initiated_by: initiated_by.into(),
            community_id: None,
            started_at: now,
            ended_at: None,
        });

        self.current_level = prev;
        Ok(prev)
    }

    /// Resolve the graduated response — the matter is closed.
    pub fn resolve(&mut self, reason: impl Into<String>) {
        let now = Utc::now();
        if let Some(current) = self.history.last_mut() {
            current.ended_at = Some(now);
        }
        self.resolved_at = Some(now);
        let _ = reason.into(); // consumed for audit
    }

    /// Whether this response is still active.
    pub fn is_active(&self) -> bool {
        self.resolved_at.is_none()
    }

    /// Attempt to escalate this graduated response to a sustained exclusion.
    ///
    /// Only available when the current level is `ProtectiveExclusion` AND at
    /// least one full cycle of exclusion -> reinstatement -> reoffense has
    /// been completed.
    ///
    /// Returns a `SustainedExclusionRequest` builder pre-populated with the
    /// `RepeatedProtectiveExclusion` basis and the target pubkey.
    pub fn escalate_to_sustained(
        &self,
    ) -> Result<crate::sustained_exclusion::SustainedExclusionRequest, JailError> {
        crate::sustained_exclusion::escalate_to_sustained(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_at_education() {
        let response = GraduatedResponse::begin("bob", "harassment concern", "alice");
        assert_eq!(response.current_level, ResponseLevel::Education);
        assert_eq!(response.history.len(), 1);
        assert!(response.is_active());
    }

    #[test]
    fn escalate_through_levels() {
        let mut response = GraduatedResponse::begin("bob", "initial", "alice");

        let next = response.escalate("no improvement", "alice").unwrap();
        assert_eq!(next, ResponseLevel::PublicCensure);

        let next = response.escalate("continued issues", "alice").unwrap();
        assert_eq!(next, ResponseLevel::EconomicDisengagement);

        let next = response.escalate("multi-community", "alice").unwrap();
        assert_eq!(next, ResponseLevel::CoordinatedNonCooperation);

        let next = response.escalate("safety requires", "alice").unwrap();
        assert_eq!(next, ResponseLevel::ProtectiveExclusion);

        // Can't go beyond exclusion
        assert!(response.escalate("more", "alice").is_err());
    }

    #[test]
    fn de_escalate() {
        let mut response = GraduatedResponse::begin("bob", "initial", "alice");
        response.escalate("reason", "alice").unwrap();
        response.escalate("reason", "alice").unwrap();
        assert_eq!(response.current_level, ResponseLevel::EconomicDisengagement);

        let prev = response.de_escalate("improvement shown", "alice").unwrap();
        assert_eq!(prev, ResponseLevel::PublicCensure);
    }

    #[test]
    fn cannot_de_escalate_below_education() {
        let mut response = GraduatedResponse::begin("bob", "initial", "alice");
        assert!(response.de_escalate("reason", "alice").is_err());
    }

    #[test]
    fn resolve_closes_response() {
        let mut response = GraduatedResponse::begin("bob", "initial", "alice");
        assert!(response.is_active());

        response.resolve("matter addressed");
        assert!(!response.is_active());
        assert!(response.resolved_at.is_some());
    }

    #[test]
    fn cannot_escalate_after_resolution() {
        let mut response = GraduatedResponse::begin("bob", "initial", "alice");
        response.resolve("done");
        assert!(response.escalate("more", "alice").is_err());
    }

    #[test]
    fn history_tracks_changes() {
        let mut response = GraduatedResponse::begin("bob", "initial", "alice");
        response.escalate("step 2", "alice").unwrap();
        response.de_escalate("improvement", "alice").unwrap();

        assert_eq!(response.history.len(), 3);
        assert_eq!(response.history[0].level, ResponseLevel::Education);
        assert_eq!(response.history[1].level, ResponseLevel::PublicCensure);
        assert_eq!(response.history[2].level, ResponseLevel::Education);

        // Previous records have ended_at
        assert!(response.history[0].ended_at.is_some());
        assert!(response.history[1].ended_at.is_some());
        assert!(response.history[2].ended_at.is_none()); // current
    }

    #[test]
    fn all_levels_reversible() {
        for level in [
            ResponseLevel::Education,
            ResponseLevel::PublicCensure,
            ResponseLevel::EconomicDisengagement,
            ResponseLevel::CoordinatedNonCooperation,
            ResponseLevel::ProtectiveExclusion,
        ] {
            assert!(level.is_reversible());
        }
    }

    #[test]
    fn level_severity_ordering() {
        assert!(ResponseLevel::Education.severity() < ResponseLevel::PublicCensure.severity());
        assert!(ResponseLevel::ProtectiveExclusion.severity() == 4);
    }

    #[test]
    fn response_serialization_roundtrip() {
        let response = GraduatedResponse::begin("bob", "test", "alice");
        let json = serde_json::to_string(&response).unwrap();
        let deserialized: GraduatedResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(response, deserialized);
    }
}
