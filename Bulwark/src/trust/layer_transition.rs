use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::age_tier::AgeTier;
use super::bond_depth::BondDepth;
use super::trust_layer::TrustLayer;

/// Requirements for advancing through trust layers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct LayerTransitionRequirements {
    pub to_verified: VerifiedRequirements,
    pub to_vouched: VouchedRequirements,
    pub to_shielded: ShieldedRequirements,
}

/// L1 → L2: At least one verification method passed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VerifiedRequirements {
    pub minimum_verifications: u32,
}

impl Default for VerifiedRequirements {
    fn default() -> Self {
        Self { minimum_verifications: 1 }
    }
}

/// L2 → L3: Multiple vouches from trusted members.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VouchedRequirements {
    pub minimum_vouches: u32,
    pub minimum_vouch_depth: BondDepth,
    pub minimum_voucher_layer: TrustLayer,
    pub minimum_voucher_network_age_days: u32,
}

impl Default for VouchedRequirements {
    fn default() -> Self {
        Self {
            minimum_vouches: 2,
            minimum_vouch_depth: BondDepth::Friend,
            minimum_voucher_layer: TrustLayer::Vouched,
            minimum_voucher_network_age_days: 180,
        }
    }
}

/// L3 → L4: Adults approved for Kids Sphere access.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ShieldedRequirements {
    pub requires_parent_vouch: bool,
    pub minimum_parent_bond_depth: BondDepth,
    pub minimum_network_age_days: u32,
    pub minimum_tier: AgeTier,
    pub requires_clean_standing: bool,
}

impl Default for ShieldedRequirements {
    fn default() -> Self {
        Self {
            requires_parent_vouch: true,
            minimum_parent_bond_depth: BondDepth::Best,
            minimum_network_age_days: 365,
            minimum_tier: AgeTier::Adult,
            requires_clean_standing: true,
        }
    }
}

/// A request to transition to a higher trust layer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LayerTransitionRequest {
    pub id: Uuid,
    pub pubkey: String,
    pub from_layer: TrustLayer,
    pub to_layer: TrustLayer,
    pub evidence: LayerTransitionEvidence,
    pub status: LayerTransitionStatus,
}

impl LayerTransitionRequest {
    pub fn new(pubkey: impl Into<String>, from: TrustLayer, to: TrustLayer) -> Self {
        Self {
            id: Uuid::new_v4(),
            pubkey: pubkey.into(),
            from_layer: from,
            to_layer: to,
            evidence: LayerTransitionEvidence::default(),
            status: LayerTransitionStatus::Pending,
        }
    }
}

/// Evidence supporting a transition request.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct LayerTransitionEvidence {
    pub verification_ids: Vec<Uuid>,
    pub vouch_ids: Vec<Uuid>,
    pub parent_vouch_pubkey: Option<String>,
}

/// Lifecycle of a transition request.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum LayerTransitionStatus {
    Pending,
    Approved,
    Denied,
    Expired,
}

/// Why a transition was blocked.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LayerTransitionBlocker {
    InsufficientVerifications { have: u32, need: u32 },
    InsufficientVouches { have: u32, need: u32 },
    VouchDepthInsufficient { have: BondDepth, need: BondDepth },
    VoucherLayerInsufficient { voucher: String, have: TrustLayer, need: TrustLayer },
    VoucherNetworkAgeTooLow { voucher: String, have_days: u32, need_days: u32 },
    MissingParentVouch,
    NetworkAgeTooLow { have_days: u32, need_days: u32 },
    TierTooLow { have: AgeTier, need: AgeTier },
    StandingNotClean(String),
    AlreadyAtLayer(TrustLayer),
    CannotSkipLayers { current: TrustLayer, requested: TrustLayer },
    MinorNotAuthorized,
}

/// Check eligibility for a transition.
pub fn check_transition(
    current: TrustLayer,
    target: TrustLayer,
    requirements: &LayerTransitionRequirements,
    evidence: &LayerTransitionEvidence,
) -> Result<(), Vec<LayerTransitionBlocker>> {
    let mut blockers = Vec::new();

    if let Some(next) = current.next() {
        if target > next {
            blockers.push(LayerTransitionBlocker::CannotSkipLayers {
                current,
                requested: target,
            });
            return Err(blockers);
        }
    }

    if current >= target {
        blockers.push(LayerTransitionBlocker::AlreadyAtLayer(current));
        return Err(blockers);
    }

    match target {
        TrustLayer::Connected => {}
        TrustLayer::Verified => {
            let req = &requirements.to_verified;
            let have = evidence.verification_ids.len() as u32;
            if have < req.minimum_verifications {
                blockers.push(LayerTransitionBlocker::InsufficientVerifications {
                    have,
                    need: req.minimum_verifications,
                });
            }
        }
        TrustLayer::Vouched => {
            let req = &requirements.to_vouched;
            let have = evidence.vouch_ids.len() as u32;
            if have < req.minimum_vouches {
                blockers.push(LayerTransitionBlocker::InsufficientVouches {
                    have,
                    need: req.minimum_vouches,
                });
            }
        }
        TrustLayer::Shielded => {
            let req = &requirements.to_shielded;
            if req.requires_parent_vouch && evidence.parent_vouch_pubkey.is_none() {
                blockers.push(LayerTransitionBlocker::MissingParentVouch);
            }
        }
    }

    if blockers.is_empty() {
        Ok(())
    } else {
        Err(blockers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_to_verified() {
        let reqs = LayerTransitionRequirements::default();
        let evidence = LayerTransitionEvidence {
            verification_ids: vec![Uuid::new_v4()],
            ..Default::default()
        };
        assert!(check_transition(TrustLayer::Connected, TrustLayer::Verified, &reqs, &evidence).is_ok());
    }

    #[test]
    fn transition_blocked_no_evidence() {
        let reqs = LayerTransitionRequirements::default();
        let evidence = LayerTransitionEvidence::default();
        assert!(check_transition(TrustLayer::Connected, TrustLayer::Verified, &reqs, &evidence).is_err());
    }

    #[test]
    fn cannot_skip_layers() {
        let reqs = LayerTransitionRequirements::default();
        let evidence = LayerTransitionEvidence::default();
        assert!(check_transition(TrustLayer::Connected, TrustLayer::Vouched, &reqs, &evidence).is_err());
    }

    #[test]
    fn shielded_needs_parent() {
        let reqs = LayerTransitionRequirements::default();
        let evidence = LayerTransitionEvidence::default();
        assert!(check_transition(TrustLayer::Vouched, TrustLayer::Shielded, &reqs, &evidence).is_err());

        let evidence = LayerTransitionEvidence {
            parent_vouch_pubkey: Some("parent".into()),
            ..Default::default()
        };
        assert!(check_transition(TrustLayer::Vouched, TrustLayer::Shielded, &reqs, &evidence).is_ok());
    }

    #[test]
    fn defaults_match_quarry() {
        let reqs = LayerTransitionRequirements::default();
        assert_eq!(reqs.to_verified.minimum_verifications, 1);
        assert_eq!(reqs.to_vouched.minimum_vouches, 2);
        assert_eq!(reqs.to_shielded.minimum_network_age_days, 365);
        assert_eq!(reqs.to_shielded.minimum_tier, AgeTier::Adult);
    }
}
