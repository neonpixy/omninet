//! Wire types for live data binding subscriptions.
//!
//! When a Studio design binds to Abacus data (e.g., a product card pulls its
//! price from a spreadsheet column), the binding is represented as a
//! [`DataSubscription`]. When the source data changes, a [`DataUpdate`] is
//! broadcast through Email so the subscriber can re-render.
//!
//! The [`BindingManager`] tracks active subscriptions and provides lookup
//! by source reference or subscriber module.
//!
//! # Example
//!
//! ```
//! use equipment::binding_wire::{BindingManager, DataSubscription};
//! use chrono::Utc;
//! use uuid::Uuid;
//!
//! let mut manager = BindingManager::new();
//! let sub = DataSubscription {
//!     id: Uuid::new_v4(),
//!     subscriber_module: "studio".into(),
//!     source_module: "abacus".into(),
//!     source_ref: "idea://sheet/products".into(),
//!     source_path: "sheet.price".into(),
//!     created_at: Utc::now(),
//! };
//!
//! let id = manager.subscribe(sub);
//! assert_eq!(manager.count(), 1);
//! assert_eq!(manager.subscriptions_for_source("idea://sheet/products").len(), 1);
//!
//! manager.unsubscribe(&id);
//! assert_eq!(manager.count(), 0);
//! ```

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A subscription request for live data binding between programs.
///
/// Created when one program (the subscriber) wants to receive updates
/// whenever data changes in another program (the source). For example,
/// Studio subscribing to Abacus cell changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataSubscription {
    /// Unique identifier for this subscription.
    pub id: Uuid,
    /// The module receiving updates (e.g., `"studio"`).
    pub subscriber_module: String,
    /// The module providing data (e.g., `"abacus"`).
    pub source_module: String,
    /// Reference to the source .idea file (e.g., `"idea://sheet/products"`).
    pub source_ref: String,
    /// Path within the source to the specific data (e.g., `"sheet.price"`).
    pub source_path: String,
    /// When this subscription was created.
    pub created_at: DateTime<Utc>,
}

/// A data update notification sent via Email when source data changes.
///
/// The source module emits this when data that has active subscriptions
/// is modified. Subscribers receive it and can re-render or re-compute.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataUpdate {
    /// The subscription this update is for.
    pub subscription_id: Uuid,
    /// Reference to the source .idea file that changed.
    pub source_ref: String,
    /// Path within the source to the changed data.
    pub source_path: String,
    /// The new value, serialized as JSON.
    pub new_value: String,
    /// When this update occurred.
    pub updated_at: DateTime<Utc>,
}

/// Manages active data binding subscriptions.
///
/// Pure data structure with no internal locking -- the caller owns it
/// and can wrap it in a `Mutex` if shared across threads. Follows the
/// same pattern as `Mailbox` and `Pager`.
#[derive(Clone, Debug, Default)]
pub struct BindingManager {
    subscriptions: HashMap<Uuid, DataSubscription>,
}

impl BindingManager {
    /// Create an empty binding manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new data subscription. Returns the subscription's UUID.
    ///
    /// If a subscription with the same ID already exists, it is replaced.
    pub fn subscribe(&mut self, sub: DataSubscription) -> Uuid {
        let id = sub.id;
        self.subscriptions.insert(id, sub);
        id
    }

    /// Remove a subscription by ID. Returns `true` if it existed.
    pub fn unsubscribe(&mut self, id: &Uuid) -> bool {
        self.subscriptions.remove(id).is_some()
    }

    /// All subscriptions watching a specific source .idea reference.
    pub fn subscriptions_for_source(&self, source_ref: &str) -> Vec<&DataSubscription> {
        self.subscriptions
            .values()
            .filter(|s| s.source_ref == source_ref)
            .collect()
    }

    /// All subscriptions for a specific subscriber module.
    pub fn subscriptions_for_subscriber(&self, module: &str) -> Vec<&DataSubscription> {
        self.subscriptions
            .values()
            .filter(|s| s.subscriber_module == module)
            .collect()
    }

    /// Look up a subscription by ID.
    pub fn get(&self, id: &Uuid) -> Option<&DataSubscription> {
        self.subscriptions.get(id)
    }

    /// Total number of active subscriptions.
    pub fn count(&self) -> usize {
        self.subscriptions.len()
    }

    /// Remove all subscriptions for a specific subscriber module.
    /// Returns the number of subscriptions removed.
    pub fn unsubscribe_all_for(&mut self, module: &str) -> usize {
        let before = self.subscriptions.len();
        self.subscriptions
            .retain(|_, s| s.subscriber_module != module);
        before - self.subscriptions.len()
    }

