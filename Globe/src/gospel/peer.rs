use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use url::Url;

use crate::client::connection::{RelayEvent, RelayHandle};
use crate::config::GlobeConfig;
use crate::error::GlobeError;
use crate::health::ConnectionState;

use super::digest::SemanticDigest;
use super::registry::{GospelRegistry, InsertResult};
use super::sync::GospelSync;
use super::tier::GospelTier;

/// Tracks sync state with a single peer relay.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PeerState {
    /// The peer's relay URL.
    pub url: Url,
    /// Unix timestamp of the last successful evangelization.
    pub last_evangelized_at: Option<i64>,
    /// Number of events received from this peer.
    pub events_received: u64,
    /// Number of events sent to this peer.
    pub events_sent: u64,
    /// Number of evangelize cycles completed.
    pub evangelize_count: u64,
}

/// A relay-to-relay peering connection for gospel evangelization.
///
/// Wraps a `RelayHandle` and tracks sync state. Supports two modes:
///
/// - **Bilateral sync** (`evangelize()`) — full catch-up sync on a timer.
/// - **Live sync** (`open_live_subscription()` + `recv_live()`) — persistent
///   subscription that receives new gospel events in real-time.
///
/// Both modes are tier-aware: only kinds matching the configured tiers
/// are requested and propagated.
pub struct GospelPeer {
    /// The underlying relay connection.
    handle: RelayHandle,
    /// Sync state.
    state: PeerState,
    /// Which gospel tiers to propagate with this peer.
    tiers: Vec<GospelTier>,
    /// Broadcast receiver for live gospel events (persistent subscription).
    live_rx: Option<broadcast::Receiver<RelayEvent>>,
    /// Subscription ID for the live gospel subscription.
    live_sub_id: Option<String>,
    /// Local semantic digest to exchange during peering.
    local_digest: SemanticDigest,
}

impl GospelPeer {
    /// Create a new gospel peer connection with tier configuration.
    pub fn new(url: Url, globe_config: Arc<GlobeConfig>, tiers: Vec<GospelTier>) -> Self {
        let handle = RelayHandle::new(url.clone(), globe_config);
        Self {
            handle,
            state: PeerState {
                url,
                last_evangelized_at: None,
                events_received: 0,
                events_sent: 0,
                evangelize_count: 0,
            },
            tiers,
            live_rx: None,
            live_sub_id: None,
            local_digest: SemanticDigest::empty(),
        }
    }

    /// Create from an existing `RelayHandle` (for testing or reuse).
    pub fn from_handle(handle: RelayHandle, tiers: Vec<GospelTier>) -> Self {
        let url = handle.url().clone();
        Self {
            handle,
            state: PeerState {
                url,
                last_evangelized_at: None,
                events_received: 0,
                events_sent: 0,
                evangelize_count: 0,
            },
            tiers,
            live_rx: None,
            live_sub_id: None,
            local_digest: SemanticDigest::empty(),
        }
    }

    /// The peer's URL.
    pub fn url(&self) -> &Url {
        &self.state.url
    }

    /// Current sync state.
    pub fn state(&self) -> &PeerState {
        &self.state
    }

    /// Whether the peer connection is alive.
    pub fn is_connected(&self) -> bool {
        matches!(self.handle.state(), ConnectionState::Connected)
    }

    /// The underlying relay handle (for direct protocol access).
    pub fn handle(&self) -> &RelayHandle {
        &self.handle
    }

    /// Which tiers this peer propagates.
    pub fn tiers(&self) -> &[GospelTier] {
        &self.tiers
    }

    /// Whether a live subscription is currently active.
    pub fn has_live_subscription(&self) -> bool {
        self.live_sub_id.is_some()
    }

    /// Set the local semantic digest to exchange during peering.
    pub fn set_digest(&mut self, digest: SemanticDigest) {
        self.local_digest = digest;
    }

    /// Exchange semantic digests with this peer.
    ///
    /// Stub: returns an empty digest. When Towers implement digest
    /// exchange over the wire, this will send our local digest and
    /// receive theirs.
    pub fn exchange_digests(&self) -> SemanticDigest {
        let _ = &self.local_digest;
        SemanticDigest::empty()
    }

