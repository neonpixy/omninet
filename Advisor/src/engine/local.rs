use super::capabilities::ProviderCapabilities;
use super::provider::{CognitiveProvider, ProviderStatus};

/// Local LLM provider — describes an on-device model backend.
///
/// Covers local inference runtimes like Ollama, MLX, llama.cpp, etc.
/// The Rust side only describes what the provider IS. The platform
/// layer handles actual model loading and inference.
///
/// # Examples
/// ```
/// use advisor::engine::local::LocalProvider;
/// use advisor::engine::provider::CognitiveProvider;
///
/// let provider = LocalProvider::new("llama-3.2-1b");
/// assert_eq!(provider.display_name(), "Local (llama-3.2-1b)");
/// assert!(!provider.is_cloud());
/// assert!(!provider.status().is_available()); // not loaded yet
/// ```
#[derive(Debug, Clone)]
pub struct LocalProvider {
    /// Whether a model is currently loaded and ready for inference
    model_loaded: bool,
    /// Name of the model (e.g., "llama-3.2-1b", "mistral-7b")
    model_name: String,
    /// Cached display name (format: "Local (model_name)")
    display_name: String,
}

impl LocalProvider {
    /// Create a new local provider for the given model.
    ///
    /// The model starts in an unloaded state. Call `set_model_loaded(true)`
    /// once the platform layer has loaded the model.
    pub fn new(model_name: &str) -> Self {
        Self {
            model_loaded: false,
            model_name: model_name.into(),
            display_name: format!("Local ({})", model_name),
        }
    }

    /// Update whether the model is loaded and ready for inference.
    pub fn set_model_loaded(&mut self, loaded: bool) {
        self.model_loaded = loaded;
    }

    /// The model name this provider targets.
    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    /// Whether the model is currently loaded.
    pub fn is_model_loaded(&self) -> bool {
        self.model_loaded
    }
}

impl CognitiveProvider for LocalProvider {
    fn id(&self) -> &str {
        "local"
    }

    fn display_name(&self) -> &str {
        &self.display_name
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::OFFLINE_CAPABLE | ProviderCapabilities::STREAMING
    }

    fn status(&self) -> ProviderStatus {
        if self.model_loaded {
            ProviderStatus::Available
        } else {
            ProviderStatus::Unavailable {
                reason: format!("Model '{}' not loaded", self.model_name),
            }
        }
    }

    fn is_cloud(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::provider::ProviderInfo;

    #[test]
    fn local_provider_defaults_unloaded() {
        let provider = LocalProvider::new("llama-3.2-1b");
        assert_eq!(provider.id(), "local");
        assert_eq!(provider.display_name(), "Local (llama-3.2-1b)");
        assert!(!provider.is_cloud());
        assert!(!provider.status().is_available());
        assert!(!provider.is_model_loaded());
    }

    #[test]
    fn local_provider_model_loaded() {
        let mut provider = LocalProvider::new("mistral-7b");
        provider.set_model_loaded(true);
        assert!(provider.status().is_available());
        assert!(provider.is_model_loaded());
    }

    #[test]
    fn local_provider_model_unloaded_reason() {
        let provider = LocalProvider::new("phi-3");
        match provider.status() {
            ProviderStatus::Unavailable { reason } => {
                assert!(reason.contains("phi-3"));
                assert!(reason.contains("not loaded"));
            }
            other => panic!("Expected Unavailable, got {:?}", other),
        }
    }

    #[test]
    fn local_provider_capabilities() {
        let provider = LocalProvider::new("test");
        let caps = provider.capabilities();
        assert!(caps.contains(ProviderCapabilities::OFFLINE_CAPABLE));
        assert!(caps.contains(ProviderCapabilities::STREAMING));
        assert!(!caps.contains(ProviderCapabilities::TOOL_CALLING));
        assert!(!caps.contains(ProviderCapabilities::LARGE_CONTEXT));
        assert!(!caps.contains(ProviderCapabilities::STRUCTURED_OUTPUT));
    }

    #[test]
    fn local_provider_model_name() {
        let provider = LocalProvider::new("llama-3.2-1b");
        assert_eq!(provider.model_name(), "llama-3.2-1b");
    }

    #[test]
    fn local_provider_toggle_loaded() {
        let mut provider = LocalProvider::new("test");
        assert!(!provider.status().is_available());

        provider.set_model_loaded(true);
        assert!(provider.status().is_available());

        provider.set_model_loaded(false);
        assert!(!provider.status().is_available());
    }

    #[test]
    fn local_provider_info() {
        let mut provider = LocalProvider::new("llama-3.2-1b");
        provider.set_model_loaded(true);

        let info = ProviderInfo::from_provider(&provider);
        assert_eq!(info.id, "local");
        assert_eq!(info.display_name, "Local (llama-3.2-1b)");
        assert!(!info.is_cloud);
        assert!(info.status.is_available());
        assert!(info
            .capabilities
            .contains(ProviderCapabilities::OFFLINE_CAPABLE));
    }

    #[test]
    fn local_provider_in_registry() {
        use crate::engine::registry::ProviderRegistry;

        let mut registry = ProviderRegistry::new();
        let mut provider = LocalProvider::new("llama-3.2-1b");
        provider.set_model_loaded(true);
        registry.register(Box::new(provider));

        let found = registry.get("local").expect("should find local provider");
        assert_eq!(found.display_name(), "Local (llama-3.2-1b)");
        assert!(found.status().is_available());
    }

    #[test]
    fn local_provider_in_router_ultimate_tier() {
        use crate::engine::registry::ProviderRegistry;
        use crate::engine::router::{ProviderRouter, SecurityTier};

        let mut registry = ProviderRegistry::new();
        let mut local = LocalProvider::new("llama-3.2-1b");
        local.set_model_loaded(true);
        registry.register(Box::new(local));

        let router = ProviderRouter::new(registry)
            .with_security_tier(SecurityTier::Ultimate);

        // Local provider should be selectable under Ultimate tier
        let selected = router
            .select(ProviderCapabilities::STREAMING)
            .expect("should select local under ultimate");
        assert_eq!(selected.id(), "local");
        assert!(!selected.is_cloud());
    }
}
