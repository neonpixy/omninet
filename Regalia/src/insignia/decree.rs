use serde::{Deserialize, Serialize};

/// Placement order: a resolved position within a sanctum's local space.
///
/// Produced by Formations. Consumed by the Arbiter to create Appointments.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Decree {
    /// Frame in sanctum's local coordinate space.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Z-ordering within the sanctum (default 0).
    pub z_index: f64,
    /// Anchor point for transitions (0.5, 0.5 = center).
    pub anchor_x: f64,
    pub anchor_y: f64,
}

impl Decree {
    /// Create a decree with position and size, using default z-index and center anchor.
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
            z_index: 0.0,
            anchor_x: 0.5,
            anchor_y: 0.5,
        }
    }

    /// Set the z-ordering index for this decree.
    pub fn with_z_index(mut self, z: f64) -> Self {
        self.z_index = z;
        self
    }

    /// Set the anchor point (0.0-1.0 per axis, 0.5 = center).
    pub fn with_anchor(mut self, x: f64, y: f64) -> Self {
        self.anchor_x = x;
        self.anchor_y = y;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_decree() {
        let d = Decree::new(10.0, 20.0, 100.0, 50.0);
        assert_eq!(d.x, 10.0);
        assert_eq!(d.y, 20.0);
        assert_eq!(d.z_index, 0.0);
        assert_eq!(d.anchor_x, 0.5);
    }

    #[test]
    fn builder_methods() {
        let d = Decree::new(0.0, 0.0, 50.0, 50.0)
            .with_z_index(5.0)
            .with_anchor(0.0, 1.0);
        assert_eq!(d.z_index, 5.0);
        assert_eq!(d.anchor_x, 0.0);
        assert_eq!(d.anchor_y, 1.0);
    }
}
