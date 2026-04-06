use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// How new members join a community.
///
/// From Constellation Art. 2 §1: "Recognition shall not depend upon formal registration,
/// external endorsement, or numerical threshold."
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum JoinProcess {
    /// Anyone can join freely.
    Open,
    /// Must submit an application for review.
    Application,
    /// Must be invited by an existing member.
    Invitation,
    /// Must be sponsored by an existing member in good standing.
    Sponsorship,
    /// Requires consensus of existing members.
    Consensus,
}

/// A request to join a community.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MembershipApplication {
    pub id: Uuid,
    pub community_id: Uuid,
    pub applicant_pubkey: String,
    pub sponsor_pubkey: Option<String>,
    pub statement: String,
    pub status: ApplicationStatus,
    pub submitted_at: DateTime<Utc>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub reviewed_by: Option<String>,
    pub rejection_reason: Option<String>,
}

impl MembershipApplication {
    /// Create a new pending membership application.
    pub fn new(
        community_id: Uuid,
        applicant_pubkey: impl Into<String>,
        statement: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            community_id,
            applicant_pubkey: applicant_pubkey.into(),
            sponsor_pubkey: None,
            statement: statement.into(),
            status: ApplicationStatus::Pending,
            submitted_at: Utc::now(),
            reviewed_at: None,
            reviewed_by: None,
            rejection_reason: None,
        }
    }

    /// Attach a sponsor to this application (builder pattern).
    pub fn with_sponsor(mut self, sponsor: impl Into<String>) -> Self {
        self.sponsor_pubkey = Some(sponsor.into());
        self
    }

    /// Approve this application. Only valid from Pending status.
    pub fn approve(&mut self, reviewer: impl Into<String>) -> Result<(), crate::KingdomError> {
        if self.status != ApplicationStatus::Pending {
            return Err(crate::KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Approved".into(),
            });
        }
        self.status = ApplicationStatus::Approved;
        self.reviewed_at = Some(Utc::now());
        self.reviewed_by = Some(reviewer.into());
        Ok(())
    }

    /// Reject this application with a reason. Only valid from Pending status.
    pub fn reject(
        &mut self,
        reviewer: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<(), crate::KingdomError> {
        if self.status != ApplicationStatus::Pending {
            return Err(crate::KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Rejected".into(),
            });
        }
        self.status = ApplicationStatus::Rejected;
        self.reviewed_at = Some(Utc::now());
        self.reviewed_by = Some(reviewer.into());
        self.rejection_reason = Some(reason.into());
        Ok(())
    }

    /// Whether this application is still awaiting review.
    pub fn is_pending(&self) -> bool {
        self.status == ApplicationStatus::Pending
    }
}

/// Status of a membership application.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ApplicationStatus {
    /// Awaiting review by community governance.
    Pending,
    /// Application accepted — applicant may join.
    Approved,
    /// Application denied by a reviewer.
    Rejected,
    /// Applicant withdrew their own application.
    Withdrawn,
}

/// Requirements for forming a new community.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FormationRequirements {
    pub minimum_founders: usize,
    pub require_charter: bool,
    pub formation_time_window_hours: u32,
}

impl Default for FormationRequirements {
    fn default() -> Self {
        Self {
            minimum_founders: 3,
            require_charter: true,
            formation_time_window_hours: 24,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_application() {
        let app = MembershipApplication::new(
            Uuid::new_v4(),
            "alice_pubkey",
            "I'd like to join your community",
        );
        assert!(app.is_pending());
        assert!(app.sponsor_pubkey.is_none());
    }

    #[test]
    fn sponsored_application() {
        let app = MembershipApplication::new(
            Uuid::new_v4(),
            "alice_pubkey",
            "Bob vouches for me",
        )
        .with_sponsor("bob_pubkey");

        assert_eq!(app.sponsor_pubkey.as_deref(), Some("bob_pubkey"));
    }

    #[test]
    fn approve_application() {
        let mut app = MembershipApplication::new(Uuid::new_v4(), "alice", "hello");
        app.approve("steward_bob").unwrap();
        assert_eq!(app.status, ApplicationStatus::Approved);
        assert!(app.reviewed_at.is_some());
        assert_eq!(app.reviewed_by.as_deref(), Some("steward_bob"));
    }

    #[test]
    fn reject_application() {
        let mut app = MembershipApplication::new(Uuid::new_v4(), "alice", "hello");
        app.reject("steward_bob", "Community is at capacity").unwrap();
        assert_eq!(app.status, ApplicationStatus::Rejected);
        assert_eq!(
            app.rejection_reason.as_deref(),
            Some("Community is at capacity")
        );
    }

    #[test]
    fn cannot_approve_already_reviewed() {
        let mut app = MembershipApplication::new(Uuid::new_v4(), "alice", "hello");
        app.approve("bob").unwrap();
        assert!(app.approve("charlie").is_err());
    }

    #[test]
    fn cannot_reject_already_reviewed() {
        let mut app = MembershipApplication::new(Uuid::new_v4(), "alice", "hello");
        app.reject("bob", "nope").unwrap();
        assert!(app.reject("charlie", "also nope").is_err());
    }

    #[test]
    fn formation_requirements_defaults() {
        let req = FormationRequirements::default();
        assert_eq!(req.minimum_founders, 3);
        assert!(req.require_charter);
    }
}
