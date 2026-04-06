use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use lru::LruCache;
use tokio::sync::broadcast;
use url::Url;

use crate::client::connection::{RelayEvent, RelayHandle};
use crate::config::GlobeConfig;
use crate::error::GlobeError;
use crate::event::OmniEvent;
use crate::filter::OmniFilter;
use crate::health::RelayHealth;

/// An event received from the pool, deduplicated across relays.
///
/// When multiple relays deliver the same event, only the first arrival
/// is broadcast as a `PoolEvent`. The `source_relay` records which relay
/// won the race.
#[derive(Clone, Debug)]
pub struct PoolEvent {
    /// The deduplicated event.
    pub event: OmniEvent,
    /// Which relay delivered it first.
    pub source_relay: Url,
}

/// Tracks a subscription across all relays.
struct PoolSubscription {
    filters: Vec<OmniFilter>,
}

/// Multi-relay coordinator.
///
/// Manages connections to multiple relays, deduplicates incoming events,
/// publishes to all relays concurrently, and aggregates subscriptions.
/// Events from individual handles are automatically forwarded through
/// deduplication to the pool broadcast.
pub struct RelayPool {
    handles: HashMap<Url, RelayHandle>,
    subscriptions: HashMap<String, PoolSubscription>,
    seen_events: Arc<Mutex<LruCache<String, ()>>>,
    config: Arc<GlobeConfig>,
    event_tx: broadcast::Sender<PoolEvent>,
    sub_counter: AtomicU64,
}

impl RelayPool {
    /// Create a new relay pool.
    pub fn new(config: GlobeConfig) -> Self {
        // Safety: 10_000 is a non-zero literal.
        let max_seen = NonZeroUsize::new(config.max_seen_events)
            .unwrap_or(NonZeroUsize::new(10_000).expect("10_000 is non-zero"));
        let (event_tx, _) = broadcast::channel(4096);

        Self {
            handles: HashMap::new(),
            subscriptions: HashMap::new(),
            seen_events: Arc::new(Mutex::new(LruCache::new(max_seen))),
            config: Arc::new(config),
            event_tx,
            sub_counter: AtomicU64::new(0),
        }
    }

