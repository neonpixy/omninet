use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Notification priority level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotificationPriority {
    /// Background info the user can check whenever.
    Low,
    /// Standard notifications (the default).
    Normal,
    /// Important -- should be seen soon.
    High,
    /// Requires immediate attention.
    Urgent,
}

/// How a notification should be presented.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotificationDelivery {
    /// A brief popup that auto-dismisses.
    Toast,
    /// A persistent alert requiring user action.
    Alert,
    /// Badge count only -- no visual popup.
    Badge,
    /// No visible presentation; data-only.
    Silent,
}

/// A notification in the Pager queue.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notification {
    id: Uuid,
    title: String,
    body: Option<String>,
    priority: NotificationPriority,
    delivery: NotificationDelivery,
    source_module: String,
    created: DateTime<Utc>,
    expires: Option<DateTime<Utc>>,
    pub(crate) read: bool,
    pub(crate) dismissed: bool,
}

impl Notification {
    /// Create a notification with required fields. Defaults to Normal priority, Toast delivery.
    pub fn new(title: impl Into<String>, source_module: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: title.into(),
            body: None,
            priority: NotificationPriority::Normal,
            delivery: NotificationDelivery::Toast,
            source_module: source_module.into(),
            created: Utc::now(),
            expires: None,
            read: false,
            dismissed: false,
        }
    }

    /// Set an optional body with more detail.
    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }

    /// Override the default priority (Normal).
    pub fn with_priority(mut self, priority: NotificationPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Override the default delivery method (Toast).
    pub fn with_delivery(mut self, delivery: NotificationDelivery) -> Self {
        self.delivery = delivery;
        self
    }

    /// Set an expiration time after which this notification is pruned.
    pub fn with_expires(mut self, expires: DateTime<Utc>) -> Self {
        self.expires = Some(expires);
        self
    }

    /// This notification's unique ID.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// The short headline of this notification.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Optional detail text.
    pub fn body(&self) -> Option<&str> {
        self.body.as_deref()
    }

    /// How urgent this notification is.
    pub fn priority(&self) -> NotificationPriority {
        self.priority
    }

    /// How this notification should be presented to the user.
    pub fn delivery(&self) -> NotificationDelivery {
        self.delivery
    }

    /// Which module created this notification.
    pub fn source_module(&self) -> &str {
        &self.source_module
    }

    /// When this notification was created.
    pub fn created(&self) -> DateTime<Utc> {
        self.created
    }

    /// When this notification expires, if set.
    pub fn expires(&self) -> Option<DateTime<Utc>> {
        self.expires
    }

    /// Whether the user has read this notification.
    pub fn is_read(&self) -> bool {
        self.read
    }

    /// Whether the user has dismissed this notification.
    pub fn is_dismissed(&self) -> bool {
        self.dismissed
    }

    /// Whether this notification has expired.
    pub fn is_expired(&self) -> bool {
        self.expires.is_some_and(|e| Utc::now() > e)
    }
}

/// Serializable snapshot of all Pager state for persistence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PagerState {
    /// All notifications at the time of export.
    pub notifications: Vec<Notification>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn create_notification_defaults() {
        let n = Notification::new("Test", "vault");
        assert_eq!(n.title(), "Test");
        assert_eq!(n.source_module(), "vault");
        assert_eq!(n.priority(), NotificationPriority::Normal);
        assert_eq!(n.delivery(), NotificationDelivery::Toast);
        assert!(n.body().is_none());
        assert!(n.expires().is_none());
        assert!(!n.is_read());
        assert!(!n.is_dismissed());
    }

    #[test]
    fn builder_methods() {
        let n = Notification::new("Alert", "crown")
            .with_body("Something happened")
            .with_priority(NotificationPriority::Urgent)
            .with_delivery(NotificationDelivery::Alert);

        assert_eq!(n.body(), Some("Something happened"));
        assert_eq!(n.priority(), NotificationPriority::Urgent);
        assert_eq!(n.delivery(), NotificationDelivery::Alert);
    }

    #[test]
    fn is_expired_no_expiry() {
        let n = Notification::new("Test", "vault");
        assert!(!n.is_expired());
    }

    #[test]
    fn is_expired_future() {
        let n = Notification::new("Test", "vault")
            .with_expires(Utc::now() + Duration::hours(1));
        assert!(!n.is_expired());
    }

    #[test]
    fn is_expired_past() {
        let n = Notification::new("Test", "vault")
            .with_expires(Utc::now() - Duration::hours(1));
        assert!(n.is_expired());
    }

    #[test]
    fn serde_round_trip() {
        let n = Notification::new("Test", "vault")
            .with_body("Details")
            .with_priority(NotificationPriority::High)
            .with_delivery(NotificationDelivery::Badge);

        let json = serde_json::to_string(&n).unwrap();
        let deserialized: Notification = serde_json::from_str(&json).unwrap();
        assert_eq!(n, deserialized);
    }

    #[test]
    fn pager_state_serde() {
        let state = PagerState {
            notifications: vec![
                Notification::new("One", "vault"),
                Notification::new("Two", "crown"),
            ],
        };

        let json = serde_json::to_string(&state).unwrap();
        let deserialized: PagerState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, deserialized);
    }
}
