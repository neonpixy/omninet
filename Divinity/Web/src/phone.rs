use equipment::phone::Phone as RustPhone;
use wasm_bindgen::prelude::*;

/// Wrapper to make JS functions Send + Sync in single-threaded WASM.
/// This is safe because WASM has no threading — the Mutex in Equipment
/// never actually contends.
struct JsFnWrapper(js_sys::Function);
unsafe impl Send for JsFnWrapper {}
unsafe impl Sync for JsFnWrapper {}

/// RPC switchboard — register handlers, make calls.
#[wasm_bindgen]
pub struct Phone {
    inner: RustPhone,
}

impl Default for Phone {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl Phone {
    /// Create a new Phone.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Phone {
        Phone {
            inner: RustPhone::new(),
        }
    }

    /// Register a raw handler for a call ID.
    ///
    /// The handler receives a Uint8Array and must return a Uint8Array.
    #[wasm_bindgen(js_name = "registerRaw")]
    pub fn register_raw(&self, call_id: &str, handler: js_sys::Function) {
        let wrapper = JsFnWrapper(handler);
        self.inner.register_raw(call_id, move |data: &[u8]| {
            let js_data = js_sys::Uint8Array::from(data);
            let result = wrapper
                .0
                .call1(&JsValue::NULL, &js_data)
                .map_err(|e| equipment::error::PhoneError::HandlerFailed {
                    call_id: "wasm".to_string(),
                    message: format!("{:?}", e),
                })?;

            if let Some(arr) = result.dyn_ref::<js_sys::Uint8Array>() {
                Ok(arr.to_vec())
            } else {
                Ok(Vec::new())
            }
        });
    }

    /// Make a raw call. Returns response bytes. Throws on error.
    #[wasm_bindgen(js_name = "callRaw")]
    pub fn call_raw(&self, call_id: &str, data: &[u8]) -> Result<Vec<u8>, JsError> {
        self.inner
            .call_raw(call_id, data)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Check if a handler is registered for a call ID.
    #[wasm_bindgen(js_name = "hasHandler")]
    pub fn has_handler(&self, call_id: &str) -> bool {
        self.inner.has_handler(call_id)
    }

    /// Get all registered call IDs as a JSON array.
    #[wasm_bindgen(js_name = "registeredCallIds")]
    pub fn registered_call_ids(&self) -> JsValue {
        let ids = self.inner.registered_call_ids();
        serde_wasm_bindgen::to_value(&ids).unwrap_or(JsValue::NULL)
    }

    /// Unregister a handler by call ID.
    pub fn unregister(&self, call_id: &str) {
        self.inner.unregister(call_id);
    }
}
