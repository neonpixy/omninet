use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Corner radii scale with custom extensibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Arch {
    pub sm: f64,
    pub md: f64,
    pub lg: f64,
    pub xl: f64,
    /// Pill/circle radius (9999 = effectively infinite).
    pub full: f64,
    pub custom: HashMap<String, f64>,
}

impl Arch {
    /// Look up a custom corner radius by name.
    pub fn get_custom(&self, name: &str) -> Option<f64> {
        self.custom.get(name).copied()
    }
}

impl Default for Arch {
    fn default() -> Self {
        Self {
            sm: 4.0,
            md: 12.0,
            lg: 20.0,
            xl: 28.0,
            full: 9999.0,
            custom: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_scale() {
        let a = Arch::default();
        assert_eq!(a.sm, 4.0);
        assert_eq!(a.md, 12.0);
        assert_eq!(a.full, 9999.0);
    }

    #[test]
    fn serde_roundtrip() {
        let a = Arch::default();
        let json = serde_json::to_string(&a).unwrap();
        let decoded: Arch = serde_json::from_str(&json).unwrap();
        assert_eq!(a, decoded);
    }
}
