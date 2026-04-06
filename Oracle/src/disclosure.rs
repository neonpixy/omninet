//! Progressive disclosure — four sovereignty tiers.
//!
//! Oracle calibrates itself to the participant's engagement level based on
//! behavior, not explicit selection. Architects find depth naturally.
//! Citizens never encounter jargon.
//!
//! | Tier | Experience |
//! |------|-----------|
//! | Sheltered | Delegated sovereignty. A parent or caretaker manages participation. |
//! | Citizen | Sensible defaults. Everything works out of the box. |
//! | Steward | Active governance. Proposes, adjudicates, moderates. |
//! | Architect | Protocol-level. Operates Towers, interprets Covenant, builds tools. |
//!
//! No tier is better. Different engagement, equal dignity.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// SovereigntyTier
// ---------------------------------------------------------------------------

/// A participant's sovereignty tier, determined by behavior.
///
/// Ordered: Sheltered < Citizen < Steward < Architect.
/// No tier is "better" — different engagement levels, equal dignity.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum SovereigntyTier {
    /// Delegated sovereignty. A parent, caretaker, or trusted person manages
    /// participation. Full capabilities exist but are exercised by the delegate.
    Sheltered = 0,

    /// Sensible defaults. Identity is yours, data is encrypted, Advisor handles
    /// governance delegation, everything works out of the box.
    #[default]
    #[serde(alias = "Regular")]
    Citizen = 1,

    /// Active governance. Proposes, adjudicates, moderates. Understands the protocol.
    #[serde(alias = "Enthusiast")]
    Steward = 2,

    /// Protocol-level. Operates Towers, proposes Covenant interpretations,
    /// builds tools, participates in Star Court.
    #[serde(alias = "Operator")]
    Architect = 3,
}

impl SovereigntyTier {
    /// All tiers in order.
    pub fn all() -> &'static [Self] {
        &[
            Self::Sheltered,
            Self::Citizen,
            Self::Steward,
            Self::Architect,
        ]
    }
}

/// Backward-compatible alias for `SovereigntyTier`.
pub type UserLevel = SovereigntyTier;

// ---------------------------------------------------------------------------
// DelegateType
// ---------------------------------------------------------------------------

/// How governance decisions are delegated at each tier.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DelegateType {
    /// Delegated to a specific person (pubkey). Default for Sheltered.
    Person(String),
    /// Delegated to Advisor AI. Default for Citizen.
    Advisor,
    /// Direct participation (no delegation). Default for Steward/Architect.
    Direct,
}

// ---------------------------------------------------------------------------
// NotificationLevel
// ---------------------------------------------------------------------------

/// Notification verbosity level.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub enum NotificationLevel {
    /// Safety-critical only. Breaches, exclusions, emergency governance.
    Essential = 0,
    /// Safety + governance + social. Default.
    #[default]
    Standard = 1,
    /// Everything above + system events, network stats, Advisor actions.
    Detailed = 2,
    /// Every event. For protocol developers and node operators.
    Everything = 3,
}

// ---------------------------------------------------------------------------
// FeatureVisibility
// ---------------------------------------------------------------------------

/// Which features Oracle's progressive disclosure shows per tier.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FeatureVisibility {
    /// Creation tools only (Throne basics). For Sheltered.
    CreationOnly,
    /// Full app features, governance via Advisor, basic settings. For Citizen.
    FullApp,
    /// Governance tools, community management, advanced settings. For Steward.
    Governance,
    /// Protocol tools, Tower management, Star Court, raw data views. For Architect.
    Protocol,
}

impl FeatureVisibility {
    /// Default feature visibility for a tier.
    pub fn for_tier(tier: SovereigntyTier) -> Self {
        match tier {
            SovereigntyTier::Sheltered => Self::CreationOnly,
            SovereigntyTier::Citizen => Self::FullApp,
            SovereigntyTier::Steward => Self::Governance,
            SovereigntyTier::Architect => Self::Protocol,
        }
    }
}

