use equipment::notification::{Notification, PagerState};
use equipment::pager::Pager as RustPager;
use wasm_bindgen::prelude::*;

/// Notification queue.
#[wasm_bindgen]
pub struct Pager {
    inner: RustPager,
}

impl Default for Pager {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl Pager {
    /// Create a new Pager.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Pager {
        Pager {
            inner: RustPager::new(),
        }
    }

    /// Push a notification from a JSON string. Returns UUID string or null.
    pub fn notify(&self, json: &str) -> JsValue {
        match serde_json::from_str::<Notification>(json) {
            Ok(notification) => {
                let uuid = self.inner.notify(notification);
                JsValue::from_str(&uuid.to_string())
            }
            Err(_) => JsValue::NULL,
        }
    }

    /// Get pending notifications as a JSON string.
    #[wasm_bindgen(js_name = "getPendingJson")]
    pub fn get_pending_json(&self) -> String {
        let pending = self.inner.get_pending(None);
        serde_json::to_string(&pending).unwrap_or_else(|_| "[]".to_string())
    }

    /// Get unread notifications as a JSON string.
    #[wasm_bindgen(js_name = "getUnreadJson")]
    pub fn get_unread_json(&self) -> String {
        let unread = self.inner.get_unread();
        serde_json::to_string(&unread).unwrap_or_else(|_| "[]".to_string())
    }

    /// Mark a notification as read. Returns true if found.
    #[wasm_bindgen(js_name = "markRead")]
    pub fn mark_read(&self, uuid_str: &str) -> bool {
        match uuid::Uuid::parse_str(uuid_str) {
            Ok(uuid) => self.inner.mark_read(uuid),
            Err(_) => false,
        }
    }

    /// Dismiss a notification. Returns true if found.
    pub fn dismiss(&self, uuid_str: &str) -> bool {
        match uuid::Uuid::parse_str(uuid_str) {
            Ok(uuid) => self.inner.dismiss(uuid),
            Err(_) => false,
        }
    }

    /// Get the count of unread notifications.
    #[wasm_bindgen(js_name = "badgeCount")]
    pub fn badge_count(&self) -> usize {
        self.inner.badge_count()
    }

    /// Prune expired notifications.
    #[wasm_bindgen(js_name = "pruneExpired")]
    pub fn prune_expired(&self) {
        self.inner.prune_expired();
    }

    /// Export pager state as a JSON string.
    #[wasm_bindgen(js_name = "exportStateJson")]
    pub fn export_state_json(&self) -> String {
        let state = self.inner.export_state();
        serde_json::to_string(&state.notifications).unwrap_or_else(|_| "[]".to_string())
    }

    /// Restore pager state from a JSON string.
    #[wasm_bindgen(js_name = "restoreState")]
    pub fn restore_state(&self, json: &str) {
        if let Ok(notifications) = serde_json::from_str::<Vec<Notification>>(json) {
            self.inner.restore_state(PagerState { notifications });
        }
    }
}