    // =================================================================
    // Live Sync
    // =================================================================

    /// Open a persistent subscription for gospel events from this peer.
    ///
    /// The subscription stays open — new gospel events are delivered in
    /// real-time via the relay's live broadcast. Call `recv_live()` to
    /// drain incoming events into the local registry.
    ///
    /// If a live subscription already exists, it is replaced.
    pub async fn open_live_subscription(&mut self) -> Result<(), GlobeError> {
        // Close existing subscription if any.
        if let Some(ref sub_id) = self.live_sub_id {
            let _ = self.handle.unsubscribe(sub_id.clone()).await;
        }

        let filter = GospelSync::sync_filter_for_tiers(
            self.state.last_evangelized_at,
            &self.tiers,
        );
        let sub_id = format!("gospel-live-{}", Utc::now().timestamp_millis());

        // Subscribe to the handle's event broadcast BEFORE sending REQ.
        self.live_rx = Some(self.handle.subscribe_events());
        self.handle
            .subscribe(sub_id.clone(), vec![filter])
            .await?;
        self.live_sub_id = Some(sub_id);

        Ok(())
    }

    /// Drain live gospel events from the persistent subscription.
    ///
    /// Non-blocking: processes all available events and returns immediately.
    /// Returns the number of new events merged into the registry.
    pub fn recv_live(&mut self, registry: &GospelRegistry) -> usize {
        let rx = match &mut self.live_rx {
            Some(rx) => rx,
            None => return 0,
        };

        let mut count = 0;
        loop {
            match rx.try_recv() {
                Ok(RelayEvent::Event {
                    event,
                    subscription_id,
                    ..
                }) => {
                    if self.live_sub_id.as_ref() == Some(&subscription_id) {
                        if registry.insert(&event) == InsertResult::Inserted {
                            count += 1;
                        }
                        self.state.events_received += 1;
                    }
                }
                Ok(RelayEvent::StoredComplete { .. }) => continue,
                Err(broadcast::error::TryRecvError::Empty) => break,
                Err(broadcast::error::TryRecvError::Lagged(n)) => {
                    log::warn!(
                        "gospel live sync lagged {n} events from {}",
                        self.state.url
                    );
                    continue;
                }
                Err(broadcast::error::TryRecvError::Closed) => {
                    self.live_rx = None;
                    self.live_sub_id = None;
                    break;
                }
            }
        }
        count
    }

    /// Push gospel events to this peer (tier-filtered).
    ///
    /// Only publishes events whose kind matches this peer's configured tiers.
    /// Returns the number of events successfully published.
    pub async fn push_events(&mut self, events: &[crate::event::OmniEvent]) -> usize {
        use super::tier::gospel_tier;
        let mut sent = 0;
        for event in events {
            if self.tiers.contains(&gospel_tier(event.kind))
                && self.handle.publish(event.clone()).await.is_ok()
            {
                sent += 1;
            }
        }
        self.state.events_sent += sent as u64;
        sent
    }

    // =================================================================
    // Bilateral Sync (catch-up)
    // =================================================================

