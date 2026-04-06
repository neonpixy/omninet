//! Privacy transforms for intermediary Tower mode.
//!
//! When a Tower operates in Intermediary mode, it forwards events to an
//! upstream relay without storing them locally. These transforms are applied
//! to forwarded events to frustrate traffic analysis:
//!
//! - **Timestamp jitter** — randomize event timestamps by ±5-60 seconds.
//! - **Metadata stripping** — remove tags that might leak client IP or location.
//! - **Decoy injection** — inject random noise events to mask real traffic patterns.
//!
//! All transforms default to disabled for backward compatibility and must be
//! explicitly opted in via [`PrivacyTransforms`] configuration.

use rand::Rng;
use serde::{Deserialize, Serialize};

/// Minimum absolute jitter in seconds.
const MIN_JITTER_SECS: i64 = 5;
/// Maximum absolute jitter in seconds.
const MAX_JITTER_SECS: i64 = 60;
/// Minimum size of a decoy event payload in bytes.
const DECOY_MIN_BYTES: usize = 64;
/// Maximum size of a decoy event payload in bytes.
const DECOY_MAX_BYTES: usize = 512;

/// Tag prefixes that may reveal client IP or geographic information.
const METADATA_TAG_PREFIXES: &[&str] = &["ip", "geo", "loc"];

/// Privacy transforms applied to events before intermediary forwarding.
///
/// All fields default to disabled (`false` / `0.0`) so that adding this
/// struct to an existing `TowerConfig` via `#[serde(default)]` is fully
/// backward-compatible.
///
/// # Example
///
/// ```
/// use tower::privacy_transforms::PrivacyTransforms;
///
/// let transforms = PrivacyTransforms {
///     randomize_timestamps: true,
///     strip_ip_metadata: true,
///     inject_decoy_events: true,
///     decoy_rate: 0.1,
/// };
/// assert!(transforms.validate().is_ok());
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PrivacyTransforms {
    /// Add ±5-60 second jitter to forwarded event timestamps.
    pub randomize_timestamps: bool,
    /// Remove tags that might leak client IP or location info.
    pub strip_ip_metadata: bool,
    /// Inject decoy (noise) events into the forwarding stream.
    pub inject_decoy_events: bool,
    /// Fraction of traffic that is decoy (0.0-1.0).
    /// Only meaningful when `inject_decoy_events` is `true`.
    pub decoy_rate: f64,
}

impl Default for PrivacyTransforms {
    fn default() -> Self {
        Self {
            randomize_timestamps: false,
            strip_ip_metadata: false,
            inject_decoy_events: false,
            decoy_rate: 0.0,
        }
    }
}

impl PrivacyTransforms {
    /// Validate the privacy transforms configuration.
    ///
    /// Returns an error if `decoy_rate` is outside the range `0.0..=1.0`.
    pub fn validate(&self) -> Result<(), String> {
        if !(0.0..=1.0).contains(&self.decoy_rate) {
            return Err(format!(
                "decoy_rate must be between 0.0 and 1.0, got {}",
                self.decoy_rate,
            ));
        }
        Ok(())
    }
}

/// Apply random timestamp jitter of ±5-60 seconds.
///
/// The offset magnitude is uniformly distributed between
/// [`MIN_JITTER_SECS`] and [`MAX_JITTER_SECS`], with a random sign.
pub fn apply_timestamp_jitter(created_at: i64) -> i64 {
    let mut rng = rand::thread_rng();
    let magnitude = rng.gen_range(MIN_JITTER_SECS..=MAX_JITTER_SECS);
    let sign: bool = rng.r#gen();
    if sign {
        created_at.saturating_add(magnitude)
    } else {
        created_at.saturating_sub(magnitude)
    }
}

/// Determine whether a decoy event should be injected at the given rate.
///
/// Returns `true` with probability `rate` (0.0 = never, 1.0 = always).
pub fn should_inject_decoy(rate: f64) -> bool {
    if rate <= 0.0 {
        return false;
    }
    if rate >= 1.0 {
        return true;
    }
    let sample: f64 = rand::thread_rng().r#gen();
    sample < rate
}

/// Create a decoy event payload — random bytes of a plausible event size.
///
/// The payload is between [`DECOY_MIN_BYTES`] and [`DECOY_MAX_BYTES`] to
/// resemble real event traffic without being parseable as a valid event.
#[must_use]
pub fn create_decoy_event() -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let size = rng.gen_range(DECOY_MIN_BYTES..=DECOY_MAX_BYTES);
    let mut buf = vec![0u8; size];
    rng.fill(buf.as_mut_slice());
    buf
}

