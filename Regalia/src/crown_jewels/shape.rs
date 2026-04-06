use serde::{Deserialize, Serialize};

/// Per-corner radii for rounded rectangle shapes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CornerRadii {
    pub top_left: f64,
    pub top_right: f64,
    pub bottom_right: f64,
    pub bottom_left: f64,
}

impl CornerRadii {
    /// Create corner radii with the same radius on all four corners.
    pub fn uniform(radius: f64) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    pub const ZERO: Self = Self {
        top_left: 0.0,
        top_right: 0.0,
        bottom_right: 0.0,
        bottom_left: 0.0,
    };

    /// Shader layout: (TL, TR, BR, BL) as [f32; 4].
    pub fn as_f32_array(&self) -> [f32; 4] {
        [
            self.top_left as f32,
            self.top_right as f32,
            self.bottom_right as f32,
            self.bottom_left as f32,
        ]
    }
}

impl Default for CornerRadii {
    fn default() -> Self {
        Self::ZERO
    }
}

/// SDF shape primitive definitions. These describe the shape to render —
/// the platform layer (Metal, Vulkan, WebGPU) interprets them via its
/// GPU shaders.
///
/// No `Custom` variant — custom SDF textures are platform-specific
/// (e.g., `MTLTexture` on Apple).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ShapeDescriptor {
    /// Rectangle with per-corner radii and optional squircle smoothing.
    RoundedRect {
        corner_radii: CornerRadii,
        /// Superellipse smoothing factor (0 = normal, >0 = squircle).
        smoothing: f64,
    },
    /// Stadium shape (rectangle with fully rounded short ends).
    Capsule,
    /// Perfect circle (equal width and height).
    Circle,
    /// Ellipse (stretched circle).
    Ellipse,
    /// Regular polygon with N sides and optional rounded corners.
    Polygon {
        sides: u32,
        corner_radius: f64,
    },
    /// Star shape with N points and configurable inner/outer radii.
    Star {
        points: u32,
        inner_radius: f64,
        outer_radius: f64,
        corner_radius: f64,
        inner_corner_radius: f64,
    },
}

impl Default for ShapeDescriptor {
    fn default() -> Self {
        Self::RoundedRect {
            corner_radii: CornerRadii::ZERO,
            smoothing: 0.0,
        }
    }
}

impl ShapeDescriptor {
    /// Convenience: rounded rectangle with uniform corner radius.
    pub fn rounded_rect(corner_radius: f64, smoothing: f64) -> Self {
        Self::RoundedRect {
            corner_radii: CornerRadii::uniform(corner_radius),
            smoothing,
        }
    }

    /// Metal shader shape type index.
    /// 0=roundedRect/capsule, 1=ellipse, 2=polygon, 3=star.
    pub fn metal_shape_type(&self) -> u32 {
        match self {
            Self::RoundedRect { .. } | Self::Capsule => 0,
            Self::Circle | Self::Ellipse => 1,
            Self::Polygon { .. } => 2,
            Self::Star { .. } => 3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corner_radii_uniform() {
        let r = CornerRadii::uniform(12.0);
        assert!((r.top_left - 12.0).abs() < 1e-10);
        assert!((r.top_right - 12.0).abs() < 1e-10);
        assert!((r.bottom_right - 12.0).abs() < 1e-10);
        assert!((r.bottom_left - 12.0).abs() < 1e-10);
    }

    #[test]
    fn corner_radii_zero() {
        let r = CornerRadii::ZERO;
        assert!(r.top_left.abs() < 1e-10);
    }

    #[test]
    fn corner_radii_f32_array() {
        let r = CornerRadii {
            top_left: 4.0,
            top_right: 8.0,
            bottom_right: 12.0,
            bottom_left: 16.0,
        };
        let arr = r.as_f32_array();
        assert_eq!(arr, [4.0f32, 8.0, 12.0, 16.0]);
    }

    #[test]
    fn shape_type_mapping() {
        assert_eq!(ShapeDescriptor::rounded_rect(8.0, 0.0).metal_shape_type(), 0);
        assert_eq!(ShapeDescriptor::Capsule.metal_shape_type(), 0);
        assert_eq!(ShapeDescriptor::Circle.metal_shape_type(), 1);
        assert_eq!(ShapeDescriptor::Ellipse.metal_shape_type(), 1);
        assert_eq!(
            ShapeDescriptor::Polygon {
                sides: 6,
                corner_radius: 0.0
            }
            .metal_shape_type(),
            2
        );
        assert_eq!(
            ShapeDescriptor::Star {
                points: 5,
                inner_radius: 0.5,
                outer_radius: 1.0,
                corner_radius: 0.0,
                inner_corner_radius: 0.0
            }
            .metal_shape_type(),
            3
        );
    }

    #[test]
    fn rounded_rect_convenience() {
        let s = ShapeDescriptor::rounded_rect(12.0, 0.6);
        match s {
            ShapeDescriptor::RoundedRect {
                corner_radii,
                smoothing,
            } => {
                assert!((corner_radii.top_left - 12.0).abs() < 1e-10);
                assert!((smoothing - 0.6).abs() < 1e-10);
            }
            _ => panic!("expected RoundedRect"),
        }
    }

    #[test]
    fn default_shape() {
        let s = ShapeDescriptor::default();
        assert_eq!(s.metal_shape_type(), 0);
    }

    #[test]
    fn serde_roundtrip_polygon() {
        let s = ShapeDescriptor::Polygon {
            sides: 6,
            corner_radius: 2.0,
        };
        let json = serde_json::to_string(&s).unwrap();
        let decoded: ShapeDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(s, decoded);
    }

    #[test]
    fn serde_roundtrip_star() {
        let s = ShapeDescriptor::Star {
            points: 5,
            inner_radius: 0.4,
            outer_radius: 1.0,
            corner_radius: 0.05,
            inner_corner_radius: 0.02,
        };
        let json = serde_json::to_string(&s).unwrap();
        let decoded: ShapeDescriptor = serde_json::from_str(&json).unwrap();
        assert_eq!(s, decoded);
    }
}