// ---------------------------------------------------------------------------
// TierDefaults
// ---------------------------------------------------------------------------

/// Sensible defaults for each sovereignty tier.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TierDefaults {
    /// The tier these defaults apply to.
    pub tier: SovereigntyTier,
    /// Default governance delegation type.
    pub delegate_type: DelegateType,
    /// Default notification level.
    pub notification_level: NotificationLevel,
    /// Default feature visibility.
    pub feature_visibility: FeatureVisibility,
}

impl TierDefaults {
    /// Get defaults for a tier.
    ///
    /// - Sheltered: Person delegation (placeholder), Essential notifications, CreationOnly
    /// - Citizen: Advisor delegation, Standard notifications, FullApp
    /// - Steward: Direct participation, Detailed notifications, Governance
    /// - Architect: Direct participation, Everything notifications, Protocol
    pub fn for_tier(tier: SovereigntyTier) -> Self {
        match tier {
            SovereigntyTier::Sheltered => Self {
                tier,
                delegate_type: DelegateType::Person(String::new()),
                notification_level: NotificationLevel::Essential,
                feature_visibility: FeatureVisibility::CreationOnly,
            },
            SovereigntyTier::Citizen => Self {
                tier,
                delegate_type: DelegateType::Advisor,
                notification_level: NotificationLevel::Standard,
                feature_visibility: FeatureVisibility::FullApp,
            },
            SovereigntyTier::Steward => Self {
                tier,
                delegate_type: DelegateType::Direct,
                notification_level: NotificationLevel::Detailed,
                feature_visibility: FeatureVisibility::Governance,
            },
            SovereigntyTier::Architect => Self {
                tier,
                delegate_type: DelegateType::Direct,
                notification_level: NotificationLevel::Everything,
                feature_visibility: FeatureVisibility::Protocol,
            },
        }
    }

    /// Get defaults for all tiers.
    pub fn all() -> Vec<Self> {
        SovereigntyTier::all()
            .iter()
            .map(|&t| Self::for_tier(t))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// DisclosureSignal
// ---------------------------------------------------------------------------

/// Signals that indicate sovereignty tier. Each signal nudges the tier up.
///
/// The `DisclosureTracker` counts signals and transitions tiers
/// when thresholds are reached.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DisclosureSignal {
    // --- Steward signals (governance participation) ---
    /// User opened the settings panel.
    OpenedSettings,
    /// User changed a non-default setting.
    ChangedSetting,
    /// User viewed network/connection stats.
    ViewedNetworkStats,
    /// User toggled a feature.
    ToggledFeature,
    /// User viewed raw event data.
    ViewedRawData,
    /// User proposed in community governance.
    ProposedInGovernance,
    /// User voted directly (not via Advisor delegation).
    VotedDirectly,
    /// User served as an adjudicator in a dispute.
    ServedAsAdjudicator,

    // --- Architect signals (protocol participation) ---
    /// User used a CLI command.
    UsedCli,
    /// User ran a Tower node.
    RanTower,
    /// User edited a config file manually.
    EditedConfig,
    /// User submitted a Covenant precedent to Star Court.
    SubmittedPrecedent,
    /// User contributed code to the protocol.
    ContributedCode,

    /// Custom signal from any crate (contributes to Steward by default).
    Custom(String),
}

impl DisclosureSignal {
    /// Which tier this signal contributes toward.
    pub fn contributes_to(&self) -> SovereigntyTier {
        match self {
            Self::OpenedSettings
            | Self::ChangedSetting
            | Self::ViewedNetworkStats
            | Self::ToggledFeature
            | Self::ViewedRawData
            | Self::ProposedInGovernance
            | Self::VotedDirectly
            | Self::ServedAsAdjudicator => SovereigntyTier::Steward,

            Self::UsedCli
            | Self::RanTower
            | Self::EditedConfig
            | Self::SubmittedPrecedent
            | Self::ContributedCode => SovereigntyTier::Architect,

            Self::Custom(_) => SovereigntyTier::Steward,
        }
    }
}

// ---------------------------------------------------------------------------
// DisclosureConfig
// ---------------------------------------------------------------------------

/// Configuration for tier transitions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DisclosureConfig {
    /// Signals needed to reach Steward (default: 3).
    #[serde(alias = "enthusiast_threshold")]
    pub steward_threshold: u32,
    /// Signals needed to reach Architect (default: 2).
    #[serde(alias = "operator_threshold")]
    pub architect_threshold: u32,
}

