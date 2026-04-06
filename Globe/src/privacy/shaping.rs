//! Relay-side traffic shaping for privacy.
//!
//! Prevents traffic analysis by obscuring the timing relationship between
//! a client's publish actions and the relay's outbound event delivery.
//! Three mechanisms work together:
//!
//! - **Publish jitter** — random delay before a publish reaches the relay,
//!   breaking the timing correlation between "user pressed send" and "event
//!   appeared on the wire."
//! - **Timestamp bucketing** — rounding `created_at` to a configurable
//!   interval so that exact-second timestamps don't leak activity patterns.
//! - **Response batching** — grouping outbound events into time-bucketed
//!   batches so that individual subscriptions can't be fingerprinted by
//!   their delivery cadence.
//!
//! This is complementary to [`crate::camouflage`], which operates at the
//! wire level (padding bytes, mimicking HTTP traffic patterns). Traffic
//! shaping operates at the event/relay level.
//!
//! # Example
//!
//! ```
//! use globe::privacy::shaping::{ShapingConfig, TrafficShaper};
//!
//! let config = ShapingConfig::default();
//! let shaper = TrafficShaper::new(config);
//!
//! // Get a random publish delay within configured bounds
//! let delay = shaper.shape_publish_delay();
//! assert!(delay.as_millis() <= 500);
//!
//! // Bucket a timestamp to the nearest 10-second window
//! let ts = 1_700_000_007;
//! let bucketed = TrafficShaper::bucket_timestamp(ts, 10);
//! assert_eq!(bucketed, 1_700_000_000);
//! ```

use std::time::Duration;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for relay-side traffic shaping.
///
/// All fields have privacy-preserving defaults. When `enabled` is false,
/// the shaper still returns valid (zero) values so callers don't need
/// conditional logic.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShapingConfig {
    /// Whether traffic shaping is active. When false, jitter is zero and
    /// bucketing/batching are no-ops.
    pub enabled: bool,

    /// Timestamp bucketing interval in seconds. Event `created_at` values
    /// are rounded down to the nearest multiple of this value.
    ///
    /// A value of 10 means timestamps land on :00, :10, :20, etc.
    /// A value of 1 effectively disables bucketing (no rounding).
    pub timestamp_bucket_secs: u32,

    /// Minimum random delay (in milliseconds) added before publishing
    /// an event to the relay.
    pub publish_jitter_min_ms: u64,

    /// Maximum random delay (in milliseconds) added before publishing
    /// an event to the relay.
    pub publish_jitter_max_ms: u64,

    /// Interval (in milliseconds) at which outbound events are batched
    /// before delivery to subscribers. Events arriving within the same
    /// interval are delivered together.
    pub response_batch_interval_ms: u64,
}

impl Default for ShapingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            timestamp_bucket_secs: 10,
            publish_jitter_min_ms: 0,
            publish_jitter_max_ms: 500,
            response_batch_interval_ms: 200,
        }
    }
}

