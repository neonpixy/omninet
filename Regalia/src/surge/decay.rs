use super::Surge;

/// Exponential decay animation curve.
#[derive(Debug, Clone, Copy)]
pub struct DecaySurge {
    pub decay_rate: f64,
}

impl DecaySurge {
    /// Create a decay curve with the given rate (higher = faster falloff).
    pub fn new(decay_rate: f64) -> Self {
        Self {
            decay_rate: decay_rate.max(0.1),
        }
    }
}

impl Default for DecaySurge {
    fn default() -> Self {
        Self::new(5.0)
    }
}

impl Surge for DecaySurge {
    fn value(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return 0.0;
        }
        1.0 - (-self.decay_rate * t).exp()
    }

    fn duration(&self) -> f64 {
        5.0 / self.decay_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_zero() {
        let d = DecaySurge::default();
        assert_eq!(d.value(0.0), 0.0);
    }

    #[test]
    fn approaches_one() {
        let d = DecaySurge::default();
        let v = d.value(3.0);
        assert!(v > 0.99, "value at t=3: {v}");
    }

    #[test]
    fn decay_rate_clamped() {
        let d = DecaySurge::new(-1.0);
        assert_eq!(d.decay_rate, 0.1);
    }
}
