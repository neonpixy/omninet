use serde::{Deserialize, Serialize};

/// Glass material variant. Extensible via custom string values.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FacetVariant(pub String);

impl FacetVariant {
    /// Full blur/refraction glass.
    pub fn regular() -> Self {
        Self("regular".into())
    }

    /// Transparent, subtle glass.
    pub fn clear() -> Self {
        Self("clear".into())
    }

    /// User-defined variant.
    pub fn custom(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn name(&self) -> &str {
        &self.0
    }
}

impl Default for FacetVariant {
    fn default() -> Self {
        Self::regular()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_variants() {
        assert_eq!(FacetVariant::regular().name(), "regular");
        assert_eq!(FacetVariant::clear().name(), "clear");
    }

    #[test]
    fn custom_variant() {
        let v = FacetVariant::custom("holographic");
        assert_eq!(v.name(), "holographic");
    }

    #[test]
    fn default_is_regular() {
        assert_eq!(FacetVariant::default(), FacetVariant::regular());
    }

    #[test]
    fn serde_roundtrip() {
        let v = FacetVariant::clear();
        let json = serde_json::to_string(&v).unwrap();
        let decoded: FacetVariant = serde_json::from_str(&json).unwrap();
        assert_eq!(v, decoded);
    }
}
