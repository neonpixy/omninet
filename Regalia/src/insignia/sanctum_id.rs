use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Unique identifier for a Sanctum. Serializes as a flat string.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SanctumID(pub String);

impl SanctumID {
    /// Create a new SanctumID from any string-like value.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The string value of this identifier.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Built-in ID for the toolbar region.
    pub fn toolbar() -> Self {
        Self("toolbar".into())
    }
    /// Built-in ID for the sidebar region.
    pub fn sidebar() -> Self {
        Self("sidebar".into())
    }
    /// Built-in ID for the main content region.
    pub fn content() -> Self {
        Self("content".into())
    }
    /// Built-in ID for the overlay region.
    pub fn overlay() -> Self {
        Self("overlay".into())
    }
}

impl From<&str> for SanctumID {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for SanctumID {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl std::fmt::Display for SanctumID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Serialize as flat string
impl Serialize for SanctumID {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for SanctumID {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_ids() {
        assert_eq!(SanctumID::toolbar().as_str(), "toolbar");
        assert_eq!(SanctumID::sidebar().as_str(), "sidebar");
        assert_eq!(SanctumID::content().as_str(), "content");
        assert_eq!(SanctumID::overlay().as_str(), "overlay");
    }

    #[test]
    fn from_str() {
        let id: SanctumID = "custom-panel".into();
        assert_eq!(id.as_str(), "custom-panel");
    }

    #[test]
    fn serde_as_flat_string() {
        let id = SanctumID::toolbar();
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"toolbar\"");
        let decoded: SanctumID = serde_json::from_str(&json).unwrap();
        assert_eq!(id, decoded);
    }

    #[test]
    fn hashable() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(SanctumID::toolbar());
        set.insert(SanctumID::toolbar());
        assert_eq!(set.len(), 1);
    }
}