/// Strip tags that might contain IP or location metadata.
///
/// Removes any tag whose first element (the tag name) starts with one of
/// the known metadata prefixes: `"ip"`, `"geo"`, `"loc"`. All other tags
/// are preserved unchanged.
///
/// # Example
///
/// ```
/// use tower::privacy_transforms::strip_metadata_tags;
///
/// let tags = vec![
///     vec!["p".into(), "abc".into()],
///     vec!["ip_address".into(), "1.2.3.4".into()],
///     vec!["geo_lat".into(), "51.5".into()],
///     vec!["e".into(), "def".into()],
/// ];
/// let cleaned = strip_metadata_tags(&tags);
/// assert_eq!(cleaned.len(), 2);
/// assert_eq!(cleaned[0][0], "p");
/// assert_eq!(cleaned[1][0], "e");
/// ```
pub fn strip_metadata_tags(tags: &[Vec<String>]) -> Vec<Vec<String>> {
    tags.iter()
        .filter(|tag| {
            if let Some(name) = tag.first() {
                let lower = name.to_lowercase();
                !METADATA_TAG_PREFIXES
                    .iter()
                    .any(|prefix| lower.starts_with(prefix))
            } else {
                // Empty tags are kept (harmless, no metadata).
                true
            }
        })
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- PrivacyTransforms defaults --

    #[test]
    fn default_transforms_all_disabled() {
        let t = PrivacyTransforms::default();
        assert!(!t.randomize_timestamps);
        assert!(!t.strip_ip_metadata);
        assert!(!t.inject_decoy_events);
        assert!((t.decoy_rate - 0.0).abs() < f64::EPSILON);
    }

    // -- Validation --

    #[test]
    fn validate_accepts_zero_rate() {
        let t = PrivacyTransforms {
            decoy_rate: 0.0,
            ..Default::default()
        };
        assert!(t.validate().is_ok());
    }

    #[test]
    fn validate_accepts_one_rate() {
        let t = PrivacyTransforms {
            decoy_rate: 1.0,
            ..Default::default()
        };
        assert!(t.validate().is_ok());
    }

    #[test]
    fn validate_accepts_mid_rate() {
        let t = PrivacyTransforms {
            decoy_rate: 0.5,
            ..Default::default()
        };
        assert!(t.validate().is_ok());
    }

    #[test]
    fn validate_rejects_negative_rate() {
        let t = PrivacyTransforms {
            decoy_rate: -0.1,
            ..Default::default()
        };
        let err = t.validate().unwrap_err();
        assert!(err.contains("decoy_rate"));
    }

    #[test]
    fn validate_rejects_rate_above_one() {
        let t = PrivacyTransforms {
            decoy_rate: 1.5,
            ..Default::default()
        };
        let err = t.validate().unwrap_err();
        assert!(err.contains("decoy_rate"));
    }

    // -- Timestamp jitter --

    #[test]
    fn timestamp_jitter_within_bounds() {
        let base = 1_700_000_000i64;
        for _ in 0..100 {
            let jittered = apply_timestamp_jitter(base);
            let diff = (jittered - base).unsigned_abs();
            assert!(
                (MIN_JITTER_SECS as u64..=MAX_JITTER_SECS as u64).contains(&diff),
                "jitter {diff} out of bounds [{MIN_JITTER_SECS}, {MAX_JITTER_SECS}]",
            );
        }
    }

    #[test]
    fn timestamp_jitter_varies() {
        let base = 1_700_000_000i64;
        let results: Vec<i64> = (0..20).map(|_| apply_timestamp_jitter(base)).collect();
        // With 20 samples, not all should be identical.
        let unique: std::collections::HashSet<i64> = results.into_iter().collect();
        assert!(unique.len() > 1, "jitter should produce varied results");
    }

    #[test]
    fn timestamp_jitter_saturates_at_max() {
        // Verify no panic on i64::MAX — saturation keeps the value in range.
        let jittered = apply_timestamp_jitter(i64::MAX);
        let diff = i64::MAX - jittered;
        // Jitter is ±5-60s. When saturating_add overflows, result is i64::MAX.
        // When saturating_sub succeeds, diff is 5-60.
        assert!(
            diff == 0 || (MIN_JITTER_SECS..=MAX_JITTER_SECS).contains(&diff),
            "jittered diff {diff} not in expected range",
        );
    }

    #[test]
    fn timestamp_jitter_saturates_at_min() {
        // Verify no panic on i64::MIN — saturation keeps the value in range.
        let jittered = apply_timestamp_jitter(i64::MIN);
        let diff = jittered - i64::MIN;
        // Jitter is ±5-60s. When saturating_sub overflows, result is i64::MIN.
        // When saturating_add succeeds, diff is 5-60.
        assert!(
            diff == 0 || (MIN_JITTER_SECS..=MAX_JITTER_SECS).contains(&diff),
            "jittered diff {diff} not in expected range",
        );
    }

    // -- Decoy injection --

    #[test]
    fn decoy_rate_zero_never_injects() {
        for _ in 0..100 {
            assert!(!should_inject_decoy(0.0));
        }
    }

    #[test]
    fn decoy_rate_one_always_injects() {
        for _ in 0..100 {
            assert!(should_inject_decoy(1.0));
        }
    }

    #[test]
    fn decoy_rate_mid_injects_sometimes() {
        let mut injected = 0;
        let trials = 1000;
        for _ in 0..trials {
            if should_inject_decoy(0.5) {
                injected += 1;
            }
        }
        // With 1000 trials at 0.5, expect roughly 500 ± some variance.
        // Use wide bounds to avoid flaky tests.
        assert!(injected > 100, "expected some injections at rate=0.5");
        assert!(injected < 900, "expected some non-injections at rate=0.5");
    }

    // -- Decoy event creation --

    #[test]
    fn decoy_event_non_empty() {
        let decoy = create_decoy_event();
        assert!(!decoy.is_empty());
    }

    #[test]
    fn decoy_event_reasonable_size() {
        for _ in 0..20 {
            let decoy = create_decoy_event();
            assert!(decoy.len() >= DECOY_MIN_BYTES);
            assert!(decoy.len() <= DECOY_MAX_BYTES);
        }
    }

    #[test]
    fn decoy_events_vary() {
        let a = create_decoy_event();
        let b = create_decoy_event();
        // Two random payloads should (almost certainly) differ.
        assert_ne!(a, b, "decoy events should be random");
    }

    // -- Metadata stripping --

    #[test]
    fn strip_removes_ip_tags() {
        let tags = vec![
            vec!["ip".into(), "1.2.3.4".into()],
            vec!["ip_address".into(), "5.6.7.8".into()],
            vec!["e".into(), "keep".into()],
        ];
        let cleaned = strip_metadata_tags(&tags);
        assert_eq!(cleaned.len(), 1);
        assert_eq!(cleaned[0][0], "e");
    }

    #[test]
    fn strip_removes_geo_tags() {
        let tags = vec![
            vec!["geo_lat".into(), "51.5".into()],
            vec!["geo_lon".into(), "-0.1".into()],
            vec!["p".into(), "keep".into()],
        ];
        let cleaned = strip_metadata_tags(&tags);
        assert_eq!(cleaned.len(), 1);
        assert_eq!(cleaned[0][0], "p");
    }

    #[test]
    fn strip_removes_location_tags() {
        let tags = vec![
            vec!["location".into(), "London".into()],
            vec!["locale".into(), "en-GB".into()],
            vec!["d".into(), "keep".into()],
        ];
        let cleaned = strip_metadata_tags(&tags);
        assert_eq!(cleaned.len(), 1);
        assert_eq!(cleaned[0][0], "d");
    }

    #[test]
    fn strip_case_insensitive() {
        let tags = vec![
            vec!["IP_SOURCE".into(), "1.2.3.4".into()],
            vec!["GeoLocation".into(), "NY".into()],
            vec!["t".into(), "keep".into()],
        ];
        let cleaned = strip_metadata_tags(&tags);
        assert_eq!(cleaned.len(), 1);
        assert_eq!(cleaned[0][0], "t");
    }

    #[test]
    fn strip_preserves_non_metadata_tags() {
        let tags = vec![
            vec!["p".into(), "pubkey".into()],
            vec!["e".into(), "event_id".into()],
            vec!["d".into(), "identifier".into()],
            vec!["t".into(), "topic".into()],
        ];
        let cleaned = strip_metadata_tags(&tags);
        assert_eq!(cleaned.len(), 4);
    }

    #[test]
    fn strip_handles_empty_tags() {
        let tags: Vec<Vec<String>> = vec![];
        let cleaned = strip_metadata_tags(&tags);
        assert!(cleaned.is_empty());
    }

    #[test]
    fn strip_handles_empty_tag_entries() {
        let tags = vec![vec![], vec!["e".into(), "keep".into()]];
        let cleaned = strip_metadata_tags(&tags);
        // Empty tag is kept (harmless), plus the "e" tag.
        assert_eq!(cleaned.len(), 2);
    }

    // -- Serde --

    #[test]
    fn transforms_serde_roundtrip() {
        let t = PrivacyTransforms {
            randomize_timestamps: true,
            strip_ip_metadata: true,
            inject_decoy_events: true,
            decoy_rate: 0.15,
        };
        let json = serde_json::to_string(&t).unwrap();
        let loaded: PrivacyTransforms = serde_json::from_str(&json).unwrap();
        assert_eq!(t, loaded);
    }

    #[test]
    fn transforms_default_serde() {
        let t = PrivacyTransforms::default();
        let json = serde_json::to_string(&t).unwrap();
        let loaded: PrivacyTransforms = serde_json::from_str(&json).unwrap();
        assert_eq!(t, loaded);
    }
}
