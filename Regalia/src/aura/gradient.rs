use serde::{Deserialize, Serialize};

use super::Ember;

/// A single color stop in a gradient.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GradientStop {
    pub color: Ember,
    /// Position along the gradient, 0.0 to 1.0.
    pub position: f64,
}

impl GradientStop {
    /// Create a gradient stop at a given position (0.0-1.0, clamped).
    pub fn new(color: Ember, position: f64) -> Self {
        Self {
            color,
            position: position.clamp(0.0, 1.0),
        }
    }

    /// A stop at position 0.0.
    pub fn start(color: Ember) -> Self {
        Self::new(color, 0.0)
    }

    /// A stop at position 1.0.
    pub fn end(color: Ember) -> Self {
        Self::new(color, 1.0)
    }
}

/// A gradient with multiple color stops.
///
/// Three variants — linear (by angle), radial (from center outward),
/// and angular (sweep around a center point).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Gradient {
    /// A linear gradient defined by an angle in degrees and color stops.
    Linear {
        /// Angle in degrees (0 = left-to-right, 90 = top-to-bottom).
        angle: f64,
        stops: Vec<GradientStop>,
    },
    /// A radial gradient radiating from a center point outward.
    Radial {
        /// Center X position (0.0–1.0, relative to bounds).
        center_x: f64,
        /// Center Y position (0.0–1.0, relative to bounds).
        center_y: f64,
        /// Radius (0.0–1.0, relative to bounds diagonal).
        radius: f64,
        stops: Vec<GradientStop>,
    },
    /// An angular (conic/sweep) gradient around a center point.
    Angular {
        /// Center X position (0.0–1.0, relative to bounds).
        center_x: f64,
        /// Center Y position (0.0–1.0, relative to bounds).
        center_y: f64,
        stops: Vec<GradientStop>,
    },
}

impl Gradient {
    /// Access the gradient's stops.
    pub fn stops(&self) -> &[GradientStop] {
        match self {
            Gradient::Linear { stops, .. } => stops,
            Gradient::Radial { stops, .. } => stops,
            Gradient::Angular { stops, .. } => stops,
        }
    }

