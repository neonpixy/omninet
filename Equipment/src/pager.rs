use std::collections::HashMap;
use std::sync::Mutex;

use uuid::Uuid;

use crate::notification::{Notification, NotificationPriority, PagerState};

/// Pure notification queue. No dependency on Email or any other actor.
///
/// Push notifications in, read them out, mark read, dismiss, prune expired.
/// The Swift/UI layer can wire up Email broadcasts on state changes
/// if needed — that's the app's business, not the protocol's.
pub struct Pager {
    inner: Mutex<PagerInner>,
}

struct PagerInner {
    notifications: HashMap<Uuid, Notification>,
    /// Insertion order for consistent sorting when created timestamps match.
    insertion_order: Vec<Uuid>,
}

impl Pager {
    /// Create an empty notification queue.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(PagerInner {
                notifications: HashMap::new(),
                insertion_order: Vec::new(),
            }),
        }
    }

    /// Queue a notification. Returns its UUID.
    pub fn notify(&self, notification: Notification) -> Uuid {
        let id = notification.id();
        let mut inner = self.inner.lock().expect("pager mutex poisoned");
        inner.insertion_order.push(id);
        inner.notifications.insert(id, notification);
        id
    }

    /// Get pending notifications (not dismissed, not expired).
    /// Optionally filter by priority. Returns newest first.
    pub fn get_pending(&self, priority: Option<NotificationPriority>) -> Vec<Notification> {
        let inner = self.inner.lock().expect("pager mutex poisoned");
        let mut result: Vec<Notification> = inner
            .insertion_order
            .iter()
            .rev()
            .filter_map(|id| {
                let n = inner.notifications.get(id)?;
                if n.is_dismissed() || n.is_expired() {
                    return None;
                }
                if let Some(p) = priority
                    && n.priority() != p
                {
                    return None;
                }
                Some(n.clone())
            })
            .collect();

        // Stable sort by created (newest first), insertion order breaks ties.
        result.sort_by_key(|n| std::cmp::Reverse(n.created()));
        result
    }

    /// Get unread notifications (pending + not read). Newest first.
    pub fn get_unread(&self) -> Vec<Notification> {
        let inner = self.inner.lock().expect("pager mutex poisoned");
        let mut result: Vec<Notification> = inner
            .insertion_order
            .iter()
            .rev()
            .filter_map(|id| {
                let n = inner.notifications.get(id)?;
                if n.is_dismissed() || n.is_expired() || n.is_read() {
                    return None;
                }
                Some(n.clone())
            })
            .collect();

        result.sort_by_key(|n| std::cmp::Reverse(n.created()));
        result
    }

    /// Mark a notification as read. Returns true if it existed.
    pub fn mark_read(&self, id: Uuid) -> bool {
        let mut inner = self.inner.lock().expect("pager mutex poisoned");
        if let Some(n) = inner.notifications.get_mut(&id) {
            n.read = true;
            true
        } else {
            false
        }
    }

    /// Dismiss a notification. Returns true if it existed.
    pub fn dismiss(&self, id: Uuid) -> bool {
        let mut inner = self.inner.lock().expect("pager mutex poisoned");
        if let Some(n) = inner.notifications.get_mut(&id) {
            n.dismissed = true;
            true
        } else {
            false
        }
    }

    /// Count of unread, non-dismissed, non-expired notifications.
    pub fn badge_count(&self) -> usize {
        let inner = self.inner.lock().expect("pager mutex poisoned");
        inner
            .notifications
            .values()
            .filter(|n| !n.is_read() && !n.is_dismissed() && !n.is_expired())
            .count()
    }

    /// Remove expired notifications. Returns count pruned.
    pub fn prune_expired(&self) -> usize {
        let mut inner = self.inner.lock().expect("pager mutex poisoned");
        let expired: Vec<Uuid> = inner
            .notifications
            .iter()
            .filter(|(_, n)| n.is_expired())
            .map(|(id, _)| *id)
            .collect();

        let count = expired.len();
        for id in &expired {
            inner.notifications.remove(id);
            inner.insertion_order.retain(|i| i != id);
        }
        count
    }

    /// Export all notification state for persistence.
    pub fn export_state(&self) -> PagerState {
        let inner = self.inner.lock().expect("pager mutex poisoned");
        let notifications = inner
            .insertion_order
            .iter()
            .filter_map(|id| inner.notifications.get(id).cloned())
            .collect();

        PagerState { notifications }
    }

    /// Restore notification state from persistence.
    pub fn restore_state(&self, state: PagerState) {
        let mut inner = self.inner.lock().expect("pager mutex poisoned");
        inner.notifications.clear();
        inner.insertion_order.clear();

        for n in state.notifications {
            if !n.is_expired() {
                inner.insertion_order.push(n.id());
                inner.notifications.insert(n.id(), n);
            }
        }
    }
}

