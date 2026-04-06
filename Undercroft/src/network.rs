//! Network health aggregation.
//!
//! Aggregates relay health data from Globe into deidentified metrics.
//! NO relay URLs are stored. NO relay identities. Just counts and averages.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use globe::{ConnectionState, RelayHealth};

/// Aggregated health of the relay network.
///
/// All data is deidentified: counts and averages only. No relay URLs,
/// no individual relay identifiers, no connection metadata.
///
/// # Examples
///
/// ```
/// use undercroft::NetworkHealth;
///
/// let health = NetworkHealth::empty();
/// assert_eq!(health.relay_count, 0);
/// assert_eq!(health.connected_count, 0);
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NetworkHealth {
    /// Total number of relays observed.
    pub relay_count: usize,
    /// Number of relays currently connected.
    pub connected_count: usize,
    /// Average composite health score across all relays (0.0-1.0).
    pub average_score: f64,
    /// Total messages sent across all relays.
    pub total_send_count: u64,
    /// Total messages received across all relays.
    pub total_receive_count: u64,
    /// Total errors across all relays.
    pub total_error_count: u64,
    /// Average latency in milliseconds (None if no latency data).
    pub average_latency_ms: Option<f64>,
    /// Number of relays operating in Intermediary mode.
    #[serde(default)]
    pub intermediary_relay_count: usize,
    /// Number of active privacy routes (using intermediaries).
    #[serde(default)]
    pub privacy_routes_active: usize,
    /// Average overhead in milliseconds from privacy routing (None if no data).
    #[serde(default)]
    pub average_privacy_overhead_ms: Option<f64>,
    /// When this snapshot was computed.
    pub computed_at: DateTime<Utc>,
}

impl NetworkHealth {
    /// Aggregate health from a slice of relay health records.
    ///
    /// Extracts only aggregate counts and averages. The relay URLs and
    /// identities in `RelayHealth` are deliberately not stored -- this
    /// is a Covenant requirement for deidentification.
    ///
    /// # Examples
    ///
    /// ```
    /// use undercroft::NetworkHealth;
    ///
    /// // From empty relays
    /// let health = NetworkHealth::from_relay_health(&[]);
    /// assert_eq!(health.relay_count, 0);
    /// assert_eq!(health.average_score, 0.0);
    /// ```
    #[must_use]
    pub fn from_relay_health(relays: &[RelayHealth]) -> Self {
        if relays.is_empty() {
            return Self::empty();
        }

        let relay_count = relays.len();
        let connected_count = relays
            .iter()
            .filter(|r| r.state == ConnectionState::Connected)
            .count();

        let total_send_count: u64 = relays.iter().map(|r| r.send_count).sum();
        let total_receive_count: u64 = relays.iter().map(|r| r.receive_count).sum();
        let total_error_count: u64 = relays.iter().map(|r| r.error_count).sum();

        let score_sum: f64 = relays.iter().map(|r| r.score()).sum();
        let average_score = score_sum / relay_count as f64;

        // Average latency across relays that have latency data.
        let latency_values: Vec<f64> = relays
            .iter()
            .filter_map(|r| r.average_latency())
            .map(|d| d.as_secs_f64() * 1000.0)
            .collect();

        let average_latency_ms = if latency_values.is_empty() {
            None
        } else {
            let sum: f64 = latency_values.iter().sum();
            Some(sum / latency_values.len() as f64)
        };

        Self {
            relay_count,
            connected_count,
            average_score,
            total_send_count,
            total_receive_count,
            total_error_count,
            average_latency_ms,
            intermediary_relay_count: 0,
            privacy_routes_active: 0,
            average_privacy_overhead_ms: None,
            computed_at: Utc::now(),
        }
    }

