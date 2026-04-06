use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A light scatter point on an Iris (thin-film) surface.
/// Position is in unit coordinates (0-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrisDimple {
    pub id: Uuid,
    /// Horizontal position (0-1).
    pub x: f64,
    /// Vertical position (0-1).
    pub y: f64,
    /// Cosine bell radius (0.01-1.0).
    pub radius: f64,
    /// Dome height (-1.0 to 1.0).
    pub depth: f64,
    /// Shared edit group (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link_group: Option<Uuid>,
}

/// Maximum dimples sent to the GPU per shape.
pub const DIMPLE_MAX_COUNT: usize = 16;

impl Default for IrisDimple {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            x: 0.5,
            y: 0.5,
            radius: 0.4,
            depth: 0.5,
            link_group: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let d = IrisDimple::default();
        assert!((d.x - 0.5).abs() < 1e-10);
        assert!((d.y - 0.5).abs() < 1e-10);
        assert!((d.radius - 0.4).abs() < 1e-10);
        assert!((d.depth - 0.5).abs() < 1e-10);
        assert!(d.link_group.is_none());
    }

    #[test]
    fn serde_roundtrip() {
        let d = IrisDimple {
            id: Uuid::new_v4(),
            x: 0.3,
            y: 0.7,
            radius: 0.2,
            depth: -0.5,
            link_group: Some(Uuid::new_v4()),
        };
        let json = serde_json::to_string(&d).unwrap();
        let decoded: IrisDimple = serde_json::from_str(&json).unwrap();
        assert_eq!(d.id, decoded.id);
        assert!((d.x - decoded.x).abs() < 1e-10);
        assert_eq!(d.link_group, decoded.link_group);
    }

    #[test]
    fn max_count() {
        assert_eq!(DIMPLE_MAX_COUNT, 16);
    }
}
