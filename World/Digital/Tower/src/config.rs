use std::path::PathBuf;

use globe::gospel::tier::GospelTier;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::privacy_transforms::PrivacyTransforms;

/// Tower operating mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TowerMode {
    /// Lightweight directory node. Gospel records only.
    /// Rejects non-gospel events. Minimal storage.
    Pharos,
    /// Community content node. Gospel + member content.
    /// Stores and serves content for configured communities.
    Harbor,
    /// Privacy-preserving forwarding node. Receives events and forwards
    /// them to an upstream relay after applying privacy transforms.
    /// Does NOT store or index events locally.
    Intermediary,
}

impl TowerMode {
    /// String representation for event tags and CLI output.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pharos => "pharos",
            Self::Harbor => "harbor",
            Self::Intermediary => "intermediary",
        }
    }
}

impl std::fmt::Display for TowerMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Configuration for a Tower node.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TowerConfig {
    /// Operating mode.
    pub mode: TowerMode,

    /// Human-readable name for this Tower node (shown in announcements).
    pub name: String,

    /// Directory for persistent storage (relay database, identity, config).
    pub data_dir: PathBuf,

    /// Port for the relay server. 0 = OS-assigned.
    pub port: u16,

    /// Whether to bind to all interfaces (true for LAN/internet reachable).
    pub bind_all: bool,

    /// Seed peer URLs for initial gospel sync.
    /// Tower connects to these on startup and periodically syncs gospel.
    pub seed_peers: Vec<Url>,

    /// Public URL of this Tower node (for lighthouse announcements).
    /// If not set, Tower will use the bind address.
    pub public_url: Option<Url>,

    /// Gospel evangelization interval in seconds (default: 60).
    pub gospel_interval_secs: u64,

    /// Maximum gospel peer connections (default: 16).
    pub max_gospel_peers: usize,

    /// Maximum events to store (Pharos: gospel only, Harbor: all).
    /// Default: 1_000_000 for Harbor, 100_000 for Pharos.
    pub max_events: Option<usize>,

    /// Maximum asset storage in bytes (Harbor only).
    /// Default: 20 GB (20 * 1024^3).
    pub max_asset_bytes: Option<u64>,

    /// Maximum connections to accept (default: 1000).
    pub max_connections: Option<usize>,

    /// Community pubkeys this Harbor serves (Harbor mode only).
    /// Content from these communities is stored and served.
    /// Ignored in Pharos mode.
    pub communities: Vec<String>,

    /// Lighthouse announcement interval in seconds (default: 300 = 5 min).
    pub announce_interval_secs: u64,

    /// Gospel tiers to propagate. Empty = derive from mode.
    ///
    /// Pharos default: Universal only (lightweight directory).
    /// Harbor default: Universal + Community.
    #[serde(default)]
    pub gospel_tiers: Vec<GospelTier>,

    /// Live sync polling interval in seconds (default: 2).
    /// How often to drain persistent gospel subscriptions for new events.
    pub gospel_live_interval_secs: u64,

    /// Upstream relay URL for Intermediary mode forwarding.
    /// Required when `mode` is `Intermediary`, ignored otherwise.
    #[serde(default)]
    pub upstream_relay: Option<String>,

    /// Privacy transforms applied to events before intermediary forwarding.
    /// Only meaningful in Intermediary mode. Defaults to all-disabled.
    #[serde(default)]
    pub privacy_transforms: PrivacyTransforms,

    /// Connection policy for incoming connections.
    /// Controls whether non-Tower IPs are accepted.
    /// Default: AllowAll (accepts all connections, backward compatible).
    #[serde(default = "default_connection_policy")]
    pub connection_policy: globe::server::network_defense::ConnectionPolicy,

    /// Whether to attempt UPnP port mapping.
    #[serde(default)]
    pub enable_upnp: bool,

    /// Whether clients must authenticate (AUTH kind 22242) before
    /// sending events or queries. Default: false.
    #[serde(default)]
    pub require_auth: bool,

    /// Community IDs that this Tower's communities are federated with.
    ///
    /// When set, Harbor mode additionally accepts content from authors
    /// who are members of federated communities. Also affects peer
    /// discovery (only peer with federated Towers for Community tier)
    /// and the IP allowlist (only include federated Tower IPs).
    ///
    /// Populated from Kingdom's FederationRegistry at startup.
    /// From Constellation Art. 3 §3.
    #[serde(default)]
    pub federated_communities: Vec<String>,

