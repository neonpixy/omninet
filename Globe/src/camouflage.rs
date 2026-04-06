//! Protocol Camouflage — making ORP traffic indistinguishable from standard web traffic.
//!
//! ORP already runs over WebSocket on port 443 with valid TLS, which makes it
//! look like most modern web applications. This module adds two additional layers
//! of traffic analysis resistance:
//!
//! - **Padding** — random bytes appended after a length delimiter to defeat
//!   message-length correlation attacks.
//! - **Traffic shaping** — scheduling message sends to match the timing patterns
//!   of common web traffic (browsing, streaming, messaging).
//!
//! # What this is NOT
//!
//! This is not a censorship circumvention system. For users in heavily censored
//! environments, Tor integration at the transport layer is the right answer
//! (separate from Omninet's protocol layer). Domain fronting and steganography
//! are deliberately excluded — they depend on fragile third-party cooperation
//! and offer diminishing returns.
//!
//! # Example
//!
//! ```
//! use globe::camouflage::{
//!     CamouflageConfig, CamouflageMode, PaddingConfig,
//!     TrafficPadder, TrafficShaper, ShapingProfile,
//! };
//!
//! // Pad a message
//! let config = PaddingConfig::default();
//! let data = b"hello world";
//! let padded = TrafficPadder::pad(data, &config);
//! let recovered = TrafficPadder::unpad(&padded).unwrap();
//! assert_eq!(recovered, data);
//!
//! // Shape a batch of messages to look like browsing
//! let messages = vec![vec![1, 2, 3], vec![4, 5, 6]];
//! let shaped = TrafficShaper::shape(&messages, &ShapingProfile::Browsing);
//! assert_eq!(shaped.len(), 2);
//! ```

use serde::{Deserialize, Serialize};

use crate::error::GlobeError;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Top-level camouflage configuration.
///
/// Defaults to `Standard` mode (ORP over WSS with valid TLS), which is
/// already indistinguishable from most web applications. Enable `Padded`
/// or `Shaped` for additional traffic analysis resistance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CamouflageConfig {
    /// Whether protocol camouflage is active.
    pub enabled: bool,
    /// The camouflage strategy to apply.
    pub mode: CamouflageMode,
}

impl Default for CamouflageConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: CamouflageMode::Standard,
        }
    }
}

impl CamouflageConfig {
    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), GlobeError> {
        match &self.mode {
            CamouflageMode::Padded(config) => config.validate(),
            CamouflageMode::Shaped(profile) => profile.validate(),
            CamouflageMode::Standard => Ok(()),
        }
    }
}

/// Strategy for making ORP traffic blend with normal web traffic.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CamouflageMode {
    /// ORP over WebSocket on port 443 with valid TLS.
    /// Already indistinguishable from most web apps. No extra processing.
    Standard,
    /// Add random padding to message sizes to defeat length-correlation attacks.
    Padded(PaddingConfig),
    /// Traffic shaping to mimic common HTTP traffic patterns.
    Shaped(ShapingProfile),
}

// ---------------------------------------------------------------------------
// Padding
// ---------------------------------------------------------------------------

/// Configuration for random message padding.
///
/// Each padded message has a 4-byte big-endian length prefix (the original
/// data length), followed by the original data, followed by random padding
/// bytes. The total padding amount is chosen uniformly at random from
/// `[min_pad, max_pad]`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaddingConfig {
    /// Minimum number of random padding bytes to append.
    pub min_pad: usize,
    /// Maximum number of random padding bytes to append.
    pub max_pad: usize,
    /// Interval (in milliseconds) at which to inject decoy padding-only
    /// messages when idle. 0 means no decoy traffic.
    pub pad_interval_ms: u64,
}

impl Default for PaddingConfig {
    fn default() -> Self {
        Self {
            min_pad: 32,
            max_pad: 256,
            pad_interval_ms: 0,
        }
    }
}

