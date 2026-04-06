use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::error::GlobeError;
use crate::privacy::AnonymousConfig;
use crate::privacy::BucketPaddingConfig;
use crate::privacy::ForwardingConfig;
use crate::privacy::ShapingConfig;

/// Configuration for the Globe relay client.
///
/// All fields have sane defaults. At minimum, add relay URLs
/// before starting Globe.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobeConfig {
    /// Initial relay URLs to connect to.
    pub relay_urls: Vec<Url>,
    /// Maximum number of simultaneous relay connections.
    pub max_relays: usize,
    /// Minimum delay between reconnection attempts.
    #[serde(with = "duration_millis")]
    pub reconnect_min_delay: Duration,
    /// Maximum delay between reconnection attempts (backoff ceiling).
    #[serde(with = "duration_millis")]
    pub reconnect_max_delay: Duration,
    /// Maximum reconnection attempts before giving up. None = infinite.
    pub reconnect_max_attempts: Option<u32>,
    /// Interval between WebSocket ping heartbeats.
    #[serde(with = "duration_millis")]
    pub heartbeat_interval: Duration,
    /// Timeout for establishing a WebSocket connection.
    #[serde(with = "duration_millis")]
    pub connection_timeout: Duration,
    /// Maximum number of event IDs in the deduplication cache.
    pub max_seen_events: usize,
    /// Maximum number of messages to queue while disconnected.
    pub max_pending_messages: usize,
    /// ORP protocol version.
    pub protocol_version: u32,
    /// Bucket-based privacy padding configuration.
    /// Defaults to `Disabled` for backward compatibility.
    #[serde(default)]
    pub padding: BucketPaddingConfig,
    /// Traffic shaping configuration for timing privacy.
    /// Defaults to disabled for backward compatibility.
    #[serde(default)]
    pub shaping: ShapingConfig,
    /// Relay forwarding configuration for multi-hop message delivery.
    /// Defaults to disabled for backward compatibility.
    #[serde(default)]
    pub forwarding: ForwardingConfig,
    /// Anonymous subscription configuration.
    /// Defaults to disabled for backward compatibility.
    #[serde(default)]
    pub anonymous: AnonymousConfig,
}

impl Default for GlobeConfig {
    fn default() -> Self {
        Self {
            relay_urls: Vec::new(),
            max_relays: 10,
            reconnect_min_delay: Duration::from_millis(500),
            reconnect_max_delay: Duration::from_secs(60),
            reconnect_max_attempts: None,
            heartbeat_interval: Duration::from_secs(30),
            connection_timeout: Duration::from_secs(10),
            max_seen_events: 10_000,
            max_pending_messages: 1_000,
            protocol_version: 1,
            padding: BucketPaddingConfig::default(),
            shaping: ShapingConfig::default(),
            forwarding: ForwardingConfig::default(),
            anonymous: AnonymousConfig::default(),
        }
    }
}

impl GlobeConfig {
    /// Validate the configuration, returning an error if any field is invalid.
    pub fn validate(&self) -> Result<(), GlobeError> {
        if self.max_relays == 0 {
            return Err(GlobeError::InvalidConfig(
                "max_relays must be > 0".into(),
            ));
        }
        if self.reconnect_min_delay >= self.reconnect_max_delay {
            return Err(GlobeError::InvalidConfig(
                "reconnect_min_delay must be < reconnect_max_delay".into(),
            ));
        }
        if self.heartbeat_interval.is_zero() {
            return Err(GlobeError::InvalidConfig(
                "heartbeat_interval must be > 0".into(),
            ));
        }
        if self.connection_timeout.is_zero() {
            return Err(GlobeError::InvalidConfig(
                "connection_timeout must be > 0".into(),
            ));
        }
        if self.max_seen_events == 0 {
            return Err(GlobeError::InvalidConfig(
                "max_seen_events must be > 0".into(),
            ));
        }
        if self.max_pending_messages == 0 {
            return Err(GlobeError::InvalidConfig(
                "max_pending_messages must be > 0".into(),
            ));
        }
        self.padding.validate()?;
        self.shaping
            .validate()
            .map_err(GlobeError::InvalidConfig)?;
        self.forwarding
            .validate()
            .map_err(GlobeError::InvalidConfig)?;
        Ok(())
    }
}

/// Serialize/deserialize Duration as milliseconds (u64).
mod duration_millis {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(dur: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(dur.as_millis() as u64)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
        let ms = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(ms))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_valid() {
        let config = GlobeConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn zero_max_relays_fails() {
        let config = GlobeConfig {
            max_relays: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn min_delay_gte_max_delay_fails() {
        let config = GlobeConfig {
            reconnect_min_delay: Duration::from_secs(120),
            reconnect_max_delay: Duration::from_secs(60),
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn config_serde_round_trip() {
        let config = GlobeConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let loaded: GlobeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.max_relays, config.max_relays);
        assert_eq!(loaded.protocol_version, config.protocol_version);
        assert_eq!(loaded.max_seen_events, config.max_seen_events);
    }
}