    /// Remove all subscriptions watching a specific source reference.
    /// Returns the number of subscriptions removed.
    pub fn unsubscribe_all_for_source(&mut self, source_ref: &str) -> usize {
        let before = self.subscriptions.len();
        self.subscriptions
            .retain(|_, s| s.source_ref != source_ref);
        before - self.subscriptions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sub(subscriber: &str, source: &str, path: &str) -> DataSubscription {
        DataSubscription {
            id: Uuid::new_v4(),
            subscriber_module: subscriber.into(),
            source_module: source.into(),
            source_ref: format!("idea://sheet/{source}"),
            source_path: path.into(),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn subscribe_and_lookup() {
        let mut manager = BindingManager::new();
        let sub = make_sub("studio", "abacus", "sheet.price");
        let id = sub.id;
        manager.subscribe(sub);

        assert_eq!(manager.count(), 1);
        let found = manager.get(&id).unwrap();
        assert_eq!(found.subscriber_module, "studio");
        assert_eq!(found.source_module, "abacus");
    }

    #[test]
    fn unsubscribe() {
        let mut manager = BindingManager::new();
        let sub = make_sub("studio", "abacus", "sheet.price");
        let id = manager.subscribe(sub);

        assert!(manager.unsubscribe(&id));
        assert_eq!(manager.count(), 0);
        assert!(manager.get(&id).is_none());
    }

    #[test]
    fn unsubscribe_nonexistent() {
        let mut manager = BindingManager::new();
        assert!(!manager.unsubscribe(&Uuid::new_v4()));
    }

    #[test]
    fn subscriptions_for_source() {
        let mut manager = BindingManager::new();

        let sub1 = make_sub("studio", "products", "sheet.price");
        let sub2 = make_sub("podium", "products", "sheet.name");
        let sub3 = make_sub("studio", "inventory", "sheet.count");

        manager.subscribe(sub1);
        manager.subscribe(sub2);
        manager.subscribe(sub3);

        let product_subs = manager.subscriptions_for_source("idea://sheet/products");
        assert_eq!(product_subs.len(), 2);

        let inventory_subs = manager.subscriptions_for_source("idea://sheet/inventory");
        assert_eq!(inventory_subs.len(), 1);

        let empty = manager.subscriptions_for_source("idea://sheet/nonexistent");
        assert!(empty.is_empty());
    }

    #[test]
    fn subscriptions_for_subscriber() {
        let mut manager = BindingManager::new();

        manager.subscribe(make_sub("studio", "abacus", "sheet.price"));
        manager.subscribe(make_sub("studio", "abacus", "sheet.name"));
        manager.subscribe(make_sub("podium", "abacus", "sheet.title"));

        let studio_subs = manager.subscriptions_for_subscriber("studio");
        assert_eq!(studio_subs.len(), 2);

        let podium_subs = manager.subscriptions_for_subscriber("podium");
        assert_eq!(podium_subs.len(), 1);
    }

    #[test]
    fn unsubscribe_all_for_module() {
        let mut manager = BindingManager::new();

        manager.subscribe(make_sub("studio", "abacus", "a"));
        manager.subscribe(make_sub("studio", "abacus", "b"));
        manager.subscribe(make_sub("podium", "abacus", "c"));

        let removed = manager.unsubscribe_all_for("studio");
        assert_eq!(removed, 2);
        assert_eq!(manager.count(), 1);
        assert_eq!(manager.subscriptions_for_subscriber("podium").len(), 1);
    }

    #[test]
    fn unsubscribe_all_for_source() {
        let mut manager = BindingManager::new();

        manager.subscribe(make_sub("studio", "products", "price"));
        manager.subscribe(make_sub("podium", "products", "name"));
        manager.subscribe(make_sub("studio", "inventory", "count"));

        let removed = manager.unsubscribe_all_for_source("idea://sheet/products");
        assert_eq!(removed, 2);
        assert_eq!(manager.count(), 1);
    }

    #[test]
    fn empty_manager() {
        let manager = BindingManager::new();
        assert_eq!(manager.count(), 0);
        assert!(manager.get(&Uuid::new_v4()).is_none());
        assert!(manager.subscriptions_for_source("anything").is_empty());
        assert!(manager.subscriptions_for_subscriber("anything").is_empty());
    }

    #[test]
    fn subscription_serde_round_trip() {
        let sub = make_sub("studio", "abacus", "sheet.price");
        let json = serde_json::to_string(&sub).unwrap();
        let loaded: DataSubscription = serde_json::from_str(&json).unwrap();
        assert_eq!(sub, loaded);
    }

    #[test]
    fn data_update_serde_round_trip() {
        let update = DataUpdate {
            subscription_id: Uuid::new_v4(),
            source_ref: "idea://sheet/products".into(),
            source_path: "sheet.price".into(),
            new_value: r#"{"value": 29.99}"#.into(),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string(&update).unwrap();
        let loaded: DataUpdate = serde_json::from_str(&json).unwrap();
        assert_eq!(update, loaded);
    }

    #[test]
    fn subscribe_replaces_existing() {
        let mut manager = BindingManager::new();
        let id = Uuid::new_v4();

        let sub1 = DataSubscription {
            id,
            subscriber_module: "studio".into(),
            source_module: "abacus".into(),
            source_ref: "idea://sheet/old".into(),
            source_path: "sheet.price".into(),
            created_at: Utc::now(),
        };

        let sub2 = DataSubscription {
            id,
            subscriber_module: "studio".into(),
            source_module: "abacus".into(),
            source_ref: "idea://sheet/new".into(),
            source_path: "sheet.price".into(),
            created_at: Utc::now(),
        };

        manager.subscribe(sub1);
        manager.subscribe(sub2);

        assert_eq!(manager.count(), 1);
        assert_eq!(manager.get(&id).unwrap().source_ref, "idea://sheet/new");
    }

    #[test]
    fn clone_manager() {
        let mut manager = BindingManager::new();
        manager.subscribe(make_sub("studio", "abacus", "sheet.price"));

        let cloned = manager.clone();
        assert_eq!(cloned.count(), 1);
    }
}
