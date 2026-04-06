//! Bucket-based traffic padding for size normalization.
//!
//! Unlike [`camouflage::TrafficPadder`](crate::camouflage::TrafficPadder) which
//! adds a random amount of padding, bucket padding rounds **every** message up
//! to the next multiple of a fixed bucket size. This means all messages within
//! a bucket are exactly the same length, defeating size-based correlation.
//!
//! Wire format: `[4-byte BE original length][original data][random fill to bucket boundary]`
//!
//! # Example
//!
//! ```
//! use globe::privacy::padding::{pad_to_bucket, unpad, BucketPaddingConfig, PaddingMode};
//!
//! let data = b"hello world";
//! let padded = pad_to_bucket(data, 64);
//! assert_eq!(padded.len() % 64, 0);
//!
//! let recovered = unpad(&padded).unwrap();
//! assert_eq!(recovered, data);
//! ```

use serde::{Deserialize, Serialize};

use crate::error::GlobeError;

// ---------------------------------------------------------------------------
// PaddingMode
// ---------------------------------------------------------------------------

/// Bucket size strategy for privacy padding.
///
/// Each mode defines a fixed bucket size. All padded messages are rounded up
/// to the next multiple of that bucket, so every message in a bucket is
/// indistinguishable by length.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PaddingMode {
    /// No bucket padding applied. Messages are sent at their natural size.
    #[default]
    Disabled,
    /// 64-byte buckets. Low overhead, suitable for small text messages.
    Minimal,
    /// 256-byte buckets. Good balance of privacy and overhead.
    Standard,
    /// 1024-byte buckets. Maximum size uniformity at higher bandwidth cost.
    Aggressive,
}

impl PaddingMode {
    /// The bucket size in bytes for this mode, or `None` if disabled.
    #[must_use]
    pub fn bucket_size(&self) -> Option<usize> {
        match self {
            PaddingMode::Disabled => None,
            PaddingMode::Minimal => Some(64),
            PaddingMode::Standard => Some(256),
            PaddingMode::Aggressive => Some(1024),
        }
    }
}

// ---------------------------------------------------------------------------
// BucketPaddingConfig
// ---------------------------------------------------------------------------

/// Configuration for bucket-based privacy padding.
///
/// When a [`PaddingMode`] is set, all outgoing messages are padded to the
/// next multiple of the corresponding bucket size. A 4-byte length delimiter
/// precedes the original data so the receiver can strip the padding.
///
/// An optional `custom_bucket_size` overrides the mode's built-in size.
///
/// `pad_binary_frames` controls whether binary WebSocket frames (e.g.,
/// MessagePack or raw blob) also get bucket-padded. Text frames are always
/// padded when the mode is not `Disabled`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BucketPaddingConfig {
    /// The padding strategy.
    pub mode: PaddingMode,
    /// Override the mode's built-in bucket size with a custom value.
    /// Ignored when mode is `Disabled`.
    pub custom_bucket_size: Option<usize>,
    /// Whether to pad binary WebSocket frames in addition to text frames.
    pub pad_binary_frames: bool,
}

impl Default for BucketPaddingConfig {
    fn default() -> Self {
        Self {
            mode: PaddingMode::Disabled,
            custom_bucket_size: None,
            pad_binary_frames: false,
        }
    }
}