impl PaddingConfig {
    /// Validate the padding configuration.
    pub fn validate(&self) -> Result<(), GlobeError> {
        if self.min_pad > self.max_pad {
            return Err(GlobeError::InvalidConfig(
                "padding min_pad must be <= max_pad".into(),
            ));
        }
        Ok(())
    }
}

/// Pads and unpads messages with random bytes for traffic analysis resistance.
///
/// Wire format: `[4-byte BE data length][original data][random padding]`
///
/// The length prefix lets the receiver strip the padding without any
/// out-of-band information.
pub struct TrafficPadder;

impl TrafficPadder {
    /// Pad `data` with random bytes according to `config`.
    ///
    /// Returns the padded message. The original data can be recovered with
    /// [`unpad`](Self::unpad).
    #[must_use]
    pub fn pad(data: &[u8], config: &PaddingConfig) -> Vec<u8> {
        let data_len = data.len() as u32;
        let pad_amount = Self::random_pad_amount(config.min_pad, config.max_pad);

        let total = 4 + data.len() + pad_amount;
        let mut out = Vec::with_capacity(total);

        // 4-byte big-endian length prefix
        out.extend_from_slice(&data_len.to_be_bytes());
        // Original data
        out.extend_from_slice(data);
        // Random padding
        out.extend(Self::random_bytes(pad_amount));

        out
    }

    /// Strip padding and recover the original data.
    ///
    /// Returns `None` if the padded message is too short or the embedded
    /// length exceeds the available bytes.
    #[must_use]
    pub fn unpad(padded: &[u8]) -> Option<Vec<u8>> {
        if padded.len() < 4 {
            return None;
        }

        let data_len = u32::from_be_bytes([padded[0], padded[1], padded[2], padded[3]]) as usize;

        if padded.len() < 4 + data_len {
            return None;
        }

        Some(padded[4..4 + data_len].to_vec())
    }

    /// Generate a random padding amount in `[min, max]`.
    fn random_pad_amount(min: usize, max: usize) -> usize {
        if min == max {
            return min;
        }
        use rand::Rng;
        let mut rng = rand::thread_rng();
        rng.gen_range(min..=max)
    }

    /// Generate `n` random bytes.
    fn random_bytes(n: usize) -> Vec<u8> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        (0..n).map(|_| rng.r#gen()).collect()
    }
}

// ---------------------------------------------------------------------------
// Traffic Shaping
// ---------------------------------------------------------------------------

/// Pre-built timing profiles that mimic common web traffic patterns.
///
/// Each profile defines a base inter-message delay and a jitter range.
/// [`TrafficShaper::shape`] uses these to schedule message sends at
/// natural-looking intervals.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ShapingProfile {
    /// Mimics casual web browsing — longer, irregular gaps between requests.
    /// Base delay: 800ms, jitter: 0–1200ms.
    Browsing,
    /// Mimics media streaming — steady, frequent, small packets.
    /// Base delay: 50ms, jitter: 0–30ms.
    Streaming,
    /// Mimics instant messaging — bursty, variable gaps.
    /// Base delay: 300ms, jitter: 0–2000ms.
    Messaging,
}

impl ShapingProfile {
    /// Validate the shaping profile (always valid for built-in profiles).
    pub fn validate(&self) -> Result<(), GlobeError> {
        // All built-in profiles are valid by construction.
        Ok(())
    }

    /// Base inter-message delay in milliseconds for this profile.
    #[must_use]
    pub fn base_delay_ms(&self) -> u64 {
        match self {
            ShapingProfile::Browsing => 800,
            ShapingProfile::Streaming => 50,
            ShapingProfile::Messaging => 300,
        }
    }

    /// Maximum random jitter in milliseconds added to the base delay.
    #[must_use]
    pub fn jitter_ms(&self) -> u64 {
        match self {
            ShapingProfile::Browsing => 1200,
            ShapingProfile::Streaming => 30,
            ShapingProfile::Messaging => 2000,
        }
    }
}