impl Default for DisclosureConfig {
    fn default() -> Self {
        Self {
            steward_threshold: 3,
            architect_threshold: 2,
        }
    }
}

// ---------------------------------------------------------------------------
// DisclosureTracker
// ---------------------------------------------------------------------------

/// Tracks participant behavior signals and computes the current sovereignty tier.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DisclosureTracker {
    /// Current tier.
    #[serde(alias = "level")]
    tier: SovereigntyTier,
    /// Steward signal count.
    #[serde(alias = "enthusiast_signals")]
    steward_signals: u32,
    /// Architect signal count.
    #[serde(alias = "operator_signals")]
    architect_signals: u32,
    /// Configuration.
    config: DisclosureConfig,
    /// Whether the tier was manually overridden.
    manual_override: bool,
}

impl DisclosureTracker {
    /// Create a new tracker at Citizen tier.
    pub fn new() -> Self {
        Self {
            tier: SovereigntyTier::Citizen,
            steward_signals: 0,
            architect_signals: 0,
            config: DisclosureConfig::default(),
            manual_override: false,
        }
    }

    /// Create with custom config.
    pub fn with_config(config: DisclosureConfig) -> Self {
        Self {
            tier: SovereigntyTier::Citizen,
            steward_signals: 0,
            architect_signals: 0,
            config,
            manual_override: false,
        }
    }

    /// Record a signal. May trigger a tier transition.
    ///
    /// Signals are ignored when the tier is manually overridden or Sheltered
    /// (Sheltered exits only via explicit un-delegation, not behavior).
    pub fn record(&mut self, signal: &DisclosureSignal) {
        if self.manual_override || self.tier == SovereigntyTier::Sheltered {
            return;
        }

        match signal.contributes_to() {
            SovereigntyTier::Steward => {
                self.steward_signals += 1;
                if self.steward_signals >= self.config.steward_threshold
                    && self.tier < SovereigntyTier::Steward
                {
                    self.tier = SovereigntyTier::Steward;
                }
            }
            SovereigntyTier::Architect => {
                self.architect_signals += 1;
                // Architect signals also count toward steward.
                self.steward_signals += 1;
                if self.architect_signals >= self.config.architect_threshold
                    && self.tier < SovereigntyTier::Architect
                {
                    self.tier = SovereigntyTier::Architect;
                } else if self.steward_signals >= self.config.steward_threshold
                    && self.tier < SovereigntyTier::Steward
                {
                    self.tier = SovereigntyTier::Steward;
                }
            }
            SovereigntyTier::Citizen | SovereigntyTier::Sheltered => {}
        }
    }

    /// Current sovereignty tier.
    pub fn tier(&self) -> SovereigntyTier {
        self.tier
    }

    /// Current tier (backward-compatible alias).
    pub fn level(&self) -> SovereigntyTier {
        self.tier
    }

    /// Get the default settings for the current tier.
    pub fn tier_defaults(&self) -> TierDefaults {
        TierDefaults::for_tier(self.tier)
    }

    /// Get the feature visibility for the current tier.
    pub fn feature_visibility(&self) -> FeatureVisibility {
        FeatureVisibility::for_tier(self.tier)
    }

    /// Manually set the tier.
    ///
    /// Allows both upward and downward transitions.
    /// An Architect can choose to be a Citizen. No tier is better.
    pub fn set_tier(&mut self, tier: SovereigntyTier) {
        self.tier = tier;
        self.manual_override = true;
    }

    /// Manually set the tier (backward-compatible alias).
    pub fn set_level(&mut self, level: SovereigntyTier) {
        self.set_tier(level);
    }

