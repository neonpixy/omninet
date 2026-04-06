use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::error::PhoneError;

/// A request that expects a response, routed by string ID.
///
/// Modules define call types by implementing this trait. The `CALL_ID`
/// is the routing key (convention: `"module.action"`). The `Response`
/// type is what the handler returns.
///
/// ```ignore
/// struct GetVaultEntries;
/// impl PhoneCall for GetVaultEntries {
///     const CALL_ID: &'static str = "vault.getEntries";
///     type Response = Vec<VaultEntryDTO>;
/// }
/// ```
pub trait PhoneCall: Serialize + DeserializeOwned + Send + Sync {
    /// The routing key (convention: `"module.action"`).
    const CALL_ID: &'static str;

    /// The response type returned by the handler.
    type Response: Serialize + DeserializeOwned + Send + Sync;
}

/// Raw handler: bytes in, bytes out.
type RawHandler = Arc<dyn Fn(&[u8]) -> Result<Vec<u8>, PhoneError> + Send + Sync>;

/// Central switchboard for request/response communication.
///
/// Modules register handlers by call ID. Callers make calls by call ID.
/// All routing is string-based — no import cycles. Data crosses the
/// boundary as JSON bytes.
pub struct Phone {
    handlers: Mutex<HashMap<String, RawHandler>>,
}

impl Phone {
    /// Create a new Phone with no registered handlers.
    pub fn new() -> Self {
        Self {
            handlers: Mutex::new(HashMap::new()),
        }
    }

    /// Register a typed handler for a `PhoneCall`.
    ///
    /// The handler receives the deserialized request and returns the response.
    /// Serialization is handled automatically.
    pub fn register<C>(
        &self,
        handler: impl Fn(C) -> Result<C::Response, PhoneError> + Send + Sync + 'static,
    ) where
        C: PhoneCall,
    {
        let call_id = C::CALL_ID.to_string();
        let raw_handler: RawHandler = Arc::new(move |data: &[u8]| {
            let request: C =
                serde_json::from_slice(data).map_err(|e| PhoneError::Serialization {
                    call_id: C::CALL_ID.to_string(),
                    source: e,
                })?;
            let response = handler(request)?;
            serde_json::to_vec(&response).map_err(|e| PhoneError::Serialization {
                call_id: C::CALL_ID.to_string(),
                source: e,
            })
        });

        self.handlers
            .lock()
            .expect("handlers mutex poisoned")
            .insert(call_id, raw_handler);
    }

    /// Register a raw handler by string call ID.
    ///
    /// Use this for cross-language FFI where typed calls aren't available.
    pub fn register_raw(
        &self,
        call_id: impl Into<String>,
        handler: impl Fn(&[u8]) -> Result<Vec<u8>, PhoneError> + Send + Sync + 'static,
    ) {
        self.handlers
            .lock()
            .expect("handlers mutex poisoned")
            .insert(call_id.into(), Arc::new(handler));
    }

    /// Make a typed call and get a typed response.
    pub fn call<C: PhoneCall>(&self, request: &C) -> Result<C::Response, PhoneError> {
        let data = serde_json::to_vec(request).map_err(|e| PhoneError::Serialization {
            call_id: C::CALL_ID.to_string(),
            source: e,
        })?;

        let response_data = self.call_raw(C::CALL_ID, &data)?;

        serde_json::from_slice(&response_data).map_err(|e| PhoneError::Serialization {
            call_id: C::CALL_ID.to_string(),
            source: e,
        })
    }

    /// Make a raw call by string call ID with byte data.
    ///
    /// The handler is cloned out of the registry before calling,
    /// so the lock is not held during handler execution. This prevents
    /// deadlocks when handlers make reentrant calls.
    pub fn call_raw(&self, call_id: &str, data: &[u8]) -> Result<Vec<u8>, PhoneError> {
        let handler = {
            let handlers = self.handlers.lock().expect("handlers mutex poisoned");
            handlers
                .get(call_id)
                .cloned()
                .ok_or_else(|| PhoneError::NoHandler(call_id.to_string()))?
        };

        handler(data)
    }

    /// Call if a handler is available, returning `None` if not registered.
    pub fn call_if_available<C: PhoneCall>(
        &self,
        request: &C,
    ) -> Result<Option<C::Response>, PhoneError> {
        if !self.has_handler(C::CALL_ID) {
            return Ok(None);
        }
        self.call(request).map(Some)
    }

    /// Check if a handler is registered for a call ID.
    pub fn has_handler(&self, call_id: &str) -> bool {
        self.handlers.lock().expect("handlers mutex poisoned").contains_key(call_id)
    }

    /// All registered call IDs.
    pub fn registered_call_ids(&self) -> Vec<String> {
        self.handlers.lock().expect("handlers mutex poisoned").keys().cloned().collect()
    }

