use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::crown_jewels::crown_role::CrownRole;
use crate::crown_jewels::material::Material;

/// CSS-like cascade for any material type. Resolution order:
/// 1. Explicit override for role (wins if present)
/// 2. Base style + delta for role (additive)
/// 3. Base style (fallback)
///
/// Generic over `M: Material`, so the same Stylesheet works for glass, iris,
/// or any future material.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(bound = "M: Material, M::Delta: Serialize + serde::de::DeserializeOwned")]
pub struct Stylesheet<M: Material> {
    /// Base style applied when no override or delta exists.
    pub base: M,
    /// Per-role full style overrides (take precedence over deltas).
    pub overrides: HashMap<CrownRole, M>,
    /// Per-role additive deltas (applied on top of base).
    pub deltas: HashMap<CrownRole, M::Delta>,
}

impl<M: Material> Stylesheet<M> {
    /// Create a stylesheet with a base style and no overrides or deltas.
    pub fn new(base: M) -> Self {
        Self {
            base,
            overrides: HashMap::new(),
            deltas: HashMap::new(),
        }
    }

    /// Resolve the style for a given role.
    pub fn style_for(&self, role: &CrownRole) -> M {
        if let Some(explicit) = self.overrides.get(role) {
            return explicit.clone();
        }
        if let Some(delta) = self.deltas.get(role) {
            return self.base.applying(delta);
        }
        self.base.clone()
    }

    /// Get the delta for a role, or identity if none.
    pub fn delta_for(&self, role: &CrownRole) -> M::Delta {
        self.deltas
            .get(role)
            .cloned()
            .unwrap_or_default()
    }

    /// Set an explicit override for a role.
    pub fn set_override(&mut self, role: CrownRole, style: M) {
        self.overrides.insert(role, style);
    }

    /// Set a delta for a role.
    pub fn set_delta(&mut self, role: CrownRole, delta: M::Delta) {
        self.deltas.insert(role, delta);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crown_jewels::material::MaterialDelta;
    use crate::crown_jewels::materials::facet::{FacetStyle, FacetStyleDelta};

    #[test]
    fn base_resolves_for_unknown_role() {
        let sheet = Stylesheet::new(FacetStyle::default());
        let style = sheet.style_for(&CrownRole::custom("unknown"));
        assert_eq!(style, FacetStyle::default());
    }

    #[test]
    fn override_wins_over_delta() {
        let mut sheet = Stylesheet::new(FacetStyle::default());

        // Set both an override and a delta for sidebar.
        let override_style = FacetStyle::clear();
        sheet.set_override(CrownRole::sidebar(), override_style.clone());
        sheet.set_delta(
            CrownRole::sidebar(),
            FacetStyleDelta {
                frost_delta: Some(0.2),
                ..Default::default()
            },
        );

        // Override should win.
        let resolved = sheet.style_for(&CrownRole::sidebar());
        assert_eq!(resolved, override_style);
    }

    #[test]
    fn delta_applied_additively() {
        let mut sheet = Stylesheet::new(FacetStyle::default());
        sheet.set_delta(
            CrownRole::tile(),
            FacetStyleDelta {
                frost_delta: Some(-0.15),
                refraction_delta: Some(0.05),
                ..Default::default()
            },
        );

        let resolved = sheet.style_for(&CrownRole::tile());
        let expected_frost = (0.5 - 0.15f64).clamp(0.0, 1.0);
        let expected_refraction = (0.35 + 0.05f64).clamp(0.0, 1.0);
        assert!((resolved.frost - expected_frost).abs() < 1e-10);
        assert!((resolved.refraction - expected_refraction).abs() < 1e-10);
    }

    #[test]
    fn serde_roundtrip() {
        let mut sheet = Stylesheet::new(FacetStyle::default());
        sheet.set_delta(
            CrownRole::sidebar(),
            FacetStyleDelta {
                frost_delta: Some(0.1),
                ..Default::default()
            },
        );
        let json = serde_json::to_string(&sheet).unwrap();
        let decoded: Stylesheet<FacetStyle> = serde_json::from_str(&json).unwrap();
        assert_eq!(
            decoded.style_for(&CrownRole::sidebar()).frost,
            sheet.style_for(&CrownRole::sidebar()).frost
        );
    }

    #[test]
    fn delta_for_returns_identity_when_missing() {
        let sheet = Stylesheet::new(FacetStyle::default());
        let delta = sheet.delta_for(&CrownRole::panel());
        assert!(delta.is_identity());
    }
}
