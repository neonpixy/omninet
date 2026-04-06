use equipment::contacts::{Contacts as RustContacts, ModuleInfo};
use wasm_bindgen::prelude::*;

/// Module registry with dependency tracking.
#[wasm_bindgen]
pub struct Contacts {
    inner: RustContacts,
}

impl Default for Contacts {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl Contacts {
    /// Create a new Contacts registry.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Contacts {
        Contacts {
            inner: RustContacts::new(),
        }
    }

    /// Register a module from a JSON string. Throws on error.
    pub fn register(&self, json: &str) -> Result<(), JsError> {
        let info: ModuleInfo =
            serde_json::from_str(json).map_err(|e| JsError::new(&e.to_string()))?;
        self.inner
            .register(info)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Unregister a module by ID. Throws on error.
    pub fn unregister(&self, module_id: &str) -> Result<(), JsError> {
        self.inner
            .unregister(module_id)
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Shut down all modules in dependency order.
    #[wasm_bindgen(js_name = "shutdownAll")]
    pub fn shutdown_all(&self) {
        self.inner.shutdown_all();
    }

    /// Look up a module by ID. Returns JSON string or null.
    pub fn lookup(&self, module_id: &str) -> JsValue {
        match self.inner.lookup(module_id) {
            Some(info) => {
                serde_wasm_bindgen::to_value(&info).unwrap_or(JsValue::NULL)
            }
            None => JsValue::NULL,
        }
    }

    /// Get all registered module IDs as a JSON array.
    #[wasm_bindgen(js_name = "registeredModuleIds")]
    pub fn registered_module_ids(&self) -> JsValue {
        let ids = self.inner.registered_module_ids();
        serde_wasm_bindgen::to_value(&ids).unwrap_or(JsValue::NULL)
    }

    /// Get all modules that depend on the given module.
    #[wasm_bindgen(js_name = "dependentsOf")]
    pub fn dependents_of(&self, module_id: &str) -> JsValue {
        let deps = self.inner.dependents_of(module_id);
        serde_wasm_bindgen::to_value(&deps).unwrap_or(JsValue::NULL)
    }
}
