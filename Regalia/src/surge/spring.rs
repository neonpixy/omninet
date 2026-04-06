use super::Surge;

/// Underdamped spring animation curve.
#[derive(Debug, Clone, Copy)]
pub struct SpringSurge {
    pub damping_ratio: f64,
    pub frequency: f64,
}

impl SpringSurge {
    /// Create a spring with the given damping ratio (0.01-1.0) and frequency (Hz).
    pub fn new(damping_ratio: f64, frequency: f64) -> Self {
        Self {
            damping_ratio: damping_ratio.clamp(0.01, 1.0),
            frequency: frequency.max(0.1),
        }
    }
}

impl Default for SpringSurge {
    fn default() -> Self {
        Self::new(0.7, 3.5)
    }
}

impl Surge for SpringSurge {
    fn value(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return 0.0;
        }
        let omega = self.frequency * std::f64::consts::TAU;
        let damped_omega = omega * (1.0 - self.damping_ratio * self.damping_ratio).max(0.0).sqrt();
        let decay = (-self.damping_ratio * omega * t).exp();

        if damped_omega.abs() < f64::EPSILON {
            // Critically damped
            1.0 - decay * (1.0 + omega * t)
        } else {
            1.0 - decay * ((self.damping_ratio * omega / damped_omega) * (damped_omega * t).sin()
                + (damped_omega * t).cos())
        }
    }

    fn is_complete(&self, t: f64, velocity: f64) -> bool {
        if self.damping_ratio >= 1.0 {
            // Critically/over-damped: check convergence
            (self.value(t) - 1.0).abs() < 0.001
        } else {
            // Underdamped: check both position convergence and low velocity
            (self.value(t) - 1.0).abs() < 0.001 && velocity.abs() < 0.01
        }
    }

    fn duration(&self) -> f64 {
        // Estimated settle time
        7.0 / (self.damping_ratio * self.frequency).max(0.01)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_zero() {
        let s = SpringSurge::default();
        assert_eq!(s.value(0.0), 0.0);
    }

    #[test]
    fn converges_to_one() {
        let s = SpringSurge::default();
        let v = s.value(3.0);
        assert!((v - 1.0).abs() < 0.01, "value at t=3: {v}");
    }

    #[test]
    fn overshoots_when_underdamped() {
        let s = SpringSurge::new(0.3, 3.5);
        // Find max value in first 2 seconds
        let max = (0..200)
            .map(|i| s.value(i as f64 * 0.01))
            .fold(0.0_f64, f64::max);
        assert!(max > 1.0, "should overshoot, max was {max}");
    }

    #[test]
    fn damping_clamped() {
        let s = SpringSurge::new(0.0, 3.5);
        assert_eq!(s.damping_ratio, 0.01);
        let s = SpringSurge::new(2.0, 3.5);
        assert_eq!(s.damping_ratio, 1.0);
    }

    #[test]
    fn frequency_clamped() {
        let s = SpringSurge::new(0.7, -1.0);
        assert_eq!(s.frequency, 0.1);
    }
}
