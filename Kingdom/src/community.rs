use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::charter::Charter;

/// A self-governing community — the fundamental unit of governance.
///
/// From Constellation Art. 1 §1: "All lawful governance under this Covenant
/// shall arise from communities."
///
/// From Constellation Art. 2 §2: "A community's standing shall originate from
/// within — affirmed by its members through lived relation, common intention,
/// and lawful conduct."
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Community {
    pub id: Uuid,
    pub name: String,
    pub basis: CommunityBasis,
    pub charter: Option<Charter>,
    pub members: Vec<CommunityMember>,
    pub founders: Vec<String>,
    pub status: CommunityStatus,
    pub founded_at: DateTime<Utc>,
    pub health_metadata: HashMap<String, String>,
}

impl Community {
    /// Create a new community in the Forming state, ready to accept founders.
    pub fn new(name: impl Into<String>, basis: CommunityBasis) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            basis,
            charter: None,
            members: Vec::new(),
            founders: Vec::new(),
            status: CommunityStatus::Forming,
            founded_at: Utc::now(),
            health_metadata: HashMap::new(),
        }
    }

    /// Attach a charter to this community (builder pattern).
    pub fn with_charter(mut self, charter: Charter) -> Self {
        self.charter = Some(charter);
        self
    }

    /// Add a founding member. Founders are automatically members with the Founder role.
    pub fn add_founder(&mut self, pubkey: impl Into<String>) {
        let pubkey = pubkey.into();
        if !self.is_member(&pubkey) {
            self.founders.push(pubkey.clone());
            self.members.push(CommunityMember {
                pubkey,
                role: CommunityRole::Founder,
                joined_at: Utc::now(),
                sponsor: None,
            });
        }
    }

    /// Add a member (non-founder).
    pub fn add_member(
        &mut self,
        pubkey: impl Into<String>,
        sponsor: Option<String>,
    ) -> Result<(), crate::KingdomError> {
        let pubkey = pubkey.into();
        if !self.is_active() {
            return Err(crate::KingdomError::CommunityNotActive(self.id.to_string()));
        }
        if self.is_member(&pubkey) {
            return Err(crate::KingdomError::AlreadyMember(pubkey));
        }
        self.members.push(CommunityMember {
            pubkey,
            role: CommunityRole::Newcomer,
            joined_at: Utc::now(),
            sponsor,
        });
        Ok(())
    }

    /// Remove a member. Founders can be removed too, but they remain in the founders list.
    pub fn remove_member(&mut self, pubkey: &str) -> Result<(), crate::KingdomError> {
        let pos = self
            .members
            .iter()
            .position(|m| m.pubkey == pubkey)
            .ok_or_else(|| crate::KingdomError::MemberNotFound(pubkey.into()))?;
        self.members.remove(pos);
        Ok(())
    }

    /// Update a member's role.
    pub fn update_member_role(
        &mut self,
        pubkey: &str,
        new_role: CommunityRole,
    ) -> Result<(), crate::KingdomError> {
        let member = self
            .members
            .iter_mut()
            .find(|m| m.pubkey == pubkey)
            .ok_or_else(|| crate::KingdomError::MemberNotFound(pubkey.into()))?;
        member.role = new_role;
        Ok(())
    }

    /// Whether this pubkey belongs to a current member.
    pub fn is_member(&self, pubkey: &str) -> bool {
        self.members.iter().any(|m| m.pubkey == pubkey)
    }

    /// Whether this pubkey is one of the original founders.
    pub fn is_founder(&self, pubkey: &str) -> bool {
        self.founders.iter().any(|f| f == pubkey)
    }

    /// Look up a member by their public key.
    pub fn member(&self, pubkey: &str) -> Option<&CommunityMember> {
        self.members.iter().find(|m| m.pubkey == pubkey)
    }

    /// Total number of current members (including founders).
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// All members who hold a specific role.
    pub fn members_with_role(&self, role: CommunityRole) -> Vec<&CommunityMember> {
        self.members.iter().filter(|m| m.role == role).collect()
    }

    /// All members with the Elder role.
    pub fn elders(&self) -> Vec<&CommunityMember> {
        self.members_with_role(CommunityRole::Elder)
    }

    /// All members with the Steward role.
    pub fn stewards(&self) -> Vec<&CommunityMember> {
        self.members_with_role(CommunityRole::Steward)
    }

    /// Whether the community is in the Active state.
    pub fn is_active(&self) -> bool {
        self.status == CommunityStatus::Active
    }

    /// Activate the community (transition from Forming to Active).
    pub fn activate(&mut self) -> Result<(), crate::KingdomError> {
        if self.status != CommunityStatus::Forming {
            return Err(crate::KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Active".into(),
            });
        }
        self.status = CommunityStatus::Active;
        Ok(())
    }

    /// Transition to dormant.
    pub fn go_dormant(&mut self) -> Result<(), crate::KingdomError> {
        if self.status != CommunityStatus::Active {
            return Err(crate::KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Dormant".into(),
            });
        }
        self.status = CommunityStatus::Dormant;
        Ok(())
    }

    /// Begin dissolution process.
    pub fn begin_dissolution(&mut self) -> Result<(), crate::KingdomError> {
        if matches!(
            self.status,
            CommunityStatus::Dissolved | CommunityStatus::Dissolving
        ) {
            return Err(crate::KingdomError::CommunityDissolved(self.id.to_string()));
        }
        self.status = CommunityStatus::Dissolving;
        Ok(())
    }

    /// Complete dissolution.
    pub fn dissolve(&mut self) -> Result<(), crate::KingdomError> {
        if self.status != CommunityStatus::Dissolving {
            return Err(crate::KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Dissolved".into(),
            });
        }
        self.status = CommunityStatus::Dissolved;
        Ok(())
    }

    /// Reawaken from dormant state.
    pub fn reactivate(&mut self) -> Result<(), crate::KingdomError> {
        if self.status != CommunityStatus::Dormant {
            return Err(crate::KingdomError::InvalidTransition {
                current: format!("{:?}", self.status),
                target: "Active".into(),
            });
        }
        self.status = CommunityStatus::Active;
        Ok(())
    }
}

