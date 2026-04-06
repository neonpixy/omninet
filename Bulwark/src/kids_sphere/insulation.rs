use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::verification::proximity::ProximityProof;

/// A family-to-family bond — REQUIRES physical proximity proof.
///
/// Physical proximity IS required for Kids Sphere.
/// Family-to-family bonds between parents/guardians must have verified proximity
/// proof before kids can connect. Non-negotiable."
///
/// A child can ONLY connect with another child if:
/// 1. Both children's parents have a FamilyBond with ProximityProof
/// 2. Both parents have approved the connection
/// 3. Both children are in Kids Sphere or Teen Sphere
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FamilyBond {
    pub id: Uuid,
    pub family_a_parent: String,
    pub family_b_parent: String,
    pub proximity_proof: ProximityProof,
    pub created_at: DateTime<Utc>,
    pub verified_at: DateTime<Utc>,
}

impl FamilyBond {
    /// Create a new family bond. REQUIRES valid proximity proof.
    pub fn new(
        family_a_parent: impl Into<String>,
        family_b_parent: impl Into<String>,
        proximity_proof: ProximityProof,
    ) -> Result<Self, crate::BulwarkError> {
        if !proximity_proof.has_proximity_evidence() {
            return Err(crate::BulwarkError::FamilyBondRequiresProximity);
        }
        Ok(Self {
            id: Uuid::new_v4(),
            family_a_parent: family_a_parent.into(),
            family_b_parent: family_b_parent.into(),
            proximity_proof,
            created_at: Utc::now(),
            verified_at: Utc::now(),
        })
    }

    /// Whether the given parent is part of this family bond.
    pub fn involves_parent(&self, pubkey: &str) -> bool {
        self.family_a_parent == pubkey || self.family_b_parent == pubkey
    }
}

/// Rules for kids connecting with other kids.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KidConnectionRules {
    /// Both must be verified kids in Kids Sphere.
    pub both_verified_kids: bool,
    /// Parents must have a FamilyBond with ProximityProof.
    pub requires_family_bond: bool,
    /// At least one parent must approve.
    pub requires_parent_approval: bool,
    /// Both parents must approve (stricter).
    pub requires_both_parents: bool,
}

impl Default for KidConnectionRules {
    fn default() -> Self {
        Self {
            both_verified_kids: true,
            requires_family_bond: true,
            requires_parent_approval: true,
            requires_both_parents: false,
        }
    }
}

/// A request for two kids to connect.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KidConnectionRequest {
    pub id: Uuid,
    pub initiator_pubkey: String,
    pub target_pubkey: String,
    pub initiator_parent_pubkey: String,
    pub target_parent_pubkey: String,
    pub family_bond_id: Uuid,
    pub parent_approvals: Vec<ParentApproval>,
    pub status: KidConnectionStatus,
    pub requested_at: DateTime<Utc>,
}

impl KidConnectionRequest {
    pub fn new(
        initiator: impl Into<String>,
        target: impl Into<String>,
        initiator_parent: impl Into<String>,
        target_parent: impl Into<String>,
        family_bond_id: Uuid,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            initiator_pubkey: initiator.into(),
            target_pubkey: target.into(),
            initiator_parent_pubkey: initiator_parent.into(),
            target_parent_pubkey: target_parent.into(),
            family_bond_id,
            parent_approvals: Vec::new(),
            status: KidConnectionStatus::Pending,
            requested_at: Utc::now(),
        }
    }

    pub fn add_approval(&mut self, approval: ParentApproval) {
        if !self.has_approval_from(&approval.parent_pubkey) {
            self.parent_approvals.push(approval);
        }
    }

    pub fn has_approval_from(&self, pubkey: &str) -> bool {
        self.parent_approvals.iter().any(|a| a.parent_pubkey == pubkey)
    }

    pub fn is_approved(&self, require_both: bool) -> bool {
        if require_both {
            self.parent_approvals.len() >= 2
        } else {
            !self.parent_approvals.is_empty()
        }
    }

    pub fn approve(&mut self, rules: &KidConnectionRules) {
        if self.is_approved(rules.requires_both_parents) {
            self.status = KidConnectionStatus::Approved;
        }
    }

    pub fn deny(&mut self) {
        self.status = KidConnectionStatus::Denied;
    }
}

/// A parent's approval for a kid connection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParentApproval {
    pub parent_pubkey: String,
    pub approved_at: DateTime<Utc>,
    pub note: Option<String>,
}

/// Lifecycle of a kid connection request.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum KidConnectionStatus {
    Pending,
    Approved,
    Denied,
    Expired,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verification::proximity::ProximityProof;

    fn valid_proof() -> ProximityProof {
        ProximityProof::new("test_nonce").with_ble(-45)
    }

    fn invalid_proof() -> ProximityProof {
        ProximityProof::new("test_nonce") // no evidence
    }

    #[test]
    fn family_bond_requires_proximity() {
        let result = FamilyBond::new("parent_a", "parent_b", valid_proof());
        assert!(result.is_ok());

        let result = FamilyBond::new("parent_a", "parent_b", invalid_proof());
        assert!(result.is_err());
        assert!(matches!(
            result,
            Err(crate::BulwarkError::FamilyBondRequiresProximity)
        ));
    }

    #[test]
    fn kid_connection_requires_approval() {
        let bond = FamilyBond::new("parent_a", "parent_b", valid_proof()).unwrap();
        let mut request = KidConnectionRequest::new(
            "kid_a",
            "kid_b",
            "parent_a",
            "parent_b",
            bond.id,
        );
        let rules = KidConnectionRules::default();

        assert_eq!(request.status, KidConnectionStatus::Pending);
        assert!(!request.is_approved(false));

        // One parent approves
        request.add_approval(ParentApproval {
            parent_pubkey: "parent_a".into(),
            approved_at: Utc::now(),
            note: None,
        });
        assert!(request.is_approved(false)); // one parent sufficient (default)
        assert!(!request.is_approved(true)); // both required if strict

        request.approve(&rules);
        assert_eq!(request.status, KidConnectionStatus::Approved);
    }

    #[test]
    fn both_parents_mode() {
        let bond = FamilyBond::new("parent_a", "parent_b", valid_proof()).unwrap();
        let mut request = KidConnectionRequest::new(
            "kid_a",
            "kid_b",
            "parent_a",
            "parent_b",
            bond.id,
        );
        let rules = KidConnectionRules {
            requires_both_parents: true,
            ..Default::default()
        };

        request.add_approval(ParentApproval {
            parent_pubkey: "parent_a".into(),
            approved_at: Utc::now(),
            note: None,
        });
        request.approve(&rules);
        assert_eq!(request.status, KidConnectionStatus::Pending); // not enough

        request.add_approval(ParentApproval {
            parent_pubkey: "parent_b".into(),
            approved_at: Utc::now(),
            note: Some("Approved".into()),
        });
        request.approve(&rules);
        assert_eq!(request.status, KidConnectionStatus::Approved);
    }

    #[test]
    fn deny_connection() {
        let bond = FamilyBond::new("parent_a", "parent_b", valid_proof()).unwrap();
        let mut request = KidConnectionRequest::new(
            "kid_a",
            "kid_b",
            "parent_a",
            "parent_b",
            bond.id,
        );
        request.deny();
        assert_eq!(request.status, KidConnectionStatus::Denied);
    }

    #[test]
    fn default_rules() {
        let rules = KidConnectionRules::default();
        assert!(rules.both_verified_kids);
        assert!(rules.requires_family_bond);
        assert!(rules.requires_parent_approval);
        assert!(!rules.requires_both_parents); // one parent default
    }
}
