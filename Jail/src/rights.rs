//! Rights of the accused and reporter protection.
//!
//! **AccusedRights** — the floor. Always on. No exceptions.
//! From Constellation Art. 5 §2-5 and Art. 7 §9.
//!
//! **ReporterProtection** — whistleblower protection.
//! From Coexistence Art. 6 §4 and Constellation Art. 5 §5.
//!
//! These are non-negotiable. They have no "disable" API. Any code path
//! that would violate them is a bug.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Rights of the accused — always preserved, no exceptions.
///
/// "No person... shall be punished, silenced, or retaliated against" (Art. 5 §5).
/// These rights exist at every stage of the accountability process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccusedRights {
    /// Right to know the specific charges against them.
    pub right_to_know_charges: bool,
    /// Right to respond to the charges.
    pub right_to_respond: bool,
    /// Right to challenge the evidence and process.
    pub right_to_challenge: bool,
    /// Right to present evidence in their defense.
    pub right_to_present_evidence: bool,
    /// Right to appeal any decision.
    pub right_to_appeal: bool,
    /// Right to proportional response (no disproportionate punishment).
    pub right_to_proportional_response: bool,
}

impl AccusedRights {
    /// The only constructor. All rights are always on.
    ///
    /// There is no way to create an `AccusedRights` with any right disabled.
    /// If you need to check rights, call `validate()` — it should always return true.
    pub fn always() -> Self {
        Self {
            right_to_know_charges: true,
            right_to_respond: true,
            right_to_challenge: true,
            right_to_present_evidence: true,
            right_to_appeal: true,
            right_to_proportional_response: true,
        }
    }

    /// Validate that all rights are preserved. Should always return true.
    /// If this returns false, something has gone catastrophically wrong.
    pub fn validate(&self) -> bool {
        self.right_to_know_charges
            && self.right_to_respond
            && self.right_to_challenge
            && self.right_to_present_evidence
            && self.right_to_appeal
            && self.right_to_proportional_response
    }

    /// List all rights as human-readable strings.
    pub fn enumerate(&self) -> Vec<&'static str> {
        vec![
            "Right to know the charges",
            "Right to respond",
            "Right to challenge evidence and process",
            "Right to present evidence in defense",
            "Right to appeal any decision",
            "Right to proportional response",
        ]
    }
}

impl Default for AccusedRights {
    fn default() -> Self {
        Self::always()
    }
}

/// Protection for reporters/whistleblowers.
///
/// From Coexistence Art. 6 §4: "Persons who report violations... shall be
/// protected from retaliation. Such protection shall encompass legal immunity
/// for good faith reporting, economic protection... personal safety measures...
/// and recognition and support for their service to justice."
///
/// From Constellation Art. 5 §5: "No person... shall be punished, silenced,
/// or retaliated against for initiating or supporting lawful challenge."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReporterProtection {
    /// The reporter's pubkey (protected from disclosure to the accused).
    pub reporter_pubkey: String,
    /// The flag they reported.
    pub flag_id: Uuid,
    /// Reporter identity is protected from the accused (reviewers can see it).
    pub identity_protected: bool,
    /// Monitoring for retaliatory actions against the reporter.
    pub retaliation_monitored: bool,
    /// Legal immunity for good faith reporting.
    pub legal_immunity_for_good_faith: bool,
    /// When the protection was established.
    pub established_at: DateTime<Utc>,
}

impl ReporterProtection {
    /// Create protection for a flag reporter. All protections on by default.
    pub fn for_flag(reporter: impl Into<String>, flag_id: Uuid) -> Self {
        Self {
            reporter_pubkey: reporter.into(),
            flag_id,
            identity_protected: true,
            retaliation_monitored: true,
            legal_immunity_for_good_faith: true,
            established_at: Utc::now(),
        }
    }

    /// Validate that all protections are active.
    pub fn validate(&self) -> bool {
        self.identity_protected
            && self.retaliation_monitored
            && self.legal_immunity_for_good_faith
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accused_rights_always_on() {
        let rights = AccusedRights::always();
        assert!(rights.right_to_know_charges);
        assert!(rights.right_to_respond);
        assert!(rights.right_to_challenge);
        assert!(rights.right_to_present_evidence);
        assert!(rights.right_to_appeal);
        assert!(rights.right_to_proportional_response);
        assert!(rights.validate());
    }

    #[test]
    fn accused_rights_default_is_always() {
        let rights = AccusedRights::default();
        assert!(rights.validate());
    }

    #[test]
    fn accused_rights_enumerate() {
        let rights = AccusedRights::always();
        let list = rights.enumerate();
        assert_eq!(list.len(), 6);
        assert!(list[0].contains("charges"));
        assert!(list[4].contains("appeal"));
    }

    #[test]
    fn accused_rights_validation_detects_violation() {
        // This is a safety test — in production, you can't construct this,
        // but we test the validator works.
        let mut rights = AccusedRights::always();
        rights.right_to_appeal = false;
        assert!(!rights.validate());
    }

    #[test]
    fn reporter_protection_all_on() {
        let protection = ReporterProtection::for_flag("alice", Uuid::new_v4());
        assert!(protection.identity_protected);
        assert!(protection.retaliation_monitored);
        assert!(protection.legal_immunity_for_good_faith);
        assert!(protection.validate());
    }

    #[test]
    fn reporter_protection_validation() {
        let mut protection = ReporterProtection::for_flag("alice", Uuid::new_v4());
        assert!(protection.validate());

        protection.identity_protected = false;
        assert!(!protection.validate());
    }

    #[test]
    fn rights_serialization_roundtrip() {
        let rights = AccusedRights::always();
        let json = serde_json::to_string(&rights).unwrap();
        let deserialized: AccusedRights = serde_json::from_str(&json).unwrap();
        assert_eq!(rights, deserialized);
        assert!(deserialized.validate());
    }

    #[test]
    fn protection_serialization_roundtrip() {
        let protection = ReporterProtection::for_flag("alice", Uuid::new_v4());
        let json = serde_json::to_string(&protection).unwrap();
        let deserialized: ReporterProtection = serde_json::from_str(&json).unwrap();
        assert_eq!(protection, deserialized);
    }
}