    /// Unregister a handler by call ID.
    pub fn unregister(&self, call_id: &str) {
        self.handlers.lock().expect("handlers mutex poisoned").remove(call_id);
    }
}

impl Default for Phone {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Serialize, Deserialize)]
    struct AddCall {
        a: i64,
        b: i64,
    }

    impl PhoneCall for AddCall {
        const CALL_ID: &'static str = "test.add";
        type Response = i64;
    }

    #[derive(Serialize, Deserialize)]
    struct EchoCall {
        message: String,
    }

    impl PhoneCall for EchoCall {
        const CALL_ID: &'static str = "test.echo";
        type Response = String;
    }

    #[test]
    fn register_and_call() {
        let phone = Phone::new();
        phone.register(|call: AddCall| Ok(call.a + call.b));

        let result = phone.call(&AddCall { a: 3, b: 4 }).unwrap();
        assert_eq!(result, 7);
    }

    #[test]
    fn call_no_handler() {
        let phone = Phone::new();
        let result = phone.call(&AddCall { a: 1, b: 2 });
        assert!(matches!(result, Err(PhoneError::NoHandler(id)) if id == "test.add"));
    }

    #[test]
    fn register_raw_and_call_raw() {
        let phone = Phone::new();
        phone.register_raw("test.double", |data: &[u8]| {
            let n: i64 = serde_json::from_slice(data).map_err(|e| PhoneError::Serialization {
                call_id: "test.double".to_string(),
                source: e,
            })?;
            serde_json::to_vec(&(n * 2)).map_err(|e| PhoneError::Serialization {
                call_id: "test.double".to_string(),
                source: e,
            })
        });

        let input = serde_json::to_vec(&5i64).unwrap();
        let output = phone.call_raw("test.double", &input).unwrap();
        let result: i64 = serde_json::from_slice(&output).unwrap();
        assert_eq!(result, 10);
    }

    #[test]
    fn call_if_available_present() {
        let phone = Phone::new();
        phone.register(|call: AddCall| Ok(call.a + call.b));

        let result = phone.call_if_available(&AddCall { a: 5, b: 6 }).unwrap();
        assert_eq!(result, Some(11));
    }

    #[test]
    fn call_if_available_missing() {
        let phone = Phone::new();
        let result = phone.call_if_available(&AddCall { a: 1, b: 2 }).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn has_handler() {
        let phone = Phone::new();
        assert!(!phone.has_handler("test.add"));

        phone.register(|call: AddCall| Ok(call.a + call.b));
        assert!(phone.has_handler("test.add"));
    }

    #[test]
    fn registered_call_ids() {
        let phone = Phone::new();
        phone.register(|call: AddCall| Ok(call.a + call.b));
        phone.register(|call: EchoCall| Ok(call.message.clone()));

        let mut ids = phone.registered_call_ids();
        ids.sort();
        assert_eq!(ids, vec!["test.add", "test.echo"]);
    }

    #[test]
    fn unregister_removes_handler() {
        let phone = Phone::new();
        phone.register(|call: AddCall| Ok(call.a + call.b));
        assert!(phone.has_handler("test.add"));

        phone.unregister("test.add");
        assert!(!phone.has_handler("test.add"));
    }

    #[test]
    fn handler_error_propagated() {
        let phone = Phone::new();
        phone.register(|_call: AddCall| {
            Err(PhoneError::HandlerFailed {
                call_id: "test.add".to_string(),
                message: "intentional failure".to_string(),
            })
        });

        let result = phone.call(&AddCall { a: 1, b: 2 });
        assert!(matches!(result, Err(PhoneError::HandlerFailed { .. })));
    }

    #[test]
    fn handler_replacement() {
        let phone = Phone::new();
        phone.register(|call: AddCall| Ok(call.a + call.b));
        assert_eq!(phone.call(&AddCall { a: 2, b: 3 }).unwrap(), 5);

        // Re-register with a different handler.
        phone.register(|call: AddCall| Ok(call.a * call.b));
        assert_eq!(phone.call(&AddCall { a: 2, b: 3 }).unwrap(), 6);
    }

    #[test]
    fn reentrant_call() {
        let phone = Arc::new(Phone::new());

        phone.register(|call: EchoCall| Ok(call.message.clone()));

        let phone_clone = phone.clone();
        phone.register(move |call: AddCall| {
            // Reentrant: this handler calls back into Phone.
            let echo_result = phone_clone
                .call(&EchoCall {
                    message: format!("{}", call.a + call.b),
                })
                .unwrap();
            Ok(echo_result.parse::<i64>().unwrap_or(0))
        });

        // This will deadlock if we hold the Mutex during handler execution.
        let result = phone.call(&AddCall { a: 10, b: 20 }).unwrap();
        assert_eq!(result, 30);
    }
}
