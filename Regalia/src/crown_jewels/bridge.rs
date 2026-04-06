use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::crown_jewels::crown_role::CrownRole;
use crate::crown_jewels::material::Material;
use crate::crown_jewels::shape::ShapeDescriptor;
use crate::crown_jewels::stylesheet::Stylesheet;
use crate::domain::{Appointment, Arbiter, Domain};
use crate::error::RegaliaError;
use crate::insignia::{BorderInsets, SanctumID};
use crate::sanctum::Sanctum;

/// A Sanctum enriched with material rendering context.
///
/// Material-generic: the actual material style lives at the platform layer.
/// CrownSanctum says WHERE and WHAT SHAPE — the material system says HOW IT LOOKS.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrownSanctum {
    pub sanctum: Sanctum,
    pub role: CrownRole,
    pub shape: ShapeDescriptor,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_insets: Option<BorderInsets>,
}

impl CrownSanctum {
    /// Sidebar preset: leading border, 240pt, Column layout.
    pub fn sidebar(
        width: Option<f64>,
        shape: ShapeDescriptor,
        content_insets: Option<BorderInsets>,
    ) -> Self {
        Self {
            sanctum: Sanctum::sidebar(width, None),
            role: CrownRole::sidebar(),
            shape,
            content_insets,
        }
    }

    /// Content area preset: free-form, panel role.
    pub fn content(shape: ShapeDescriptor, content_insets: Option<BorderInsets>) -> Self {
        Self {
            sanctum: Sanctum::content(None),
            role: CrownRole::panel(),
            shape,
            content_insets,
        }
    }

    /// Toolbar preset: top border, 44pt, Rank layout.
    pub fn toolbar(
        height: Option<f64>,
        shape: ShapeDescriptor,
        content_insets: Option<BorderInsets>,
    ) -> Self {
        Self {
            sanctum: Sanctum::toolbar(height, None),
            role: CrownRole::control_bar(),
            shape,
            content_insets,
        }
    }

    /// Overlay preset: floating, overlay role.
    pub fn overlay(shape: ShapeDescriptor, content_insets: Option<BorderInsets>) -> Self {
        Self {
            sanctum: Sanctum::overlay(None),
            role: CrownRole::overlay(),
            shape,
            content_insets,
        }
    }

    /// The sanctum identifier for this crown sanctum.
    pub fn id(&self) -> &SanctumID {
        &self.sanctum.id
    }
}

/// An Appointment enriched with its resolved shape and role.
/// The actual material style is resolved lazily from a Stylesheet at render time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrownAppointment {
    pub appointment: Appointment,
    pub shape: ShapeDescriptor,
    pub role: CrownRole,
}

/// A complete layout result with material rendering context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrownDomain {
    pub domain: Domain,
    pub appointments: Vec<CrownAppointment>,
    pub sanctum_shapes: HashMap<String, ShapeDescriptor>,
}

/// Resolves layout + material cascade for rendering.
///
/// Delegates layout to Regalia's Arbiter, then maps each Appointment to a
/// CrownAppointment with its associated shape and role.
pub struct CrownArbiter;

