//! Client-side privacy route selection based on event sensitivity.
//!
//! Maps event kinds to sensitivity levels and selects relay routing
//! strategies accordingly. Sensitive events (DMs, financial, medical)
//! can be routed through intermediary relays for unlinkability, while
//! public events (profiles, beacons) go direct for efficiency.
//!
//! This module is the client-side complement to Globe's relay forwarding
//! infrastructure (`Globe::privacy::relay_forward`). Globe handles the
//! server-side envelope processing; this module decides which path to use.
//!
//! # Example
//!
//! ```
//! use omnibus::privacy::{PrivacyConfig, RouteStrategy, classify_sensitivity, select_route, build_relay_path};
//!
//! let config = PrivacyConfig {
//!     sensitive_route: RouteStrategy::SingleIntermediary("wss://guard.example.com".into()),
//!     ..Default::default()
//! };
//!
//! // A DM (kind 4) is Confidential — routes through the intermediary.
//! let sensitivity = classify_sensitivity(4);
//! let strategy = select_route(sensitivity, &config);
//! let path = build_relay_path(strategy, "wss://dest.example.com");
//! assert_eq!(path, vec!["wss://guard.example.com", "wss://dest.example.com"]);
//! ```

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Sensitivity
// ---------------------------------------------------------------------------

/// How sensitive an event is, based on its kind.
///
/// Determines which routing strategy is applied. Ordered from least
/// to most sensitive — `Public < Standard < Confidential < Secret`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Sensitivity {
    /// Beacons, profiles, gospel records, commons. Publicly discoverable.
    Public,
    /// Community posts, text notes, comments. Normal social activity.
    Standard,
    /// Direct messages, financial transactions, communicator signaling.
    Confidential,
    /// Medical, legal, intimate content. Maximum protection.
    Secret,
}

// ---------------------------------------------------------------------------
// RouteStrategy
// ---------------------------------------------------------------------------

/// How events should be routed to their destination relay.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RouteStrategy {
    /// Send directly to the destination relay. Current default behavior.
    Direct,
    /// Route through one intermediary relay before reaching the destination.
    SingleIntermediary(String),
    /// Route through two intermediary relays (entry + middle) before
    /// reaching the destination.
    DoubleIntermediary(String, String),
    /// Automatically select intermediary relays from known Gospel peers.
    /// Falls back to `Direct` until Gospel integration is wired.
    AutoSelect,
}

// ---------------------------------------------------------------------------
// PrivacyConfig
// ---------------------------------------------------------------------------

/// Client-side privacy routing configuration.
///
/// Controls how events at different sensitivity levels are routed.
/// Both routes default to `Direct` for backward compatibility.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PrivacyConfig {
    /// Routing strategy for `Public` and `Standard` events.
    pub default_route: RouteStrategy,
    /// Routing strategy for `Confidential` and `Secret` events.
    pub sensitive_route: RouteStrategy,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        Self {
            default_route: RouteStrategy::Direct,
            sensitive_route: RouteStrategy::Direct,
        }
    }
}

impl PrivacyConfig {
    /// Validate that any intermediary relay URLs are non-empty.
    ///
    /// Returns `Ok(())` if the configuration is valid, or a human-readable
    /// error describing the problem.
    pub fn validate(&self) -> Result<(), String> {
        validate_route(&self.default_route, "default_route")?;
        validate_route(&self.sensitive_route, "sensitive_route")?;
        Ok(())
    }
}

