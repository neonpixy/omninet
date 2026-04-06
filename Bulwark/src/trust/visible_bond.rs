use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::bond_depth::BondDepth;

/// A visible trust bond between two people — asymmetric depth, public history.
///
/// Each party independently declares how deep they see the relationship.
/// The effective depth is the MINIMUM of both declarations — the more
/// conservative view governs capabilities.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VisibleBond {
    pub id: Uuid,
    pub party_a: String,
    pub party_b: String,
    pub depth_from_a: BondDepth,
    pub depth_from_b: BondDepth,
    pub formed_at: DateTime<Utc>,
    pub history: Vec<BondChange>,
}

impl VisibleBond {
    pub fn new(
        party_a: impl Into<String>,
        party_b: impl Into<String>,
        initial_depth: BondDepth,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            party_a: party_a.into(),
            party_b: party_b.into(),
            depth_from_a: initial_depth,
            depth_from_b: initial_depth,
            formed_at: Utc::now(),
            history: Vec::new(),
        }
    }

    /// Effective depth = min(A's view, B's view).
    pub fn effective_depth(&self) -> BondDepth {
        self.depth_from_a.min(self.depth_from_b)
    }

    /// Whether both parties see the same depth.
    pub fn is_mutual(&self) -> bool {
        self.depth_from_a == self.depth_from_b
    }

    /// Check if a pubkey is part of this bond.
    pub fn involves(&self, pubkey: &str) -> bool {
        self.party_a == pubkey || self.party_b == pubkey
    }

    /// Get the other party's pubkey.
    pub fn other_party(&self, pubkey: &str) -> Option<&str> {
        if self.party_a == pubkey {
            Some(&self.party_b)
        } else if self.party_b == pubkey {
            Some(&self.party_a)
        } else {
            None
        }
    }

    /// Get how a specific party sees the bond.
    pub fn depth_from(&self, pubkey: &str) -> Option<BondDepth> {
        if self.party_a == pubkey {
            Some(self.depth_from_a)
        } else if self.party_b == pubkey {
            Some(self.depth_from_b)
        } else {
            None
        }
    }

    /// Update one party's view of the bond depth.
    pub fn update_depth(&mut self, pubkey: &str, new_depth: BondDepth) {
        let previous = if self.party_a == pubkey {
            let prev = self.depth_from_a;
            self.depth_from_a = new_depth;
            prev
        } else if self.party_b == pubkey {
            let prev = self.depth_from_b;
            self.depth_from_b = new_depth;
            prev
        } else {
            return;
        };

        self.history.push(BondChange {
            changed_by: pubkey.into(),
            previous_depth: previous,
            new_depth,
            changed_at: Utc::now(),
            reason: None,
        });
    }
}

/// A recorded change in bond depth.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BondChange {
    pub changed_by: String,
    pub previous_depth: BondDepth,
    pub new_depth: BondDepth,
    pub changed_at: DateTime<Utc>,
    pub reason: Option<String>,
}

impl BondChange {
    pub fn is_increase(&self) -> bool {
        self.new_depth > self.previous_depth
    }

    pub fn is_decrease(&self) -> bool {
        self.new_depth < self.previous_depth
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asymmetric_effective_depth() {
        let mut bond = VisibleBond::new("alice", "bob", BondDepth::Friend);
        assert_eq!(bond.effective_depth(), BondDepth::Friend);
        assert!(bond.is_mutual());

        // Alice sees deeper, Bob stays at Friend
        bond.depth_from_a = BondDepth::Best;
        assert_eq!(bond.effective_depth(), BondDepth::Friend); // min wins
        assert!(!bond.is_mutual());
    }

    #[test]
    fn update_depth_with_history() {
        let mut bond = VisibleBond::new("alice", "bob", BondDepth::Casual);
        bond.update_depth("alice", BondDepth::Friend);

        assert_eq!(bond.depth_from_a, BondDepth::Friend);
        assert_eq!(bond.depth_from_b, BondDepth::Casual);
        assert_eq!(bond.history.len(), 1);
        assert!(bond.history[0].is_increase());
    }

    #[test]
    fn bond_decrease_recorded() {
        let mut bond = VisibleBond::new("alice", "bob", BondDepth::Life);
        bond.update_depth("bob", BondDepth::Acquaintance);

        assert_eq!(bond.history.len(), 1);
        assert!(bond.history[0].is_decrease());
        assert_eq!(bond.effective_depth(), BondDepth::Acquaintance);
    }

    #[test]
    fn involves_and_other_party() {
        let bond = VisibleBond::new("alice", "bob", BondDepth::Friend);
        assert!(bond.involves("alice"));
        assert!(bond.involves("bob"));
        assert!(!bond.involves("charlie"));

        assert_eq!(bond.other_party("alice"), Some("bob"));
        assert_eq!(bond.other_party("bob"), Some("alice"));
        assert_eq!(bond.other_party("charlie"), None);
    }

    #[test]
    fn depth_from_specific_party() {
        let mut bond = VisibleBond::new("alice", "bob", BondDepth::Casual);
        bond.depth_from_a = BondDepth::Best;
        bond.depth_from_b = BondDepth::Friend;

        assert_eq!(bond.depth_from("alice"), Some(BondDepth::Best));
        assert_eq!(bond.depth_from("bob"), Some(BondDepth::Friend));
        assert_eq!(bond.depth_from("charlie"), None);
    }
}