impl CrownArbiter {
    /// Resolve layout and material context.
    pub fn resolve(
        bounds: (f64, f64, f64, f64),
        crown_sanctums: &[CrownSanctum],
        vassals: &HashMap<SanctumID, Vec<&dyn crate::domain::Clansman>>,
        formation_resolver: Option<crate::FormationResolver<'_>>,
    ) -> Result<CrownDomain, RegaliaError> {
        // Extract plain sanctums for Arbiter.
        let sanctums: Vec<Sanctum> = crown_sanctums.iter().map(|cs| cs.sanctum.clone()).collect();

        // Build insets map.
        let mut insets = HashMap::new();
        for cs in crown_sanctums {
            let id = cs.sanctum.id.clone();
            if let Some(ref ci) = cs.content_insets {
                insets.insert(id, *ci);
            } else {
                // Role-based defaults.
                let default = Self::default_insets(&cs.role);
                if default != BorderInsets::ZERO {
                    insets.insert(id, default);
                }
            }
        }

        // Delegate to Regalia's Arbiter.
        let domain = Arbiter::resolve(bounds, &sanctums, vassals, &insets, formation_resolver)?;

        // Build shape + role lookup.
        let mut sanctum_shapes = HashMap::new();
        let mut role_map = HashMap::new();
        for cs in crown_sanctums {
            sanctum_shapes.insert(cs.sanctum.id.as_str().to_string(), cs.shape.clone());
            role_map.insert(cs.sanctum.id.as_str().to_string(), cs.role.clone());
        }

        // Map appointments to CrownAppointments.
        let appointments = domain
            .appointments
            .iter()
            .map(|appt| {
                let sid = appt.sanctum_id.as_str();
                CrownAppointment {
                    appointment: appt.clone(),
                    shape: sanctum_shapes
                        .get(sid)
                        .cloned()
                        .unwrap_or_default(),
                    role: role_map
                        .get(sid)
                        .cloned()
                        .unwrap_or_default(),
                }
            })
            .collect();

        Ok(CrownDomain {
            domain,
            appointments,
            sanctum_shapes,
        })
    }

    /// Resolve the style for a role from a stylesheet.
    pub fn resolve_style<M: Material>(role: &CrownRole, stylesheet: &Stylesheet<M>) -> M {
        stylesheet.style_for(role)
    }

    /// Default content insets per role.
    pub fn default_insets(role: &CrownRole) -> BorderInsets {
        match role.name() {
            "sidebar" => BorderInsets::uniform(12.0),
            "controlBar" => BorderInsets::uniform(8.0),
            "overlay" => BorderInsets::uniform(8.0),
            _ => BorderInsets::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crown_jewels::materials::facet::FacetStyle;

    #[test]
    fn crown_sanctum_presets() {
        let sidebar = CrownSanctum::sidebar(None, ShapeDescriptor::default(), None);
        assert_eq!(sidebar.role, CrownRole::sidebar());
        assert_eq!(sidebar.id().as_str(), "sidebar");

        let toolbar = CrownSanctum::toolbar(None, ShapeDescriptor::default(), None);
        assert_eq!(toolbar.role, CrownRole::control_bar());

        let content = CrownSanctum::content(ShapeDescriptor::default(), None);
        assert_eq!(content.role, CrownRole::panel());

        let overlay = CrownSanctum::overlay(ShapeDescriptor::default(), None);
        assert_eq!(overlay.role, CrownRole::overlay());
    }

    #[test]
    fn default_insets_per_role() {
        let sidebar = CrownArbiter::default_insets(&CrownRole::sidebar());
        assert!((sidebar.top - 12.0).abs() < 1e-10);

        let control_bar = CrownArbiter::default_insets(&CrownRole::control_bar());
        assert!((control_bar.top - 8.0).abs() < 1e-10);

        let panel = CrownArbiter::default_insets(&CrownRole::panel());
        assert_eq!(panel, BorderInsets::ZERO);
    }

    #[test]
    fn resolve_style_from_stylesheet() {
        let sheet = Stylesheet::new(FacetStyle::regular());
        let style = CrownArbiter::resolve_style(&CrownRole::panel(), &sheet);
        assert!((style.frost - 0.5).abs() < 1e-10);
    }

    #[test]
    fn serde_crown_sanctum_roundtrip() {
        let cs = CrownSanctum::sidebar(
            Some(200.0),
            ShapeDescriptor::rounded_rect(12.0, 0.0),
            Some(BorderInsets::uniform(8.0)),
        );
        let json = serde_json::to_string(&cs).unwrap();
        let decoded: CrownSanctum = serde_json::from_str(&json).unwrap();
        assert_eq!(cs.role, decoded.role);
        assert_eq!(cs.shape, decoded.shape);
    }
}