impl BucketPaddingConfig {
    /// Resolve the effective bucket size.
    ///
    /// Returns `None` when the mode is `Disabled` and no custom size is set.
    /// A custom size always wins when the mode is not `Disabled`.
    #[must_use]
    pub fn effective_bucket_size(&self) -> Option<usize> {
        if self.mode == PaddingMode::Disabled && self.custom_bucket_size.is_none() {
            return None;
        }
        // Custom overrides the mode's built-in size.
        self.custom_bucket_size.or_else(|| self.mode.bucket_size())
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), GlobeError> {
        if let Some(size) = self.custom_bucket_size {
            if size == 0 {
                return Err(GlobeError::InvalidConfig(
                    "custom_bucket_size must be > 0".into(),
                ));
            }
            // The 4-byte length prefix must fit inside one bucket.
            if size < 5 {
                return Err(GlobeError::InvalidConfig(
                    "custom_bucket_size must be >= 5 (4-byte header + at least 1 byte of data)".into(),
                ));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Pad / Unpad
// ---------------------------------------------------------------------------

/// Length-prefix size in bytes (big-endian u32).
const LENGTH_PREFIX: usize = 4;

/// Pad `data` to the next multiple of `bucket_size`.
///
/// Wire format: `[4-byte BE original length][original data][random fill]`
///
/// The total output length is always a multiple of `bucket_size`. If the
/// data + 4-byte header already fills a bucket exactly, a full extra bucket
/// is **not** added (unlike PKCS#7) — the length prefix is sufficient to
/// distinguish "no padding" from "data that ends in zero bytes."
///
/// # Panics
///
/// Panics if `bucket_size` is less than 5 (must fit at least the 4-byte
/// header plus one data byte).
#[must_use]
pub fn pad_to_bucket(data: &[u8], bucket_size: usize) -> Vec<u8> {
    assert!(bucket_size >= 5, "bucket_size must be >= 5");

    let raw_len = LENGTH_PREFIX + data.len();
    let padded_len = next_multiple(raw_len, bucket_size);
    let fill = padded_len - raw_len;

    let mut out = Vec::with_capacity(padded_len);

    // 4-byte big-endian original data length.
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    // Original data.
    out.extend_from_slice(data);
    // Random fill bytes to reach the bucket boundary.
    out.extend(random_bytes(fill));

    out
}

/// Strip bucket padding and recover the original data.
///
/// Reads the 4-byte big-endian length prefix, validates it against the
/// available bytes, and returns the original data slice.
pub fn unpad(padded: &[u8]) -> Result<Vec<u8>, GlobeError> {
    if padded.len() < LENGTH_PREFIX {
        return Err(GlobeError::InvalidMessage(format!(
            "padded data too short: expected at least {LENGTH_PREFIX} bytes, got {}",
            padded.len()
        )));
    }

    let data_len =
        u32::from_be_bytes([padded[0], padded[1], padded[2], padded[3]]) as usize;

    if padded.len() < LENGTH_PREFIX + data_len {
        return Err(GlobeError::InvalidMessage(format!(
            "padded data truncated: header says {} bytes but only {} available",
            data_len,
            padded.len() - LENGTH_PREFIX
        )));
    }

    Ok(padded[LENGTH_PREFIX..LENGTH_PREFIX + data_len].to_vec())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Round `n` up to the next multiple of `multiple`.
///
/// If `n` is already aligned, returns `n` unchanged (the 4-byte length
/// prefix makes unpadding unambiguous, so no extra bucket is needed).
fn next_multiple(n: usize, multiple: usize) -> usize {
    if n == 0 {
        return multiple;
    }
    let remainder = n % multiple;
    if remainder == 0 {
        n
    } else {
        n + (multiple - remainder)
    }
}

/// Generate `n` random bytes for padding fill.
fn random_bytes(n: usize) -> Vec<u8> {
    if n == 0 {
        return Vec::new();
    }
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..n).map(|_| rng.r#gen()).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- PaddingMode tests --

    #[test]
    fn padding_mode_default_is_disabled() {
        assert_eq!(PaddingMode::default(), PaddingMode::Disabled);
    }

    #[test]
    fn padding_mode_bucket_sizes() {
        assert_eq!(PaddingMode::Disabled.bucket_size(), None);
        assert_eq!(PaddingMode::Minimal.bucket_size(), Some(64));
        assert_eq!(PaddingMode::Standard.bucket_size(), Some(256));
        assert_eq!(PaddingMode::Aggressive.bucket_size(), Some(1024));
    }

    #[test]
    fn padding_mode_serde_roundtrip() {
        for mode in &[
            PaddingMode::Disabled,
            PaddingMode::Minimal,
            PaddingMode::Standard,
            PaddingMode::Aggressive,
        ] {
            let json = serde_json::to_string(mode).unwrap();
            let loaded: PaddingMode = serde_json::from_str(&json).unwrap();
            assert_eq!(*mode, loaded);
        }
    }

    // -- BucketPaddingConfig tests --

    #[test]
    fn default_config_is_disabled() {
        let config = BucketPaddingConfig::default();
        assert_eq!(config.mode, PaddingMode::Disabled);
        assert_eq!(config.custom_bucket_size, None);
        assert!(!config.pad_binary_frames);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn effective_bucket_size_from_mode() {
        let config = BucketPaddingConfig {
            mode: PaddingMode::Standard,
            custom_bucket_size: None,
            pad_binary_frames: false,
        };
        assert_eq!(config.effective_bucket_size(), Some(256));
    }

    #[test]
    fn effective_bucket_size_custom_overrides_mode() {
        let config = BucketPaddingConfig {
            mode: PaddingMode::Standard,
            custom_bucket_size: Some(512),
            pad_binary_frames: false,
        };
        assert_eq!(config.effective_bucket_size(), Some(512));
    }

    #[test]
    fn effective_bucket_size_disabled_with_no_custom() {
        let config = BucketPaddingConfig::default();
        assert_eq!(config.effective_bucket_size(), None);
    }

    #[test]
    fn validate_zero_custom_bucket_fails() {
        let config = BucketPaddingConfig {
            mode: PaddingMode::Minimal,
            custom_bucket_size: Some(0),
            pad_binary_frames: false,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_too_small_custom_bucket_fails() {
        let config = BucketPaddingConfig {
            mode: PaddingMode::Minimal,
            custom_bucket_size: Some(4),
            pad_binary_frames: false,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_minimum_custom_bucket_succeeds() {
        let config = BucketPaddingConfig {
            mode: PaddingMode::Minimal,
            custom_bucket_size: Some(5),
            pad_binary_frames: false,
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn config_serde_roundtrip() {
        let config = BucketPaddingConfig {
            mode: PaddingMode::Aggressive,
            custom_bucket_size: Some(2048),
            pad_binary_frames: true,
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: BucketPaddingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, loaded);
    }

    // -- pad_to_bucket / unpad roundtrip tests --

    #[test]
    fn roundtrip_64_byte_bucket() {
        let data = b"hello world";
        let padded = pad_to_bucket(data, 64);
        assert_eq!(padded.len() % 64, 0);
        let recovered = unpad(&padded).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn roundtrip_256_byte_bucket() {
        let data = b"a slightly longer message for testing bucket padding";
        let padded = pad_to_bucket(data, 256);
        assert_eq!(padded.len() % 256, 0);
        let recovered = unpad(&padded).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn roundtrip_1024_byte_bucket() {
        let data: Vec<u8> = (0..500).map(|i| (i % 256) as u8).collect();
        let padded = pad_to_bucket(&data, 1024);
        assert_eq!(padded.len() % 1024, 0);
        let recovered = unpad(&padded).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn roundtrip_custom_bucket() {
        let data = b"custom bucket test";
        let padded = pad_to_bucket(data, 100);
        assert_eq!(padded.len() % 100, 0);
        let recovered = unpad(&padded).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn roundtrip_empty_data() {
        let padded = pad_to_bucket(b"", 64);
        assert_eq!(padded.len() % 64, 0);
        // 4 bytes header + 0 bytes data → rounds up to 64
        assert_eq!(padded.len(), 64);
        let recovered = unpad(&padded).unwrap();
        assert!(recovered.is_empty());
    }

    #[test]
    fn roundtrip_data_exactly_at_bucket_boundary() {
        // 64 - 4 (header) = 60 bytes of data fills one bucket exactly
        let data = vec![0xAB_u8; 60];
        let padded = pad_to_bucket(&data, 64);
        // Already aligned: header + data = 64, no extra bucket needed
        assert_eq!(padded.len(), 64);
        let recovered = unpad(&padded).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn roundtrip_data_one_byte_over_bucket() {
        // 61 bytes of data + 4 header = 65 → rounds to 128
        let data = vec![0xCD_u8; 61];
        let padded = pad_to_bucket(&data, 64);
        assert_eq!(padded.len(), 128);
        let recovered = unpad(&padded).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn roundtrip_data_larger_than_single_bucket() {
        // 300 bytes of data + 4 header = 304 → rounds to 320 (5 * 64)
        let data: Vec<u8> = (0..300).map(|i| (i % 256) as u8).collect();
        let padded = pad_to_bucket(&data, 64);
        assert_eq!(padded.len() % 64, 0);
        assert_eq!(padded.len(), 320);
        let recovered = unpad(&padded).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn roundtrip_data_much_larger_than_bucket() {
        let data: Vec<u8> = (0..10_000).map(|i| (i % 256) as u8).collect();
        let padded = pad_to_bucket(&data, 256);
        assert_eq!(padded.len() % 256, 0);
        let recovered = unpad(&padded).unwrap();
        assert_eq!(recovered, data);
    }

    #[test]
    fn padded_output_is_always_bucket_aligned() {
        for len in 0..300 {
            let data: Vec<u8> = (0..len).map(|i| (i % 256) as u8).collect();
            let padded = pad_to_bucket(&data, 64);
            assert_eq!(
                padded.len() % 64,
                0,
                "data len={len}, padded len={}",
                padded.len()
            );
            let recovered = unpad(&padded).unwrap();
            assert_eq!(recovered, data, "data len={len}");
        }
    }

    // -- unpad error cases --

    #[test]
    fn unpad_too_short_fails() {
        assert!(unpad(&[]).is_err());
        assert!(unpad(&[0, 0, 0]).is_err());
    }

    #[test]
    fn unpad_truncated_data_fails() {
        // Header claims 100 bytes but only 10 available
        let mut bad = Vec::new();
        bad.extend_from_slice(&100u32.to_be_bytes());
        bad.extend_from_slice(&[0u8; 10]);
        assert!(unpad(&bad).is_err());
    }

    #[test]
    fn unpad_header_only_zero_length_succeeds() {
        // Header says 0 bytes of data — valid (empty message)
        let mut msg = Vec::new();
        msg.extend_from_slice(&0u32.to_be_bytes());
        let recovered = unpad(&msg).unwrap();
        assert!(recovered.is_empty());
    }

    // -- Binary frame tests --

    #[test]
    fn pad_binary_frames_config_flag() {
        let config = BucketPaddingConfig {
            mode: PaddingMode::Standard,
            custom_bucket_size: None,
            pad_binary_frames: true,
        };
        assert!(config.pad_binary_frames);
        assert!(config.validate().is_ok());
    }

    // -- next_multiple helper --

    #[test]
    fn next_multiple_basic() {
        assert_eq!(next_multiple(0, 64), 64);
        assert_eq!(next_multiple(1, 64), 64);
        assert_eq!(next_multiple(63, 64), 64);
        assert_eq!(next_multiple(64, 64), 64);
        assert_eq!(next_multiple(65, 64), 128);
    }

    // -- GlobeConfig backward compatibility --

    #[test]
    fn globe_config_without_padding_field_deserializes() {
        // Simulate a GlobeConfig JSON from before the padding field existed.
        // The #[serde(default)] attribute should fill in BucketPaddingConfig::default().
        let json = r#"{
            "relay_urls": [],
            "max_relays": 10,
            "reconnect_min_delay": 500,
            "reconnect_max_delay": 60000,
            "reconnect_max_attempts": null,
            "heartbeat_interval": 30000,
            "connection_timeout": 10000,
            "max_seen_events": 10000,
            "max_pending_messages": 1000,
            "protocol_version": 1
        }"#;
        let config: crate::config::GlobeConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.padding.mode, PaddingMode::Disabled);
        assert_eq!(config.padding.custom_bucket_size, None);
        assert!(!config.padding.pad_binary_frames);
    }

    #[test]
    fn globe_config_with_padding_roundtrip() {
        let config = crate::config::GlobeConfig {
            padding: BucketPaddingConfig {
                mode: PaddingMode::Aggressive,
                custom_bucket_size: Some(2048),
                pad_binary_frames: true,
            },
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: crate::config::GlobeConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.padding.mode, PaddingMode::Aggressive);
        assert_eq!(loaded.padding.custom_bucket_size, Some(2048));
        assert!(loaded.padding.pad_binary_frames);
    }

    #[test]
    #[should_panic]
    fn pad_to_bucket_panics_on_too_small_bucket() {
        let _ = pad_to_bucket(b"data", 4);
    }
}
