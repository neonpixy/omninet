use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{Ember, Flame};

/// Full semantic color palette with 11 named colors, color families, and custom extensibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Crest {
    /// Named color families (e.g., "yellow" → Flame ramp)
    pub families: HashMap<String, Flame>,

    // 11 semantic colors
    pub primary: Ember,
    pub secondary: Ember,
    pub accent: Ember,
    pub background: Ember,
    pub surface: Ember,
    pub on_primary: Ember,
    pub on_background: Ember,
    pub danger: Ember,
    pub success: Ember,
    pub warning: Ember,
    pub info: Ember,

    /// Extensibility: custom named colors
    pub custom: HashMap<String, Ember>,
}

impl Crest {
    /// Look up a custom color by name.
    pub fn get_custom(&self, name: &str) -> Option<&Ember> {
        self.custom.get(name)
    }

    /// Look up a color family by name.
    pub fn get_family(&self, name: &str) -> Option<&Flame> {
        self.families.get(name)
    }

    /// Dark mode defaults.
    pub fn dark_default() -> Self {
        Self {
            families: HashMap::new(),
            primary: Ember::WHITE,
            secondary: Ember::from_hex("#8E8E93").expect("hardcoded preset hex"),
            accent: Ember::from_hex("#007AFF").expect("hardcoded preset hex"),
            background: Ember::BLACK,
            surface: Ember::from_hex("#1C1C1E").expect("hardcoded preset hex"),
            on_primary: Ember::BLACK,
            on_background: Ember::WHITE,
            danger: Ember::from_hex("#FF3B30").expect("hardcoded preset hex"),
            success: Ember::from_hex("#34C759").expect("hardcoded preset hex"),
            warning: Ember::from_hex("#FF9500").expect("hardcoded preset hex"),
            info: Ember::from_hex("#5AC8FA").expect("hardcoded preset hex"),
            custom: HashMap::new(),
        }
    }

    /// Light mode defaults.
    pub fn light_default() -> Self {
        Self {
            families: HashMap::new(),
            primary: Ember::BLACK,
            secondary: Ember::from_hex("#8E8E93").expect("hardcoded preset hex"),
            accent: Ember::from_hex("#007AFF").expect("hardcoded preset hex"),
            background: Ember::WHITE,
            surface: Ember::from_hex("#F2F2F7").expect("hardcoded preset hex"),
            on_primary: Ember::WHITE,
            on_background: Ember::BLACK,
            danger: Ember::from_hex("#FF3B30").expect("hardcoded preset hex"),
            success: Ember::from_hex("#34C759").expect("hardcoded preset hex"),
            warning: Ember::from_hex("#FF9500").expect("hardcoded preset hex"),
            info: Ember::from_hex("#5AC8FA").expect("hardcoded preset hex"),
            custom: HashMap::new(),
        }
    }
}

impl Default for Crest {
    fn default() -> Self {
        Self::dark_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_default_has_expected_colors() {
        let c = Crest::dark_default();
        assert_eq!(c.primary, Ember::WHITE);
        assert_eq!(c.background, Ember::BLACK);
        assert_eq!(c.accent.to_hex(), "#007AFF");
    }

    #[test]
    fn light_default_inverts_primary() {
        let c = Crest::light_default();
        assert_eq!(c.primary, Ember::BLACK);
        assert_eq!(c.background, Ember::WHITE);
    }

    #[test]
    fn custom_colors() {
        let mut c = Crest::default();
        c.custom
            .insert("brand".into(), Ember::from_hex("#FF6600").unwrap());
        assert!(c.get_custom("brand").is_some());
        assert!(c.get_custom("missing").is_none());
    }

    #[test]
    fn families() {
        let mut c = Crest::default();
        c.families.insert(
            "blue".into(),
            Flame::from_base(Ember::from_hex("#007AFF").unwrap()),
        );
        assert!(c.get_family("blue").is_some());
    }

    #[test]
    fn serde_roundtrip() {
        let c = Crest::dark_default();
        let json = serde_json::to_string(&c).unwrap();
        let decoded: Crest = serde_json::from_str(&json).unwrap();
        assert_eq!(c, decoded);
    }
}
