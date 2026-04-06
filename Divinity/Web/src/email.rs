use equipment::email::Email as RustEmail;
use wasm_bindgen::prelude::*;

/// Wrapper to make JS functions Send + Sync in single-threaded WASM.
struct JsFnWrapper(js_sys::Function);
unsafe impl Send for JsFnWrapper {}
unsafe impl Sync for JsFnWrapper {}

/// Pub/sub event hub.
#[wasm_bindgen]
pub struct Email {
    inner: RustEmail,
}

impl Default for Email {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl Email {
    /// Create a new Email hub.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Email {
        Email {
            inner: RustEmail::new(),
        }
    }

    /// Subscribe to an email ID. Returns the subscriber UUID as a string.
    ///
    /// The handler receives a Uint8Array (fire-and-forget).
    #[wasm_bindgen(js_name = "subscribeRaw")]
    pub fn subscribe_raw(&self, email_id: &str, handler: js_sys::Function) -> String {
        let wrapper = JsFnWrapper(handler);
        let uuid = self.inner.subscribe_raw(email_id, move |data: &[u8]| {
            let js_data = js_sys::Uint8Array::from(data);
            let _ = wrapper.0.call1(&JsValue::NULL, &js_data);
        });
        uuid.to_string()
    }

    /// Send raw bytes to all subscribers of an email ID.
    #[wasm_bindgen(js_name = "sendRaw")]
    pub fn send_raw(&self, email_id: &str, data: &[u8]) {
        self.inner.send_raw(email_id, data);
    }

    /// Unsubscribe by UUID string.
    pub fn unsubscribe(&self, subscriber_id: &str) {
        if let Ok(uuid) = uuid::Uuid::parse_str(subscriber_id) {
            self.inner.unsubscribe(uuid);
        }
    }

    /// Unsubscribe all subscribers for an email ID.
    #[wasm_bindgen(js_name = "unsubscribeAll")]
    pub fn unsubscribe_all(&self, email_id: &str) {
        self.inner.unsubscribe_all(email_id);
    }

    /// Check if any subscribers exist for an email ID.
    #[wasm_bindgen(js_name = "hasSubscribers")]
    pub fn has_subscribers(&self, email_id: &str) -> bool {
        self.inner.has_subscribers(email_id)
    }

    /// Get all active email IDs as a JSON array.
    #[wasm_bindgen(js_name = "activeEmailIds")]
    pub fn active_email_ids(&self) -> JsValue {
        let ids = self.inner.active_email_ids();
        serde_wasm_bindgen::to_value(&ids).unwrap_or(JsValue::NULL)
    }
}