    /// Interpolate the color at a given position (0.0–1.0) by linearly blending
    /// between the surrounding stops.
    pub fn color_at(&self, position: f64) -> Ember {
        let t = position.clamp(0.0, 1.0);
        let stops = self.stops();

        if stops.is_empty() {
            return Ember::CLEAR;
        }
        if stops.len() == 1 {
            return stops[0].color;
        }

        // Ensure we work with sorted stops.
        let mut sorted: Vec<&GradientStop> = stops.iter().collect();
        sorted.sort_by(|a, b| {
            a.position
                .partial_cmp(&b.position)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Before first stop or after last stop — clamp.
        if t <= sorted[0].position {
            return sorted[0].color;
        }
        if t >= sorted[sorted.len() - 1].position {
            return sorted[sorted.len() - 1].color;
        }

        // Find the surrounding stops.
        let mut lower = sorted[0];
        let mut upper = sorted[sorted.len() - 1];

        for i in 0..sorted.len() - 1 {
            if sorted[i].position <= t && sorted[i + 1].position >= t {
                lower = sorted[i];
                upper = sorted[i + 1];
                break;
            }
        }

        let range = upper.position - lower.position;
        if range < f64::EPSILON {
            return lower.color;
        }
        let local_t = (t - lower.position) / range;

        // Linear interpolation of RGBA channels.
        Ember::new(
            lower.color.red + (upper.color.red - lower.color.red) * local_t,
            lower.color.green + (upper.color.green - lower.color.green) * local_t,
            lower.color.blue + (upper.color.blue - lower.color.blue) * local_t,
            lower.color.alpha + (upper.color.alpha - lower.color.alpha) * local_t,
        )
    }

    /// Return a new gradient with stop positions flipped (1.0 - position) and reversed.
    pub fn reversed(&self) -> Gradient {
        fn reverse_stops(stops: &[GradientStop]) -> Vec<GradientStop> {
            stops
                .iter()
                .rev()
                .map(|s| GradientStop::new(s.color, 1.0 - s.position))
                .collect()
        }

        match self {
            Gradient::Linear { angle, stops } => Gradient::Linear {
                angle: *angle,
                stops: reverse_stops(stops),
            },
            Gradient::Radial {
                center_x,
                center_y,
                radius,
                stops,
            } => Gradient::Radial {
                center_x: *center_x,
                center_y: *center_y,
                radius: *radius,
                stops: reverse_stops(stops),
            },
            Gradient::Angular {
                center_x,
                center_y,
                stops,
            } => Gradient::Angular {
                center_x: *center_x,
                center_y: *center_y,
                stops: reverse_stops(stops),
            },
        }
    }

    // ── Preset Gradients ──

    /// A sunset gradient (warm orange to hot pink), angled diagonally.
    pub fn sunset() -> Self {
        Gradient::Linear {
            angle: 135.0,
            stops: vec![
                GradientStop::new(
                    Ember::from_hex("#FF512F").expect("hardcoded preset hex"),
                    0.0,
                ),
                GradientStop::new(
                    Ember::from_hex("#DD2476").expect("hardcoded preset hex"),
                    1.0,
                ),
            ],
        }
    }

    /// An ocean gradient (teal to light blue), vertical.
    pub fn ocean() -> Self {
        Gradient::Linear {
            angle: 90.0,
            stops: vec![
                GradientStop::new(
                    Ember::from_hex("#2193B0").expect("hardcoded preset hex"),
                    0.0,
                ),
                GradientStop::new(
                    Ember::from_hex("#6DD5ED").expect("hardcoded preset hex"),
                    1.0,
                ),
            ],
        }
    }

    /// A forest gradient (deep teal to soft green), angled diagonally.
    pub fn forest() -> Self {
        Gradient::Linear {
            angle: 135.0,
            stops: vec![
                GradientStop::new(
                    Ember::from_hex("#134E5E").expect("hardcoded preset hex"),
                    0.0,
                ),
                GradientStop::new(
                    Ember::from_hex("#71B280").expect("hardcoded preset hex"),
                    1.0,
                ),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_stop_clamps_position() {
        let s = GradientStop::new(Ember::BLACK, 1.5);
        assert_eq!(s.position, 1.0);
        let s = GradientStop::new(Ember::BLACK, -0.5);
        assert_eq!(s.position, 0.0);
    }

    #[test]
    fn gradient_stop_convenience() {
        let start = GradientStop::start(Ember::WHITE);
        assert_eq!(start.position, 0.0);
        let end = GradientStop::end(Ember::BLACK);
        assert_eq!(end.position, 1.0);
    }

    #[test]
    fn stops_accessor() {
        let g = Gradient::sunset();
        assert_eq!(g.stops().len(), 2);
    }

    #[test]
    fn color_at_endpoints() {
        let g = Gradient::Linear {
            angle: 0.0,
            stops: vec![
                GradientStop::start(Ember::BLACK),
                GradientStop::end(Ember::WHITE),
            ],
        };
        let start_color = g.color_at(0.0);
        assert!((start_color.red - 0.0).abs() < 0.01);
        let end_color = g.color_at(1.0);
        assert!((end_color.red - 1.0).abs() < 0.01);
    }

    #[test]
    fn color_at_midpoint() {
        let g = Gradient::Linear {
            angle: 0.0,
            stops: vec![
                GradientStop::start(Ember::BLACK),
                GradientStop::end(Ember::WHITE),
            ],
        };
        let mid = g.color_at(0.5);
        assert!((mid.red - 0.5).abs() < 0.01);
        assert!((mid.green - 0.5).abs() < 0.01);
        assert!((mid.blue - 0.5).abs() < 0.01);
    }

    #[test]
    fn color_at_clamps_out_of_range() {
        let g = Gradient::Linear {
            angle: 0.0,
            stops: vec![
                GradientStop::start(Ember::BLACK),
                GradientStop::end(Ember::WHITE),
            ],
        };
        let below = g.color_at(-1.0);
        assert!((below.red - 0.0).abs() < 0.01);
        let above = g.color_at(2.0);
        assert!((above.red - 1.0).abs() < 0.01);
    }

    #[test]
    fn color_at_empty_stops() {
        let g = Gradient::Linear {
            angle: 0.0,
            stops: vec![],
        };
        let c = g.color_at(0.5);
        assert_eq!(c, Ember::CLEAR);
    }

    #[test]
    fn color_at_single_stop() {
        let g = Gradient::Linear {
            angle: 0.0,
            stops: vec![GradientStop::new(Ember::WHITE, 0.5)],
        };
        let c = g.color_at(0.0);
        assert_eq!(c, Ember::WHITE);
    }

    #[test]
    fn color_at_multi_stop() {
        let g = Gradient::Linear {
            angle: 0.0,
            stops: vec![
                GradientStop::new(Ember::BLACK, 0.0),
                GradientStop::new(Ember::rgb(1.0, 0.0, 0.0), 0.5),
                GradientStop::new(Ember::WHITE, 1.0),
            ],
        };
        // At 0.25, interpolate between black and red
        let c = g.color_at(0.25);
        assert!((c.red - 0.5).abs() < 0.01);
        assert!((c.green - 0.0).abs() < 0.01);
    }

    #[test]
    fn reversed_linear() {
        let g = Gradient::Linear {
            angle: 90.0,
            stops: vec![
                GradientStop::start(Ember::BLACK),
                GradientStop::end(Ember::WHITE),
            ],
        };
        let r = g.reversed();
        let stops = r.stops();
        assert_eq!(stops.len(), 2);
        // First stop should now be white at 0.0
        assert_eq!(stops[0].color, Ember::WHITE);
        assert!((stops[0].position - 0.0).abs() < 0.01);
        // Second stop should be black at 1.0
        assert_eq!(stops[1].color, Ember::BLACK);
        assert!((stops[1].position - 1.0).abs() < 0.01);
    }

    #[test]
    fn reversed_radial() {
        let g = Gradient::Radial {
            center_x: 0.5,
            center_y: 0.5,
            radius: 0.5,
            stops: vec![
                GradientStop::start(Ember::BLACK),
                GradientStop::end(Ember::WHITE),
            ],
        };
        let r = g.reversed();
        assert_eq!(r.stops()[0].color, Ember::WHITE);
    }

    #[test]
    fn reversed_angular() {
        let g = Gradient::Angular {
            center_x: 0.5,
            center_y: 0.5,
            stops: vec![
                GradientStop::start(Ember::BLACK),
                GradientStop::end(Ember::WHITE),
            ],
        };
        let r = g.reversed();
        assert_eq!(r.stops()[0].color, Ember::WHITE);
    }

    #[test]
    fn preset_sunset() {
        let g = Gradient::sunset();
        assert_eq!(g.stops().len(), 2);
        if let Gradient::Linear { angle, .. } = &g {
            assert!((angle - 135.0).abs() < 0.01);
        } else {
            panic!("sunset should be linear");
        }
    }

    #[test]
    fn preset_ocean() {
        let g = Gradient::ocean();
        assert_eq!(g.stops().len(), 2);
        if let Gradient::Linear { angle, .. } = &g {
            assert!((angle - 90.0).abs() < 0.01);
        } else {
            panic!("ocean should be linear");
        }
    }

    #[test]
    fn preset_forest() {
        let g = Gradient::forest();
        assert_eq!(g.stops().len(), 2);
        if let Gradient::Linear { angle, .. } = &g {
            assert!((angle - 135.0).abs() < 0.01);
        } else {
            panic!("forest should be linear");
        }
    }

    #[test]
    fn serde_roundtrip_linear() {
        let g = Gradient::sunset();
        let json = serde_json::to_string(&g).unwrap();
        let decoded: Gradient = serde_json::from_str(&json).unwrap();
        assert_eq!(g, decoded);
    }

    #[test]
    fn serde_roundtrip_radial() {
        let g = Gradient::Radial {
            center_x: 0.5,
            center_y: 0.5,
            radius: 0.8,
            stops: vec![
                GradientStop::start(Ember::BLACK),
                GradientStop::end(Ember::WHITE),
            ],
        };
        let json = serde_json::to_string(&g).unwrap();
        let decoded: Gradient = serde_json::from_str(&json).unwrap();
        assert_eq!(g, decoded);
    }

    #[test]
    fn serde_roundtrip_angular() {
        let g = Gradient::Angular {
            center_x: 0.3,
            center_y: 0.7,
            stops: vec![
                GradientStop::start(Ember::WHITE),
                GradientStop::end(Ember::BLACK),
            ],
        };
        let json = serde_json::to_string(&g).unwrap();
        let decoded: Gradient = serde_json::from_str(&json).unwrap();
        assert_eq!(g, decoded);
    }

    #[test]
    fn radial_stops_accessor() {
        let g = Gradient::Radial {
            center_x: 0.5,
            center_y: 0.5,
            radius: 0.5,
            stops: vec![
                GradientStop::start(Ember::BLACK),
                GradientStop::end(Ember::WHITE),
            ],
        };
        assert_eq!(g.stops().len(), 2);
    }

    #[test]
    fn angular_stops_accessor() {
        let g = Gradient::Angular {
            center_x: 0.5,
            center_y: 0.5,
            stops: vec![
                GradientStop::start(Ember::BLACK),
                GradientStop::end(Ember::WHITE),
            ],
        };
        assert_eq!(g.stops().len(), 2);
    }
}
