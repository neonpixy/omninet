use serde::{Deserialize, Serialize};

use super::capabilities::ProviderCapabilities;
use super::provider::CognitiveProvider;
use super::registry::ProviderRegistry;
use crate::error::AdvisorError;

/// Multi-provider selection with scoring and security enforcement.
///
/// Routes generation requests to the best available provider based on
/// strategy (cost/quality/speed) and security tier constraints.
pub struct ProviderRouter {
    pub registry: ProviderRegistry,
    pub preferences: ProviderPreferences,
    pub security_tier: SecurityTier,
}

/// Security tier controls which providers are allowed.
///
/// Covenant: Sovereignty — your thoughts stay on your device if you choose.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SecurityTier {
    /// Cloud providers allowed
    Balanced,
    /// Cloud providers allowed but with additional encryption
    Hardened,
    /// Local providers only — cloud blocked
    Ultimate,
}

impl SecurityTier {
    /// Whether cloud providers are blocked at this tier.
    pub fn blocks_cloud(&self) -> bool {
        matches!(self, SecurityTier::Ultimate)
    }
}

/// How to choose among available providers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderPreferences {
    /// Selection strategy
    pub strategy: SelectionStrategy,
    /// Preferred provider ID (overrides strategy if available)
    pub preferred_provider: Option<String>,
    /// Providers to never use
    pub excluded_providers: Vec<String>,
}

impl Default for ProviderPreferences {
    fn default() -> Self {
        Self {
            strategy: SelectionStrategy::PriorityOrder,
            preferred_provider: None,
            excluded_providers: Vec::new(),
        }
    }
}

/// Strategy for selecting among eligible providers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SelectionStrategy {
    /// Use the preference order from the registry
    PriorityOrder,
    /// Minimize cost (prefer local/free providers)
    CostOptimized,
    /// Maximize quality (prefer capable cloud providers)
    QualityOptimized,
    /// Minimize latency (prefer local providers)
    SpeedOptimized,
}

impl ProviderRouter {
    pub fn new(registry: ProviderRegistry) -> Self {
        Self {
            registry,
            preferences: ProviderPreferences::default(),
            security_tier: SecurityTier::Balanced,
        }
    }

    pub fn with_preferences(mut self, preferences: ProviderPreferences) -> Self {
        self.preferences = preferences;
        self
    }

    pub fn with_security_tier(mut self, tier: SecurityTier) -> Self {
        self.security_tier = tier;
        self
    }

    /// Select the best provider for a request with the given requirements.
    pub fn select(
        &self,
        required: ProviderCapabilities,
    ) -> Result<&dyn CognitiveProvider, AdvisorError> {
        // Check preferred provider first
        if let Some(ref preferred_id) = self.preferences.preferred_provider {
            if let Some(provider) = self.registry.get(preferred_id) {
                if self.is_eligible(provider, required) {
                    return Ok(provider);
                }
            }
        }

        // Build candidate list
        let candidates: Vec<&dyn CognitiveProvider> = self
            .registry
            .provider_ids()
            .iter()
            .filter_map(|id| self.registry.get(id))
            .filter(|p| self.is_eligible(*p, required))
            .collect();

        if candidates.is_empty() {
            return Err(AdvisorError::NoProvidersAvailable);
        }

        // Apply selection strategy
        let selected = match self.preferences.strategy {
            SelectionStrategy::PriorityOrder => candidates[0],
            SelectionStrategy::CostOptimized => {
                *candidates.iter().min_by_key(|p| cost_score(p))
                    .expect("candidates verified non-empty above")
            }
            SelectionStrategy::QualityOptimized => {
                *candidates.iter().max_by_key(|p| quality_score(p))
                    .expect("candidates verified non-empty above")
            }
            SelectionStrategy::SpeedOptimized => {
                *candidates.iter().max_by_key(|p| speed_score(p))
                    .expect("candidates verified non-empty above")
            }
        };

        Ok(selected)
    }

    /// Build a fallback chain of eligible providers.
    pub fn fallback_chain(
        &self,
        required: ProviderCapabilities,
    ) -> Vec<&dyn CognitiveProvider> {
        self.registry
            .provider_ids()
            .iter()
            .filter_map(|id| self.registry.get(id))
            .filter(|p| self.is_eligible(*p, required))
            .collect()
    }

    /// Check if a provider is eligible given current constraints.
    fn is_eligible(&self, provider: &dyn CognitiveProvider, required: ProviderCapabilities) -> bool {
        // Must be available
        if !provider.status().is_available() {
            return false;
        }

        // Must have required capabilities
        if !provider.capabilities().satisfies(required) {
            return false;
        }

        // Security tier enforcement
        if self.security_tier.blocks_cloud() && provider.is_cloud() {
            return false;
        }

        // Not excluded
        if self.preferences.excluded_providers.iter().any(|id| id == provider.id()) {
            return false;
        }

        true
    }
}