    /// Rate limit configuration for connection defense.
    ///
    /// Controls per-IP connection limits, event rate limits, etc.
    /// If `None`, uses the default `RateLimitConfig` (10 connections/IP,
    /// 120 events/min/IP, 1000 total connections).
    #[serde(default)]
    pub rate_limit_config: Option<globe::server::network_defense::RateLimitConfig>,
}

fn default_connection_policy() -> globe::server::network_defense::ConnectionPolicy {
    globe::server::network_defense::ConnectionPolicy::AllowAll
}

impl Default for TowerConfig {
    fn default() -> Self {
        Self {
            mode: TowerMode::Pharos,
            name: "Omnidea Tower".into(),
            data_dir: PathBuf::from("tower_data"),
            port: 7777,
            bind_all: true,
            seed_peers: Vec::new(),
            public_url: None,
            gospel_interval_secs: 60,
            max_gospel_peers: 16,
            max_events: None,
            max_asset_bytes: None,
            max_connections: Some(10_000),
            communities: Vec::new(),
            announce_interval_secs: 300,
            gospel_tiers: Vec::new(), // empty = derive from mode
            gospel_live_interval_secs: 2,
            upstream_relay: None,
            privacy_transforms: PrivacyTransforms::default(),
            connection_policy: default_connection_policy(),
            enable_upnp: false,
            require_auth: false,
            federated_communities: Vec::new(),
            rate_limit_config: None,
        }
    }
}

impl TowerConfig {
    /// Effective max events based on mode.
    ///
    /// Intermediary mode defaults to a minimal event limit since it
    /// does not store forwarded content — only gospel records.
    pub fn effective_max_events(&self) -> usize {
        self.max_events.unwrap_or(match self.mode {
            TowerMode::Pharos => 100_000,
            TowerMode::Harbor => 1_000_000,
            TowerMode::Intermediary => 10_000,
        })
    }

    /// Effective max asset bytes (Harbor only).
    pub fn effective_max_asset_bytes(&self) -> u64 {
        self.max_asset_bytes
            .unwrap_or(20 * 1024 * 1024 * 1024) // 20 GB
    }

    /// Effective gospel tiers based on mode (if not explicitly configured).
    ///
    /// Pharos: Universal only (lightweight directory node).
    /// Harbor: Universal + Community (community content node).
    /// Intermediary: Universal only (forwards events, minimal storage).
    pub fn effective_gospel_tiers(&self) -> Vec<GospelTier> {
        if self.gospel_tiers.is_empty() {
            match self.mode {
                TowerMode::Pharos | TowerMode::Intermediary => vec![GospelTier::Universal],
                TowerMode::Harbor => vec![GospelTier::Universal, GospelTier::Community],
            }
        } else {
            self.gospel_tiers.clone()
        }
    }

    /// Validate the configuration.
    ///
    /// Returns an error if:
    /// - Intermediary mode is set without an `upstream_relay`.
    /// - Privacy transforms are invalid (e.g., `decoy_rate` out of range).
    pub fn validate(&self) -> Result<(), String> {
        if self.mode == TowerMode::Intermediary && self.upstream_relay.is_none() {
            return Err(
                "Intermediary mode requires upstream_relay to be set".into(),
            );
        }
        self.privacy_transforms.validate()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = TowerConfig::default();
        assert_eq!(config.mode, TowerMode::Pharos);
        assert_eq!(config.port, 7777);
        assert!(config.bind_all);
        assert!(config.seed_peers.is_empty());
        assert!(config.communities.is_empty());
        assert_eq!(config.gospel_interval_secs, 60);
        assert_eq!(config.announce_interval_secs, 300);
    }

    #[test]
    fn mode_display() {
        assert_eq!(TowerMode::Pharos.as_str(), "pharos");
        assert_eq!(TowerMode::Harbor.as_str(), "harbor");
        assert_eq!(format!("{}", TowerMode::Pharos), "pharos");
        assert_eq!(format!("{}", TowerMode::Harbor), "harbor");
    }

    #[test]
    fn effective_max_events_pharos() {
        let config = TowerConfig::default();
        assert_eq!(config.effective_max_events(), 100_000);
    }

    #[test]
    fn effective_max_events_harbor() {
        let config = TowerConfig {
            mode: TowerMode::Harbor,
            ..Default::default()
        };
        assert_eq!(config.effective_max_events(), 1_000_000);
    }

    #[test]
    fn effective_max_events_override() {
        let config = TowerConfig {
            max_events: Some(500_000),
            ..Default::default()
        };
        assert_eq!(config.effective_max_events(), 500_000);
    }

