use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Category for digit types.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DigitCategory {
    /// Written content: text, code, headings.
    Text,
    /// Visual and embedded content: images, video, embeds.
    Media,
    /// Layout and organizational: containers, tables, dividers.
    Structure,
    /// Links and cross-references to other content.
    Reference,
    /// Third-party or program-specific custom digit types.
    Extension,
}

/// Defines a known digit type's capabilities and metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DigitTypeDefinition {
    pub digit_type: String,
    pub name: String,
    pub icon: String,
    pub category: DigitCategory,
    pub searchable: bool,
    pub previewable: bool,
}

/// Registry mapping digit type strings to their definitions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DigitTypeRegistry {
    types: HashMap<String, DigitTypeDefinition>,
}

impl DigitTypeRegistry {
    /// Create an empty registry with no types registered.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a registry pre-loaded with the 9 core types.
    pub fn with_core_types() -> Self {
        let mut r = Self::new();
        for def in Self::core_types() {
            r.register(def);
        }
        r
    }

    /// Register a digit type definition. Overwrites if the type already exists.
    pub fn register(&mut self, def: DigitTypeDefinition) {
        self.types.insert(def.digit_type.clone(), def);
    }

    /// Look up a type definition by its identifier string.
    pub fn get(&self, digit_type: &str) -> Option<&DigitTypeDefinition> {
        self.types.get(digit_type)
    }

    /// Check whether a digit type is registered.
    pub fn contains(&self, digit_type: &str) -> bool {
        self.types.contains_key(digit_type)
    }

    /// Iterate over all registered type definitions.
    pub fn all_types(&self) -> impl Iterator<Item = &DigitTypeDefinition> {
        self.types.values()
    }

    /// Get all type definitions belonging to a specific category.
    pub fn types_in_category(&self, cat: &DigitCategory) -> Vec<&DigitTypeDefinition> {
        self.types.values().filter(|d| &d.category == cat).collect()
    }

    /// Number of registered digit types.
    pub fn count(&self) -> usize {
        self.types.len()
    }

    /// The 9 core digit types.
    fn core_types() -> Vec<DigitTypeDefinition> {
        vec![
            DigitTypeDefinition {
                digit_type: "text".into(),
                name: "Text".into(),
                icon: "text.alignleft".into(),
                category: DigitCategory::Text,
                searchable: true,
                previewable: true,
            },
            DigitTypeDefinition {
                digit_type: "code".into(),
                name: "Code".into(),
                icon: "chevron.left.forwardslash.chevron.right".into(),
                category: DigitCategory::Text,
                searchable: true,
                previewable: true,
            },
            DigitTypeDefinition {
                digit_type: "image".into(),
                name: "Image".into(),
                icon: "photo".into(),
                category: DigitCategory::Media,
                searchable: false,
                previewable: true,
            },
            DigitTypeDefinition {
                digit_type: "embed".into(),
                name: "Embed".into(),
                icon: "rectangle.on.rectangle".into(),
                category: DigitCategory::Media,
                searchable: false,
                previewable: true,
            },
            DigitTypeDefinition {
                digit_type: "document".into(),
                name: "Document".into(),
                icon: "doc".into(),
                category: DigitCategory::Structure,
                searchable: false,
                previewable: false,
            },
            DigitTypeDefinition {
                digit_type: "container".into(),
                name: "Container".into(),
                icon: "square.dashed".into(),
                category: DigitCategory::Structure,
                searchable: false,
                previewable: false,
            },
            DigitTypeDefinition {
                digit_type: "table".into(),
                name: "Table".into(),
                icon: "tablecells".into(),
                category: DigitCategory::Structure,
                searchable: true,
                previewable: true,
            },
            DigitTypeDefinition {
                digit_type: "divider".into(),
                name: "Divider".into(),
                icon: "minus".into(),
                category: DigitCategory::Structure,
                searchable: false,
                previewable: true,
            },
            DigitTypeDefinition {
                digit_type: "link".into(),
                name: "Link".into(),
                icon: "link".into(),
                category: DigitCategory::Reference,
                searchable: true,
                previewable: true,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_core_types_has_nine() {
        let r = DigitTypeRegistry::with_core_types();
        assert_eq!(r.count(), 9);
    }

    #[test]
    fn register_custom_type() {
        let mut r = DigitTypeRegistry::new();
        r.register(DigitTypeDefinition {
            digit_type: "widget".into(),
            name: "Widget".into(),
            icon: "gear".into(),
            category: DigitCategory::Extension,
            searchable: false,
            previewable: false,
        });
        assert!(r.contains("widget"));
        assert_eq!(r.get("widget").unwrap().name, "Widget");
    }

    #[test]
    fn get_returns_none_for_unknown() {
        let r = DigitTypeRegistry::new();
        assert!(r.get("unknown").is_none());
    }

    #[test]
    fn contains_check() {
        let r = DigitTypeRegistry::with_core_types();
        assert!(r.contains("text"));
        assert!(r.contains("image"));
        assert!(!r.contains("unknown"));
    }

    #[test]
    fn category_filtering() {
        let r = DigitTypeRegistry::with_core_types();
        let text_types = r.types_in_category(&DigitCategory::Text);
        assert_eq!(text_types.len(), 2); // text + code
        let media_types = r.types_in_category(&DigitCategory::Media);
        assert_eq!(media_types.len(), 2); // image + embed
    }

    #[test]
    fn duplicate_registration_overwrites() {
        let mut r = DigitTypeRegistry::new();
        r.register(DigitTypeDefinition {
            digit_type: "text".into(),
            name: "Text v1".into(),
            icon: "a".into(),
            category: DigitCategory::Text,
            searchable: true,
            previewable: true,
        });
        r.register(DigitTypeDefinition {
            digit_type: "text".into(),
            name: "Text v2".into(),
            icon: "b".into(),
            category: DigitCategory::Text,
            searchable: true,
            previewable: true,
        });
        assert_eq!(r.count(), 1);
        assert_eq!(r.get("text").unwrap().name, "Text v2");
    }

    #[test]
    fn serde_roundtrip() {
        let r = DigitTypeRegistry::with_core_types();
        let json = serde_json::to_string(&r).unwrap();
        let decoded: DigitTypeRegistry = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.count(), 9);
    }

    #[test]
    fn all_types_iteration() {
        let r = DigitTypeRegistry::with_core_types();
        let all: Vec<_> = r.all_types().collect();
        assert_eq!(all.len(), 9);
    }
}