/// Why the community formed — its organizing principle.
///
/// From Constellation Art. 2 §1: "Any collective of persons formed through place,
/// practice, kinship, culture, resistance, or mutual care shall hold lawful standing."
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CommunityBasis {
    /// Geographical — neighborhood, bioregion, watershed.
    Place,
    /// Shared interest or passion.
    Interest,
    /// Shared identity or cultural background.
    Identity,
    /// Shared profession or craft.
    Practice,
    /// Family or chosen family bonds.
    Kinship,
    /// Primarily digital — no geographic anchor.
    Digital,
    /// Mix of physical and digital.
    Hybrid,
}

/// A person participating in a community.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommunityMember {
    pub pubkey: String,
    pub role: CommunityRole,
    pub joined_at: DateTime<Utc>,
    pub sponsor: Option<String>,
}

impl CommunityMember {
    /// Trust level implied by role — higher means more established.
    pub fn trust_level(&self) -> u8 {
        match self.role {
            CommunityRole::Founder => 4,
            CommunityRole::Elder => 4,
            CommunityRole::Steward => 3,
            CommunityRole::Member => 2,
            CommunityRole::Newcomer => 1,
            CommunityRole::Observer => 0,
        }
    }
}

/// Role within a community.
///
/// From Constellation Art. 8 §6: "Strict single-term limits for all coordinating
/// positions, prohibition on consecutive service in similar roles."
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CommunityRole {
    /// Original creator of the community.
    Founder,
    /// Earned wisdom and long service.
    Elder,
    /// Currently serving in a governance/stewardship capacity.
    Steward,
    /// Full community member.
    Member,
    /// Recently joined, in probation or orientation.
    Newcomer,
    /// Can observe but not vote or propose.
    Observer,
}

