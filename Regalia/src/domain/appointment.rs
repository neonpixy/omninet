use serde::{Deserialize, Serialize};

use crate::insignia::SanctumID;

/// A frame: position + size.
pub type Frame = (f64, f64, f64, f64);

/// A resolved layout node: position + size in root coordinate space.
///
/// Produced by the Arbiter. Consumed by rendering systems.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Appointment {
    /// Stable identity (matches Clansman ID).
    pub id: String,
    /// Position and size in root space.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Content frame (inset by sanctum content insets).
    pub content_x: f64,
    pub content_y: f64,
    pub content_width: f64,
    pub content_height: f64,
    /// Which sanctum owns this appointment.
    pub sanctum_id: SanctumID,
    /// Composite z-order: sanctum.z_layer * 1000 + decree.z_index.
    pub composite_z_order: f64,
}

impl Appointment {
    /// Create a new appointment from a frame, sanctum, and z-ordering values.
    pub fn new(
        id: impl Into<String>,
        frame: Frame,
        sanctum_id: SanctumID,
        z_layer: i32,
        z_index: f64,
    ) -> Self {
        let (x, y, width, height) = frame;
        Self {
            id: id.into(),
            x,
            y,
            width,
            height,
            content_x: x,
            content_y: y,
            content_width: width,
            content_height: height,
            sanctum_id,
            composite_z_order: z_layer as f64 * 1000.0 + z_index,
        }
    }

    /// Override the content frame (inset region for rendering child content).
    pub fn with_content_frame(mut self, frame: Frame) -> Self {
        self.content_x = frame.0;
        self.content_y = frame.1;
        self.content_width = frame.2;
        self.content_height = frame.3;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn composite_z_order() {
        let a = Appointment::new(
            "test",
            (0.0, 0.0, 100.0, 50.0),
            SanctumID::content(),
            2,
            3.0,
        );
        assert_eq!(a.composite_z_order, 2003.0);
    }

    #[test]
    fn content_frame_default_matches_frame() {
        let a = Appointment::new(
            "test",
            (10.0, 20.0, 100.0, 50.0),
            SanctumID::content(),
            0,
            0.0,
        );
        assert_eq!(a.content_x, a.x);
        assert_eq!(a.content_y, a.y);
        assert_eq!(a.content_width, a.width);
        assert_eq!(a.content_height, a.height);
    }

    #[test]
    fn with_content_frame() {
        let a = Appointment::new(
            "test",
            (0.0, 0.0, 100.0, 50.0),
            SanctumID::content(),
            0,
            0.0,
        )
        .with_content_frame((10.0, 10.0, 80.0, 30.0));
        assert_eq!(a.content_x, 10.0);
        assert_eq!(a.content_width, 80.0);
    }
}
