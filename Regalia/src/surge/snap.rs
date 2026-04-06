use super::Surge;

/// Instant jump animation (no transition).
#[derive(Debug, Clone, Copy)]
pub struct SnapSurge;

impl Surge for SnapSurge {
    fn value(&self, _t: f64) -> f64 {
        1.0
    }

    fn is_complete(&self, _t: f64, _velocity: f64) -> bool {
        true
    }

    fn duration(&self) -> f64 {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn always_complete() {
        let s = SnapSurge;
        assert_eq!(s.value(0.0), 1.0);
        assert!(s.is_complete(0.0, 0.0));
        assert_eq!(s.duration(), 0.0);
    }
}