impl Default for Pager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    #[test]
    fn notify_and_get_pending() {
        let pager = Pager::new();
        pager.notify(Notification::new("Test", "vault"));

        let pending = pager.get_pending(None);
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].title(), "Test");
    }

    #[test]
    fn get_pending_excludes_dismissed() {
        let pager = Pager::new();
        let id = pager.notify(Notification::new("Test", "vault"));

        pager.dismiss(id);
        assert!(pager.get_pending(None).is_empty());
    }

    #[test]
    fn get_pending_excludes_expired() {
        let pager = Pager::new();
        pager.notify(
            Notification::new("Old", "vault").with_expires(Utc::now() - Duration::hours(1)),
        );

        assert!(pager.get_pending(None).is_empty());
    }

    #[test]
    fn get_pending_filter_by_priority() {
        let pager = Pager::new();
        pager.notify(Notification::new("Normal", "vault"));
        pager.notify(
            Notification::new("Urgent", "vault").with_priority(NotificationPriority::Urgent),
        );

        let urgent = pager.get_pending(Some(NotificationPriority::Urgent));
        assert_eq!(urgent.len(), 1);
        assert_eq!(urgent[0].title(), "Urgent");
    }

    #[test]
    fn get_unread() {
        let pager = Pager::new();
        let id1 = pager.notify(Notification::new("One", "vault"));
        pager.notify(Notification::new("Two", "vault"));

        pager.mark_read(id1);

        let unread = pager.get_unread();
        assert_eq!(unread.len(), 1);
        assert_eq!(unread[0].title(), "Two");
    }

    #[test]
    fn mark_read_returns_bool() {
        let pager = Pager::new();
        let id = pager.notify(Notification::new("Test", "vault"));

        assert!(pager.mark_read(id));
        assert!(!pager.mark_read(Uuid::new_v4())); // Nonexistent.
    }

    #[test]
    fn dismiss_returns_bool() {
        let pager = Pager::new();
        let id = pager.notify(Notification::new("Test", "vault"));

        assert!(pager.dismiss(id));
        assert!(!pager.dismiss(Uuid::new_v4())); // Nonexistent.
    }

    #[test]
    fn badge_count() {
        let pager = Pager::new();
        pager.notify(Notification::new("One", "vault"));
        pager.notify(Notification::new("Two", "vault"));
        let id3 = pager.notify(Notification::new("Three", "vault"));

        assert_eq!(pager.badge_count(), 3);

        pager.mark_read(id3);
        assert_eq!(pager.badge_count(), 2);
    }

    #[test]
    fn prune_expired() {
        let pager = Pager::new();
        pager.notify(
            Notification::new("Old", "vault").with_expires(Utc::now() - Duration::hours(1)),
        );
        pager.notify(Notification::new("Active", "vault"));

        let pruned = pager.prune_expired();
        assert_eq!(pruned, 1);
        assert_eq!(pager.get_pending(None).len(), 1);
    }

    #[test]
    fn export_and_restore_state() {
        let pager = Pager::new();
        let id = pager.notify(Notification::new("Test", "vault"));
        pager.mark_read(id);

        let state = pager.export_state();
        assert_eq!(state.notifications.len(), 1);
        assert!(state.notifications[0].is_read());

        // Restore into a fresh Pager.
        let pager2 = Pager::new();
        pager2.restore_state(state);

        let pending = pager2.get_pending(None);
        assert_eq!(pending.len(), 1);
        assert!(pending[0].is_read());
    }

    #[test]
    fn restore_state_prunes_expired() {
        let pager = Pager::new();
        let state = PagerState {
            notifications: vec![
                Notification::new("Old", "vault")
                    .with_expires(Utc::now() - Duration::hours(1)),
                Notification::new("Active", "vault"),
            ],
        };

        pager.restore_state(state);
        assert_eq!(pager.get_pending(None).len(), 1);
        assert_eq!(pager.get_pending(None)[0].title(), "Active");
    }
}
