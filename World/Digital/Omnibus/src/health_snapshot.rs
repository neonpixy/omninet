use chrono::{DateTime, Utc};
use globe::health::{ConnectionState, RelayHealth};
use serde::{Deserialize, Serialize};

/// A serializable summary of a relay's health.
///
/// Globe's `RelayHealth` contains a private `VecDeque<Duration>` for latency
/// tracking, so it can't derive Serialize. This snapshot captures everything
/// the console needs to display relay status.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RelayHealthSnapshot {
    /// The relay URL.
    pub url: String,
    /// Connection state as a string: "connected", "disconnected", "connecting",
    /// "reconnecting", or "failed".
    pub state: String,
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
    /// Average round-trip latency in milliseconds (over the rolling window).
    pub average_latency_ms: Option<f64>,
    /// Composite health score (0.0 = worst, 1.0 = best).
    pub score: f64,
}

/// Convert a `ConnectionState` to a human-readable string.
fn state_to_string(state: &ConnectionState) -> String {
    match state {
        ConnectionState::Disconnected => "disconnected".into(),
        ConnectionState::Connecting => "connecting".into(),
        ConnectionState::Connected => "connected".into(),
        ConnectionState::Reconnecting { .. } => "reconnecting".into(),
        ConnectionState::Failed { .. } => "failed".into(),
    }
}

impl From<&RelayHealth> for RelayHealthSnapshot {
    fn from(h: &RelayHealth) -> Self {
        Self {
            url: h.url.to_string(),
            state: state_to_string(&h.state),
            connected_since: h.connected_since,
            last_activity: h.last_activity,
            send_count: h.send_count,
            receive_count: h.receive_count,
            error_count: h.error_count,
            average_latency_ms: h.average_latency().map(|d| d.as_secs_f64() * 1000.0),
            score: h.score(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use url::Url;

    #[test]
    fn snapshot_from_relay_health() {
        let mut health = RelayHealth::new(Url::parse("wss://relay.example.com").unwrap());
        health.state = ConnectionState::Connected;
        health.record_send();
        health.record_send();
        health.record_receive();
        health.record_latency(std::time::Duration::from_millis(100));
        health.record_latency(std::time::Duration::from_millis(200));

        let snap = RelayHealthSnapshot::from(&health);
        assert_eq!(snap.url, "wss://relay.example.com/");
        assert_eq!(snap.state, "connected");
        assert_eq!(snap.send_count, 2);
        assert_eq!(snap.receive_count, 1);
        assert_eq!(snap.error_count, 0);
        assert!(snap.connected_since.is_none()); // We didn't set it.
        assert!(snap.last_activity.is_some()); // record_send/receive sets it.

        // Average of 100ms and 200ms = 150ms.
        let avg = snap.average_latency_ms.unwrap();
        assert!((avg - 150.0).abs() < 1.0, "expected ~150ms, got {avg}");

        assert!(snap.score > 0.0);
    }

    #[test]
    fn snapshot_from_disconnected_health() {
        let health = RelayHealth::new(Url::parse("wss://relay.example.com").unwrap());
        let snap = RelayHealthSnapshot::from(&health);

        assert_eq!(snap.state, "disconnected");
        assert_eq!(snap.send_count, 0);
        assert_eq!(snap.receive_count, 0);
        assert!(snap.average_latency_ms.is_none());
        assert_eq!(snap.score, 0.0);
    }

    #[test]
    fn snapshot_serde_round_trip() {
        let snap = RelayHealthSnapshot {
            url: "wss://relay.test.com".into(),
            state: "connected".into(),
            connected_since: Some(Utc::now()),
            last_activity: Some(Utc::now()),
            send_count: 42,
            receive_count: 100,
            error_count: 3,
            average_latency_ms: Some(55.5),
            score: 0.85,
        };

        let json = serde_json::to_string(&snap).unwrap();
        let loaded: RelayHealthSnapshot = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.url, snap.url);
        assert_eq!(loaded.state, snap.state);
        assert_eq!(loaded.send_count, snap.send_count);
        assert_eq!(loaded.receive_count, snap.receive_count);
        assert_eq!(loaded.error_count, snap.error_count);
        assert_eq!(loaded.average_latency_ms, snap.average_latency_ms);
        assert!((loaded.score - snap.score).abs() < f64::EPSILON);
    }

    #[test]
    fn state_strings() {
        assert_eq!(state_to_string(&ConnectionState::Disconnected), "disconnected");
        assert_eq!(state_to_string(&ConnectionState::Connecting), "connecting");
        assert_eq!(state_to_string(&ConnectionState::Connected), "connected");
        assert_eq!(
            state_to_string(&ConnectionState::Reconnecting { attempt: 5 }),
            "reconnecting"
        );
        assert_eq!(
            state_to_string(&ConnectionState::Failed {
                reason: "timeout".into()
            }),
            "failed"
        );
    }
}
