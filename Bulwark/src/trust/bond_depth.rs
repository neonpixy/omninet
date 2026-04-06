use serde::{Deserialize, Serialize};

/// How deep a trust bond goes — from casual acquaintance to life bond.
///
/// From the quarry's MasterShield: "Trust flows through people, not systems."
/// Each depth unlocks additional capabilities.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BondDepth {
    /// We've met.
    Casual,
    /// We know each other.
    Acquaintance,
    /// I trust them.
    Friend,
    /// I trust them deeply.
    Best,
    /// I trust them with my life.
    Life,
}

impl BondDepth {
    /// What capabilities this bond depth unlocks.
    pub fn capabilities(&self) -> BondCapabilities {
        match self {
            BondDepth::Casual => BondCapabilities {
                can_message: true,
                can_see_public: true,
                can_invite_to_collective: false,
                can_vouch_adult: false,
                can_vouch_young_adult: false,
                can_vouch_minor: false,
                can_sponsor_family: false,
                can_be_emergency_contact: false,
            },
            BondDepth::Acquaintance => BondCapabilities {
                can_message: true,
                can_see_public: true,
                can_invite_to_collective: true,
                can_vouch_adult: false,
                can_vouch_young_adult: false,
                can_vouch_minor: false,
                can_sponsor_family: false,
                can_be_emergency_contact: false,
            },
            BondDepth::Friend => BondCapabilities {
                can_message: true,
                can_see_public: true,
                can_invite_to_collective: true,
                can_vouch_adult: true,
                can_vouch_young_adult: false,
                can_vouch_minor: false,
                can_sponsor_family: false,
                can_be_emergency_contact: false,
            },
            BondDepth::Best => BondCapabilities {
                can_message: true,
                can_see_public: true,
                can_invite_to_collective: true,
                can_vouch_adult: true,
                can_vouch_young_adult: true,
                can_vouch_minor: true,
                can_sponsor_family: false,
                can_be_emergency_contact: false,
            },
            BondDepth::Life => BondCapabilities {
                can_message: true,
                can_see_public: true,
                can_invite_to_collective: true,
                can_vouch_adult: true,
                can_vouch_young_adult: true,
                can_vouch_minor: true,
                can_sponsor_family: true,
                can_be_emergency_contact: true,
            },
        }
    }
}

/// What a bond at a given depth allows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BondCapabilities {
    pub can_message: bool,
    pub can_see_public: bool,
    pub can_invite_to_collective: bool,
    pub can_vouch_adult: bool,
    pub can_vouch_young_adult: bool,
    pub can_vouch_minor: bool,
    pub can_sponsor_family: bool,
    pub can_be_emergency_contact: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depth_ordering() {
        assert!(BondDepth::Casual < BondDepth::Acquaintance);
        assert!(BondDepth::Acquaintance < BondDepth::Friend);
        assert!(BondDepth::Friend < BondDepth::Best);
        assert!(BondDepth::Best < BondDepth::Life);
    }

    #[test]
    fn capabilities_cumulative() {
        let casual = BondDepth::Casual.capabilities();
        let life = BondDepth::Life.capabilities();

        // Life has everything casual has plus more
        assert!(casual.can_message);
        assert!(life.can_message);
        assert!(!casual.can_sponsor_family);
        assert!(life.can_sponsor_family);
    }

    #[test]
    fn vouch_requires_friend_or_deeper() {
        assert!(!BondDepth::Casual.capabilities().can_vouch_adult);
        assert!(!BondDepth::Acquaintance.capabilities().can_vouch_adult);
        assert!(BondDepth::Friend.capabilities().can_vouch_adult);
        assert!(BondDepth::Best.capabilities().can_vouch_adult);
        assert!(BondDepth::Life.capabilities().can_vouch_adult);
    }

    #[test]
    fn minor_vouch_requires_best_or_deeper() {
        assert!(!BondDepth::Friend.capabilities().can_vouch_minor);
        assert!(BondDepth::Best.capabilities().can_vouch_minor);
        assert!(BondDepth::Life.capabilities().can_vouch_minor);
    }

    #[test]
    fn sponsor_requires_life() {
        assert!(!BondDepth::Best.capabilities().can_sponsor_family);
        assert!(BondDepth::Life.capabilities().can_sponsor_family);
    }
}
