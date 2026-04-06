use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A consequence applied to an account.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Consequence {
    pub recipient_pubkey: String,
    pub consequence_type: ConsequenceType,
    pub reason: String,
    pub duration: ConsequenceDuration,
    pub applied_at: DateTime<Utc>,
    pub source: String,
}

/// Types of consequences — graduated, not punitive.
///
/// From Constellation Art. 7 §3: "Graduated Response Protocol —
/// enforcement shall proceed through graduated response, escalating
/// only when lesser measures prove insufficient."
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ConsequenceType {
    Warning,
    VouchingSuspension,
    SponsorshipRevocation,
    SphereRestriction,
    AccountRestriction,
}

/// How long a consequence lasts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ConsequenceDuration {
    Permanent,
    Until(DateTime<Utc>),
    UntilConditionsMet(String),
}

impl ConsequenceDuration {
    pub fn is_expired(&self) -> bool {
        match self {
            ConsequenceDuration::Permanent => false,
            ConsequenceDuration::Until(date) => Utc::now() > *date,
            ConsequenceDuration::UntilConditionsMet(_) => false, // manual check
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consequence_duration_expiry() {
        let permanent = ConsequenceDuration::Permanent;
        assert!(!permanent.is_expired());

        let future = ConsequenceDuration::Until(Utc::now() + chrono::Duration::days(30));
        assert!(!future.is_expired());

        let past = ConsequenceDuration::Until(Utc::now() - chrono::Duration::days(1));
        assert!(past.is_expired());

        let conditional = ConsequenceDuration::UntilConditionsMet("complete training".into());
        assert!(!conditional.is_expired());
    }
}
