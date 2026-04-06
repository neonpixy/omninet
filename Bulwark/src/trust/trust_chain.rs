use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::bond_depth::BondDepth;

/// A person's provenance in the trust network — how they entered and who vouched.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrustChain {
    pub id: Uuid,
    pub pubkey: String,
    pub entry_method: EntryMethod,
    pub joined_at: DateTime<Utc>,
    pub vouchers: Vec<VouchRecord>,
    pub sponsor: Option<SponsorRecord>,
    /// Distance from origin (0 = origin, 1 = directly vouched by origin, etc).
    pub depth: u32,
}

impl TrustChain {
    pub fn origin(pubkey: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            pubkey: pubkey.into(),
            entry_method: EntryMethod::Origin,
            joined_at: Utc::now(),
            vouchers: Vec::new(),
            sponsor: None,
            depth: 0,
        }
    }

    pub fn connected(pubkey: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            pubkey: pubkey.into(),
            entry_method: EntryMethod::Connected,
            joined_at: Utc::now(),
            vouchers: Vec::new(),
            sponsor: None,
            depth: u32::MAX, // not in chain yet
        }
    }

    pub fn sponsored(pubkey: impl Into<String>, sponsor: SponsorRecord) -> Self {
        let depth = sponsor.sponsor_depth + 1;
        Self {
            id: Uuid::new_v4(),
            pubkey: pubkey.into(),
            entry_method: EntryMethod::Sponsored,
            joined_at: Utc::now(),
            vouchers: Vec::new(),
            sponsor: Some(sponsor),
            depth,
        }
    }

    pub fn vouched(pubkey: impl Into<String>, vouchers: Vec<VouchRecord>) -> Self {
        let depth = vouchers
            .iter()
            .map(|v| v.voucher_depth + 1)
            .min()
            .unwrap_or(u32::MAX);
        Self {
            id: Uuid::new_v4(),
            pubkey: pubkey.into(),
            entry_method: EntryMethod::Vouched,
            joined_at: Utc::now(),
            vouchers,
            sponsor: None,
            depth,
        }
    }

    pub fn is_in_chain(&self) -> bool {
        self.depth < u32::MAX
    }

    pub fn is_on_probation(&self) -> bool {
        self.sponsor
            .as_ref()
            .is_some_and(|s| !s.is_probation_complete())
    }
}

/// How someone entered the trust network.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EntryMethod {
    /// Root of trust (the origin account).
    Origin,
    /// Layer 1 open join (no bond required).
    Connected,
    /// Brought in by a sponsor (family fast-track).
    Sponsored,
    /// Vouched in by trusted individuals.
    Vouched,
}

/// A record of someone vouching for another.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VouchRecord {
    pub voucher_pubkey: String,
    pub bond_depth_at_vouch: BondDepth,
    pub vouched_at: DateTime<Utc>,
    pub voucher_depth: u32,
}

/// A record of sponsorship.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SponsorRecord {
    pub sponsor_pubkey: String,
    pub bond_depth_at_sponsorship: BondDepth,
    pub sponsored_at: DateTime<Utc>,
    pub probation_ends_at: DateTime<Utc>,
    pub sponsor_depth: u32,
}

impl SponsorRecord {
    pub fn new(
        sponsor_pubkey: impl Into<String>,
        bond_depth: BondDepth,
        sponsor_depth: u32,
        probation_days: u32,
    ) -> Self {
        Self {
            sponsor_pubkey: sponsor_pubkey.into(),
            bond_depth_at_sponsorship: bond_depth,
            sponsored_at: Utc::now(),
            probation_ends_at: Utc::now() + chrono::Duration::days(i64::from(probation_days)),
            sponsor_depth,
        }
    }

    pub fn is_probation_complete(&self) -> bool {
        Utc::now() >= self.probation_ends_at
    }

    pub fn probation_days_remaining(&self) -> i64 {
        let diff = self.probation_ends_at - Utc::now();
        diff.num_days().max(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_chain() {
        let chain = TrustChain::origin("sam");
        assert_eq!(chain.depth, 0);
        assert!(chain.is_in_chain());
        assert_eq!(chain.entry_method, EntryMethod::Origin);
    }

    #[test]
    fn connected_not_in_chain() {
        let chain = TrustChain::connected("alice");
        assert!(!chain.is_in_chain());
        assert_eq!(chain.entry_method, EntryMethod::Connected);
    }

    #[test]
    fn vouched_depth_from_vouchers() {
        let vouchers = vec![
            VouchRecord {
                voucher_pubkey: "a".into(),
                bond_depth_at_vouch: BondDepth::Friend,
                vouched_at: Utc::now(),
                voucher_depth: 1,
            },
            VouchRecord {
                voucher_pubkey: "b".into(),
                bond_depth_at_vouch: BondDepth::Friend,
                vouched_at: Utc::now(),
                voucher_depth: 3,
            },
        ];
        let chain = TrustChain::vouched("alice", vouchers);
        assert_eq!(chain.depth, 2); // min(1,3) + 1
        assert!(chain.is_in_chain());
    }

    #[test]
    fn sponsored_depth_from_sponsor() {
        let sponsor = SponsorRecord::new("bob", BondDepth::Life, 1, 365);
        let chain = TrustChain::sponsored("alice", sponsor);
        assert_eq!(chain.depth, 2); // sponsor_depth(1) + 1
    }

    #[test]
    fn probation_tracking() {
        let sponsor = SponsorRecord::new("bob", BondDepth::Life, 0, 365);
        let chain = TrustChain::sponsored("alice", sponsor);
        assert!(chain.is_on_probation());
        assert!(chain.sponsor.as_ref().unwrap().probation_days_remaining() > 360);
    }
}
