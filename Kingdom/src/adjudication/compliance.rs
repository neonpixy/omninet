use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::dispute::EvidenceItem;

/// Tracking whether a remedy from a resolution has been complied with.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComplianceRecord {
    pub id: Uuid,
    pub resolution_id: Uuid,
    pub remedy_id: Uuid,
    pub reported_by: String,
    pub evidence: Vec<EvidenceItem>,
    pub status: ComplianceStatus,
    pub verified_by: Option<String>,
    pub verified_at: Option<DateTime<Utc>>,
    pub notes: Option<String>,
    pub reported_at: DateTime<Utc>,
}

impl ComplianceRecord {
    /// Create a new compliance record in Pending status.
    pub fn new(
        resolution_id: Uuid,
        remedy_id: Uuid,
        reported_by: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            resolution_id,
            remedy_id,
            reported_by: reported_by.into(),
            evidence: Vec::new(),
            status: ComplianceStatus::Pending,
            verified_by: None,
            verified_at: None,
            notes: None,
            reported_at: Utc::now(),
        }
    }

    pub fn with_evidence(mut self, evidence: Vec<EvidenceItem>) -> Self {
        self.evidence = evidence;
        self
    }

    /// Verify that the remedy has been complied with.
    pub fn verify(
        &mut self,
        verifier: impl Into<String>,
        notes: Option<String>,
    ) {
        self.status = ComplianceStatus::Verified;
        self.verified_by = Some(verifier.into());
        self.verified_at = Some(Utc::now());
        self.notes = notes;
    }

    /// Reject the compliance report as insufficient.
    pub fn reject(
        &mut self,
        verifier: impl Into<String>,
        notes: Option<String>,
    ) {
        self.status = ComplianceStatus::Rejected;
        self.verified_by = Some(verifier.into());
        self.verified_at = Some(Utc::now());
        self.notes = notes;
    }

    /// Whether this compliance record has been verified.
    pub fn is_verified(&self) -> bool {
        self.status == ComplianceStatus::Verified
    }
}

/// Status of compliance verification.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ComplianceStatus {
    /// Compliance has been reported but not yet verified.
    Pending,
    /// Compliance has been confirmed by a verifier.
    Verified,
    /// The compliance report was found insufficient.
    Rejected,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compliance_lifecycle() {
        let mut record = ComplianceRecord::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            "bob",
        );
        assert_eq!(record.status, ComplianceStatus::Pending);
        assert!(!record.is_verified());

        record.verify("steward_alice", Some("Tools returned in good condition".into()));
        assert!(record.is_verified());
        assert_eq!(record.verified_by.as_deref(), Some("steward_alice"));
    }

    #[test]
    fn compliance_rejection() {
        let mut record = ComplianceRecord::new(Uuid::new_v4(), Uuid::new_v4(), "bob");
        record.reject("steward_alice", Some("Tools still missing".into()));
        assert_eq!(record.status, ComplianceStatus::Rejected);
        assert!(!record.is_verified());
    }
}
