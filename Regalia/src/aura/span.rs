use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Spacing scale: xs through xxl with custom extensibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Span {
    pub xs: f64,
    pub sm: f64,
    pub md: f64,
    pub lg: f64,
    pub xl: f64,
    pub xxl: f64,
    pub custom: HashMap<String, f64>,
}

impl Span {
    /// Look up a custom spacing value by name.
    pub fn get_custom(&self, name: &str) -> Option<f64> {
        self.custom.get(name).copied()
    }
}

impl Default for Span {
    fn default() -> Self {
        Self {
            xs: 4.0,
            sm: 8.0,
            md: 16.0,
            lg: 24.0,
            xl: 32.0,
            xxl: 48.0,
            custom: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_scale() {
        let s = Span::default();
        assert_eq!(s.xs, 4.0);
        assert_eq!(s.md, 16.0);
        assert_eq!(s.xxl, 48.0);
    }

    #[test]
    fn custom_spacing() {
        let mut s = Span::default();
        s.custom.insert("page-margin".into(), 64.0);
        assert_eq!(s.get_custom("page-margin"), Some(64.0));
        assert_eq!(s.get_custom("missing"), None);
    }

    #[test]
    fn serde_roundtrip() {
        let s = Span::default();
        let json = serde_json::to_string(&s).unwrap();
        let decoded: Span = serde_json::from_str(&json).unwrap();
        assert_eq!(s, decoded);
    }
}
