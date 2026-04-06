use serde::{Deserialize, Serialize};

/// Stratified trust layers — progressive access based on verification depth.
///
/// Connected → Verified → Vouched → Shielded
/// Each layer unlocks cumulative capabilities. Must progress sequentially.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TrustLayer {
    /// L1: Open network join, no bonds required. Can message peers.
    Connected,
    /// L2: Verified through at least one method. Checkmark, UBI eligible.
    Verified,
    /// L3: Multiple vouches from trusted members. Can vouch and sponsor.
    Vouched,
    /// L4: Adults approved for Kids Sphere access. Highest trust.
    Shielded,
}

impl TrustLayer {
    /// What this layer allows.
    pub fn capabilities(&self) -> LayerCapabilities {
        match self {
            TrustLayer::Connected => LayerCapabilities {
                can_message_peers: true,
                has_checkmark: false,
                can_receive_vouches: false,
                can_vouch_for_others: false,
                ubi_eligible: false,
                kids_sphere_access: false,
                can_sponsor: false,
            },
            TrustLayer::Verified => LayerCapabilities {
                can_message_peers: true,
                has_checkmark: true,
                can_receive_vouches: true,
                can_vouch_for_others: false,
                ubi_eligible: true,
                kids_sphere_access: false,
                can_sponsor: false,
            },
            TrustLayer::Vouched => LayerCapabilities {
                can_message_peers: true,
                has_checkmark: true,
                can_receive_vouches: true,
                can_vouch_for_others: true,
                ubi_eligible: true,
                kids_sphere_access: false,
                can_sponsor: true,
            },
            TrustLayer::Shielded => LayerCapabilities {
                can_message_peers: true,
                has_checkmark: true,
                can_receive_vouches: true,
                can_vouch_for_others: true,
                ubi_eligible: true,
                kids_sphere_access: true,
                can_sponsor: true,
            },
        }
    }

    /// The next layer up (if any).
    pub fn next(&self) -> Option<TrustLayer> {
        match self {
            TrustLayer::Connected => Some(TrustLayer::Verified),
            TrustLayer::Verified => Some(TrustLayer::Vouched),
            TrustLayer::Vouched => Some(TrustLayer::Shielded),
            TrustLayer::Shielded => None,
        }
    }
}

/// Cumulative capabilities at a trust layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LayerCapabilities {
    pub can_message_peers: bool,
    pub has_checkmark: bool,
    pub can_receive_vouches: bool,
    pub can_vouch_for_others: bool,
    pub ubi_eligible: bool,
    pub kids_sphere_access: bool,
    pub can_sponsor: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_ordering() {
        assert!(TrustLayer::Connected < TrustLayer::Verified);
        assert!(TrustLayer::Verified < TrustLayer::Vouched);
        assert!(TrustLayer::Vouched < TrustLayer::Shielded);
    }

    #[test]
    fn capabilities_are_cumulative() {
        let l1 = TrustLayer::Connected.capabilities();
        let l2 = TrustLayer::Verified.capabilities();
        let l3 = TrustLayer::Vouched.capabilities();
        let l4 = TrustLayer::Shielded.capabilities();

        // All can message
        assert!(l1.can_message_peers);

        // Checkmark from L2+
        assert!(!l1.has_checkmark);
        assert!(l2.has_checkmark);

        // UBI from L2+
        assert!(!l1.ubi_eligible);
        assert!(l2.ubi_eligible);

        // Vouch from L3+
        assert!(!l2.can_vouch_for_others);
        assert!(l3.can_vouch_for_others);

        // Kids Sphere only L4
        assert!(!l3.kids_sphere_access);
        assert!(l4.kids_sphere_access);
    }

    #[test]
    fn next_layer() {
        assert_eq!(TrustLayer::Connected.next(), Some(TrustLayer::Verified));
        assert_eq!(TrustLayer::Verified.next(), Some(TrustLayer::Vouched));
        assert_eq!(TrustLayer::Vouched.next(), Some(TrustLayer::Shielded));
        assert_eq!(TrustLayer::Shielded.next(), None);
    }
}