    /// Add a relay, connect to it, and start forwarding its events.
    ///
    /// Existing pool subscriptions are automatically sent to the new relay.
    pub fn add_relay(&mut self, url: Url) -> Result<(), GlobeError> {
        if self.handles.len() >= self.config.max_relays {
            return Err(GlobeError::InvalidConfig(format!(
                "max relays ({}) reached",
                self.config.max_relays
            )));
        }
        if self.handles.contains_key(&url) {
            return Ok(());
        }

        let handle = RelayHandle::new(url.clone(), self.config.clone());

        // Spawn a forwarding task: relay events → pool broadcast (with dedup).
        let mut event_rx = handle.subscribe_events();
        let pool_tx = self.event_tx.clone();
        let seen = self.seen_events.clone();
        let relay_url = url.clone();
        tokio::spawn(async move {
            loop {
                match event_rx.recv().await {
                    Ok(RelayEvent::Event { event, .. }) => {
                        let is_new = {
                            let mut cache = seen.lock().unwrap_or_else(|e| e.into_inner());
                            if cache.get(&event.id).is_some() {
                                false
                            } else {
                                cache.push(event.id.clone(), ());
                                true
                            }
                        };
                        if is_new {
                            let _ = pool_tx.send(PoolEvent {
                                event,
                                source_relay: relay_url.clone(),
                            });
                        }
                    }
                    Ok(RelayEvent::StoredComplete { .. }) => {}
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        log::warn!("pool forwarding lagged {n} events for {relay_url}");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        // Send existing subscriptions to the new handle.
        if !self.subscriptions.is_empty() {
            let handle_clone = handle.clone();
            let subs: Vec<(String, Vec<OmniFilter>)> = self
                .subscriptions
                .iter()
                .map(|(id, sub)| (id.clone(), sub.filters.clone()))
                .collect();
            tokio::spawn(async move {
                for (id, filters) in subs {
                    let _ = handle_clone.subscribe(id, filters).await;
                }
            });
        }

        self.handles.insert(url, handle);
        Ok(())
    }

    /// Remove a relay and disconnect.
    pub fn remove_relay(&mut self, url: &Url) {
        self.handles.remove(url);
    }

    /// Publish an event to all connected relays.
    ///
    /// Pre-registers the event ID for dedup so we don't receive our
    /// own event back through the forwarding tasks.
    /// Succeeds if at least one relay accepts (at-least-once semantics).
    pub async fn publish(&self, event: OmniEvent) -> Result<(), GlobeError> {
        if self.handles.is_empty() {
            return Err(GlobeError::NotConnected);
        }

        // Pre-register for dedup — avoid echoing our own event.
        {
            let mut cache = self.seen_events.lock().unwrap_or_else(|e| e.into_inner());
            cache.push(event.id.clone(), ());
        }

        let mut last_error = None;
        let mut any_success = false;

        for handle in self.handles.values() {
            match handle.publish(event.clone()).await {
                Ok(()) => any_success = true,
                Err(e) => last_error = Some(e),
            }
        }

        if any_success {
            Ok(())
        } else {
            Err(last_error
                .unwrap_or_else(|| GlobeError::PublishFailed("all relays failed".into())))
        }
    }

    /// Subscribe to events matching the given filters.
    ///
    /// Sends the subscription to all connected relays and returns a
    /// broadcast receiver for deduplicated pool events.
    pub fn subscribe(
        &mut self,
        filters: Vec<OmniFilter>,
    ) -> (String, broadcast::Receiver<PoolEvent>) {
        let id = format!(
            "sub-{}",
            self.sub_counter.fetch_add(1, Ordering::Relaxed)
        );
        let rx = self.event_tx.subscribe();

        // Send to all current handles.
        for handle in self.handles.values() {
            let h = handle.clone();
            let sub_id = id.clone();
            let f = filters.clone();
            tokio::spawn(async move {
                let _ = h.subscribe(sub_id, f).await;
            });
        }

        self.subscriptions.insert(
            id.clone(),
            PoolSubscription {
                filters,
            },
        );

        (id, rx)
    }

    /// Unsubscribe from a subscription.
    pub fn unsubscribe(&mut self, id: &str) {
        // Send CLOSE to all handles.
        for handle in self.handles.values() {
            let h = handle.clone();
            let sub_id = id.to_string();
            tokio::spawn(async move {
                let _ = h.unsubscribe(sub_id).await;
            });
        }
        self.subscriptions.remove(id);
    }

    /// Check if an event has been seen before (deduplication).
    pub fn is_duplicate(&self, event_id: &str) -> bool {
        let mut cache = self.seen_events.lock().unwrap_or_else(|e| e.into_inner());
        if cache.get(event_id).is_some() {
            return true;
        }
        cache.push(event_id.to_string(), ());
        false
    }

    /// Process an incoming event: deduplicate and broadcast.
    pub fn process_event(&self, event: OmniEvent, source_relay: Url) -> bool {
        if self.is_duplicate(&event.id) {
            return false;
        }
        let pool_event = PoolEvent {
            event,
            source_relay,
        };
        let _ = self.event_tx.send(pool_event);
        true
    }

    /// Get health for all relays.
    pub fn relay_health(&self) -> Vec<RelayHealth> {
        self.handles.values().map(|h| h.health()).collect()
    }

    /// Number of connected relays.
    pub fn relay_count(&self) -> usize {
        self.handles.len()
    }

    /// Whether any relays are configured.
    pub fn has_relays(&self) -> bool {
        !self.handles.is_empty()
    }

    /// Number of active subscriptions.
    pub fn subscription_count(&self) -> usize {
        self.subscriptions.len()
    }

    /// Get the filters for all active subscriptions.
    pub fn active_subscriptions(&self) -> Vec<(String, Vec<OmniFilter>)> {
        self.subscriptions
            .iter()
            .map(|(id, sub)| (id.clone(), sub.filters.clone()))
            .collect()
    }

    /// Subscribe to pool events.
    pub fn subscribe_events(&self) -> broadcast::Receiver<PoolEvent> {
        self.event_tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn test_event(id: &str) -> OmniEvent {
        OmniEvent {
            id: id.to_string(),
            author: "b".repeat(64),
            created_at: Utc::now().timestamp(),
            kind: 1,
            tags: vec![],
            content: "test".into(),
            sig: "c".repeat(128),
        }
    }

    #[test]
    fn new_pool_is_empty() {
        let pool = RelayPool::new(GlobeConfig::default());
        assert_eq!(pool.relay_count(), 0);
        assert_eq!(pool.subscription_count(), 0);
        assert!(!pool.has_relays());
    }

    #[tokio::test]
    async fn add_and_remove_relay() {
        let mut pool = RelayPool::new(GlobeConfig::default());
        let url = Url::parse("wss://relay.example.com").unwrap();

        pool.add_relay(url.clone()).unwrap();
        assert_eq!(pool.relay_count(), 1);
        assert!(pool.has_relays());

        pool.remove_relay(&url);
        assert_eq!(pool.relay_count(), 0);
    }

    #[tokio::test]
    async fn add_relay_respects_max() {
        let config = GlobeConfig {
            max_relays: 2,
            ..Default::default()
        };
        let mut pool = RelayPool::new(config);

        pool.add_relay(Url::parse("wss://r1.com").unwrap()).unwrap();
        pool.add_relay(Url::parse("wss://r2.com").unwrap()).unwrap();
        let result = pool.add_relay(Url::parse("wss://r3.com").unwrap());
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn add_duplicate_relay_is_noop() {
        let mut pool = RelayPool::new(GlobeConfig::default());
        let url = Url::parse("wss://relay.example.com").unwrap();

        pool.add_relay(url.clone()).unwrap();
        pool.add_relay(url).unwrap();
        assert_eq!(pool.relay_count(), 1);
    }

    #[test]
    fn deduplication_works() {
        let pool = RelayPool::new(GlobeConfig::default());

        assert!(!pool.is_duplicate("event1"));
        assert!(pool.is_duplicate("event1"));
        assert!(!pool.is_duplicate("event2"));
    }

    #[test]
    fn process_event_deduplicates_and_broadcasts() {
        let pool = RelayPool::new(GlobeConfig::default());
        let mut rx = pool.subscribe_events();
        let url = Url::parse("wss://relay.example.com").unwrap();

        let event = test_event(&"a".repeat(64));

        assert!(pool.process_event(event.clone(), url.clone()));
        assert!(!pool.process_event(event.clone(), url.clone()));

        let received = rx.try_recv().unwrap();
        assert_eq!(received.event.id, event.id);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn subscribe_and_unsubscribe() {
        let mut pool = RelayPool::new(GlobeConfig::default());

        let (id1, _rx1) = pool.subscribe(vec![OmniFilter::default()]);
        let (id2, _rx2) = pool.subscribe(vec![OmniFilter::default()]);

        assert_eq!(pool.subscription_count(), 2);
        assert_ne!(id1, id2);

        pool.unsubscribe(&id1);
        assert_eq!(pool.subscription_count(), 1);
    }

    #[test]
    fn subscription_ids_are_sequential() {
        let mut pool = RelayPool::new(GlobeConfig::default());

        let (id1, _) = pool.subscribe(vec![]);
        let (id2, _) = pool.subscribe(vec![]);
        let (id3, _) = pool.subscribe(vec![]);

        assert_eq!(id1, "sub-0");
        assert_eq!(id2, "sub-1");
        assert_eq!(id3, "sub-2");
    }
}