    /// Clear the manual override (return to behavior-driven tier).
    ///
    /// Recomputes from accumulated signals. Never auto-assigns Sheltered.
    pub fn clear_override(&mut self) {
        self.manual_override = false;
        if self.architect_signals >= self.config.architect_threshold {
            self.tier = SovereigntyTier::Architect;
        } else if self.steward_signals >= self.config.steward_threshold {
            self.tier = SovereigntyTier::Steward;
        } else {
            self.tier = SovereigntyTier::Citizen;
        }
    }

    /// Whether the tier was manually overridden.
    pub fn is_overridden(&self) -> bool {
        self.manual_override
    }

    /// Signal counts for diagnostics: (steward_signals, architect_signals).
    pub fn signal_counts(&self) -> (u32, u32) {
        (self.steward_signals, self.architect_signals)
    }
}

impl Default for DisclosureTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- Tier basics ---

    #[test]
    fn default_tier_is_citizen() {
        let tracker = DisclosureTracker::new();
        assert_eq!(tracker.tier(), SovereigntyTier::Citizen);
        assert!(!tracker.is_overridden());
    }

    #[test]
    fn tier_ordering() {
        assert!(SovereigntyTier::Sheltered < SovereigntyTier::Citizen);
        assert!(SovereigntyTier::Citizen < SovereigntyTier::Steward);
        assert!(SovereigntyTier::Steward < SovereigntyTier::Architect);
    }

    #[test]
    fn all_tiers() {
        let tiers = SovereigntyTier::all();
        assert_eq!(tiers.len(), 4);
        assert_eq!(tiers[0], SovereigntyTier::Sheltered);
        assert_eq!(tiers[1], SovereigntyTier::Citizen);
        assert_eq!(tiers[2], SovereigntyTier::Steward);
        assert_eq!(tiers[3], SovereigntyTier::Architect);
    }

    // --- Transitions ---

    #[test]
    fn transitions_to_steward() {
        let mut tracker = DisclosureTracker::new();
        tracker.record(&DisclosureSignal::OpenedSettings);
        assert_eq!(tracker.tier(), SovereigntyTier::Citizen);
        tracker.record(&DisclosureSignal::ChangedSetting);
        assert_eq!(tracker.tier(), SovereigntyTier::Citizen);
        tracker.record(&DisclosureSignal::ViewedNetworkStats);
        assert_eq!(tracker.tier(), SovereigntyTier::Steward);
    }

    #[test]
    fn transitions_to_architect() {
        let mut tracker = DisclosureTracker::with_config(DisclosureConfig {
            steward_threshold: 1,
            architect_threshold: 2,
        });

        tracker.record(&DisclosureSignal::UsedCli);
        // Architect signal also counts toward steward (threshold 1), so:
        assert_eq!(tracker.tier(), SovereigntyTier::Steward);

        tracker.record(&DisclosureSignal::RanTower);
        assert_eq!(tracker.tier(), SovereigntyTier::Architect);
    }

    #[test]
    fn governance_signals_trigger_steward() {
        let mut tracker = DisclosureTracker::with_config(DisclosureConfig {
            steward_threshold: 3,
            architect_threshold: 2,
        });

        tracker.record(&DisclosureSignal::ProposedInGovernance);
        tracker.record(&DisclosureSignal::VotedDirectly);
        assert_eq!(tracker.tier(), SovereigntyTier::Citizen);
        tracker.record(&DisclosureSignal::ServedAsAdjudicator);
        assert_eq!(tracker.tier(), SovereigntyTier::Steward);
    }

    #[test]
    fn protocol_signals_trigger_architect() {
        let mut tracker = DisclosureTracker::with_config(DisclosureConfig {
            steward_threshold: 1,
            architect_threshold: 2,
        });

        tracker.record(&DisclosureSignal::SubmittedPrecedent);
        assert_eq!(tracker.tier(), SovereigntyTier::Steward); // Cross-counts
        tracker.record(&DisclosureSignal::ContributedCode);
        assert_eq!(tracker.tier(), SovereigntyTier::Architect);
    }

    // --- Sheltered behavior ---

    #[test]
    fn sheltered_ignores_signals() {
        let mut tracker = DisclosureTracker::new();
        tracker.set_tier(SovereigntyTier::Sheltered);
        // Clear override so we're testing the Sheltered check, not the override check.
        tracker.manual_override = false;

        tracker.record(&DisclosureSignal::ProposedInGovernance);
        tracker.record(&DisclosureSignal::VotedDirectly);
        tracker.record(&DisclosureSignal::RanTower);
        assert_eq!(tracker.tier(), SovereigntyTier::Sheltered);
        assert_eq!(tracker.signal_counts(), (0, 0));
    }

    // --- Manual override ---

    #[test]
    fn manual_override_up() {
        let mut tracker = DisclosureTracker::new();
        tracker.set_tier(SovereigntyTier::Architect);
        assert_eq!(tracker.tier(), SovereigntyTier::Architect);
        assert!(tracker.is_overridden());
    }

    #[test]
    fn manual_override_down() {
        let mut tracker = DisclosureTracker::new();
        // Accumulate enough signals for Steward.
        tracker.record(&DisclosureSignal::OpenedSettings);
        tracker.record(&DisclosureSignal::ChangedSetting);
        tracker.record(&DisclosureSignal::ViewedNetworkStats);
        assert_eq!(tracker.tier(), SovereigntyTier::Steward);

        // Override down to Citizen. No tier is better.
        tracker.set_tier(SovereigntyTier::Citizen);
        assert_eq!(tracker.tier(), SovereigntyTier::Citizen);
        assert!(tracker.is_overridden());

        // Signals don't change tier when overridden.
        tracker.record(&DisclosureSignal::RanTower);
        assert_eq!(tracker.tier(), SovereigntyTier::Citizen);
    }

    #[test]
    fn clear_override_recomputes() {
        let mut tracker = DisclosureTracker::new();
        // Accumulate steward signals.
        tracker.record(&DisclosureSignal::OpenedSettings);
        tracker.record(&DisclosureSignal::ChangedSetting);
        tracker.record(&DisclosureSignal::ViewedNetworkStats);
        assert_eq!(tracker.tier(), SovereigntyTier::Steward);

        // Override to Citizen.
        tracker.set_tier(SovereigntyTier::Citizen);
        assert_eq!(tracker.tier(), SovereigntyTier::Citizen);

        // Clear override — should recompute to Steward.
        tracker.clear_override();
        assert_eq!(tracker.tier(), SovereigntyTier::Steward);
    }

    #[test]
    fn clear_override_never_assigns_sheltered() {
        let mut tracker = DisclosureTracker::new();
        tracker.set_tier(SovereigntyTier::Sheltered);
        // Clear with no signals accumulated — should go to Citizen, not Sheltered.
        tracker.clear_override();
        assert_eq!(tracker.tier(), SovereigntyTier::Citizen);
    }

    // --- Signal counts ---

    #[test]
    fn signal_counts() {
        let mut tracker = DisclosureTracker::new();
        tracker.record(&DisclosureSignal::OpenedSettings);
        tracker.record(&DisclosureSignal::UsedCli);

        let (steward, architect) = tracker.signal_counts();
        assert_eq!(steward, 2); // OpenedSettings + UsedCli (cross-counts).
        assert_eq!(architect, 1);
    }

    #[test]
    fn custom_signal() {
        let mut tracker = DisclosureTracker::with_config(DisclosureConfig {
            steward_threshold: 1,
            ..Default::default()
        });

        tracker.record(&DisclosureSignal::Custom("viewed_api_docs".into()));
        assert_eq!(tracker.tier(), SovereigntyTier::Steward);
    }

    // --- TierDefaults ---

    #[test]
    fn tier_defaults_sheltered() {
        let defaults = TierDefaults::for_tier(SovereigntyTier::Sheltered);
        assert_eq!(defaults.delegate_type, DelegateType::Person(String::new()));
        assert_eq!(defaults.notification_level, NotificationLevel::Essential);
        assert_eq!(defaults.feature_visibility, FeatureVisibility::CreationOnly);
    }

    #[test]
    fn tier_defaults_citizen() {
        let defaults = TierDefaults::for_tier(SovereigntyTier::Citizen);
        assert_eq!(defaults.delegate_type, DelegateType::Advisor);
        assert_eq!(defaults.notification_level, NotificationLevel::Standard);
        assert_eq!(defaults.feature_visibility, FeatureVisibility::FullApp);
    }

    #[test]
    fn tier_defaults_steward() {
        let defaults = TierDefaults::for_tier(SovereigntyTier::Steward);
        assert_eq!(defaults.delegate_type, DelegateType::Direct);
        assert_eq!(defaults.notification_level, NotificationLevel::Detailed);
        assert_eq!(defaults.feature_visibility, FeatureVisibility::Governance);
    }

    #[test]
    fn tier_defaults_architect() {
        let defaults = TierDefaults::for_tier(SovereigntyTier::Architect);
        assert_eq!(defaults.delegate_type, DelegateType::Direct);
        assert_eq!(defaults.notification_level, NotificationLevel::Everything);
        assert_eq!(defaults.feature_visibility, FeatureVisibility::Protocol);
    }

    #[test]
    fn tier_defaults_all() {
        let all = TierDefaults::all();
        assert_eq!(all.len(), 4);
        assert_eq!(all[0].tier, SovereigntyTier::Sheltered);
        assert_eq!(all[3].tier, SovereigntyTier::Architect);
    }

    #[test]
    fn tracker_tier_defaults() {
        let tracker = DisclosureTracker::new();
        let defaults = tracker.tier_defaults();
        assert_eq!(defaults.tier, SovereigntyTier::Citizen);
        assert_eq!(defaults.delegate_type, DelegateType::Advisor);
    }

    #[test]
    fn tracker_feature_visibility() {
        let tracker = DisclosureTracker::new();
        assert_eq!(tracker.feature_visibility(), FeatureVisibility::FullApp);
    }

    // --- FeatureVisibility ---

    #[test]
    fn feature_visibility_per_tier() {
        assert_eq!(
            FeatureVisibility::for_tier(SovereigntyTier::Sheltered),
            FeatureVisibility::CreationOnly
        );
        assert_eq!(
            FeatureVisibility::for_tier(SovereigntyTier::Citizen),
            FeatureVisibility::FullApp
        );
        assert_eq!(
            FeatureVisibility::for_tier(SovereigntyTier::Steward),
            FeatureVisibility::Governance
        );
        assert_eq!(
            FeatureVisibility::for_tier(SovereigntyTier::Architect),
            FeatureVisibility::Protocol
        );
    }

    // --- NotificationLevel ---

    #[test]
    fn notification_level_ordering() {
        assert!(NotificationLevel::Essential < NotificationLevel::Standard);
        assert!(NotificationLevel::Standard < NotificationLevel::Detailed);
        assert!(NotificationLevel::Detailed < NotificationLevel::Everything);
    }

    // --- Serde ---

    #[test]
    fn sovereignty_tier_serde() {
        let tier = SovereigntyTier::Steward;
        let json = serde_json::to_string(&tier).unwrap();
        let loaded: SovereigntyTier = serde_json::from_str(&json).unwrap();
        assert_eq!(tier, loaded);
    }

    #[test]
    fn sovereignty_tier_backward_compat() {
        // Old UserLevel names should deserialize to new SovereigntyTier.
        let citizen: SovereigntyTier = serde_json::from_str("\"Regular\"").unwrap();
        assert_eq!(citizen, SovereigntyTier::Citizen);
        let steward: SovereigntyTier = serde_json::from_str("\"Enthusiast\"").unwrap();
        assert_eq!(steward, SovereigntyTier::Steward);
        let architect: SovereigntyTier = serde_json::from_str("\"Operator\"").unwrap();
        assert_eq!(architect, SovereigntyTier::Architect);
    }

    #[test]
    fn disclosure_signal_serde() {
        let signal = DisclosureSignal::RanTower;
        let json = serde_json::to_string(&signal).unwrap();
        let loaded: DisclosureSignal = serde_json::from_str(&json).unwrap();
        assert_eq!(signal, loaded);

        let governance = DisclosureSignal::ProposedInGovernance;
        let json = serde_json::to_string(&governance).unwrap();
        let loaded: DisclosureSignal = serde_json::from_str(&json).unwrap();
        assert_eq!(governance, loaded);
    }

    #[test]
    fn tracker_serde() {
        let mut tracker = DisclosureTracker::new();
        tracker.record(&DisclosureSignal::OpenedSettings);
        tracker.record(&DisclosureSignal::ChangedSetting);

        let json = serde_json::to_string(&tracker).unwrap();
        let loaded: DisclosureTracker = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.tier(), SovereigntyTier::Citizen); // Not yet at threshold.
        assert_eq!(loaded.signal_counts(), (2, 0));
    }

    #[test]
    fn config_serde() {
        let config = DisclosureConfig {
            steward_threshold: 5,
            architect_threshold: 3,
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: DisclosureConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.steward_threshold, 5);
        assert_eq!(loaded.architect_threshold, 3);
    }

    #[test]
    fn config_backward_compat() {
        // Old field names should deserialize via aliases.
        let json = r#"{"enthusiast_threshold":4,"operator_threshold":3}"#;
        let loaded: DisclosureConfig = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.steward_threshold, 4);
        assert_eq!(loaded.architect_threshold, 3);
    }

    #[test]
    fn delegate_type_serde() {
        let dt = DelegateType::Person("abc123".into());
        let json = serde_json::to_string(&dt).unwrap();
        let loaded: DelegateType = serde_json::from_str(&json).unwrap();
        assert_eq!(dt, loaded);

        let advisor = DelegateType::Advisor;
        let json = serde_json::to_string(&advisor).unwrap();
        let loaded: DelegateType = serde_json::from_str(&json).unwrap();
        assert_eq!(advisor, loaded);
    }

    #[test]
    fn tier_defaults_serde() {
        let defaults = TierDefaults::for_tier(SovereigntyTier::Steward);
        let json = serde_json::to_string(&defaults).unwrap();
        let loaded: TierDefaults = serde_json::from_str(&json).unwrap();
        assert_eq!(defaults, loaded);
    }

    #[test]
    fn feature_visibility_serde() {
        let fv = FeatureVisibility::Protocol;
        let json = serde_json::to_string(&fv).unwrap();
        let loaded: FeatureVisibility = serde_json::from_str(&json).unwrap();
        assert_eq!(fv, loaded);
    }

    // --- Contributes-to ---

    #[test]
    fn contributes_to() {
        assert_eq!(
            DisclosureSignal::OpenedSettings.contributes_to(),
            SovereigntyTier::Steward
        );
        assert_eq!(
            DisclosureSignal::ProposedInGovernance.contributes_to(),
            SovereigntyTier::Steward
        );
        assert_eq!(
            DisclosureSignal::UsedCli.contributes_to(),
            SovereigntyTier::Architect
        );
        assert_eq!(
            DisclosureSignal::SubmittedPrecedent.contributes_to(),
            SovereigntyTier::Architect
        );
        assert_eq!(
            DisclosureSignal::Custom("x".into()).contributes_to(),
            SovereigntyTier::Steward
        );
    }

    // --- Backward-compat alias ---

    #[test]
    fn user_level_alias_works() {
        let level: UserLevel = SovereigntyTier::Citizen;
        assert_eq!(level, SovereigntyTier::Citizen);

        let mut tracker = DisclosureTracker::new();
        assert_eq!(tracker.level(), SovereigntyTier::Citizen);
        tracker.set_level(SovereigntyTier::Architect);
        assert_eq!(tracker.level(), SovereigntyTier::Architect);
    }
}
