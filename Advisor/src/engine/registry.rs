use std::collections::HashMap;

use crate::error::AdvisorError;

use super::capabilities::ProviderCapabilities;
use super::provider::{CognitiveProvider, ProviderInfo};

/// Registry of available cognitive providers.
///
/// Manages provider lifecycle and preference ordering.
pub struct ProviderRegistry {
    providers: HashMap<String, Box<dyn CognitiveProvider>>,
    preference_order: Vec<String>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            preference_order: Vec::new(),
        }
    }

    /// Register a provider. Added to end of preference order.
    pub fn register(&mut self, provider: Box<dyn CognitiveProvider>) {
        let id = provider.id().to_string();
        if !self.preference_order.contains(&id) {
            self.preference_order.push(id.clone());
        }
        self.providers.insert(id, provider);
    }

    /// Remove a provider.
    pub fn unregister(&mut self, id: &str) -> Result<(), AdvisorError> {
        if self.providers.remove(id).is_none() {
            return Err(AdvisorError::ProviderNotFound(id.into()));
        }
        self.preference_order.retain(|p| p != id);
        Ok(())
    }

    /// Get a provider by ID.
    pub fn get(&self, id: &str) -> Option<&dyn CognitiveProvider> {
        self.providers.get(id).map(|p| p.as_ref())
    }

    /// Set the preference order for provider selection.
    pub fn set_preference_order(&mut self, order: Vec<String>) {
        self.preference_order = order;
    }

    /// Find the best available provider that satisfies the required capabilities.
    pub fn best_available(
        &self,
        required: ProviderCapabilities,
    ) -> Option<&dyn CognitiveProvider> {
        for id in &self.preference_order {
            if let Some(provider) = self.providers.get(id) {
                if provider.status().is_available()
                    && provider.capabilities().satisfies(required)
                {
                    return Some(provider.as_ref());
                }
            }
        }
        None
    }

    /// Get info for all registered providers.
    pub fn provider_info(&self) -> Vec<ProviderInfo> {
        self.preference_order
            .iter()
            .filter_map(|id| self.providers.get(id))
            .map(|p| ProviderInfo::from_provider(p.as_ref()))
            .collect()
    }

    /// Number of registered providers.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// IDs of all registered providers in preference order.
    pub fn provider_ids(&self) -> &[String] {
        &self.preference_order
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::provider::ProviderStatus;

    struct TestProvider {
        id: String,
        caps: ProviderCapabilities,
        available: bool,
        cloud: bool,
    }

    impl CognitiveProvider for TestProvider {
        fn id(&self) -> &str { &self.id }
        fn display_name(&self) -> &str { "Test" }
        fn capabilities(&self) -> ProviderCapabilities { self.caps }
        fn status(&self) -> ProviderStatus {
            if self.available {
                ProviderStatus::Available
            } else {
                ProviderStatus::Unavailable { reason: "offline".into() }
            }
        }
        fn is_cloud(&self) -> bool { self.cloud }
    }

    fn make_provider(id: &str, caps: ProviderCapabilities, available: bool) -> Box<dyn CognitiveProvider> {
        Box::new(TestProvider {
            id: id.into(),
            caps,
            available,
            cloud: false,
        })
    }

    #[test]
    fn register_and_get() {
        let mut reg = ProviderRegistry::new();
        reg.register(make_provider("a", ProviderCapabilities::STREAMING, true));
        assert_eq!(reg.len(), 1);
        assert!(reg.get("a").is_some());
        assert!(reg.get("b").is_none());
    }

    #[test]
    fn unregister() {
        let mut reg = ProviderRegistry::new();
        reg.register(make_provider("a", ProviderCapabilities::empty(), true));
        assert!(reg.unregister("a").is_ok());
        assert!(reg.is_empty());
        assert!(reg.unregister("a").is_err());
    }

    #[test]
    fn best_available_by_preference() {
        let mut reg = ProviderRegistry::new();
        reg.register(make_provider("a", ProviderCapabilities::STREAMING, true));
        reg.register(make_provider("b", ProviderCapabilities::STREAMING | ProviderCapabilities::TOOL_CALLING, true));

        let best = reg.best_available(ProviderCapabilities::STREAMING);
        assert_eq!(best.unwrap().id(), "a"); // first in preference

        let best2 = reg.best_available(ProviderCapabilities::TOOL_CALLING);
        assert_eq!(best2.unwrap().id(), "b"); // only b has tool calling
    }

    #[test]
    fn best_available_skips_unavailable() {
        let mut reg = ProviderRegistry::new();
        reg.register(make_provider("a", ProviderCapabilities::STREAMING, false));
        reg.register(make_provider("b", ProviderCapabilities::STREAMING, true));

        let best = reg.best_available(ProviderCapabilities::STREAMING);
        assert_eq!(best.unwrap().id(), "b");
    }

    #[test]
    fn no_providers_returns_none() {
        let reg = ProviderRegistry::new();
        assert!(reg.best_available(ProviderCapabilities::STREAMING).is_none());
    }

    #[test]
    fn preference_order() {
        let mut reg = ProviderRegistry::new();
        reg.register(make_provider("a", ProviderCapabilities::STREAMING, true));
        reg.register(make_provider("b", ProviderCapabilities::STREAMING, true));
        reg.set_preference_order(vec!["b".into(), "a".into()]);

        let best = reg.best_available(ProviderCapabilities::STREAMING);
        assert_eq!(best.unwrap().id(), "b");
    }

    #[test]
    fn provider_info_list() {
        let mut reg = ProviderRegistry::new();
        reg.register(make_provider("a", ProviderCapabilities::STREAMING, true));
        reg.register(make_provider("b", ProviderCapabilities::TOOL_CALLING, false));
        let info = reg.provider_info();
        assert_eq!(info.len(), 2);
        assert_eq!(info[0].id, "a");
    }
}