    /// An empty network health snapshot (no relays).
    ///
    /// # Examples
    ///
    /// ```
    /// use undercroft::NetworkHealth;
    ///
    /// let health = NetworkHealth::empty();
    /// assert_eq!(health.relay_count, 0);
    /// assert!(health.average_latency_ms.is_none());
    /// ```
    #[must_use]
    pub fn empty() -> Self {
        Self {
            relay_count: 0,
            connected_count: 0,
            average_score: 0.0,
            total_send_count: 0,
            total_receive_count: 0,
            total_error_count: 0,
            average_latency_ms: None,
            intermediary_relay_count: 0,
            privacy_routes_active: 0,
            average_privacy_overhead_ms: None,
            computed_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use url::Url;

    fn test_url(n: u32) -> Url {
        Url::parse(&format!("wss://relay{n}.example.com")).unwrap()
    }

    #[test]
    fn empty_relays_produce_empty_health() {
        let health = NetworkHealth::from_relay_health(&[]);
        assert_eq!(health.relay_count, 0);
        assert_eq!(health.connected_count, 0);
        assert_eq!(health.average_score, 0.0);
        assert_eq!(health.total_send_count, 0);
        assert_eq!(health.total_receive_count, 0);
        assert_eq!(health.total_error_count, 0);
        assert!(health.average_latency_ms.is_none());
    }

    #[test]
    fn single_connected_relay() {
        let mut r = RelayHealth::new(test_url(1));
        r.state = ConnectionState::Connected;
        r.record_send();
        r.record_send();
        r.record_receive();
        r.record_latency(Duration::from_millis(50));

        let health = NetworkHealth::from_relay_health(&[r]);
        assert_eq!(health.relay_count, 1);
        assert_eq!(health.connected_count, 1);
        assert_eq!(health.total_send_count, 2);
        assert_eq!(health.total_receive_count, 1);
        assert_eq!(health.total_error_count, 0);
        assert!(health.average_score > 0.0);
        assert!(health.average_latency_ms.is_some());
        let latency = health.average_latency_ms.unwrap();
        assert!((latency - 50.0).abs() < 1.0);
    }

    #[test]
    fn mixed_connected_and_disconnected() {
        let mut r1 = RelayHealth::new(test_url(1));
        r1.state = ConnectionState::Connected;
        r1.record_send();
        r1.record_receive();
        r1.record_latency(Duration::from_millis(100));

        let r2 = RelayHealth::new(test_url(2));
        // r2 is disconnected by default

        let mut r3 = RelayHealth::new(test_url(3));
        r3.state = ConnectionState::Connected;
        r3.record_send();
        r3.record_receive();
        r3.record_receive();
        r3.record_latency(Duration::from_millis(200));

        let health = NetworkHealth::from_relay_health(&[r1, r2, r3]);
        assert_eq!(health.relay_count, 3);
        assert_eq!(health.connected_count, 2);
        assert_eq!(health.total_send_count, 2);
        assert_eq!(health.total_receive_count, 3);
        assert_eq!(health.total_error_count, 0);

        // Average latency should be (100 + 200) / 2 = 150 ms
        let latency = health.average_latency_ms.unwrap();
        assert!((latency - 150.0).abs() < 1.0);
    }

    #[test]
    fn error_counts_aggregate() {
        let mut r1 = RelayHealth::new(test_url(1));
        r1.state = ConnectionState::Connected;
        r1.record_error();
        r1.record_error();

        let mut r2 = RelayHealth::new(test_url(2));
        r2.state = ConnectionState::Connected;
        r2.record_error();

        let health = NetworkHealth::from_relay_health(&[r1, r2]);
        assert_eq!(health.total_error_count, 3);
    }

    #[test]
    fn no_latency_data_produces_none() {
        let mut r = RelayHealth::new(test_url(1));
        r.state = ConnectionState::Connected;
        r.record_send();

        let health = NetworkHealth::from_relay_health(&[r]);
        assert!(health.average_latency_ms.is_none());
    }

    #[test]
    fn no_urls_in_serialized_output() {
        let mut r = RelayHealth::new(test_url(1));
        r.state = ConnectionState::Connected;
        r.record_send();

        let health = NetworkHealth::from_relay_health(&[r]);
        let json = serde_json::to_string(&health).unwrap();

        // Verify no relay URL leaked into the output
        assert!(!json.contains("relay1.example.com"));
        assert!(!json.contains("wss://"));
    }

    #[test]
    fn serde_round_trip() {
        let mut r = RelayHealth::new(test_url(1));
        r.state = ConnectionState::Connected;
        r.record_send();
        r.record_latency(Duration::from_millis(42));

        let health = NetworkHealth::from_relay_health(&[r]);
        let json = serde_json::to_string(&health).unwrap();
        let restored: NetworkHealth = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.relay_count, health.relay_count);
        assert_eq!(restored.connected_count, health.connected_count);
        assert_eq!(restored.total_send_count, health.total_send_count);
        assert_eq!(restored.average_latency_ms, health.average_latency_ms);
    }

    #[test]
    fn empty_serde_round_trip() {
        let health = NetworkHealth::empty();
        let json = serde_json::to_string(&health).unwrap();
        let restored: NetworkHealth = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.relay_count, 0);
        assert_eq!(restored.connected_count, 0);
        assert!(restored.average_latency_ms.is_none());
    }
}