    #[test]
    fn serde_round_trip() {
        let config = TowerConfig {
            mode: TowerMode::Harbor,
            name: "Test Tower".into(),
            port: 8888,
            communities: vec!["abc123".into()],
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: TowerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.mode, TowerMode::Harbor);
        assert_eq!(loaded.name, "Test Tower");
        assert_eq!(loaded.port, 8888);
        assert_eq!(loaded.communities, vec!["abc123"]);
    }

    #[test]
    fn mode_serde_round_trip() {
        let pharos = serde_json::to_string(&TowerMode::Pharos).unwrap();
        let harbor = serde_json::to_string(&TowerMode::Harbor).unwrap();
        assert_eq!(pharos, "\"pharos\"");
        assert_eq!(harbor, "\"harbor\"");

        let loaded: TowerMode = serde_json::from_str(&pharos).unwrap();
        assert_eq!(loaded, TowerMode::Pharos);
    }

    #[test]
    fn pharos_effective_tiers_universal_only() {
        let config = TowerConfig::default(); // Pharos
        let tiers = config.effective_gospel_tiers();
        assert_eq!(tiers, vec![GospelTier::Universal]);
    }

    #[test]
    fn harbor_effective_tiers_universal_and_community() {
        let config = TowerConfig {
            mode: TowerMode::Harbor,
            ..Default::default()
        };
        let tiers = config.effective_gospel_tiers();
        assert_eq!(tiers, vec![GospelTier::Universal, GospelTier::Community]);
    }

    #[test]
    fn explicit_tiers_override_mode() {
        let config = TowerConfig {
            gospel_tiers: GospelTier::all(),
            ..Default::default()
        };
        let tiers = config.effective_gospel_tiers();
        assert_eq!(tiers.len(), 3);
    }

    #[test]
    fn default_live_interval() {
        let config = TowerConfig::default();
        assert_eq!(config.gospel_live_interval_secs, 2);
    }

    // =================================================================
    // Intermediary mode tests
    // =================================================================

    #[test]
    fn intermediary_mode_display() {
        assert_eq!(TowerMode::Intermediary.as_str(), "intermediary");
        assert_eq!(format!("{}", TowerMode::Intermediary), "intermediary");
    }

