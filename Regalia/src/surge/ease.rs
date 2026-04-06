use super::Surge;

/// Cubic ease-in-out animation curve.
#[derive(Debug, Clone, Copy)]
pub struct EaseSurge {
    dur: f64,
}

impl EaseSurge {
    /// Create an ease curve with the given duration in seconds.
    pub fn new(duration: f64) -> Self {
        Self {
            dur: duration.max(0.0),
        }
    }
}

impl Default for EaseSurge {
    fn default() -> Self {
        Self::new(0.35)
    }
}

impl Surge for EaseSurge {
    fn value(&self, t: f64) -> f64 {
        if self.dur <= 0.0 || t >= self.dur {
            return 1.0;
        }
        if t <= 0.0 {
            return 0.0;
        }
        let p = t / self.dur;
        // Cubic ease-in-out
        if p < 0.5 {
            4.0 * p * p * p
        } else {
            1.0 - (-2.0 * p + 2.0).powi(3) / 2.0
        }
    }

    fn duration(&self) -> f64 {
        self.dur
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_zero_ends_at_one() {
        let e = EaseSurge::default();
        assert_eq!(e.value(0.0), 0.0);
        assert_eq!(e.value(0.35), 1.0);
    }

    #[test]
    fn midpoint_is_half() {
        let e = EaseSurge::new(1.0);
        let v = e.value(0.5);
        assert!((v - 0.5).abs() < 0.01, "midpoint: {v}");
    }

    #[test]
    fn monotonic() {
        let e = EaseSurge::new(1.0);
        let mut prev = 0.0;
        for i in 1..=100 {
            let v = e.value(i as f64 * 0.01);
            assert!(v >= prev, "not monotonic at step {i}");
            prev = v;
        }
    }
}
