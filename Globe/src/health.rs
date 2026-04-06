use std::collections::VecDeque;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

/// State of a single relay connection.
///
/// Progresses through: Disconnected -> Connecting -> Connected.
/// On failure: Connected -> Reconnecting -> Connected (or Failed after max attempts).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    /// Not connected. Initial state, or after explicit disconnect.
    Disconnected,
    /// WebSocket handshake is in progress.
    Connecting,
    /// WebSocket is open and active.
    Connected,
    /// Lost connection, trying to reconnect with exponential backoff.
    Reconnecting {
        /// Which reconnection attempt this is (1-based).
        attempt: u32,
    },
    /// All reconnection attempts exhausted. Manual intervention needed.
    Failed {
        /// Why reconnection was abandoned.
        reason: String,
    },
}

/// Health metrics for a single relay.
#[derive(Clone, Debug)]
pub struct RelayHealth {
    /// The relay URL.
    pub url: Url,
    /// Current connection state.
    pub state: ConnectionState,
    /// When the current connection was established.
    pub connected_since: Option<DateTime<Utc>>,
    /// When the last message was sent or received.
    pub last_activity: Option<DateTime<Utc>>,
    /// Total messages sent.
    pub send_count: u64,
    /// Total messages received.
    pub receive_count: u64,
    /// Total errors encountered.
    pub error_count: u64,
    /// Rolling window of recent latencies (max 100 samples).
    latencies: VecDeque<Duration>,
}

const MAX_LATENCY_SAMPLES: usize = 100;

impl RelayHealth {
    /// Create a new health tracker for a relay.
    pub fn new(url: Url) -> Self {
        Self {
            url,
            state: ConnectionState::Disconnected,
            connected_since: None,
            last_activity: None,
            send_count: 0,
            receive_count: 0,
            error_count: 0,
            latencies: VecDeque::with_capacity(MAX_LATENCY_SAMPLES),
        }
    }

    /// Record a sent message.
    pub fn record_send(&mut self) {
        self.send_count += 1;
        self.last_activity = Some(Utc::now());
    }

    /// Record a received message.
    pub fn record_receive(&mut self) {
        self.receive_count += 1;
        self.last_activity = Some(Utc::now());
    }

    /// Record an error.
    pub fn record_error(&mut self) {
        self.error_count += 1;
    }

    /// Record a round-trip latency measurement.
    pub fn record_latency(&mut self, latency: Duration) {
        if self.latencies.len() >= MAX_LATENCY_SAMPLES {
            self.latencies.pop_front();
        }
        self.latencies.push_back(latency);
    }

    /// Average latency over the rolling window.
    pub fn average_latency(&self) -> Option<Duration> {
        if self.latencies.is_empty() {
            return None;
        }
        let total: Duration = self.latencies.iter().sum();
        Some(total / self.latencies.len() as u32)
    }

    /// Composite health score (0.0 = worst, 1.0 = best).
    ///
    /// Considers:
    /// - Connected state (0.0 if not connected)
    /// - Error rate (fewer errors = higher score)
    /// - Average latency (lower = higher score)
    pub fn score(&self) -> f64 {
        if self.state != ConnectionState::Connected {
            return 0.0;
        }

        let total_messages = self.send_count + self.receive_count;
        let error_score = if total_messages > 0 {
            1.0 - (self.error_count as f64 / total_messages as f64).min(1.0)
        } else {
            0.5 // No data yet.
        };

        let latency_score = match self.average_latency() {
            Some(avg) => {
                let ms = avg.as_millis() as f64;
                // 0ms = 1.0, 1000ms = 0.5, 5000ms+ = ~0.0
                1.0 / (1.0 + ms / 1000.0)
            }
            None => 0.5, // No data yet.
        };

        // Weighted: 60% error rate, 40% latency.
        error_score * 0.6 + latency_score * 0.4
    }

    /// Total number of latency samples collected.
    pub fn latency_sample_count(&self) -> usize {
        self.latencies.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_url() -> Url {
        Url::parse("wss://relay.example.com").unwrap()
    }

    #[test]
    fn new_health_is_disconnected() {
        let health = RelayHealth::new(test_url());
        assert_eq!(health.state, ConnectionState::Disconnected);
        assert_eq!(health.send_count, 0);
        assert_eq!(health.receive_count, 0);
        assert_eq!(health.error_count, 0);
        assert!(health.average_latency().is_none());
    }

    #[test]
    fn record_send_receive_updates_counts() {
        let mut health = RelayHealth::new(test_url());
        health.record_send();
        health.record_send();
        health.record_receive();
        assert_eq!(health.send_count, 2);
        assert_eq!(health.receive_count, 1);
        assert!(health.last_activity.is_some());
    }

    #[test]
    fn latency_tracking() {
        let mut health = RelayHealth::new(test_url());
        health.record_latency(Duration::from_millis(100));
        health.record_latency(Duration::from_millis(200));
        health.record_latency(Duration::from_millis(300));

        let avg = health.average_latency().unwrap();
        assert_eq!(avg, Duration::from_millis(200));
        assert_eq!(health.latency_sample_count(), 3);
    }

    #[test]
    fn latency_window_caps_at_max() {
        let mut health = RelayHealth::new(test_url());
        for i in 0..150 {
            health.record_latency(Duration::from_millis(i));
        }
        assert_eq!(health.latency_sample_count(), MAX_LATENCY_SAMPLES);
    }

    #[test]
    fn disconnected_score_is_zero() {
        let health = RelayHealth::new(test_url());
        assert_eq!(health.score(), 0.0);
    }

    #[test]
    fn connected_with_no_errors_scores_high() {
        let mut health = RelayHealth::new(test_url());
        health.state = ConnectionState::Connected;
        health.record_send();
        health.record_receive();
        health.record_latency(Duration::from_millis(50));

        let score = health.score();
        assert!(score > 0.8, "score was {score}");
    }

    #[test]
    fn connection_state_serde() {
        let state = ConnectionState::Reconnecting { attempt: 3 };
        let json = serde_json::to_string(&state).unwrap();
        let loaded: ConnectionState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, loaded);
    }
}
