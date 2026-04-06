//! Gospel tiering — controls propagation scope.
//!
//! Not all gospel events need to reach every node. Tiering lets nodes
//! choose how much gospel they carry based on their role and capacity.
//!
//! | Tier | Contents | Propagation |
//! |------|----------|-------------|
//! | Universal | Names, relay hints, lighthouse | Eager — every node |
//! | Community | Beacons, asset announcements | Eager within community peers |
//! | Extended | Future: full profiles, history | Pull-on-demand only |

use serde::{Deserialize, Serialize};

use crate::kind;

/// Controls how eagerly a gospel event propagates across the network.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GospelTier {
    /// Everyone gets it. Names, lighthouse announcements, relay hints.
    /// The directory layer — without it, the network can't route.
    Universal,
    /// Peers within a community. Beacons, asset announcements.
    /// Flows between nodes serving the same communities.
    Community,
    /// Pull-on-demand only. Full profiles, historical records.
    /// Never eagerly pushed — fetched when a client asks for it.
    Extended,
}

impl GospelTier {
    /// All tiers in propagation order.
    pub fn all() -> Vec<Self> {
        vec![Self::Universal, Self::Community, Self::Extended]
    }
}

/// Determine the tier for a given gospel kind.
///
/// Non-gospel kinds return `Extended` (they won't match any gospel filter
/// anyway, but this gives a safe default).
pub fn gospel_tier(kind: u32) -> GospelTier {
    match kind {
        // Universal: the directory (everyone needs names and routing info)
        kind::NAME_CLAIM
        | kind::NAME_UPDATE
        | kind::NAME_TRANSFER
        | kind::NAME_DELEGATE
        | kind::NAME_REVOKE
        | kind::NAME_RENEWAL
        | kind::RELAY_HINT
        | kind::LIGHTHOUSE_ANNOUNCE
        | kind::SEMANTIC_PROFILE => GospelTier::Universal,

        // Community: discovery and content (peers within community)
        kind::BEACON | kind::BEACON_UPDATE | kind::ASSET_ANNOUNCE => GospelTier::Community,

        // Everything else: pull-on-demand
        _ => GospelTier::Extended,
    }
}

/// Get the gospel kinds that belong to the given tiers.
///
/// Only considers kinds in `GOSPEL_REGISTRY_KINDS` — non-gospel kinds
/// are never included regardless of tier.
pub fn kinds_for_tiers(tiers: &[GospelTier]) -> Vec<u32> {
    kind::GOSPEL_REGISTRY_KINDS
        .iter()
        .filter(|&&k| tiers.contains(&gospel_tier(k)))
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn universal_tier_includes_names_and_routing() {
        assert_eq!(gospel_tier(kind::NAME_CLAIM), GospelTier::Universal);
        assert_eq!(gospel_tier(kind::NAME_UPDATE), GospelTier::Universal);
        assert_eq!(gospel_tier(kind::NAME_TRANSFER), GospelTier::Universal);
        assert_eq!(gospel_tier(kind::NAME_DELEGATE), GospelTier::Universal);
        assert_eq!(gospel_tier(kind::NAME_REVOKE), GospelTier::Universal);
        assert_eq!(gospel_tier(kind::NAME_RENEWAL), GospelTier::Universal);
        assert_eq!(gospel_tier(kind::RELAY_HINT), GospelTier::Universal);
        assert_eq!(gospel_tier(kind::LIGHTHOUSE_ANNOUNCE), GospelTier::Universal);
        assert_eq!(gospel_tier(kind::SEMANTIC_PROFILE), GospelTier::Universal);
    }

    #[test]
    fn community_tier_includes_beacons_and_assets() {
        assert_eq!(gospel_tier(kind::BEACON), GospelTier::Community);
        assert_eq!(gospel_tier(kind::BEACON_UPDATE), GospelTier::Community);
        assert_eq!(gospel_tier(kind::ASSET_ANNOUNCE), GospelTier::Community);
    }

    #[test]
    fn non_gospel_kinds_are_extended() {
        assert_eq!(gospel_tier(kind::PROFILE), GospelTier::Extended);
        assert_eq!(gospel_tier(kind::TEXT_NOTE), GospelTier::Extended);
        assert_eq!(gospel_tier(42), GospelTier::Extended);
    }

    #[test]
    fn kinds_for_universal_only() {
        let kinds = kinds_for_tiers(&[GospelTier::Universal]);
        assert!(kinds.contains(&kind::NAME_CLAIM));
        assert!(kinds.contains(&kind::NAME_UPDATE));
        assert!(kinds.contains(&kind::NAME_TRANSFER));
        assert!(kinds.contains(&kind::NAME_DELEGATE));
        assert!(kinds.contains(&kind::NAME_REVOKE));
        assert!(kinds.contains(&kind::RELAY_HINT));
        assert!(kinds.contains(&kind::LIGHTHOUSE_ANNOUNCE));
        assert!(kinds.contains(&kind::SEMANTIC_PROFILE));
        // Community kinds excluded
        assert!(!kinds.contains(&kind::BEACON));
        assert!(!kinds.contains(&kind::BEACON_UPDATE));
        assert!(!kinds.contains(&kind::ASSET_ANNOUNCE));
    }

    #[test]
    fn kinds_for_universal_and_community() {
        let kinds = kinds_for_tiers(&[GospelTier::Universal, GospelTier::Community]);
        assert!(kinds.contains(&kind::NAME_CLAIM));
        assert!(kinds.contains(&kind::RELAY_HINT));
        assert!(kinds.contains(&kind::LIGHTHOUSE_ANNOUNCE));
        assert!(kinds.contains(&kind::BEACON));
        assert!(kinds.contains(&kind::BEACON_UPDATE));
        assert!(kinds.contains(&kind::ASSET_ANNOUNCE));
    }

    #[test]
    fn kinds_for_all_tiers_matches_gospel_registry() {
        let all = kinds_for_tiers(&GospelTier::all());
        assert_eq!(all.len(), kind::GOSPEL_REGISTRY_KINDS.len());
        for &k in kind::GOSPEL_REGISTRY_KINDS {
            assert!(all.contains(&k), "missing kind {k}");
        }
    }

    #[test]
    fn kinds_for_empty_tiers_is_empty() {
        let kinds = kinds_for_tiers(&[]);
        assert!(kinds.is_empty());
    }

    #[test]
    fn all_tiers_returns_three() {
        assert_eq!(GospelTier::all().len(), 3);
    }

    #[test]
    fn tier_serde_round_trip() {
        for tier in GospelTier::all() {
            let json = serde_json::to_string(&tier).unwrap();
            let loaded: GospelTier = serde_json::from_str(&json).unwrap();
            assert_eq!(tier, loaded);
        }
    }
}
