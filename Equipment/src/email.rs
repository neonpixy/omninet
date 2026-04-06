use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::de::DeserializeOwned;
use serde::Serialize;
use uuid::Uuid;

/// A broadcast event, routed by string ID.
///
/// Modules define event types by implementing this trait. The `EMAIL_ID`
/// is the routing key (convention: `"module.eventName"`).
///
/// ```ignore
/// struct DocumentChanged { idea_id: Uuid }
/// impl EmailEvent for DocumentChanged {
///     const EMAIL_ID: &'static str = "crdt.documentChanged";
/// }
/// ```
pub trait EmailEvent: Serialize + DeserializeOwned + Send + Sync {
    /// The routing key (convention: `"module.eventName"`).
    const EMAIL_ID: &'static str;
}

/// Raw subscriber handler: bytes in, nothing out.
type RawSubscriber = Arc<dyn Fn(&[u8]) + Send + Sync>;

/// Central hub for fire-and-forget event broadcasts.
///
/// Subscribers register by event ID. Senders broadcast to all subscribers.
/// Errors in handlers are silently ignored — that's the fire-and-forget contract.
pub struct Email {
    /// email_id -> (subscriber_id -> handler)
    subscribers: Mutex<HashMap<String, HashMap<Uuid, RawSubscriber>>>,
}

impl Email {
    /// Create a new Email hub with no subscribers.
    pub fn new() -> Self {
        Self {
            subscribers: Mutex::new(HashMap::new()),
        }
    }

    /// Subscribe with a typed handler. Returns a subscriber ID for unsubscription.
    pub fn subscribe<E>(
        &self,
        handler: impl Fn(E) + Send + Sync + 'static,
    ) -> Uuid
    where
        E: EmailEvent,
    {
        let raw_handler: RawSubscriber = Arc::new(move |data: &[u8]| {
            if let Ok(event) = serde_json::from_slice::<E>(data) {
                handler(event);
            }
            // Deserialization failures silently ignored (fire-and-forget).
        });

        let subscriber_id = Uuid::new_v4();
        self.subscribers
            .lock()
            .expect("subscribers mutex poisoned")
            .entry(E::EMAIL_ID.to_string())
            .or_default()
            .insert(subscriber_id, raw_handler);

        subscriber_id
    }

    /// Subscribe with a raw handler by string email ID. Returns a subscriber ID.
    pub fn subscribe_raw(
        &self,
        email_id: impl Into<String>,
        handler: impl Fn(&[u8]) + Send + Sync + 'static,
    ) -> Uuid {
        let subscriber_id = Uuid::new_v4();
        self.subscribers
            .lock()
            .expect("subscribers mutex poisoned")
            .entry(email_id.into())
            .or_default()
            .insert(subscriber_id, Arc::new(handler));

        subscriber_id
    }

    /// Send a typed event to all subscribers. Fire-and-forget.
    pub fn send<E: EmailEvent + Serialize>(&self, event: &E) {
        let data = match serde_json::to_vec(event) {
            Ok(d) => d,
            Err(_) => return, // Can't serialize — silently drop.
        };

        self.send_raw(E::EMAIL_ID, &data);
    }

    /// Send raw bytes to all subscribers of an email ID. Fire-and-forget.
    ///
    /// Subscribers are cloned out of the registry before calling,
    /// so the lock is not held during handler execution.
    pub fn send_raw(&self, email_id: &str, data: &[u8]) {
        let handlers: Vec<RawSubscriber> = {
            let subs = self.subscribers.lock().expect("subscribers mutex poisoned");
            subs.get(email_id)
                .map(|m| m.values().cloned().collect())
                .unwrap_or_default()
        };

        for handler in handlers {
            handler(data);
        }
    }

    /// Unsubscribe a specific subscriber by ID.
    pub fn unsubscribe(&self, subscriber_id: Uuid) {
        let mut subs = self.subscribers.lock().expect("subscribers mutex poisoned");
        for map in subs.values_mut() {
            if map.remove(&subscriber_id).is_some() {
                return;
            }
        }
    }

    /// Unsubscribe all subscribers for an email ID.
    pub fn unsubscribe_all(&self, email_id: &str) {
        self.subscribers.lock().expect("subscribers mutex poisoned").remove(email_id);
    }

    /// Check if any subscribers exist for an email ID.
    pub fn has_subscribers(&self, email_id: &str) -> bool {
        self.subscribers
            .lock()
            .expect("subscribers mutex poisoned")
            .get(email_id)
            .is_some_and(|m| !m.is_empty())
    }

