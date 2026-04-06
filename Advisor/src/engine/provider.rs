use serde::{Deserialize, Serialize};

use super::capabilities::ProviderCapabilities;

/// A pluggable LLM backend.
///
/// This trait defines what a provider IS — not what it DOES.
/// Actual generation (async, network) happens outside the crate.
/// The crate defines GenerationContext (request) and consumes
/// GenerationResult (response). The platform layer calls the provider.
pub trait CognitiveProvider: Send + Sync {
    /// Unique identifier (e.g., "anthropic", "ollama", "mlx")
    fn id(&self) -> &str;
    /// Human-readable name (e.g., "Claude API", "Ollama Local")
    fn display_name(&self) -> &str;
    /// What this provider can do
    fn capabilities(&self) -> ProviderCapabilities;
    /// Current availability status
    fn status(&self) -> ProviderStatus;
    /// Whether this provider requires cloud access
    fn is_cloud(&self) -> bool;
}

/// Whether a provider is ready to use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProviderStatus {
    /// Ready to generate
    Available,
    /// Not currently usable
    Unavailable { reason: String },
    /// Needs configuration (API key, model path, etc.)
    RequiresSetup { message: String },
}

impl ProviderStatus {
    /// Returns true if the provider is ready to generate.
    pub fn is_available(&self) -> bool {
        matches!(self, ProviderStatus::Available)
    }
}

/// Information about a registered provider (for UI/logging).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderInfo {
    pub id: String,
    pub display_name: String,
    pub capabilities: ProviderCapabilities,
    pub status: ProviderStatus,
    pub is_cloud: bool,
}

impl ProviderInfo {
    /// Create a snapshot of provider information from a live provider reference.
    pub fn from_provider(provider: &dyn CognitiveProvider) -> Self {
        Self {
            id: provider.id().to_string(),
            display_name: provider.display_name().to_string(),
            capabilities: provider.capabilities(),
            status: provider.status(),
            is_cloud: provider.is_cloud(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockProvider {
        id: String,
        cloud: bool,
    }

    impl CognitiveProvider for MockProvider {
        fn id(&self) -> &str { &self.id }
        fn display_name(&self) -> &str { "Mock Provider" }
        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities::STREAMING | ProviderCapabilities::TOOL_CALLING
        }
        fn status(&self) -> ProviderStatus { ProviderStatus::Available }
        fn is_cloud(&self) -> bool { self.cloud }
    }

    #[test]
    fn provider_info_from_trait() {
        let provider = MockProvider { id: "mock".into(), cloud: false };
        let info = ProviderInfo::from_provider(&provider);
        assert_eq!(info.id, "mock");
        assert!(info.capabilities.contains(ProviderCapabilities::STREAMING));
        assert!(!info.is_cloud);
    }

    #[test]
    fn provider_status_variants() {
        assert!(ProviderStatus::Available.is_available());
        assert!(!ProviderStatus::Unavailable { reason: "offline".into() }.is_available());
        assert!(!ProviderStatus::RequiresSetup { message: "need key".into() }.is_available());
    }
}
