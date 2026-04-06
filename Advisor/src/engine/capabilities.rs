use serde::{Deserialize, Serialize};

bitflags::bitflags! {
    /// What a cognitive provider can do.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
    pub struct ProviderCapabilities: u32 {
        /// Supports streaming token-by-token responses
        const STREAMING        = 0b0000_0001;
        /// Supports tool/function calling
        const TOOL_CALLING     = 0b0000_0010;
        /// Supports large context windows (>4K tokens)
        const LARGE_CONTEXT    = 0b0000_0100;
        /// Works without network access
        const OFFLINE_CAPABLE  = 0b0000_1000;
        /// Supports structured JSON output
        const STRUCTURED_OUTPUT = 0b0001_0000;
    }
}

impl ProviderCapabilities {
    /// Check if all required capabilities are present.
    pub fn satisfies(&self, required: ProviderCapabilities) -> bool {
        self.contains(required)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_flags() {
        let caps = ProviderCapabilities::STREAMING | ProviderCapabilities::TOOL_CALLING;
        assert!(caps.contains(ProviderCapabilities::STREAMING));
        assert!(caps.contains(ProviderCapabilities::TOOL_CALLING));
        assert!(!caps.contains(ProviderCapabilities::OFFLINE_CAPABLE));
    }

    #[test]
    fn satisfies_check() {
        let provider_caps = ProviderCapabilities::STREAMING
            | ProviderCapabilities::TOOL_CALLING
            | ProviderCapabilities::LARGE_CONTEXT;

        let required = ProviderCapabilities::STREAMING | ProviderCapabilities::TOOL_CALLING;
        assert!(provider_caps.satisfies(required));

        let too_much =
            ProviderCapabilities::STREAMING | ProviderCapabilities::OFFLINE_CAPABLE;
        assert!(!provider_caps.satisfies(too_much));
    }

    #[test]
    fn empty_capabilities() {
        let caps = ProviderCapabilities::empty();
        assert!(!caps.contains(ProviderCapabilities::STREAMING));
        // Empty satisfies empty
        assert!(caps.satisfies(ProviderCapabilities::empty()));
    }

    #[test]
    fn capabilities_serialization() {
        let caps = ProviderCapabilities::STREAMING | ProviderCapabilities::LARGE_CONTEXT;
        let json = serde_json::to_string(&caps).unwrap();
        let deserialized: ProviderCapabilities = serde_json::from_str(&json).unwrap();
        assert_eq!(caps, deserialized);
    }
}
