use serde::{Deserialize, Serialize};

/// Semantic role for material cascade resolution. Determines which style
/// overrides or deltas apply to a layout region.
///
/// Extensible via custom string values — new roles can be created without
/// modifying the type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CrownRole(pub String);

impl CrownRole {
    /// Large surface area, no delta applied.
    pub fn panel() -> Self {
        Self("panel".into())
    }

    /// Dense UI controls: +frost, -refraction, -light.
    pub fn control_bar() -> Self {
        Self("controlBar".into())
    }

    /// Navigation surface: +frost, -light.
    pub fn sidebar() -> Self {
        Self("sidebar".into())
    }

    /// Small repeated element: -frost, +refraction.
    pub fn tile() -> Self {
        Self("tile".into())
    }

    /// Floating surface: +frost, subtle depth.
    pub fn overlay() -> Self {
        Self("overlay".into())
    }

    /// User-defined role.
    pub fn custom(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the name of this role.
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl Default for CrownRole {
    fn default() -> Self {
        Self::panel()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_roles() {
        assert_eq!(CrownRole::panel().name(), "panel");
        assert_eq!(CrownRole::control_bar().name(), "controlBar");
        assert_eq!(CrownRole::sidebar().name(), "sidebar");
        assert_eq!(CrownRole::tile().name(), "tile");
        assert_eq!(CrownRole::overlay().name(), "overlay");
    }

    #[test]
    fn custom_role() {
        let r = CrownRole::custom("toolbar");
        assert_eq!(r.name(), "toolbar");
    }

    #[test]
    fn default_is_panel() {
        assert_eq!(CrownRole::default(), CrownRole::panel());
    }

    #[test]
    fn serde_roundtrip() {
        let r = CrownRole::sidebar();
        let json = serde_json::to_string(&r).unwrap();
        let decoded: CrownRole = serde_json::from_str(&json).unwrap();
        assert_eq!(r, decoded);
    }

    #[test]
    fn hashable() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(CrownRole::panel());
        set.insert(CrownRole::sidebar());
        set.insert(CrownRole::panel()); // duplicate
        assert_eq!(set.len(), 2);
    }
}
