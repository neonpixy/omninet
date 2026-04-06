use super::{EaseSurge, SnapSurge, SpringSurge, Surge};

/// A layout transition: wraps a Surge animation curve.
pub struct Shift {
    surge: Box<dyn Surge>,
}

impl Shift {
    /// Wrap any Surge animation curve into a boxed Shift.
    pub fn new(surge: impl Surge + 'static) -> Self {
        Self {
            surge: Box::new(surge),
        }
    }

    /// Progress at elapsed time t seconds.
    pub fn value(&self, t: f64) -> f64 {
        self.surge.value(t)
    }

    /// Whether the animation is finished.
    pub fn is_complete(&self, t: f64, velocity: f64) -> bool {
        self.surge.is_complete(t, velocity)
    }

    /// Nominal duration in seconds.
    pub fn duration(&self) -> f64 {
        self.surge.duration()
    }

    /// Instant transition preset.
    pub fn snap() -> Self {
        Self::new(SnapSurge)
    }

    /// Ease-in-out preset for smooth transitions.
    pub fn smooth() -> Self {
        Self::new(EaseSurge::default())
    }

    /// Spring preset with natural overshoot.
    pub fn bouncy() -> Self {
        Self::new(SpringSurge::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_preset() {
        let s = Shift::snap();
        assert_eq!(s.value(0.0), 1.0);
        assert!(s.is_complete(0.0, 0.0));
    }

    #[test]
    fn smooth_preset() {
        let s = Shift::smooth();
        assert_eq!(s.value(0.0), 0.0);
        assert_eq!(s.value(s.duration()), 1.0);
    }

    #[test]
    fn bouncy_preset() {
        let s = Shift::bouncy();
        assert_eq!(s.value(0.0), 0.0);
        // Spring converges
        let v = s.value(3.0);
        assert!((v - 1.0).abs() < 0.01);
    }
}