/// Lifecycle of a community.
///
/// From Constellation Art. 2 §4: "Communities shall retain the lawful right to
/// evolve, dissolve, merge, or reform themselves."
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CommunityStatus {
    /// Being organized, not yet active.
    Forming,
    /// Fully operational.
    Active,
    /// Inactive but not dissolved.
    Dormant,
    /// In process of winding down.
    Dissolving,
    /// Formally ended.
    Dissolved,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_community() {
        let c = Community::new("Village Alpha", CommunityBasis::Place);
        assert_eq!(c.name, "Village Alpha");
        assert_eq!(c.basis, CommunityBasis::Place);
        assert_eq!(c.status, CommunityStatus::Forming);
        assert_eq!(c.member_count(), 0);
    }

    #[test]
    fn add_founders() {
        let mut c = Community::new("Test", CommunityBasis::Interest);
        c.add_founder("alice");
        c.add_founder("bob");
        c.add_founder("charlie");
        c.add_founder("alice"); // duplicate ignored

        assert_eq!(c.member_count(), 3);
        assert_eq!(c.founders.len(), 3);
        assert!(c.is_founder("alice"));
        assert!(c.is_member("alice"));
        assert_eq!(
            c.member("alice").unwrap().role,
            CommunityRole::Founder
        );
    }

    #[test]
    fn add_and_remove_members() {
        let mut c = Community::new("Test", CommunityBasis::Digital);
        c.add_founder("founder");
        c.activate().unwrap();

        c.add_member("alice", Some("founder".into())).unwrap();
        assert_eq!(c.member_count(), 2);
        assert_eq!(
            c.member("alice").unwrap().role,
            CommunityRole::Newcomer
        );
        assert_eq!(
            c.member("alice").unwrap().sponsor.as_deref(),
            Some("founder")
        );

        c.remove_member("alice").unwrap();
        assert_eq!(c.member_count(), 1);
        assert!(!c.is_member("alice"));
    }

    #[test]
    fn cannot_add_member_to_inactive_community() {
        let mut c = Community::new("Test", CommunityBasis::Digital);
        // Still in Forming status
        assert!(c.add_member("alice", None).is_err());
    }

    #[test]
    fn cannot_add_duplicate_member() {
        let mut c = Community::new("Test", CommunityBasis::Digital);
        c.add_founder("alice");
        c.activate().unwrap();

        assert!(c.add_member("alice", None).is_err());
    }

    #[test]
    fn update_member_role() {
        let mut c = Community::new("Test", CommunityBasis::Digital);
        c.add_founder("founder");
        c.activate().unwrap();
        c.add_member("alice", None).unwrap();

        c.update_member_role("alice", CommunityRole::Member).unwrap();
        assert_eq!(c.member("alice").unwrap().role, CommunityRole::Member);

        c.update_member_role("alice", CommunityRole::Steward).unwrap();
        assert_eq!(c.member("alice").unwrap().role, CommunityRole::Steward);
    }

    #[test]
    fn community_lifecycle() {
        let mut c = Community::new("Test", CommunityBasis::Place);
        assert_eq!(c.status, CommunityStatus::Forming);

        c.activate().unwrap();
        assert!(c.is_active());

        c.go_dormant().unwrap();
        assert_eq!(c.status, CommunityStatus::Dormant);

        c.reactivate().unwrap();
        assert!(c.is_active());

        c.begin_dissolution().unwrap();
        assert_eq!(c.status, CommunityStatus::Dissolving);

        c.dissolve().unwrap();
        assert_eq!(c.status, CommunityStatus::Dissolved);
    }

    #[test]
    fn invalid_transitions() {
        let mut c = Community::new("Test", CommunityBasis::Place);
        // Can't go dormant from Forming
        assert!(c.go_dormant().is_err());
        // Can't dissolve without dissolving first
        assert!(c.dissolve().is_err());

        c.activate().unwrap();
        // Can't activate twice
        assert!(c.activate().is_err());
    }

    #[test]
    fn dissolved_community_cannot_dissolve_again() {
        let mut c = Community::new("Test", CommunityBasis::Place);
        c.activate().unwrap();
        c.begin_dissolution().unwrap();
        c.dissolve().unwrap();
        assert!(c.begin_dissolution().is_err());
    }

    #[test]
    fn role_queries() {
        let mut c = Community::new("Test", CommunityBasis::Practice);
        c.add_founder("founder1");
        c.add_founder("founder2");
        c.activate().unwrap();
        c.add_member("alice", None).unwrap();
        c.update_member_role("alice", CommunityRole::Elder).unwrap();
        c.add_member("bob", None).unwrap();
        c.update_member_role("bob", CommunityRole::Steward).unwrap();

        assert_eq!(c.elders().len(), 1);
        assert_eq!(c.stewards().len(), 1);
        assert_eq!(c.members_with_role(CommunityRole::Founder).len(), 2);
    }

    #[test]
    fn member_trust_levels() {
        let founder = CommunityMember {
            pubkey: "a".into(),
            role: CommunityRole::Founder,
            joined_at: Utc::now(),
            sponsor: None,
        };
        let newcomer = CommunityMember {
            pubkey: "b".into(),
            role: CommunityRole::Newcomer,
            joined_at: Utc::now(),
            sponsor: None,
        };
        let observer = CommunityMember {
            pubkey: "c".into(),
            role: CommunityRole::Observer,
            joined_at: Utc::now(),
            sponsor: None,
        };

        assert!(founder.trust_level() > newcomer.trust_level());
        assert!(newcomer.trust_level() > observer.trust_level());
        assert_eq!(observer.trust_level(), 0);
    }

    #[test]
    fn community_serialization_roundtrip() {
        let mut c = Community::new("Roundtrip", CommunityBasis::Hybrid);
        c.add_founder("alice");
        c.health_metadata
            .insert("last_pulse".into(), "2026-03-03".into());

        let json = serde_json::to_string(&c).unwrap();
        let restored: Community = serde_json::from_str(&json).unwrap();
        assert_eq!(c.name, restored.name);
        assert_eq!(c.member_count(), restored.member_count());
        assert_eq!(c.health_metadata, restored.health_metadata);
    }
}
