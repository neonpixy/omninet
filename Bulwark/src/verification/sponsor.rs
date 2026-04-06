use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::age_tier::AgeTier;
use crate::trust::bond_depth::BondDepth;

/// Sponsorship rules — how established members bring families in.
///
/// Requires: Adult tier, 2+ years in network, Life bond depth, max 3 active, 90-day cooldown.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SponsorEligibility {
    pub minimum_tier: AgeTier,
    pub minimum_network_age_days: u32,
    pub required_bond_depth: BondDepth,
    pub max_active_sponsorships: u32,
    pub sponsorship_cooldown_days: u32,
}

impl Default for SponsorEligibility {
    fn default() -> Self {
        Self {
            minimum_tier: AgeTier::Adult,
            minimum_network_age_days: 730, // 2 years
            required_bond_depth: BondDepth::Life,
            max_active_sponsorships: 3,
            sponsorship_cooldown_days: 90,
        }
    }
}

/// An active sponsorship — sponsor brings a family into the network.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Sponsorship {
    pub id: Uuid,
    pub sponsor_pubkey: String,
    pub sponsored_members: Vec<SponsoredMember>,
    pub primary_contact_pubkey: String,
    pub sponsored_at: DateTime<Utc>,
    pub probation_ends_at: DateTime<Utc>,
    pub status: SponsorshipStatus,
}

impl Sponsorship {
    pub fn new(
        sponsor_pubkey: impl Into<String>,
        primary_contact: impl Into<String>,
        probation_days: u32,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            sponsor_pubkey: sponsor_pubkey.into(),
            sponsored_members: Vec::new(),
            primary_contact_pubkey: primary_contact.into(),
            sponsored_at: Utc::now(),
            probation_ends_at: Utc::now() + chrono::Duration::days(i64::from(probation_days)),
            status: SponsorshipStatus::Active,
        }
    }

    pub fn add_member(&mut self, member: SponsoredMember) {
        self.sponsored_members.push(member);
    }

    pub fn is_probation_complete(&self) -> bool {
        Utc::now() >= self.probation_ends_at
    }

    pub fn adults(&self) -> Vec<&SponsoredMember> {
        self.sponsored_members
            .iter()
            .filter(|m| m.tier.can_access_adult_sphere())
            .collect()
    }

    pub fn children(&self) -> Vec<&SponsoredMember> {
        self.sponsored_members
            .iter()
            .filter(|m| m.tier.is_in_kids_sphere())
            .collect()
    }
}

/// A sponsored family member.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SponsoredMember {
    pub pubkey: String,
    pub tier: AgeTier,
    pub relationship: Option<FamilyRelationship>,
}

/// Family relationship type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum FamilyRelationship {
    Spouse,
    Partner,
    Child,
    Parent,
    Sibling,
    Grandparent,
    Grandchild,
    Other,
}

/// Lifecycle of a sponsorship.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum SponsorshipStatus {
    /// On probation (1 year default).
    Active,
    /// Probation completed successfully.
    Completed,
    /// Revoked due to violations.
    Revoked,
    /// Under review for issues.
    UnderReview,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sponsor_eligibility_defaults() {
        let elig = SponsorEligibility::default();
        assert_eq!(elig.minimum_tier, AgeTier::Adult);
        assert_eq!(elig.minimum_network_age_days, 730);
        assert_eq!(elig.required_bond_depth, BondDepth::Life);
        assert_eq!(elig.max_active_sponsorships, 3);
    }

    #[test]
    fn sponsorship_creation() {
        let mut s = Sponsorship::new("sponsor_alice", "parent_bob", 365);
        s.add_member(SponsoredMember {
            pubkey: "parent_bob".into(),
            tier: AgeTier::Adult,
            relationship: Some(FamilyRelationship::Parent),
        });
        s.add_member(SponsoredMember {
            pubkey: "kid_charlie".into(),
            tier: AgeTier::Kid,
            relationship: Some(FamilyRelationship::Child),
        });

        assert_eq!(s.adults().len(), 1);
        assert_eq!(s.children().len(), 1);
        assert!(!s.is_probation_complete());
    }

    #[test]
    fn sponsorship_status_lifecycle() {
        let s = Sponsorship::new("alice", "bob", 365);
        assert_eq!(s.status, SponsorshipStatus::Active);
        assert!(!s.is_probation_complete());
    }
}