/// A message with a scheduled send time, produced by [`TrafficShaper`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShapedMessage {
    /// The message payload.
    pub data: Vec<u8>,
    /// When this message should be sent, as a millisecond offset from
    /// the shaping start time.
    pub scheduled_send_at_ms: u64,
}

/// Shapes a batch of messages to match a [`ShapingProfile`]'s timing pattern.
///
/// The shaper does not delay messages itself — it computes scheduled send
/// times and returns [`ShapedMessage`]s. The caller (typically the relay
/// connection task) is responsible for holding messages until their
/// scheduled time.
pub struct TrafficShaper;

impl TrafficShaper {
    /// Schedule `messages` according to `profile`'s timing pattern.
    ///
    /// Returns one [`ShapedMessage`] per input message, with monotonically
    /// increasing `scheduled_send_at_ms` values starting from 0.
    #[must_use]
    pub fn shape(messages: &[Vec<u8>], profile: &ShapingProfile) -> Vec<ShapedMessage> {
        if messages.is_empty() {
            return Vec::new();
        }

        let base = profile.base_delay_ms();
        let jitter = profile.jitter_ms();

        let mut result = Vec::with_capacity(messages.len());
        let mut current_time_ms: u64 = 0;

        for (i, msg) in messages.iter().enumerate() {
            result.push(ShapedMessage {
                data: msg.clone(),
                scheduled_send_at_ms: current_time_ms,
            });

            // Don't add delay after the last message
            if i < messages.len() - 1 {
                let delay = base + Self::random_jitter(jitter);
                current_time_ms = current_time_ms.saturating_add(delay);
            }
        }

        result
    }