    /// All email IDs with active subscribers.
    pub fn active_email_ids(&self) -> Vec<String> {
        self.subscribers
            .lock()
            .expect("subscribers mutex poisoned")
            .iter()
            .filter(|(_, m)| !m.is_empty())
            .map(|(id, _)| id.clone())
            .collect()
    }
}

impl Default for Email {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::sync::atomic::{AtomicI64, Ordering};

    #[derive(Serialize, Deserialize)]
    struct CountEvent {
        value: i64,
    }

    impl EmailEvent for CountEvent {
        const EMAIL_ID: &'static str = "test.count";
    }

    #[derive(Serialize, Deserialize)]
    struct PingEvent;

    impl EmailEvent for PingEvent {
        const EMAIL_ID: &'static str = "test.ping";
    }

    #[test]
    fn subscribe_and_send() {
        let email = Email::new();
        let received = Arc::new(AtomicI64::new(0));

        let received_clone = received.clone();
        email.subscribe(move |event: CountEvent| {
            received_clone.store(event.value, Ordering::SeqCst);
        });

        email.send(&CountEvent { value: 42 });
        assert_eq!(received.load(Ordering::SeqCst), 42);
    }

    #[test]
    fn multiple_subscribers() {
        let email = Email::new();
        let count = Arc::new(AtomicI64::new(0));

        for _ in 0..3 {
            let count_clone = count.clone();
            email.subscribe(move |_: PingEvent| {
                count_clone.fetch_add(1, Ordering::SeqCst);
            });
        }

        email.send(&PingEvent);
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn no_subscribers_no_panic() {
        let email = Email::new();
        email.send(&PingEvent); // Should not panic.
    }

    #[test]
    fn unsubscribe_stops_delivery() {
        let email = Email::new();
        let count = Arc::new(AtomicI64::new(0));

        let count_clone = count.clone();
        let sub_id = email.subscribe(move |_: PingEvent| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        email.send(&PingEvent);
        assert_eq!(count.load(Ordering::SeqCst), 1);

        email.unsubscribe(sub_id);
        email.send(&PingEvent);
        assert_eq!(count.load(Ordering::SeqCst), 1); // No change.
    }

    #[test]
    fn unsubscribe_all_clears_email_id() {
        let email = Email::new();
        let count = Arc::new(AtomicI64::new(0));

        for _ in 0..3 {
            let count_clone = count.clone();
            email.subscribe(move |_: PingEvent| {
                count_clone.fetch_add(1, Ordering::SeqCst);
            });
        }

        assert!(email.has_subscribers("test.ping"));
        email.unsubscribe_all("test.ping");
        assert!(!email.has_subscribers("test.ping"));

        email.send(&PingEvent);
        assert_eq!(count.load(Ordering::SeqCst), 0); // Nobody received it.
    }

    #[test]
    fn has_subscribers_correct() {
        let email = Email::new();
        assert!(!email.has_subscribers("test.ping"));

        let sub_id = email.subscribe(move |_: PingEvent| {});
        assert!(email.has_subscribers("test.ping"));

        email.unsubscribe(sub_id);
        assert!(!email.has_subscribers("test.ping"));
    }

    #[test]
    fn active_email_ids() {
        let email = Email::new();
        email.subscribe(move |_: PingEvent| {});
        email.subscribe(move |_: CountEvent| {});

        let mut ids = email.active_email_ids();
        ids.sort();
        assert_eq!(ids, vec!["test.count", "test.ping"]);
    }

    #[test]
    fn raw_subscribe_and_send() {
        let email = Email::new();
        let received = Arc::new(Mutex::new(Vec::new()));

        let received_clone = received.clone();
        email.subscribe_raw("test.raw", move |data: &[u8]| {
            received_clone.lock().unwrap().extend_from_slice(data);
        });

        email.send_raw("test.raw", b"hello");
        assert_eq!(*received.lock().unwrap(), b"hello");
    }

    #[test]
    fn deserialization_failure_ignored() {
        let email = Email::new();
        let count = Arc::new(AtomicI64::new(0));

        let count_clone = count.clone();
        email.subscribe(move |_: CountEvent| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        // Send garbage data — handler should not be called, no panic.
        email.send_raw("test.count", b"not json");
        assert_eq!(count.load(Ordering::SeqCst), 0);
    }
}
