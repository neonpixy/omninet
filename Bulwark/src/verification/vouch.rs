use serde::{Deserialize, Serialize};

use crate::age_tier::AgeTier;
use crate::trust::bond_depth::BondDepth;

/// Rules for vouching — stricter for younger tiers.
///
/// Minors require 3 vouches (not 2), parent must initiate,
/// vouchers must be from diverse sources (anti-gaming).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VouchRules {
    pub for_adult: VouchRule,
    pub for_young_adult: VouchRule,
    pub for_minor: VouchRule,
}

impl Default for VouchRules {
    fn default() -> Self {
        Self {
            for_adult: VouchRule {
                voucher_minimum_tier: AgeTier::Adult,
                voucher_minimum_network_age_days: 365,
                required_bond_depth: BondDepth::Friend,
                required_vouch_count: 2,
                requires_parent: false,
                requires_diverse_vouchers: false,
            },
            for_young_adult: VouchRule {
                voucher_minimum_tier: AgeTier::Adult,
                voucher_minimum_network_age_days: 365,
                required_bond_depth: BondDepth::Best,
                required_vouch_count: 2,
                requires_parent: false,
                requires_diverse_vouchers: false,
            },
            for_minor: VouchRule {
                voucher_minimum_tier: AgeTier::Adult,
                voucher_minimum_network_age_days: 365,
                required_bond_depth: BondDepth::Best,
                required_vouch_count: 3,
                requires_parent: true,
                requires_diverse_vouchers: true,
            },
        }
    }
}

impl VouchRules {
    pub fn rule_for(&self, tier: AgeTier) -> &VouchRule {
        match tier {
            AgeTier::Adult => &self.for_adult,
            AgeTier::YoungAdult => &self.for_young_adult,
            AgeTier::Kid | AgeTier::Teen => &self.for_minor,
        }
    }
}

/// Requirements for a single vouch category.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VouchRule {
    pub voucher_minimum_tier: AgeTier,
    pub voucher_minimum_network_age_days: u32,
    pub required_bond_depth: BondDepth,
    pub required_vouch_count: u32,
    pub requires_parent: bool,
    pub requires_diverse_vouchers: bool,
}

/// A mutual vouch between two people.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MutualVouch {
    pub voucher_pubkey: String,
    pub vouchee_pubkey: String,
    pub bond_depth: BondDepth,
    pub voucher_tier: AgeTier,
    pub voucher_network_age_days: u32,
}

/// Check if a vouch is eligible.
#[derive(Debug, Clone, PartialEq)]
pub enum VouchEligibility {
    Eligible,
    Ineligible(Vec<VouchIneligibilityReason>),
}

/// Why a vouch can't be accepted.
#[derive(Debug, Clone, PartialEq)]
pub enum VouchIneligibilityReason {
    TierTooLow { have: AgeTier, need: AgeTier },
    NetworkAgeTooLow { have_days: u32, need_days: u32 },
    BondDepthInsufficient { have: BondDepth, need: BondDepth },
    DiversityNotMet,
}

impl MutualVouch {
    /// Check if this vouch meets the requirements for a given vouchee tier.
    pub fn check_eligibility(&self, rule: &VouchRule) -> VouchEligibility {
        let mut reasons = Vec::new();

        if self.voucher_tier < rule.voucher_minimum_tier {
            reasons.push(VouchIneligibilityReason::TierTooLow {
                have: self.voucher_tier,
                need: rule.voucher_minimum_tier,
            });
        }

        if self.voucher_network_age_days < rule.voucher_minimum_network_age_days {
            reasons.push(VouchIneligibilityReason::NetworkAgeTooLow {
                have_days: self.voucher_network_age_days,
                need_days: rule.voucher_minimum_network_age_days,
            });
        }

        if self.bond_depth < rule.required_bond_depth {
            reasons.push(VouchIneligibilityReason::BondDepthInsufficient {
                have: self.bond_depth,
                need: rule.required_bond_depth,
            });
        }

        if reasons.is_empty() {
            VouchEligibility::Eligible
        } else {
            VouchEligibility::Ineligible(reasons)
        }
    }
}

/// Diversity check for minor vouching — prevents gaming.
///
/// Out of 3 required vouches: max 2 from same collective,
/// at least 1 from outside parent's primary collectives.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VouchDiversityCheck {
    pub max_from_same_collective: u32,
    pub require_outside_voucher: bool,
    pub max_shared_vouch_ancestor: u32,
}

impl Default for VouchDiversityCheck {
    fn default() -> Self {
        Self {
            max_from_same_collective: 2,
            require_outside_voucher: true,
            max_shared_vouch_ancestor: 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adult_vouch_eligible() {
        let rules = VouchRules::default();
        let vouch = MutualVouch {
            voucher_pubkey: "alice".into(),
            vouchee_pubkey: "bob".into(),
            bond_depth: BondDepth::Friend,
            voucher_tier: AgeTier::Adult,
            voucher_network_age_days: 400,
        };
        assert_eq!(
            vouch.check_eligibility(rules.rule_for(AgeTier::Adult)),
            VouchEligibility::Eligible
        );
    }

    #[test]
    fn adult_vouch_tier_too_low() {
        let rules = VouchRules::default();
        let vouch = MutualVouch {
            voucher_pubkey: "alice".into(),
            vouchee_pubkey: "bob".into(),
            bond_depth: BondDepth::Friend,
            voucher_tier: AgeTier::YoungAdult, // need Adult
            voucher_network_age_days: 400,
        };
        assert!(matches!(
            vouch.check_eligibility(rules.rule_for(AgeTier::Adult)),
            VouchEligibility::Ineligible(_)
        ));
    }

    #[test]
    fn minor_requires_best_depth() {
        let rules = VouchRules::default();
        let vouch = MutualVouch {
            voucher_pubkey: "alice".into(),
            vouchee_pubkey: "kid".into(),
            bond_depth: BondDepth::Friend, // need Best for minors
            voucher_tier: AgeTier::Adult,
            voucher_network_age_days: 400,
        };
        assert!(matches!(
            vouch.check_eligibility(rules.rule_for(AgeTier::Kid)),
            VouchEligibility::Ineligible(_)
        ));
    }

    #[test]
    fn minor_vouch_count_is_three() {
        let rules = VouchRules::default();
        assert_eq!(rules.for_minor.required_vouch_count, 3);
        assert_eq!(rules.for_adult.required_vouch_count, 2);
    }

    #[test]
    fn minor_requires_parent_and_diversity() {
        let rules = VouchRules::default();
        assert!(rules.for_minor.requires_parent);
        assert!(rules.for_minor.requires_diverse_vouchers);
        assert!(!rules.for_adult.requires_parent);
    }

    #[test]
    fn diversity_defaults() {
        let check = VouchDiversityCheck::default();
        assert_eq!(check.max_from_same_collective, 2);
        assert!(check.require_outside_voucher);
    }
}