    /// Generate a random jitter value in `[0, max_jitter]`.
    fn random_jitter(max_jitter: u64) -> u64 {
        if max_jitter == 0 {
            return 0;
        }
        use rand::Rng;
        let mut rng = rand::thread_rng();
        rng.gen_range(0..=max_jitter)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- PaddingConfig tests --

    #[test]
    fn default_padding_config_is_valid() {
        let config = PaddingConfig::default();
        assert!(config.validate().is_ok());
        assert_eq!(config.min_pad, 32);
        assert_eq!(config.max_pad, 256);
        assert_eq!(config.pad_interval_ms, 0);
    }

    #[test]
    fn padding_config_min_exceeds_max_fails() {
        let config = PaddingConfig {
            min_pad: 100,
            max_pad: 50,
            pad_interval_ms: 0,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn padding_config_equal_min_max_valid() {
        let config = PaddingConfig {
            min_pad: 64,
            max_pad: 64,
            pad_interval_ms: 0,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn padding_config_zero_pad_valid() {
        let config = PaddingConfig {
            min_pad: 0,
            max_pad: 0,
            pad_interval_ms: 0,
        };
        assert!(config.validate().is_ok());
    }

    // -- TrafficPadder tests --

    #[test]
    fn pad_unpad_roundtrip() {
        let config = PaddingConfig::default();
        let data = b"hello world";
        let padded = TrafficPadder::pad(data, &config);
        let recovered = TrafficPadder::unpad(&padded).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn pad_unpad_empty_data() {
        let config = PaddingConfig::default();
        let data = b"";
        let padded = TrafficPadder::pad(data, &config);
        let recovered = TrafficPadder::unpad(&padded).unwrap();
        assert_eq!(recovered, data.to_vec());
    }

    #[test]
    fn pad_unpad_large_data() {
        let config = PaddingConfig {
            min_pad: 10,
            max_pad: 100,
            pad_interval_ms: 0,
        };
        let data: Vec<u8> = (0..10_000).map(|i| (i % 256) as u8).collect();
        let padded = TrafficPadder::pad(&data, &config);
        let recovered = TrafficPadder::unpad(&padded).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn padded_output_is_larger_than_input() {
        let config = PaddingConfig {
            min_pad: 32,
            max_pad: 32,
            pad_interval_ms: 0,
        };
        let data = b"test";
        let padded = TrafficPadder::pad(data, &config);
        // 4 (length prefix) + 4 (data) + 32 (padding) = 40
        assert_eq!(padded.len(), 40);
    }

    #[test]
    fn padded_length_within_expected_range() {
        let config = PaddingConfig {
            min_pad: 10,
            max_pad: 50,
            pad_interval_ms: 0,
        };
        let data = b"twelve chars";
        let padded = TrafficPadder::pad(data, &config);
        // 4 + 12 + [10..50] = [26..66]
        assert!(padded.len() >= 26, "too short: {}", padded.len());
        assert!(padded.len() <= 66, "too long: {}", padded.len());
    }

    #[test]
    fn unpad_too_short_returns_none() {
        assert!(TrafficPadder::unpad(&[]).is_none());
        assert!(TrafficPadder::unpad(&[0, 0, 0]).is_none());
    }

    #[test]
    fn unpad_truncated_data_returns_none() {
        // Claim 100 bytes but only provide 10
        let mut bad = Vec::new();
        bad.extend_from_slice(&100u32.to_be_bytes());
        bad.extend_from_slice(&[0u8; 10]);
        assert!(TrafficPadder::unpad(&bad).is_none());
    }

    #[test]
    fn unpad_ignores_excess_padding() {
        // Manually construct: length=5, data="hello", then 100 bytes of junk
        let mut msg = Vec::new();
        msg.extend_from_slice(&5u32.to_be_bytes());
        msg.extend_from_slice(b"hello");
        msg.extend_from_slice(&[0xFFu8; 100]);
        let recovered = TrafficPadder::unpad(&msg).unwrap();
        assert_eq!(recovered, b"hello");
    }

    #[test]
    fn pad_with_zero_padding() {
        let config = PaddingConfig {
            min_pad: 0,
            max_pad: 0,
            pad_interval_ms: 0,
        };
        let data = b"no padding";
        let padded = TrafficPadder::pad(data, &config);
        // 4 (length prefix) + 10 (data) + 0 (padding) = 14
        assert_eq!(padded.len(), 14);
        let recovered = TrafficPadder::unpad(&padded).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn multiple_pad_unpad_cycles_all_recover() {
        let config = PaddingConfig::default();
        for i in 0..50 {
            let data: Vec<u8> = (0..i).map(|x| x as u8).collect();
            let padded = TrafficPadder::pad(&data, &config);
            let recovered = TrafficPadder::unpad(&padded).unwrap();
            assert_eq!(recovered, data, "failed at iteration {i}");
        }
    }

    // -- CamouflageConfig tests --

    #[test]
    fn default_camouflage_config() {
        let config = CamouflageConfig::default();
        assert!(config.enabled);
        assert_eq!(config.mode, CamouflageMode::Standard);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn camouflage_config_serde_roundtrip() {
        let config = CamouflageConfig {
            enabled: true,
            mode: CamouflageMode::Padded(PaddingConfig {
                min_pad: 16,
                max_pad: 128,
                pad_interval_ms: 500,
            }),
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: CamouflageConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, loaded);
    }

    #[test]
    fn camouflage_config_invalid_padding_fails_validation() {
        let config = CamouflageConfig {
            enabled: true,
            mode: CamouflageMode::Padded(PaddingConfig {
                min_pad: 200,
                max_pad: 100,
                pad_interval_ms: 0,
            }),
        };
        assert!(config.validate().is_err());
    }

    // -- ShapingProfile tests --

    #[test]
    fn shaping_profiles_have_distinct_timing() {
        let browsing = ShapingProfile::Browsing;
        let streaming = ShapingProfile::Streaming;
        let messaging = ShapingProfile::Messaging;

        // Streaming should be fastest (lowest base delay)
        assert!(streaming.base_delay_ms() < messaging.base_delay_ms());
        assert!(messaging.base_delay_ms() < browsing.base_delay_ms());
    }

    #[test]
    fn shaping_profile_serde_roundtrip() {
        for profile in &[
            ShapingProfile::Browsing,
            ShapingProfile::Streaming,
            ShapingProfile::Messaging,
        ] {
            let json = serde_json::to_string(profile).unwrap();
            let loaded: ShapingProfile = serde_json::from_str(&json).unwrap();
            assert_eq!(*profile, loaded);
        }
    }

    // -- TrafficShaper tests --

    #[test]
    fn shape_empty_messages() {
        let shaped = TrafficShaper::shape(&[], &ShapingProfile::Browsing);
        assert!(shaped.is_empty());
    }

    #[test]
    fn shape_single_message_starts_at_zero() {
        let messages = vec![vec![1, 2, 3]];
        let shaped = TrafficShaper::shape(&messages, &ShapingProfile::Browsing);
        assert_eq!(shaped.len(), 1);
        assert_eq!(shaped[0].data, vec![1, 2, 3]);
        assert_eq!(shaped[0].scheduled_send_at_ms, 0);
    }

    #[test]
    fn shape_preserves_message_order() {
        let messages: Vec<Vec<u8>> = (0..5).map(|i| vec![i]).collect();
        let shaped = TrafficShaper::shape(&messages, &ShapingProfile::Messaging);
        for (i, msg) in shaped.iter().enumerate() {
            assert_eq!(msg.data, vec![i as u8]);
        }
    }

    #[test]
    fn shaped_times_are_monotonically_increasing() {
        let messages: Vec<Vec<u8>> = (0..10).map(|i| vec![i]).collect();
        let shaped = TrafficShaper::shape(&messages, &ShapingProfile::Browsing);
        for i in 1..shaped.len() {
            assert!(
                shaped[i].scheduled_send_at_ms >= shaped[i - 1].scheduled_send_at_ms,
                "time at index {} ({}) < time at index {} ({})",
                i,
                shaped[i].scheduled_send_at_ms,
                i - 1,
                shaped[i - 1].scheduled_send_at_ms,
            );
        }
    }

    #[test]
    fn streaming_profile_has_tight_timing() {
        let messages: Vec<Vec<u8>> = (0..10).map(|i| vec![i]).collect();
        let shaped = TrafficShaper::shape(&messages, &ShapingProfile::Streaming);

        // With base=50ms, jitter=0-30ms, max gap between any two consecutive
        // messages should be at most 80ms.
        for i in 1..shaped.len() {
            let gap = shaped[i].scheduled_send_at_ms - shaped[i - 1].scheduled_send_at_ms;
            assert!(
                gap <= 80,
                "streaming gap at index {i} was {gap}ms (expected <= 80ms)"
            );
        }
    }

    #[test]
    fn browsing_profile_has_wider_timing() {
        let messages: Vec<Vec<u8>> = (0..10).map(|i| vec![i]).collect();
        let shaped = TrafficShaper::shape(&messages, &ShapingProfile::Browsing);

        // With base=800ms, each gap should be at least 800ms
        for i in 1..shaped.len() {
            let gap = shaped[i].scheduled_send_at_ms - shaped[i - 1].scheduled_send_at_ms;
            assert!(
                gap >= 800,
                "browsing gap at index {i} was {gap}ms (expected >= 800ms)"
            );
        }
    }

    #[test]
    fn shape_data_integrity() {
        let messages = vec![
            b"first message".to_vec(),
            b"second message".to_vec(),
            b"third message".to_vec(),
        ];
        let shaped = TrafficShaper::shape(&messages, &ShapingProfile::Messaging);
        assert_eq!(shaped.len(), 3);
        assert_eq!(shaped[0].data, b"first message");
        assert_eq!(shaped[1].data, b"second message");
        assert_eq!(shaped[2].data, b"third message");
    }
}
