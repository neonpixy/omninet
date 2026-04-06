use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A composite style that references Aura token keys by name.
///
/// ComponentStyle doesn't hold actual values — it holds string keys that resolve
/// against the active Aura. This allows the same style definition to adapt when
/// the theme changes.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComponentStyle {
    pub name: String,
    /// Crest color key for foreground/text color.
    pub crest_color: Option<String>,
    /// Crest color key for background.
    pub background_color: Option<String>,
    /// Span size key for padding.
    pub span_padding: Option<String>,
    /// Arch key for corner radius.
    pub arch_radius: Option<String>,
    /// Inscription level key for typography.
    pub inscription_level: Option<String>,
    /// Umbra key for shadow.
    pub umbra_shadow: Option<String>,
    /// Gradient key (into Aura's gradients map).
    pub gradient: Option<String>,
    /// Material variant name.
    pub material: Option<String>,
}

impl ComponentStyle {
    /// A primary button style: accent background, white text, medium padding and radius.
    pub fn primary_button() -> Self {
        Self {
            name: "primary_button".into(),
            crest_color: Some("on_primary".into()),
            background_color: Some("accent".into()),
            span_padding: Some("md".into()),
            arch_radius: Some("md".into()),
            inscription_level: Some("body".into()),
            umbra_shadow: Some("subtle".into()),
            gradient: None,
            material: None,
        }
    }

    /// A card style: surface background, default text, medium radius, elevated shadow.
    pub fn card() -> Self {
        Self {
            name: "card".into(),
            crest_color: Some("primary".into()),
            background_color: Some("surface".into()),
            span_padding: Some("md".into()),
            arch_radius: Some("lg".into()),
            inscription_level: None,
            umbra_shadow: Some("elevated".into()),
            gradient: None,
            material: None,
        }
    }

    /// An input field style: surface background, body text, small radius, subtle shadow.
    pub fn input_field() -> Self {
        Self {
            name: "input_field".into(),
            crest_color: Some("primary".into()),
            background_color: Some("surface".into()),
            span_padding: Some("sm".into()),
            arch_radius: Some("sm".into()),
            inscription_level: Some("body".into()),
            umbra_shadow: Some("subtle".into()),
            gradient: None,
            material: None,
        }
    }

    /// A body text style: primary color, body typography, no background or decorations.
    pub fn text_body() -> Self {
        Self {
            name: "text_body".into(),
            crest_color: Some("primary".into()),
            background_color: None,
            span_padding: None,
            arch_radius: None,
            inscription_level: Some("body".into()),
            umbra_shadow: None,
            gradient: None,
            material: None,
        }
    }
}

/// Registry of named component styles.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ComponentStyleRegistry {
    styles: HashMap<String, ComponentStyle>,
}

impl ComponentStyleRegistry {
    pub fn new() -> Self {
        Self {
            styles: HashMap::new(),
        }
    }

    /// Register a style. Overwrites if a style with the same name exists.
    pub fn register(&mut self, style: ComponentStyle) {
        self.styles.insert(style.name.clone(), style);
    }

    /// Look up a style by name.
    pub fn get(&self, name: &str) -> Option<&ComponentStyle> {
        self.styles.get(name)
    }

    /// List all registered style names.
    pub fn list(&self) -> Vec<&str> {
        self.styles.keys().map(|k| k.as_str()).collect()
    }

    /// Remove a style by name, returning it if it existed.
    pub fn remove(&mut self, name: &str) -> Option<ComponentStyle> {
        self.styles.remove(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_button_preset() {
        let s = ComponentStyle::primary_button();
        assert_eq!(s.name, "primary_button");
        assert_eq!(s.crest_color.as_deref(), Some("on_primary"));
        assert_eq!(s.background_color.as_deref(), Some("accent"));
        assert_eq!(s.span_padding.as_deref(), Some("md"));
        assert_eq!(s.arch_radius.as_deref(), Some("md"));
    }

    #[test]
    fn card_preset() {
        let s = ComponentStyle::card();
        assert_eq!(s.name, "card");
        assert_eq!(s.background_color.as_deref(), Some("surface"));
        assert_eq!(s.arch_radius.as_deref(), Some("lg"));
        assert_eq!(s.umbra_shadow.as_deref(), Some("elevated"));
    }

    #[test]
    fn input_field_preset() {
        let s = ComponentStyle::input_field();
        assert_eq!(s.name, "input_field");
        assert_eq!(s.span_padding.as_deref(), Some("sm"));
        assert_eq!(s.arch_radius.as_deref(), Some("sm"));
    }

    #[test]
    fn text_body_preset() {
        let s = ComponentStyle::text_body();
        assert_eq!(s.name, "text_body");
        assert_eq!(s.inscription_level.as_deref(), Some("body"));
        assert!(s.background_color.is_none());
        assert!(s.umbra_shadow.is_none());
    }

    #[test]
    fn registry_new_is_empty() {
        let reg = ComponentStyleRegistry::new();
        assert!(reg.list().is_empty());
    }

    #[test]
    fn registry_default_is_empty() {
        let reg = ComponentStyleRegistry::default();
        assert!(reg.list().is_empty());
    }

    #[test]
    fn registry_register_and_get() {
        let mut reg = ComponentStyleRegistry::new();
        reg.register(ComponentStyle::primary_button());
        assert!(reg.get("primary_button").is_some());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn registry_list() {
        let mut reg = ComponentStyleRegistry::new();
        reg.register(ComponentStyle::primary_button());
        reg.register(ComponentStyle::card());
        let names = reg.list();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"primary_button"));
        assert!(names.contains(&"card"));
    }

    #[test]
    fn registry_remove() {
        let mut reg = ComponentStyleRegistry::new();
        reg.register(ComponentStyle::primary_button());
        let removed = reg.remove("primary_button");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().name, "primary_button");
        assert!(reg.get("primary_button").is_none());
    }

    #[test]
    fn registry_remove_nonexistent() {
        let mut reg = ComponentStyleRegistry::new();
        assert!(reg.remove("ghost").is_none());
    }

    #[test]
    fn registry_overwrite() {
        let mut reg = ComponentStyleRegistry::new();
        reg.register(ComponentStyle::primary_button());
        // Register again with same name
        let mut modified = ComponentStyle::primary_button();
        modified.gradient = Some("sunset".into());
        reg.register(modified);
        let style = reg.get("primary_button").unwrap();
        assert_eq!(style.gradient.as_deref(), Some("sunset"));
    }

    #[test]
    fn serde_roundtrip_style() {
        let s = ComponentStyle::primary_button();
        let json = serde_json::to_string(&s).unwrap();
        let decoded: ComponentStyle = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.name, s.name);
        assert_eq!(decoded.crest_color, s.crest_color);
    }

    #[test]
    fn serde_roundtrip_registry() {
        let mut reg = ComponentStyleRegistry::new();
        reg.register(ComponentStyle::primary_button());
        reg.register(ComponentStyle::card());
        let json = serde_json::to_string(&reg).unwrap();
        let decoded: ComponentStyleRegistry = serde_json::from_str(&json).unwrap();
        assert!(decoded.get("primary_button").is_some());
        assert!(decoded.get("card").is_some());
    }
}
