use serde::{Deserialize, Serialize};

use crate::formation::FormationKind;
use crate::insignia::{Border, SanctumID, Seat};

/// A named layout region. The fundamental unit of Regalia's layout system.
///
/// Sanctums can be attached to an edge (via `border`) or float freely.
/// They define a `FormationKind` that determines how children are laid out.
/// They can nest via `subsanctums` (max 8 levels, enforced by the Arbiter).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sanctum {
    pub id: SanctumID,
    /// Edge attachment: Top, Bottom, Leading, Trailing, or None for free-form.
    pub border: Option<Border>,
    /// Fixed width (for Leading/Trailing) or height (for Top/Bottom).
    /// None = fill remaining space.
    pub fixed_extent: Option<f64>,
    /// Coordinate origin (default Center).
    pub seat: Seat,
    /// Depth layer for z-ordering (default 0).
    pub z_layer: i32,
    /// Whether to clip children to this sanctum's bounds (default true).
    pub clips: bool,
    /// Layout algorithm (default OpenCourt).
    pub formation_kind: FormationKind,
    /// Nested sanctums (max 8 levels).
    pub subsanctums: Vec<Sanctum>,
}

impl Sanctum {
    /// Sidebar preset: leading border, default width 240, Column layout, z=100.
    pub fn sidebar(width: Option<f64>, formation: Option<FormationKind>) -> Self {
        Self {
            id: SanctumID::sidebar(),
            border: Some(Border::Leading),
            fixed_extent: Some(width.unwrap_or(240.0)),
            seat: Seat::Center,
            z_layer: 100,
            clips: true,
            formation_kind: formation.unwrap_or(FormationKind::Column {
                spacing: 8.0,
                alignment: crate::formation::ColumnAlignment::Leading,
                justification: crate::formation::ColumnJustification::Top,
            }),
            subsanctums: vec![],
        }
    }

    /// Toolbar preset: top border, default height 44, Rank layout, z=100.
    pub fn toolbar(height: Option<f64>, formation: Option<FormationKind>) -> Self {
        Self {
            id: SanctumID::toolbar(),
            border: Some(Border::Top),
            fixed_extent: Some(height.unwrap_or(44.0)),
            seat: Seat::Center,
            z_layer: 100,
            clips: true,
            formation_kind: formation.unwrap_or(FormationKind::Rank {
                spacing: 8.0,
                alignment: crate::formation::RankAlignment::Center,
                justification: crate::formation::RankJustification::Center,
            }),
            subsanctums: vec![],
        }
    }

    /// Content preset: free-form, OpenCourt layout, z=0.
    pub fn content(formation: Option<FormationKind>) -> Self {
        Self {
            id: SanctumID::content(),
            border: None,
            fixed_extent: None,
            seat: Seat::Center,
            z_layer: 0,
            clips: true,
            formation_kind: formation.unwrap_or_default(),
            subsanctums: vec![],
        }
    }

    /// Overlay preset: free-form, OpenCourt, z=200, no clipping.
    pub fn overlay(formation: Option<FormationKind>) -> Self {
        Self {
            id: SanctumID::overlay(),
            border: None,
            fixed_extent: None,
            seat: Seat::Center,
            z_layer: 200,
            clips: false,
            formation_kind: formation.unwrap_or_default(),
            subsanctums: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sidebar_defaults() {
        let s = Sanctum::sidebar(None, None);
        assert_eq!(s.id, SanctumID::sidebar());
        assert_eq!(s.border, Some(Border::Leading));
        assert_eq!(s.fixed_extent, Some(240.0));
        assert_eq!(s.z_layer, 100);
    }

    #[test]
    fn toolbar_custom_height() {
        let s = Sanctum::toolbar(Some(60.0), None);
        assert_eq!(s.fixed_extent, Some(60.0));
        assert_eq!(s.border, Some(Border::Top));
    }

    #[test]
    fn content_defaults() {
        let s = Sanctum::content(None);
        assert!(s.border.is_none());
        assert!(s.fixed_extent.is_none());
        assert_eq!(s.z_layer, 0);
        assert!(s.clips);
    }

    #[test]
    fn overlay_no_clip() {
        let s = Sanctum::overlay(None);
        assert!(!s.clips);
        assert_eq!(s.z_layer, 200);
    }

    #[test]
    fn subsanctums() {
        let s = Sanctum {
            id: SanctumID::new("parent"),
            border: None,
            fixed_extent: None,
            seat: Seat::Center,
            z_layer: 0,
            clips: true,
            formation_kind: FormationKind::OpenCourt,
            subsanctums: vec![
                Sanctum::toolbar(None, None),
                Sanctum::content(None),
            ],
        };
        assert_eq!(s.subsanctums.len(), 2);
    }

    #[test]
    fn serde_roundtrip() {
        let s = Sanctum::sidebar(Some(300.0), None);
        let json = serde_json::to_string(&s).unwrap();
        let decoded: Sanctum = serde_json::from_str(&json).unwrap();
        assert_eq!(s, decoded);
    }
}