    /// Perform a bilateral evangelization cycle (tier-aware).
    ///
    /// 1. Build a tier-aware sync filter (gospel events since our last sync).
    /// 2. Subscribe to event broadcast (before sending REQ to avoid race).
    /// 3. Send subscription request to the relay.
    /// 4. Collect received events (until STORED marker).
    /// 5. Merge received events into our local registry.
    /// 6. Compute events the peer is missing (tier-filtered diff).
    /// 7. Publish those events to the peer.
    /// 8. Update our sync cursor.
    ///
    /// Returns `(received_count, sent_count)`.
    pub async fn evangelize(
        &mut self,
        registry: &GospelRegistry,
    ) -> Result<(usize, usize), GlobeError> {
        // Step 1: Build tier-aware sync filter.
        let filter = GospelSync::sync_filter_for_tiers(
            self.state.last_evangelized_at,
            &self.tiers,
        );
        let sub_id = format!("gospel-{}", Utc::now().timestamp_millis());

        // Step 2: Subscribe to event broadcast BEFORE sending REQ
        // to avoid missing events that arrive between send and subscribe.
        let mut rx = self.handle.subscribe_events();

        // Step 3: Send the subscription request to the relay.
        self.handle
            .subscribe(sub_id.clone(), vec![filter])
            .await?;
        let mut received_events = Vec::new();
        let timeout = Duration::from_secs(10);

        loop {
            match tokio::time::timeout(timeout, rx.recv()).await {
                Ok(Ok(RelayEvent::Event {
                    subscription_id,
                    event,
                    ..
                })) => {
                    if subscription_id == sub_id {
                        received_events.push(event);
                    }
                }
                Ok(Ok(RelayEvent::StoredComplete {
                    subscription_id, ..
                })) => {
                    if subscription_id == sub_id {
                        break;
                    }
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(n))) => {
                    log::warn!("gospel sync lagged {n} events");
                    continue;
                }
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) | Err(_) => {
                    break;
                }
            }
        }

        // Step 4: Merge received events into our registry.
        let stats = GospelSync::merge_events(registry, &received_events);
        self.state.events_received += received_events.len() as u64;

        // Step 5: Compute what we have that they need (tier-aware).
        let our_events = registry.events_since_for_tiers(
            self.state.last_evangelized_at.unwrap_or(0),
            &self.tiers,
        );
        let their_ids: Vec<String> = received_events.iter().map(|e| e.id.clone()).collect();
        let to_send = GospelSync::diff(&our_events, &their_ids);

        // Step 6: Publish our events to the peer.
        let mut sent_count = 0;
        for event in &to_send {
            if self.handle.publish(event.clone()).await.is_ok() {
                sent_count += 1;
            }
        }
        self.state.events_sent += sent_count as u64;

        // Step 7: Close subscription and update cursor.
        let _ = self.handle.unsubscribe(sub_id).await;
        self.state.last_evangelized_at = Some(Utc::now().timestamp());
        self.state.evangelize_count += 1;

        Ok((stats.inserted, sent_count))
    }

    /// Disconnect from the peer.
    pub async fn disconnect(&self) -> Result<(), GlobeError> {
        self.handle.disconnect().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_url() -> Url {
        Url::parse("wss://relay.test.com").unwrap()
    }

    fn test_config() -> Arc<GlobeConfig> {
        Arc::new(GlobeConfig::default())
    }

    #[tokio::test]
    async fn new_peer_state() {
        let peer = GospelPeer::new(test_url(), test_config(), GospelTier::all());
        assert_eq!(peer.url().as_str(), "wss://relay.test.com/");
        assert!(peer.state().last_evangelized_at.is_none());
        assert_eq!(peer.state().events_received, 0);
        assert_eq!(peer.state().events_sent, 0);
        assert_eq!(peer.state().evangelize_count, 0);
        assert_eq!(peer.tiers().len(), 3);
    }

    #[tokio::test]
    async fn peer_from_handle() {
        let handle = RelayHandle::new(test_url(), test_config());
        let peer = GospelPeer::from_handle(handle, GospelTier::all());
        assert_eq!(peer.url().as_str(), "wss://relay.test.com/");
    }

    #[tokio::test]
    async fn peer_with_universal_only() {
        let peer = GospelPeer::new(
            test_url(),
            test_config(),
            vec![GospelTier::Universal],
        );
        assert_eq!(peer.tiers().len(), 1);
        assert_eq!(peer.tiers()[0], GospelTier::Universal);
        assert!(!peer.has_live_subscription());
    }

    #[test]
    fn peer_state_serde_round_trip() {
        let state = PeerState {
            url: test_url(),
            last_evangelized_at: Some(1000),
            events_received: 42,
            events_sent: 7,
            evangelize_count: 3,
        };
        let json = serde_json::to_string(&state).unwrap();
        let loaded: PeerState = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.url, state.url);
        assert_eq!(loaded.last_evangelized_at, Some(1000));
        assert_eq!(loaded.events_received, 42);
        assert_eq!(loaded.evangelize_count, 3);
    }

    #[tokio::test]
    async fn recv_live_without_subscription_returns_zero() {
        let config = super::super::config::GospelConfig::default();
        let registry = GospelRegistry::new(&config);
        let mut peer = GospelPeer::new(test_url(), test_config(), GospelTier::all());
        assert_eq!(peer.recv_live(&registry), 0);
    }
}