impl ShapingConfig {
    /// Validate the configuration, returning a human-readable error if
    /// any field is invalid.
    pub fn validate(&self) -> Result<(), String> {
        if self.timestamp_bucket_secs == 0 {
            return Err("timestamp_bucket_secs must be > 0".into());
        }
        if self.publish_jitter_min_ms > self.publish_jitter_max_ms {
            return Err(format!(
                "publish_jitter_min_ms ({}) must be <= publish_jitter_max_ms ({})",
                self.publish_jitter_min_ms, self.publish_jitter_max_ms,
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// TrafficShaper
// ---------------------------------------------------------------------------

/// Relay-side traffic shaper.
///
/// Holds a [`ShapingConfig`] and provides stateless helpers for publish
/// jitter, timestamp bucketing, and response batching. All methods are
/// safe to call from any thread.
#[derive(Clone, Debug)]
pub struct TrafficShaper {
    config: ShapingConfig,
}

impl TrafficShaper {
    /// Create a new traffic shaper with the given configuration.
    #[must_use]
    pub fn new(config: ShapingConfig) -> Self {
        Self { config }
    }

    /// Returns the shaper's configuration.
    #[must_use]
    pub fn config(&self) -> &ShapingConfig {
        &self.config
    }

    /// Compute a random publish delay within the configured jitter range.
    ///
    /// When shaping is disabled, returns `Duration::ZERO`.
    #[must_use]
    pub fn shape_publish_delay(&self) -> Duration {
        if !self.config.enabled {
            return Duration::ZERO;
        }

        let min = self.config.publish_jitter_min_ms;
        let max = self.config.publish_jitter_max_ms;

        if min == max {
            return Duration::from_millis(min);
        }

        use rand::Rng;
        let mut rng = rand::thread_rng();
        let jitter_ms = rng.gen_range(min..=max);
        Duration::from_millis(jitter_ms)
    }

    /// Round a unix timestamp (seconds) down to the nearest bucket boundary.
    ///
    /// This is a pure function — it does not depend on the shaper's config,
    /// so it is provided as an associated function for flexibility.
    ///
    /// # Panics
    ///
    /// Panics if `bucket_secs` is zero. Callers should validate configuration
    /// before calling this.
    #[must_use]
    pub fn bucket_timestamp(created_at: i64, bucket_secs: u32) -> i64 {
        assert!(bucket_secs > 0, "bucket_secs must be > 0");
        let bucket = bucket_secs as i64;
        // For negative timestamps (pre-epoch), Rust's integer division
        // truncates toward zero. We use Euclidean remainder to always
        // round down (toward negative infinity).
        created_at - created_at.rem_euclid(bucket)
    }

    /// Bucket a timestamp using this shaper's configured bucket size.
    ///
    /// When shaping is disabled, returns the original timestamp unchanged.
    #[must_use]
    pub fn bucket(&self, created_at: i64) -> i64 {
        if !self.config.enabled {
            return created_at;
        }
        Self::bucket_timestamp(created_at, self.config.timestamp_bucket_secs)
    }

    /// Group timestamps into batches by the configured response interval.
    ///
    /// Each input is an index paired with its timestamp (in milliseconds).
    /// Returns groups of indices whose timestamps fall within the same
    /// batch interval. Groups are returned in chronological order, and
    /// indices within each group preserve their input order.
    ///
    /// This is generic over the items being batched — the caller provides
    /// `(index, timestamp_ms)` pairs and gets back groups of indices. This
    /// avoids coupling to any particular event type.
    ///
    /// When shaping is disabled or the interval is zero, all indices are
    /// returned as a single batch.
    ///
    /// # Example
    ///
    /// ```
    /// use globe::privacy::shaping::{ShapingConfig, TrafficShaper};
    ///
    /// let config = ShapingConfig {
    ///     enabled: true,
    ///     response_batch_interval_ms: 100,
    ///     ..ShapingConfig::default()
    /// };
    /// let shaper = TrafficShaper::new(config);
    ///
    /// // Three events: two at ~0ms, one at ~150ms
    /// let items = vec![(0, 10u64), (1, 50), (2, 150)];
    /// let batches = shaper.batch_indices(&items);
    /// assert_eq!(batches.len(), 2);
    /// assert_eq!(batches[0], vec![0, 1]);
    /// assert_eq!(batches[1], vec![2]);
    /// ```
    #[must_use]
    pub fn batch_indices(&self, items: &[(usize, u64)]) -> Vec<Vec<usize>> {
        if items.is_empty() {
            return Vec::new();
        }

        if !self.config.enabled || self.config.response_batch_interval_ms == 0 {
            // Everything in one batch.
            return vec![items.iter().map(|(idx, _)| *idx).collect()];
        }

        let interval = self.config.response_batch_interval_ms;

        // Sort by timestamp to produce chronological batches.
        let mut sorted: Vec<(usize, u64)> = items.to_vec();
        sorted.sort_by_key(|(_, ts)| *ts);

        let mut batches: Vec<Vec<usize>> = Vec::new();
        let mut current_batch: Vec<usize> = Vec::new();
        let mut batch_start_ts: u64 = sorted[0].1;

        for &(idx, ts) in &sorted {
            if ts >= batch_start_ts + interval {
                // Start a new batch.
                if !current_batch.is_empty() {
                    batches.push(std::mem::take(&mut current_batch));
                }
                batch_start_ts = ts;
            }
            current_batch.push(idx);
        }

        if !current_batch.is_empty() {
            batches.push(current_batch);
        }

        batches
    }

    /// Convenience: batch `OmniEvent`s by their `created_at` timestamp.
    ///
    /// Events are grouped into batches where all events in a batch have
    /// `created_at` values within `response_batch_interval_ms` of the
    /// batch's earliest event (converted from seconds to milliseconds).
    ///
    /// Returns groups of events in chronological batch order.
    #[must_use]
    pub fn batch_events(&self, events: &[crate::event::OmniEvent]) -> Vec<Vec<crate::event::OmniEvent>> {
        if events.is_empty() {
            return Vec::new();
        }

        // Convert created_at (seconds) to milliseconds for batching.
        let items: Vec<(usize, u64)> = events
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let ts_ms = (e.created_at as u64).saturating_mul(1000);
                (i, ts_ms)
            })
            .collect();

        let index_batches = self.batch_indices(&items);

        index_batches
            .into_iter()
            .map(|indices| indices.into_iter().map(|i| events[i].clone()).collect())
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::OmniEvent;

    /// Build a minimal test event with the given `created_at` timestamp.
    fn test_event(created_at: i64) -> OmniEvent {
        OmniEvent {
            id: "a".repeat(64),
            author: "b".repeat(64),
            created_at,
            kind: 1,
            tags: Vec::new(),
            content: String::new(),
            sig: "c".repeat(128),
        }
    }

    // -- ShapingConfig defaults --

    #[test]
    fn default_config_values() {
        let config = ShapingConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.timestamp_bucket_secs, 10);
        assert_eq!(config.publish_jitter_min_ms, 0);
        assert_eq!(config.publish_jitter_max_ms, 500);
        assert_eq!(config.response_batch_interval_ms, 200);
    }

    #[test]
    fn default_config_is_valid() {
        let config = ShapingConfig::default();
        assert!(config.validate().is_ok());
    }

    // -- ShapingConfig validation --

    #[test]
    fn zero_bucket_secs_fails_validation() {
        let config = ShapingConfig {
            timestamp_bucket_secs: 0,
            ..ShapingConfig::default()
        };
        let err = config.validate().unwrap_err();
        assert!(err.contains("timestamp_bucket_secs"));
    }

    #[test]
    fn jitter_min_exceeds_max_fails_validation() {
        let config = ShapingConfig {
            publish_jitter_min_ms: 1000,
            publish_jitter_max_ms: 500,
            ..ShapingConfig::default()
        };
        let err = config.validate().unwrap_err();
        assert!(err.contains("publish_jitter_min_ms"));
    }

    #[test]
    fn equal_jitter_min_max_is_valid() {
        let config = ShapingConfig {
            publish_jitter_min_ms: 250,
            publish_jitter_max_ms: 250,
            ..ShapingConfig::default()
        };
        assert!(config.validate().is_ok());
    }

    // -- ShapingConfig serde --

    #[test]
    fn config_serde_roundtrip() {
        let config = ShapingConfig {
            enabled: true,
            timestamp_bucket_secs: 30,
            publish_jitter_min_ms: 100,
            publish_jitter_max_ms: 1000,
            response_batch_interval_ms: 500,
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: ShapingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, loaded);
    }

    #[test]
    fn config_deserializes_from_partial_json() {
        // Verify serde defaults work when fields are missing (for forward compat).
        let json = r#"{"enabled":true,"timestamp_bucket_secs":5}"#;
        let result = serde_json::from_str::<ShapingConfig>(json);
        // This will fail because serde doesn't use Default for missing fields
        // without #[serde(default)] — but ShapingConfig is expected to be nested
        // in GlobeConfig with #[serde(default)] on the field, not on each member.
        // Documenting this expectation:
        assert!(result.is_err(), "partial JSON should fail without #[serde(default)] on struct");
    }

    // -- Publish jitter --

    #[test]
    fn jitter_disabled_returns_zero() {
        let config = ShapingConfig {
            enabled: false,
            ..ShapingConfig::default()
        };
        let shaper = TrafficShaper::new(config);
        let delay = shaper.shape_publish_delay();
        assert_eq!(delay, Duration::ZERO);
    }

    #[test]
    fn jitter_within_bounds() {
        let config = ShapingConfig {
            enabled: true,
            publish_jitter_min_ms: 100,
            publish_jitter_max_ms: 500,
            ..ShapingConfig::default()
        };
        let shaper = TrafficShaper::new(config);

        // Run many iterations to check bounds statistically.
        for _ in 0..200 {
            let delay = shaper.shape_publish_delay();
            let ms = delay.as_millis() as u64;
            assert!(
                (100..=500).contains(&ms),
                "jitter {ms}ms outside bounds [100, 500]"
            );
        }
    }

    #[test]
    fn zero_jitter_range_returns_zero() {
        let config = ShapingConfig {
            enabled: true,
            publish_jitter_min_ms: 0,
            publish_jitter_max_ms: 0,
            ..ShapingConfig::default()
        };
        let shaper = TrafficShaper::new(config);
        let delay = shaper.shape_publish_delay();
        assert_eq!(delay, Duration::ZERO);
    }

    #[test]
    fn equal_min_max_jitter_returns_exact() {
        let config = ShapingConfig {
            enabled: true,
            publish_jitter_min_ms: 250,
            publish_jitter_max_ms: 250,
            ..ShapingConfig::default()
        };
        let shaper = TrafficShaper::new(config);
        let delay = shaper.shape_publish_delay();
        assert_eq!(delay, Duration::from_millis(250));
    }

    // -- Timestamp bucketing --

    #[test]
    fn bucket_rounds_down() {
        assert_eq!(TrafficShaper::bucket_timestamp(1_700_000_007, 10), 1_700_000_000);
        assert_eq!(TrafficShaper::bucket_timestamp(1_700_000_009, 10), 1_700_000_000);
        assert_eq!(TrafficShaper::bucket_timestamp(1_700_000_010, 10), 1_700_000_010);
    }

    #[test]
    fn bucket_exact_boundary() {
        // Timestamps exactly on a boundary should stay unchanged.
        assert_eq!(TrafficShaper::bucket_timestamp(1_700_000_000, 10), 1_700_000_000);
        assert_eq!(TrafficShaper::bucket_timestamp(0, 10), 0);
        assert_eq!(TrafficShaper::bucket_timestamp(100, 100), 100);
    }

    #[test]
    fn bucket_with_different_sizes() {
        let ts = 1_700_000_037;
        assert_eq!(TrafficShaper::bucket_timestamp(ts, 1), ts); // no rounding
        assert_eq!(TrafficShaper::bucket_timestamp(ts, 5), 1_700_000_035);
        assert_eq!(TrafficShaper::bucket_timestamp(ts, 10), 1_700_000_030);
        assert_eq!(TrafficShaper::bucket_timestamp(ts, 60), 1_699_999_980);
        assert_eq!(TrafficShaper::bucket_timestamp(ts, 3600), 1_699_999_200);
    }

    #[test]
    fn bucket_very_large_timestamp() {
        // Year ~2106 timestamp
        let ts: i64 = 4_294_967_295;
        let bucketed = TrafficShaper::bucket_timestamp(ts, 10);
        assert_eq!(bucketed, 4_294_967_290);
        assert_eq!(bucketed % 10, 0);
    }

    #[test]
    fn bucket_negative_timestamp() {
        // Pre-epoch timestamps should still bucket correctly (round toward -inf).
        assert_eq!(TrafficShaper::bucket_timestamp(-1, 10), -10);
        assert_eq!(TrafficShaper::bucket_timestamp(-10, 10), -10);
        assert_eq!(TrafficShaper::bucket_timestamp(-11, 10), -20);
        assert_eq!(TrafficShaper::bucket_timestamp(-5, 3), -6);
    }

    #[test]
    fn bucket_size_one_is_identity() {
        for ts in [-100, -1, 0, 1, 42, 1_700_000_000] {
            assert_eq!(TrafficShaper::bucket_timestamp(ts, 1), ts);
        }
    }

    #[test]
    #[should_panic(expected = "bucket_secs must be > 0")]
    fn bucket_zero_panics() {
        let _ = TrafficShaper::bucket_timestamp(100, 0);
    }

    #[test]
    fn bucket_convenience_method_when_enabled() {
        let config = ShapingConfig {
            enabled: true,
            timestamp_bucket_secs: 60,
            ..ShapingConfig::default()
        };
        let shaper = TrafficShaper::new(config);
        assert_eq!(shaper.bucket(1_700_000_037), 1_699_999_980);
    }

    #[test]
    fn bucket_convenience_method_when_disabled() {
        let config = ShapingConfig {
            enabled: false,
            timestamp_bucket_secs: 60,
            ..ShapingConfig::default()
        };
        let shaper = TrafficShaper::new(config);
        // Should return the timestamp unchanged.
        assert_eq!(shaper.bucket(1_700_000_037), 1_700_000_037);
    }

    // -- Response batching (index-based) --

    #[test]
    fn batch_empty_list() {
        let shaper = TrafficShaper::new(ShapingConfig {
            enabled: true,
            ..ShapingConfig::default()
        });
        let batches = shaper.batch_indices(&[]);
        assert!(batches.is_empty());
    }

    #[test]
    fn batch_single_item() {
        let shaper = TrafficShaper::new(ShapingConfig {
            enabled: true,
            response_batch_interval_ms: 100,
            ..ShapingConfig::default()
        });
        let batches = shaper.batch_indices(&[(0, 50)]);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0], vec![0]);
    }

    #[test]
    fn batch_groups_by_interval() {
        let config = ShapingConfig {
            enabled: true,
            response_batch_interval_ms: 100,
            ..ShapingConfig::default()
        };
        let shaper = TrafficShaper::new(config);

        // Three items: two within 100ms of each other, one 150ms later.
        let items = vec![(0, 10u64), (1, 50), (2, 150)];
        let batches = shaper.batch_indices(&items);
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0], vec![0, 1]);
        assert_eq!(batches[1], vec![2]);
    }

    #[test]
    fn batch_all_same_timestamp() {
        let config = ShapingConfig {
            enabled: true,
            response_batch_interval_ms: 100,
            ..ShapingConfig::default()
        };
        let shaper = TrafficShaper::new(config);

        let items: Vec<(usize, u64)> = (0..5).map(|i| (i, 500u64)).collect();
        let batches = shaper.batch_indices(&items);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 5);
    }

    #[test]
    fn batch_disabled_returns_single_batch() {
        let config = ShapingConfig {
            enabled: false,
            response_batch_interval_ms: 100,
            ..ShapingConfig::default()
        };
        let shaper = TrafficShaper::new(config);

        let items = vec![(0, 0u64), (1, 1000), (2, 5000)];
        let batches = shaper.batch_indices(&items);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 3);
    }

    #[test]
    fn batch_zero_interval_returns_single_batch() {
        let config = ShapingConfig {
            enabled: true,
            response_batch_interval_ms: 0,
            ..ShapingConfig::default()
        };
        let shaper = TrafficShaper::new(config);

        let items = vec![(0, 0u64), (1, 1000), (2, 5000)];
        let batches = shaper.batch_indices(&items);
        assert_eq!(batches.len(), 1);
    }

    #[test]
    fn batch_each_item_in_own_batch() {
        let config = ShapingConfig {
            enabled: true,
            response_batch_interval_ms: 10,
            ..ShapingConfig::default()
        };
        let shaper = TrafficShaper::new(config);

        // Each item is >10ms apart.
        let items = vec![(0, 0u64), (1, 100), (2, 200), (3, 300)];
        let batches = shaper.batch_indices(&items);
        assert_eq!(batches.len(), 4);
        for (i, batch) in batches.iter().enumerate() {
            assert_eq!(batch, &vec![i]);
        }
    }

    // -- Response batching (event-based) --

    #[test]
    fn batch_events_empty() {
        let shaper = TrafficShaper::new(ShapingConfig {
            enabled: true,
            ..ShapingConfig::default()
        });
        let batches = shaper.batch_events(&[]);
        assert!(batches.is_empty());
    }

    #[test]
    fn batch_events_groups_by_created_at() {
        let config = ShapingConfig {
            enabled: true,
            // 5 second interval (events use seconds, converted to ms internally).
            response_batch_interval_ms: 5_000,
            ..ShapingConfig::default()
        };
        let shaper = TrafficShaper::new(config);

        let events = vec![
            test_event(1_700_000_000),
            test_event(1_700_000_002), // within 5s of first
            test_event(1_700_000_010), // 10s later, separate batch
        ];

        let batches = shaper.batch_events(&events);
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].len(), 2);
        assert_eq!(batches[0][0].created_at, 1_700_000_000);
        assert_eq!(batches[0][1].created_at, 1_700_000_002);
        assert_eq!(batches[1].len(), 1);
        assert_eq!(batches[1][0].created_at, 1_700_000_010);
    }

    #[test]
    fn batch_events_disabled_single_batch() {
        let config = ShapingConfig {
            enabled: false,
            response_batch_interval_ms: 100,
            ..ShapingConfig::default()
        };
        let shaper = TrafficShaper::new(config);

        let events = vec![
            test_event(1_700_000_000),
            test_event(1_700_100_000),
        ];

        let batches = shaper.batch_events(&events);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].len(), 2);
    }
}
