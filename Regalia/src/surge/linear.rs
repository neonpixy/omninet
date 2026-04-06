use super::Surge;

/// Linear interpolation animation curve.
#[derive(Debug, Clone, Copy)]
pub struct LinearSurge {
    dur: f64,
}

impl LinearSurge {
    /// Create a linear curve with the given duration in seconds.
    pub fn new(duration: f64) -> Self {
        Self {
            dur: duration.max(0.0),
        }
    }
}

impl Default for LinearSurge {
    fn default() -> Self {
        Self::new(0.3)
    }
}

impl Surge for LinearSurge {
    fn value(&self, t: f64) -> f64 {
        if self.dur <= 0.0 || t >= self.dur {
            return 1.0;
        }
        if t <= 0.0 {
            return 0.0;
        }
        t / self.dur
    }

    fn duration(&self) -> f64 {
        self.dur
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_progression() {
        let l = LinearSurge::new(1.0);
        assert_eq!(l.value(0.0), 0.0);
        assert!((l.value(0.5) - 0.5).abs() < f64::EPSILON);
        assert_eq!(l.value(1.0), 1.0);
    }

    #[test]
    fn zero_duration() {
        let l = LinearSurge::new(0.0);
        assert_eq!(l.value(0.0), 1.0);
    }
}