/// Validate a single route strategy's URLs.
fn validate_route(route: &RouteStrategy, field_name: &str) -> Result<(), String> {
    match route {
        RouteStrategy::SingleIntermediary(url) => {
            if url.trim().is_empty() {
                return Err(format!(
                    "{field_name}: SingleIntermediary URL must not be empty"
                ));
            }
        }
        RouteStrategy::DoubleIntermediary(url1, url2) => {
            if url1.trim().is_empty() {
                return Err(format!(
                    "{field_name}: DoubleIntermediary first URL must not be empty"
                ));
            }
            if url2.trim().is_empty() {
                return Err(format!(
                    "{field_name}: DoubleIntermediary second URL must not be empty"
                ));
            }
        }
        RouteStrategy::Direct | RouteStrategy::AutoSelect => {}
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------

/// Classify the privacy sensitivity of an event based on its kind number.
///
/// Uses Globe's kind taxonomy (see `Globe::kind`) to map event kinds
/// to sensitivity levels:
///
/// - **Public**: profiles (0), contacts (3), naming (7000-7004),
///   relay hints (7010), asset announcements (7020), beacons (7030-7032),
///   lighthouse (7032), commons (7100+), product listings (6100),
///   storefront declarations (6200), reviews (6400-6500),
///   semantic profiles (26000).
/// - **Standard**: text notes (1), beacon updates (7031).
/// - **Confidential**: DMs (4), communicator signaling (5100-5113),
///   Fortune financial (6000-6999 except public commerce kinds),
///   orders (6300), cart suggestions (6150), network key events (7040-7042).
/// - **Secret**: not assigned by default — reserved for application-layer
///   classification of medical, legal, or intimate content.
///
/// Unrecognized kinds default to `Standard` (safe middle ground).
#[must_use]
pub fn classify_sensitivity(kind: u32) -> Sensitivity {
    match kind {
        // -- Public --
        // Profile metadata
        0 => Sensitivity::Public,
        // Contact/following list
        3 => Sensitivity::Public,
        // Naming system (7000-7004)
        7000..=7004 => Sensitivity::Public,
        // Relay hints
        7010 => Sensitivity::Public,
        // Asset announcements
        7020 => Sensitivity::Public,
        // Beacons and lighthouse
        7030 | 7032 => Sensitivity::Public,
        // Commons publications (7100+, within Globe range)
        7100..=7999 => Sensitivity::Public,
        // Public commerce: product listings, storefront declarations, reviews
        6100 | 6200 | 6400 | 6500 => Sensitivity::Public,
        // Semantic profiles (Zeitgeist)
        26000 => Sensitivity::Public,

        // -- Confidential --
        // Direct messages
        4 => Sensitivity::Confidential,
        // Communicator signaling (5100-5113)
        5100..=5113 => Sensitivity::Confidential,
        // Cart suggestions, orders (sensitive commerce)
        6150 | 6300 => Sensitivity::Confidential,
        // Remaining Fortune range (financial, not already classified as public)
        6000..=6099 | 6101..=6149 | 6151..=6199 | 6201..=6299 | 6301..=6399
        | 6401..=6499 | 6501..=6999 => Sensitivity::Confidential,
        // Network key delivery/rotation/invitation (sensitive discovery)
        7040..=7042 => Sensitivity::Confidential,

        // -- Standard (everything else) --
        // Text notes
        1 => Sensitivity::Standard,
        // Beacon updates
        7031 => Sensitivity::Standard,
        // All other kinds default to Standard
        _ => Sensitivity::Standard,
    }
}

// ---------------------------------------------------------------------------
// Route selection
// ---------------------------------------------------------------------------

/// Select the routing strategy for an event based on its sensitivity.
///
/// - `Public` and `Standard` events use `config.default_route`.
/// - `Confidential` and `Secret` events use `config.sensitive_route`.
#[must_use]
pub fn select_route(sensitivity: Sensitivity, config: &PrivacyConfig) -> &RouteStrategy {
    match sensitivity {
        Sensitivity::Public | Sensitivity::Standard => &config.default_route,
        Sensitivity::Confidential | Sensitivity::Secret => &config.sensitive_route,
    }
}

// ---------------------------------------------------------------------------
// Path building
// ---------------------------------------------------------------------------

/// Build an ordered relay path from a routing strategy and destination.
///
/// The returned vector is ordered from first hop to last (destination).
/// Compatible with `Globe::privacy::relay_forward::build_forward_envelope`.
///
/// `AutoSelect` falls back to direct routing (single-element path) until
/// Gospel relay discovery integration is complete.
#[must_use]
pub fn build_relay_path(strategy: &RouteStrategy, destination: &str) -> Vec<String> {
    match strategy {
        RouteStrategy::Direct => vec![destination.to_string()],
        RouteStrategy::SingleIntermediary(relay) => {
            vec![relay.clone(), destination.to_string()]
        }
        RouteStrategy::DoubleIntermediary(r1, r2) => {
            vec![r1.clone(), r2.clone(), destination.to_string()]
        }
        RouteStrategy::AutoSelect => {
            // Placeholder: fall back to direct until Gospel integration.
            vec![destination.to_string()]
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Sensitivity classification --

    #[test]
    fn classify_profile_is_public() {
        assert_eq!(classify_sensitivity(0), Sensitivity::Public);
    }

    #[test]
    fn classify_contacts_is_public() {
        assert_eq!(classify_sensitivity(3), Sensitivity::Public);
    }

    #[test]
    fn classify_text_note_is_standard() {
        assert_eq!(classify_sensitivity(1), Sensitivity::Standard);
    }

    #[test]
    fn classify_dm_is_confidential() {
        assert_eq!(classify_sensitivity(4), Sensitivity::Confidential);
    }

    #[test]
    fn classify_naming_kinds_are_public() {
        for kind in 7000..=7004 {
            assert_eq!(classify_sensitivity(kind), Sensitivity::Public, "kind {kind}");
        }
    }

    #[test]
    fn classify_relay_hints_public() {
        assert_eq!(classify_sensitivity(7010), Sensitivity::Public);
    }

    #[test]
    fn classify_beacon_public() {
        assert_eq!(classify_sensitivity(7030), Sensitivity::Public);
    }

    #[test]
    fn classify_beacon_update_standard() {
        assert_eq!(classify_sensitivity(7031), Sensitivity::Standard);
    }

    #[test]
    fn classify_lighthouse_public() {
        assert_eq!(classify_sensitivity(7032), Sensitivity::Public);
    }

    #[test]
    fn classify_commons_public() {
        assert_eq!(classify_sensitivity(7100), Sensitivity::Public);
        assert_eq!(classify_sensitivity(7500), Sensitivity::Public);
        assert_eq!(classify_sensitivity(7999), Sensitivity::Public);
    }

    #[test]
    fn classify_communicator_signaling_confidential() {
        for kind in 5100..=5113 {
            assert_eq!(
                classify_sensitivity(kind),
                Sensitivity::Confidential,
                "kind {kind}"
            );
        }
    }

    #[test]
    fn classify_fortune_commerce_public_kinds() {
        // Product listing, storefront, reviews are publicly discoverable.
        assert_eq!(classify_sensitivity(6100), Sensitivity::Public);
        assert_eq!(classify_sensitivity(6200), Sensitivity::Public);
        assert_eq!(classify_sensitivity(6400), Sensitivity::Public);
        assert_eq!(classify_sensitivity(6500), Sensitivity::Public);
    }

    #[test]
    fn classify_fortune_sensitive_kinds() {
        // Cart suggestions, orders are confidential.
        assert_eq!(classify_sensitivity(6150), Sensitivity::Confidential);
        assert_eq!(classify_sensitivity(6300), Sensitivity::Confidential);
        // Unclassified Fortune range kinds are also confidential.
        assert_eq!(classify_sensitivity(6050), Sensitivity::Confidential);
        assert_eq!(classify_sensitivity(6750), Sensitivity::Confidential);
    }

    #[test]
    fn classify_network_key_events_confidential() {
        assert_eq!(classify_sensitivity(7040), Sensitivity::Confidential);
        assert_eq!(classify_sensitivity(7041), Sensitivity::Confidential);
        assert_eq!(classify_sensitivity(7042), Sensitivity::Confidential);
    }

    #[test]
    fn classify_semantic_profile_public() {
        assert_eq!(classify_sensitivity(26000), Sensitivity::Public);
    }

    #[test]
    fn classify_unknown_kind_defaults_to_standard() {
        // Kinds with no explicit classification default to Standard.
        assert_eq!(classify_sensitivity(999), Sensitivity::Standard);
        assert_eq!(classify_sensitivity(2500), Sensitivity::Standard);
        assert_eq!(classify_sensitivity(50000), Sensitivity::Standard);
    }

    // -- Sensitivity ordering --

    #[test]
    fn sensitivity_ordering() {
        assert!(Sensitivity::Public < Sensitivity::Standard);
        assert!(Sensitivity::Standard < Sensitivity::Confidential);
        assert!(Sensitivity::Confidential < Sensitivity::Secret);
    }

    // -- Route selection --

    #[test]
    fn select_route_public_uses_default() {
        let config = PrivacyConfig {
            default_route: RouteStrategy::Direct,
            sensitive_route: RouteStrategy::SingleIntermediary("wss://guard.com".into()),
        };
        assert_eq!(
            select_route(Sensitivity::Public, &config),
            &RouteStrategy::Direct
        );
    }

    #[test]
    fn select_route_standard_uses_default() {
        let config = PrivacyConfig {
            default_route: RouteStrategy::Direct,
            sensitive_route: RouteStrategy::SingleIntermediary("wss://guard.com".into()),
        };
        assert_eq!(
            select_route(Sensitivity::Standard, &config),
            &RouteStrategy::Direct
        );
    }

    #[test]
    fn select_route_confidential_uses_sensitive() {
        let config = PrivacyConfig {
            default_route: RouteStrategy::Direct,
            sensitive_route: RouteStrategy::SingleIntermediary("wss://guard.com".into()),
        };
        assert_eq!(
            select_route(Sensitivity::Confidential, &config),
            &RouteStrategy::SingleIntermediary("wss://guard.com".into())
        );
    }

    #[test]
    fn select_route_secret_uses_sensitive() {
        let config = PrivacyConfig {
            default_route: RouteStrategy::Direct,
            sensitive_route: RouteStrategy::DoubleIntermediary(
                "wss://r1.com".into(),
                "wss://r2.com".into(),
            ),
        };
        assert_eq!(
            select_route(Sensitivity::Secret, &config),
            &RouteStrategy::DoubleIntermediary("wss://r1.com".into(), "wss://r2.com".into())
        );
    }

    // -- Build relay path --

    #[test]
    fn build_path_direct() {
        let path = build_relay_path(&RouteStrategy::Direct, "wss://dest.com");
        assert_eq!(path, vec!["wss://dest.com"]);
    }

    #[test]
    fn build_path_single_intermediary() {
        let path = build_relay_path(
            &RouteStrategy::SingleIntermediary("wss://guard.com".into()),
            "wss://dest.com",
        );
        assert_eq!(path, vec!["wss://guard.com", "wss://dest.com"]);
    }

    #[test]
    fn build_path_double_intermediary() {
        let path = build_relay_path(
            &RouteStrategy::DoubleIntermediary("wss://r1.com".into(), "wss://r2.com".into()),
            "wss://dest.com",
        );
        assert_eq!(
            path,
            vec!["wss://r1.com", "wss://r2.com", "wss://dest.com"]
        );
    }

    #[test]
    fn build_path_autoselect_falls_back_to_direct() {
        let path = build_relay_path(&RouteStrategy::AutoSelect, "wss://dest.com");
        assert_eq!(path, vec!["wss://dest.com"]);
    }

    // -- Config defaults --

    #[test]
    fn config_defaults_both_direct() {
        let config = PrivacyConfig::default();
        assert_eq!(config.default_route, RouteStrategy::Direct);
        assert_eq!(config.sensitive_route, RouteStrategy::Direct);
    }

    // -- Config validation --

    #[test]
    fn config_valid_direct() {
        let config = PrivacyConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn config_valid_with_intermediaries() {
        let config = PrivacyConfig {
            default_route: RouteStrategy::Direct,
            sensitive_route: RouteStrategy::DoubleIntermediary(
                "wss://r1.com".into(),
                "wss://r2.com".into(),
            ),
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn config_invalid_empty_single_url() {
        let config = PrivacyConfig {
            default_route: RouteStrategy::SingleIntermediary("".into()),
            sensitive_route: RouteStrategy::Direct,
        };
        let err = config.validate().unwrap_err();
        assert!(err.contains("default_route"));
        assert!(err.contains("empty"));
    }

    #[test]
    fn config_invalid_empty_double_first_url() {
        let config = PrivacyConfig {
            default_route: RouteStrategy::Direct,
            sensitive_route: RouteStrategy::DoubleIntermediary("".into(), "wss://r2.com".into()),
        };
        let err = config.validate().unwrap_err();
        assert!(err.contains("sensitive_route"));
        assert!(err.contains("first"));
    }

    #[test]
    fn config_invalid_empty_double_second_url() {
        let config = PrivacyConfig {
            default_route: RouteStrategy::Direct,
            sensitive_route: RouteStrategy::DoubleIntermediary("wss://r1.com".into(), "  ".into()),
        };
        let err = config.validate().unwrap_err();
        assert!(err.contains("sensitive_route"));
        assert!(err.contains("second"));
    }

    #[test]
    fn config_autoselect_valid() {
        let config = PrivacyConfig {
            default_route: RouteStrategy::AutoSelect,
            sensitive_route: RouteStrategy::AutoSelect,
        };
        assert!(config.validate().is_ok());
    }

    // -- Serde roundtrip --

    #[test]
    fn sensitivity_serde_roundtrip() {
        for sensitivity in [
            Sensitivity::Public,
            Sensitivity::Standard,
            Sensitivity::Confidential,
            Sensitivity::Secret,
        ] {
            let json = serde_json::to_string(&sensitivity).unwrap();
            let loaded: Sensitivity = serde_json::from_str(&json).unwrap();
            assert_eq!(sensitivity, loaded);
        }
    }

    #[test]
    fn route_strategy_serde_roundtrip() {
        let strategies = vec![
            RouteStrategy::Direct,
            RouteStrategy::SingleIntermediary("wss://relay.com".into()),
            RouteStrategy::DoubleIntermediary("wss://r1.com".into(), "wss://r2.com".into()),
            RouteStrategy::AutoSelect,
        ];
        for strategy in strategies {
            let json = serde_json::to_string(&strategy).unwrap();
            let loaded: RouteStrategy = serde_json::from_str(&json).unwrap();
            assert_eq!(strategy, loaded);
        }
    }

    #[test]
    fn privacy_config_serde_roundtrip() {
        let config = PrivacyConfig {
            default_route: RouteStrategy::AutoSelect,
            sensitive_route: RouteStrategy::DoubleIntermediary(
                "wss://entry.com".into(),
                "wss://middle.com".into(),
            ),
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: PrivacyConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.default_route, config.default_route);
        assert_eq!(loaded.sensitive_route, config.sensitive_route);
    }

    // -- Integration: end-to-end kind -> path --

    #[test]
    fn end_to_end_dm_routes_through_intermediary() {
        let config = PrivacyConfig {
            default_route: RouteStrategy::Direct,
            sensitive_route: RouteStrategy::SingleIntermediary("wss://guard.com".into()),
        };
        let sensitivity = classify_sensitivity(4); // DM
        let strategy = select_route(sensitivity, &config);
        let path = build_relay_path(strategy, "wss://dest.com");
        assert_eq!(path, vec!["wss://guard.com", "wss://dest.com"]);
    }

    #[test]
    fn end_to_end_profile_routes_direct() {
        let config = PrivacyConfig {
            default_route: RouteStrategy::Direct,
            sensitive_route: RouteStrategy::SingleIntermediary("wss://guard.com".into()),
        };
        let sensitivity = classify_sensitivity(0); // Profile
        let strategy = select_route(sensitivity, &config);
        let path = build_relay_path(strategy, "wss://dest.com");
        assert_eq!(path, vec!["wss://dest.com"]);
    }
}