    #[test]
    fn intermediary_serde_roundtrip() {
        let json = serde_json::to_string(&TowerMode::Intermediary).unwrap();
        assert_eq!(json, "\"intermediary\"");
        let loaded: TowerMode = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded, TowerMode::Intermediary);
    }

    #[test]
    fn mode_serde_backward_compat_pharos_harbor() {
        // Existing Pharos and Harbor modes must still deserialize correctly.
        let pharos: TowerMode = serde_json::from_str("\"pharos\"").unwrap();
        assert_eq!(pharos, TowerMode::Pharos);
        let harbor: TowerMode = serde_json::from_str("\"harbor\"").unwrap();
        assert_eq!(harbor, TowerMode::Harbor);
    }

    #[test]
    fn intermediary_requires_upstream_relay() {
        let config = TowerConfig {
            mode: TowerMode::Intermediary,
            upstream_relay: None,
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        assert!(err.contains("upstream_relay"));
    }

    #[test]
    fn intermediary_valid_with_upstream() {
        let config = TowerConfig {
            mode: TowerMode::Intermediary,
            upstream_relay: Some("wss://upstream.example.com".into()),
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn intermediary_with_all_transforms() {
        let config = TowerConfig {
            mode: TowerMode::Intermediary,
            upstream_relay: Some("wss://upstream.example.com".into()),
            privacy_transforms: PrivacyTransforms {
                randomize_timestamps: true,
                strip_ip_metadata: true,
                inject_decoy_events: true,
                decoy_rate: 0.25,
            },
            ..Default::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn intermediary_invalid_decoy_rate() {
        let config = TowerConfig {
            mode: TowerMode::Intermediary,
            upstream_relay: Some("wss://upstream.example.com".into()),
            privacy_transforms: PrivacyTransforms {
                decoy_rate: 2.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let err = config.validate().unwrap_err();
        assert!(err.contains("decoy_rate"));
    }

    #[test]
    fn pharos_harbor_validate_ok() {
        // Non-intermediary modes should always validate without upstream_relay.
        let pharos = TowerConfig::default();
        assert!(pharos.validate().is_ok());

        let harbor = TowerConfig {
            mode: TowerMode::Harbor,
            ..Default::default()
        };
        assert!(harbor.validate().is_ok());
    }

    #[test]
    fn intermediary_effective_max_events() {
        let config = TowerConfig {
            mode: TowerMode::Intermediary,
            ..Default::default()
        };
        assert_eq!(config.effective_max_events(), 10_000);
    }

    #[test]
    fn intermediary_effective_gospel_tiers_universal_only() {
        let config = TowerConfig {
            mode: TowerMode::Intermediary,
            ..Default::default()
        };
        let tiers = config.effective_gospel_tiers();
        assert_eq!(tiers, vec![GospelTier::Universal]);
    }

    #[test]
    fn config_without_privacy_fields_deserializes() {
        // Backward compatibility: a TowerConfig JSON from before P3B
        // (without upstream_relay or privacy_transforms) should still parse.
        let json = r#"{
            "mode": "pharos",
            "name": "Old Tower",
            "data_dir": "tower_data",
            "port": 7777,
            "bind_all": true,
            "seed_peers": [],
            "public_url": null,
            "gospel_interval_secs": 60,
            "max_gospel_peers": 16,
            "max_events": null,
            "max_asset_bytes": null,
            "max_connections": 1000,
            "communities": [],
            "announce_interval_secs": 300,
            "gospel_live_interval_secs": 2
        }"#;
        let config: TowerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.mode, TowerMode::Pharos);
        assert!(config.upstream_relay.is_none());
        assert!(!config.privacy_transforms.randomize_timestamps);
        assert!(!config.privacy_transforms.strip_ip_metadata);
        assert!(!config.privacy_transforms.inject_decoy_events);
        assert!((config.privacy_transforms.decoy_rate - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn default_config_has_no_upstream() {
        let config = TowerConfig::default();
        assert!(config.upstream_relay.is_none());
    }

    #[test]
    fn default_config_has_disabled_transforms() {
        let config = TowerConfig::default();
        assert_eq!(config.privacy_transforms, PrivacyTransforms::default());
    }

    // =================================================================
    // Connection defense config tests
    // =================================================================

    #[test]
    fn connection_policy_defaults_to_allow_all() {
        let config = TowerConfig::default();
        assert_eq!(config.connection_policy, globe::server::network_defense::ConnectionPolicy::AllowAll);
        assert!(!config.require_auth);
    }

    #[test]
    fn backward_compat_without_connection_fields() {
        // Old config JSON without new fields should still parse.
        let json = r#"{
            "mode": "pharos",
            "name": "Old Tower",
            "data_dir": "tower_data",
            "port": 7777,
            "bind_all": true,
            "seed_peers": [],
            "gospel_interval_secs": 60,
            "max_gospel_peers": 16,
            "max_connections": 1000,
            "communities": [],
            "announce_interval_secs": 300,
            "gospel_live_interval_secs": 2
        }"#;
        let config: TowerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.connection_policy, globe::server::network_defense::ConnectionPolicy::AllowAll);
        assert!(!config.require_auth);
    }

    #[test]
    fn connection_policy_serde_round_trip() {
        let config = TowerConfig {
            connection_policy: globe::server::network_defense::ConnectionPolicy::AllowlistOnly,
            require_auth: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: TowerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.connection_policy, globe::server::network_defense::ConnectionPolicy::AllowlistOnly);
        assert!(loaded.require_auth);
    }

    // =================================================================
    // Federation config tests
    // =================================================================

    #[test]
    fn federated_communities_default_empty() {
        let config = TowerConfig::default();
        assert!(config.federated_communities.is_empty());
    }

    #[test]
    fn federated_communities_serde() {
        let config = TowerConfig {
            federated_communities: vec!["community_b".into(), "community_c".into()],
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: TowerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.federated_communities, vec!["community_b", "community_c"]);
    }

    #[test]
    fn backward_compat_without_federation_field() {
        // Config JSON from before federation should still parse.
        let json = r#"{
            "mode": "harbor",
            "name": "Old Tower",
            "data_dir": "tower_data",
            "port": 7777,
            "bind_all": true,
            "seed_peers": [],
            "gospel_interval_secs": 60,
            "max_gospel_peers": 16,
            "max_connections": 1000,
            "communities": ["abc123"],
            "announce_interval_secs": 300,
            "gospel_live_interval_secs": 2
        }"#;
        let config: TowerConfig = serde_json::from_str(json).unwrap();
        assert!(config.federated_communities.is_empty());
        assert_eq!(config.communities, vec!["abc123"]);
    }
}
