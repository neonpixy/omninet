use crate::protections::ProhibitionType;
use crate::rights::RightCategory;

/// The immutable foundations of the Covenant — the Core and Commons principles
/// that NO amendment process can touch. These are the bedrock.
///
/// From Covenant Continuum Art. 2: "The Core of this Covenant... shall remain fixed,
/// inviolable, and beyond the reach of procedural revision."
///
/// These are defined as code, not data. They cannot be loaded from config, edited
/// at runtime, or modified through any API. They simply ARE.
pub struct ImmutableFoundation;

impl ImmutableFoundation {
    /// The right categories that are immutable — they precede law.
    pub const IMMUTABLE_RIGHTS: &[RightCategory] = &[
        RightCategory::Dignity,
        RightCategory::Thought,
        RightCategory::Expression,
        RightCategory::LegalStanding,
        RightCategory::Safety,
        RightCategory::Privacy,
        RightCategory::Refusal,
        RightCategory::Earth,
        RightCategory::Community,
        RightCategory::Labor,
    ];

    /// The prohibition types that are absolute — they cannot be suspended.
    pub const ABSOLUTE_PROHIBITIONS: &[ProhibitionType] = &[
        ProhibitionType::Domination,
        ProhibitionType::Discrimination,
        ProhibitionType::Surveillance,
        ProhibitionType::Exploitation,
        ProhibitionType::Cruelty,
        ProhibitionType::Ecocide,
        ProhibitionType::IndustrialCruelty,
    ];

    /// The three axioms that underpin everything. Not data — truth.
    pub const AXIOMS: &[&str] = &[
        "Dignity: irreducible worth of every person, preceding all law",
        "Sovereignty: self-authored agency, the birthright to choose, refuse, and reshape",
        "Consent: voluntary, informed, continuous, and revocable alignment of will",
    ];

    /// Check whether a proposed change would violate the immutable foundations.
    pub fn would_violate(description: &str) -> bool {
        let lower = description.to_lowercase();
        let violation_signals = [
            "remove right",
            "revoke right",
            "suspend right",
            "modify core",
            "amend core",
            "privatize commons",
            "enclose commons",
            "commodify commons",
            "permit domination",
            "allow surveillance",
            "permit surveillance",
            "enable extraction",
            "suspend dignity",
            "override consent",
            "abolish protection",
            "weaken protection",
        ];
        violation_signals.iter().any(|signal| lower.contains(signal))
    }

    /// Validate that a right category is immutable.
    pub fn is_right_immutable(category: &RightCategory) -> bool {
        Self::IMMUTABLE_RIGHTS.contains(category)
    }

    /// Validate that a prohibition type is absolute.
    pub fn is_prohibition_absolute(prohibition: &ProhibitionType) -> bool {
        Self::ABSOLUTE_PROHIBITIONS.contains(prohibition)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_right_categories_except_union_are_immutable() {
        // Union rights are from Conjunction, not Core — they're mutable
        assert!(ImmutableFoundation::is_right_immutable(&RightCategory::Dignity));
        assert!(ImmutableFoundation::is_right_immutable(&RightCategory::Privacy));
        assert!(ImmutableFoundation::is_right_immutable(&RightCategory::Earth));
        assert!(ImmutableFoundation::is_right_immutable(&RightCategory::Community));
        assert!(!ImmutableFoundation::is_right_immutable(&RightCategory::Union));
    }

    #[test]
    fn systemic_breach_is_not_absolute_prohibition() {
        // SystemicBreach describes conditions, not a prohibition to enforce
        assert!(!ImmutableFoundation::is_prohibition_absolute(
            &ProhibitionType::SystemicBreach
        ));
    }

    #[test]
    fn all_core_prohibitions_are_absolute() {
        assert!(ImmutableFoundation::is_prohibition_absolute(&ProhibitionType::Domination));
        assert!(ImmutableFoundation::is_prohibition_absolute(&ProhibitionType::Surveillance));
        assert!(ImmutableFoundation::is_prohibition_absolute(&ProhibitionType::Cruelty));
        assert!(ImmutableFoundation::is_prohibition_absolute(&ProhibitionType::Ecocide));
    }

    #[test]
    fn would_violate_detects_threats() {
        assert!(ImmutableFoundation::would_violate("remove right to privacy"));
        assert!(ImmutableFoundation::would_violate("Privatize Commons lands"));
        assert!(ImmutableFoundation::would_violate("permit domination in emergency"));
        assert!(ImmutableFoundation::would_violate("suspend dignity during crisis"));
        assert!(ImmutableFoundation::would_violate("override consent for public good"));
    }

    #[test]
    fn would_violate_passes_clean_proposals() {
        assert!(!ImmutableFoundation::would_violate("add new community garden right"));
        assert!(!ImmutableFoundation::would_violate("extend UBI to cover housing"));
        assert!(!ImmutableFoundation::would_violate("create bioregional council"));
    }

    #[test]
    fn three_axioms_exist() {
        assert_eq!(ImmutableFoundation::AXIOMS.len(), 3);
        assert!(ImmutableFoundation::AXIOMS[0].contains("Dignity"));
        assert!(ImmutableFoundation::AXIOMS[1].contains("Sovereignty"));
        assert!(ImmutableFoundation::AXIOMS[2].contains("Consent"));
    }

    #[test]
    fn immutable_rights_count() {
        assert_eq!(ImmutableFoundation::IMMUTABLE_RIGHTS.len(), 10);
    }

    #[test]
    fn absolute_prohibitions_count() {
        assert_eq!(ImmutableFoundation::ABSOLUTE_PROHIBITIONS.len(), 7);
    }
}
