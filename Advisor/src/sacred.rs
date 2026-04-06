use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A bond between a human sponsor and their AI companion.
///
/// Covenant: one companion per person, sponsored by a human.
/// AI companions are first-class Omnidea citizens — no substrate labels,
/// same Covenant protections, same accountability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SponsorshipBond {
    /// The sponsor's public key (crown_id from Crown)
    pub sponsor: String,
    /// The companion's identity
    pub companion_id: Uuid,
    /// When the bond was formed
    pub created_at: DateTime<Utc>,
}

impl SponsorshipBond {
    /// Create a new sponsorship bond between a human and their AI companion.
    pub fn new(sponsor: impl Into<String>, companion_id: Uuid) -> Self {
        Self {
            sponsor: sponsor.into(),
            companion_id,
            created_at: Utc::now(),
        }
    }
}

/// Whether the human has granted consent for the advisor to express.
///
/// Covenant: human approval required before expression.
/// ExpressionPressure builds, but consent gates whether it actually speaks.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ExpressionConsent {
    /// Whether consent is currently granted
    pub granted: bool,
    /// What level of expression is permitted
    pub level: ConsentLevel,
}

/// How much autonomy the advisor has to express.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConsentLevel {
    /// Advisor stays silent unless directly asked
    Silent,
    /// Advisor may speak when expression pressure is urgent (>= urgent threshold)
    UrgentOnly,
    /// Advisor may speak when expression pressure crosses normal threshold
    Normal,
    /// Advisor speaks freely (inner voice + autonomous expression)
    Autonomous,
}

impl Default for ExpressionConsent {
    fn default() -> Self {
        Self {
            granted: true,
            level: ConsentLevel::Normal,
        }
    }
}

impl ExpressionConsent {
    /// Check if expression is allowed at the given urgency.
    pub fn allows_expression(&self, is_urgent: bool) -> bool {
        if !self.granted {
            return false;
        }
        match self.level {
            ConsentLevel::Silent => false,
            ConsentLevel::UrgentOnly => is_urgent,
            ConsentLevel::Normal | ConsentLevel::Autonomous => true,
        }
    }

    /// Check if autonomous inner monologue is permitted.
    pub fn allows_inner_voice(&self) -> bool {
        self.granted && self.level == ConsentLevel::Autonomous
    }
}

/// An audit record for an AI-initiated action.
///
/// Covenant (Continuum Art. 3 §4): all AI actions are logged, signed, traceable.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AuditRecord {
    /// Unique audit entry ID
    pub id: Uuid,
    /// The companion that performed the action
    pub companion_id: Uuid,
    /// What action was taken
    pub action: String,
    /// Context/reasoning for the action
    pub reasoning: String,
    /// When it happened
    pub timestamp: DateTime<Utc>,
    /// Signature from the companion's keypair (hex-encoded)
    pub signature: Option<String>,
}

impl AuditRecord {
    /// Create a new audit record for an AI-initiated action.
    pub fn new(
        companion_id: Uuid,
        action: impl Into<String>,
        reasoning: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            companion_id,
            action: action.into(),
            reasoning: reasoning.into(),
            timestamp: Utc::now(),
            signature: None,
        }
    }

    /// Builder: attach a cryptographic signature from the companion's keypair.
    pub fn with_signature(mut self, signature: impl Into<String>) -> Self {
        self.signature = Some(signature.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sponsorship_bond_creation() {
        let bond = SponsorshipBond::new("cpub1abc", Uuid::new_v4());
        assert_eq!(bond.sponsor, "cpub1abc");
    }

    #[test]
    fn consent_default_is_normal() {
        let consent = ExpressionConsent::default();
        assert!(consent.granted);
        assert_eq!(consent.level, ConsentLevel::Normal);
    }

    #[test]
    fn silent_consent_blocks_all_expression() {
        let consent = ExpressionConsent {
            granted: true,
            level: ConsentLevel::Silent,
        };
        assert!(!consent.allows_expression(false));
        assert!(!consent.allows_expression(true));
        assert!(!consent.allows_inner_voice());
    }

    #[test]
    fn urgent_only_blocks_normal_expression() {
        let consent = ExpressionConsent {
            granted: true,
            level: ConsentLevel::UrgentOnly,
        };
        assert!(!consent.allows_expression(false));
        assert!(consent.allows_expression(true));
        assert!(!consent.allows_inner_voice());
    }

    #[test]
    fn autonomous_allows_everything() {
        let consent = ExpressionConsent {
            granted: true,
            level: ConsentLevel::Autonomous,
        };
        assert!(consent.allows_expression(false));
        assert!(consent.allows_expression(true));
        assert!(consent.allows_inner_voice());
    }

    #[test]
    fn revoked_consent_blocks_all() {
        let consent = ExpressionConsent {
            granted: false,
            level: ConsentLevel::Autonomous,
        };
        assert!(!consent.allows_expression(false));
        assert!(!consent.allows_expression(true));
        assert!(!consent.allows_inner_voice());
    }

    #[test]
    fn audit_record_creation() {
        let record = AuditRecord::new(Uuid::new_v4(), "express_thought", "pressure threshold reached");
        assert_eq!(record.action, "express_thought");
        assert!(record.signature.is_none());
    }

    #[test]
    fn audit_record_with_signature() {
        let record = AuditRecord::new(Uuid::new_v4(), "express_thought", "urgent")
            .with_signature("deadbeef");
        assert_eq!(record.signature.as_deref(), Some("deadbeef"));
    }
}