/// Cost score: lower is cheaper. Cloud providers cost more.
fn cost_score(provider: &&dyn CognitiveProvider) -> u32 {
    if provider.is_cloud() { 3 } else { 1 }
}

/// Quality score: higher is better. Cloud providers generally higher quality.
fn quality_score(provider: &&dyn CognitiveProvider) -> u32 {
    let mut score = 2u32;
    if provider.is_cloud() {
        score += 2;
    }
    if provider.capabilities().contains(ProviderCapabilities::TOOL_CALLING) {
        score += 1;
    }
    if provider.capabilities().contains(ProviderCapabilities::LARGE_CONTEXT) {
        score += 1;
    }
    score
}

/// Speed score: higher is faster. Local providers generally faster.
fn speed_score(provider: &&dyn CognitiveProvider) -> u32 {
    if provider.is_cloud() { 2 } else { 4 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::provider::ProviderStatus;

    struct TestProvider {
        id: String,
        caps: ProviderCapabilities,
        cloud: bool,
    }

    impl CognitiveProvider for TestProvider {
        fn id(&self) -> &str { &self.id }
        fn display_name(&self) -> &str { &self.id }
        fn capabilities(&self) -> ProviderCapabilities { self.caps }
        fn status(&self) -> ProviderStatus { ProviderStatus::Available }
        fn is_cloud(&self) -> bool { self.cloud }
    }

    fn make(id: &str, caps: ProviderCapabilities, cloud: bool) -> Box<dyn CognitiveProvider> {
        Box::new(TestProvider { id: id.into(), caps, cloud })
    }

    fn test_router() -> ProviderRouter {
        let mut reg = ProviderRegistry::new();
        reg.register(make("local", ProviderCapabilities::STREAMING | ProviderCapabilities::OFFLINE_CAPABLE, false));
        reg.register(make("claude", ProviderCapabilities::STREAMING | ProviderCapabilities::TOOL_CALLING | ProviderCapabilities::LARGE_CONTEXT, true));
        ProviderRouter::new(reg)
    }

    #[test]
    fn priority_order_selects_first() {
        let router = test_router();
        let selected = router.select(ProviderCapabilities::STREAMING).unwrap();
        assert_eq!(selected.id(), "local"); // first registered
    }

    #[test]
    fn quality_optimized_prefers_cloud() {
        let router = test_router().with_preferences(ProviderPreferences {
            strategy: SelectionStrategy::QualityOptimized,
            ..Default::default()
        });
        let selected = router.select(ProviderCapabilities::STREAMING).unwrap();
        assert_eq!(selected.id(), "claude");
    }

    #[test]
    fn speed_optimized_prefers_local() {
        let router = test_router().with_preferences(ProviderPreferences {
            strategy: SelectionStrategy::SpeedOptimized,
            ..Default::default()
        });
        let selected = router.select(ProviderCapabilities::STREAMING).unwrap();
        assert_eq!(selected.id(), "local");
    }

    #[test]
    fn ultimate_tier_blocks_cloud() {
        let router = test_router().with_security_tier(SecurityTier::Ultimate);
        let selected = router.select(ProviderCapabilities::STREAMING).unwrap();
        assert_eq!(selected.id(), "local");

        // Tool calling requires cloud, which is blocked
        let result = router.select(ProviderCapabilities::TOOL_CALLING);
        assert!(result.is_err());
    }

    #[test]
    fn preferred_provider_override() {
        let router = test_router().with_preferences(ProviderPreferences {
            strategy: SelectionStrategy::CostOptimized,
            preferred_provider: Some("claude".into()),
            excluded_providers: Vec::new(),
        });
        let selected = router.select(ProviderCapabilities::STREAMING).unwrap();
        assert_eq!(selected.id(), "claude"); // preferred overrides strategy
    }

    #[test]
    fn excluded_providers_skipped() {
        let router = test_router().with_preferences(ProviderPreferences {
            strategy: SelectionStrategy::PriorityOrder,
            preferred_provider: None,
            excluded_providers: vec!["local".into()],
        });
        let selected = router.select(ProviderCapabilities::STREAMING).unwrap();
        assert_eq!(selected.id(), "claude");
    }

    #[test]
    fn fallback_chain() {
        let router = test_router();
        let chain = router.fallback_chain(ProviderCapabilities::STREAMING);
        assert_eq!(chain.len(), 2);
    }

    #[test]
    fn no_providers_error() {
        let router = ProviderRouter::new(ProviderRegistry::new());
        assert!(router.select(ProviderCapabilities::STREAMING).is_err());
    }

    #[test]
    fn security_tier_blocks_cloud() {
        assert!(!SecurityTier::Balanced.blocks_cloud());
        assert!(!SecurityTier::Hardened.blocks_cloud());
        assert!(SecurityTier::Ultimate.blocks_cloud());
    }
}
