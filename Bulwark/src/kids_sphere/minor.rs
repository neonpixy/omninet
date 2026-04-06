use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A minor's registration state — siloed until parent authorizes.
///
/// Flow: Siloed (local-only) → ParentLinked → Authorized (Kids Sphere access)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MinorRegistrationState {
    /// Local-only access. No network connectivity. Nothing leaves device.
    Siloed,
    /// Parent account exists and has linked this child. Awaiting approval.
    ParentLinked,
    /// Parent has approved network access. Child enters Kids Sphere.
    Authorized,
}

/// A minor waiting for parent authorization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SiloedMinor {
    pub id: Uuid,
    pub pubkey: String,
    pub claimed_age: Option<u8>,
    pub detected_as_minor: MinorDetectionReason,
    pub state: MinorRegistrationState,
    pub created_at: DateTime<Utc>,
    pub parent_link: Option<ParentLink>,
    pub authorized_at: Option<DateTime<Utc>>,
}

impl SiloedMinor {
    pub fn new(
        pubkey: impl Into<String>,
        claimed_age: Option<u8>,
        reason: MinorDetectionReason,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            pubkey: pubkey.into(),
            claimed_age,
            detected_as_minor: reason,
            state: MinorRegistrationState::Siloed,
            created_at: Utc::now(),
            parent_link: None,
            authorized_at: None,
        }
    }

    /// Link a parent to this minor.
    pub fn link_parent(&mut self, link: ParentLink) -> Result<(), crate::BulwarkError> {
        if self.state != MinorRegistrationState::Siloed {
            return Err(crate::BulwarkError::MinorNotAuthorized(
                "already linked or authorized".into(),
            ));
        }
        self.parent_link = Some(link);
        self.state = MinorRegistrationState::ParentLinked;
        Ok(())
    }

    /// Parent authorizes network access → child enters Kids Sphere.
    pub fn authorize(&mut self) -> Result<(), crate::BulwarkError> {
        if self.state != MinorRegistrationState::ParentLinked {
            return Err(crate::BulwarkError::ParentLinkRequired);
        }
        self.state = MinorRegistrationState::Authorized;
        self.authorized_at = Some(Utc::now());
        Ok(())
    }

    pub fn is_authorized(&self) -> bool {
        self.state == MinorRegistrationState::Authorized
    }

    pub fn has_parent(&self) -> bool {
        self.parent_link.is_some()
    }

    pub fn days_waiting(&self) -> i64 {
        Utc::now()
            .signed_duration_since(self.created_at)
            .num_days()
    }
}

/// A link between a parent and their child.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParentLink {
    pub parent_pubkey: String,
    pub relationship: ParentRelationship,
    pub linked_at: DateTime<Utc>,
}

/// Type of parent/guardian relationship.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ParentRelationship {
    Parent,
    LegalGuardian,
    StepParent,
    Grandparent,
    FosterParent,
    Other,
}

/// How the minor was detected.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MinorDetectionReason {
    /// User stated age < 18.
    SelfDeclared,
    /// Parent explicitly registered them as a child.
    ParentRegistered,
    /// Voucher flagged wrong age tier.
    VoucherFlagged,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minor_lifecycle() {
        let mut minor = SiloedMinor::new("kid_alice", Some(10), MinorDetectionReason::SelfDeclared);
        assert_eq!(minor.state, MinorRegistrationState::Siloed);
        assert!(!minor.is_authorized());
        assert!(!minor.has_parent());

        // Link parent
        minor
            .link_parent(ParentLink {
                parent_pubkey: "parent_bob".into(),
                relationship: ParentRelationship::Parent,
                linked_at: Utc::now(),
            })
            .unwrap();
        assert_eq!(minor.state, MinorRegistrationState::ParentLinked);
        assert!(minor.has_parent());

        // Authorize
        minor.authorize().unwrap();
        assert!(minor.is_authorized());
        assert!(minor.authorized_at.is_some());
    }

    #[test]
    fn cannot_authorize_without_parent() {
        let mut minor = SiloedMinor::new("kid", None, MinorDetectionReason::SelfDeclared);
        assert!(minor.authorize().is_err());
    }

    #[test]
    fn cannot_link_parent_twice() {
        let mut minor = SiloedMinor::new("kid", Some(12), MinorDetectionReason::ParentRegistered);
        minor
            .link_parent(ParentLink {
                parent_pubkey: "parent".into(),
                relationship: ParentRelationship::Parent,
                linked_at: Utc::now(),
            })
            .unwrap();
        // Try linking again
        assert!(minor
            .link_parent(ParentLink {
                parent_pubkey: "other_parent".into(),
                relationship: ParentRelationship::LegalGuardian,
                linked_at: Utc::now(),
            })
            .is_err());
    }
}
