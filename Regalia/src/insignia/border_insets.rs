use serde::{Deserialize, Serialize};

/// Inset distances (padding/margin) for all four edges.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BorderInsets {
    pub top: f64,
    pub leading: f64,
    pub bottom: f64,
    pub trailing: f64,
}

impl BorderInsets {
    /// Create insets with individual values for each edge.
    pub fn new(top: f64, leading: f64, bottom: f64, trailing: f64) -> Self {
        Self {
            top,
            leading,
            bottom,
            trailing,
        }
    }

    /// Create uniform insets with the same value on all four edges.
    pub fn uniform(value: f64) -> Self {
        Self::new(value, value, value, value)
    }

    /// Inset a rectangle by these distances.
    pub fn inset(&self, x: f64, y: f64, width: f64, height: f64) -> (f64, f64, f64, f64) {
        (
            x + self.leading,
            y + self.top,
            (width - self.leading - self.trailing).max(0.0),
            (height - self.top - self.bottom).max(0.0),
        )
    }

    pub const ZERO: Self = Self {
        top: 0.0,
        leading: 0.0,
        bottom: 0.0,
        trailing: 0.0,
    };
}

impl Default for BorderInsets {
    fn default() -> Self {
        Self::ZERO
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_insets() {
        let i = BorderInsets::uniform(10.0);
        assert_eq!(i.top, 10.0);
        assert_eq!(i.leading, 10.0);
        assert_eq!(i.bottom, 10.0);
        assert_eq!(i.trailing, 10.0);
    }

    #[test]
    fn inset_rect() {
        let i = BorderInsets::new(10.0, 20.0, 10.0, 20.0);
        let (x, y, w, h) = i.inset(0.0, 0.0, 100.0, 80.0);
        assert_eq!(x, 20.0);
        assert_eq!(y, 10.0);
        assert_eq!(w, 60.0);
        assert_eq!(h, 60.0);
    }

    #[test]
    fn inset_clamps_to_zero() {
        let i = BorderInsets::uniform(100.0);
        let (_, _, w, h) = i.inset(0.0, 0.0, 50.0, 50.0);
        assert_eq!(w, 0.0);
        assert_eq!(h, 0.0);
    }

    #[test]
    fn serde_roundtrip() {
        let i = BorderInsets::new(5.0, 10.0, 15.0, 20.0);
        let json = serde_json::to_string(&i).unwrap();
        let decoded: BorderInsets = serde_json::from_str(&json).unwrap();
        assert_eq!(i, decoded);
    }
}
