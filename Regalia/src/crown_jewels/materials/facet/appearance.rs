use serde::{Deserialize, Serialize};

/// Glass appearance mode. Controls foreground color and contrast behavior.
/// Extensible via custom string values.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FacetAppearance(pub String);

impl FacetAppearance {
    /// No appearance enforcement — foreground colors are unmodified.
    pub fn base() -> Self {
        Self("base".into())
    }

    /// Light glass: dark foreground content.
    pub fn light() -> Self {
        Self("light".into())
    }

    /// Dark glass: white foreground + contrast shadow layer.
    pub fn dark() -> Self {
        Self("dark".into())
    }

    /// Follows the system appearance (light/dark mode).
    pub fn auto_mode() -> Self {
        Self("auto".into())
    }

    /// User-defined appearance mode.
    pub fn custom(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn name(&self) -> &str {
        &self.0
    }
}

impl Default for FacetAppearance {
    fn default() -> Self {
        Self::base()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_appearances() {
        assert_eq!(FacetAppearance::base().name(), "base");
        assert_eq!(FacetAppearance::light().name(), "light");
        assert_eq!(FacetAppearance::dark().name(), "dark");
        assert_eq!(FacetAppearance::auto_mode().name(), "auto");
    }

    #[test]
    fn custom_appearance() {
        let a = FacetAppearance::custom("highContrast");
        assert_eq!(a.name(), "highContrast");
    }

    #[test]
    fn default_is_base() {
        assert_eq!(FacetAppearance::default(), FacetAppearance::base());
    }

    #[test]
    fn serde_roundtrip() {
        let a = FacetAppearance::dark();
        let json = serde_json::to_string(&a).unwrap();
        let decoded: FacetAppearance = serde_json::from_str(&json).unwrap();
        assert_eq!(a, decoded);
    }
}
