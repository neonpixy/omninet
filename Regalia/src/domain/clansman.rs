/// A placeable child in the layout system. Provides size hints for formations.
pub trait Clansman: Send + Sync {
    /// Preferred size, or None if flexible.
    fn intrinsic_size(&self) -> Option<(f64, f64)> {
        None
    }

    /// Minimum size (default 40x40).
    fn min_size(&self) -> (f64, f64) {
        (40.0, 40.0)
    }

    /// Maximum size (default unbounded).
    fn max_size(&self) -> (f64, f64) {
        (f64::INFINITY, f64::INFINITY)
    }

    /// Layout priority for space allocation (higher = first dibs).
    fn layout_priority(&self) -> f64 {
        0.0
    }

    /// Stable identity for tracking across layout passes.
    fn id(&self) -> &str;
}

/// Test helper: a mock clansman with configurable sizes.
pub struct MockClansman {
    pub name: String,
    pub intrinsic: Option<(f64, f64)>,
    pub min: (f64, f64),
    pub max: (f64, f64),
    pub priority: f64,
}

impl MockClansman {
    pub fn new(intrinsic: Option<(f64, f64)>) -> Self {
        Self {
            name: uuid::Uuid::new_v4().to_string(),
            intrinsic,
            min: (0.0, 0.0),
            max: (f64::INFINITY, f64::INFINITY),
            priority: 0.0,
        }
    }

    pub fn named(name: impl Into<String>, intrinsic: Option<(f64, f64)>) -> Self {
        Self {
            name: name.into(),
            intrinsic,
            min: (0.0, 0.0),
            max: (f64::INFINITY, f64::INFINITY),
            priority: 0.0,
        }
    }

    pub fn with_min(intrinsic: Option<(f64, f64)>, min: (f64, f64)) -> Self {
        Self {
            name: uuid::Uuid::new_v4().to_string(),
            intrinsic,
            min,
            max: (f64::INFINITY, f64::INFINITY),
            priority: 0.0,
        }
    }
}

impl Clansman for MockClansman {
    fn intrinsic_size(&self) -> Option<(f64, f64)> {
        self.intrinsic
    }

    fn min_size(&self) -> (f64, f64) {
        self.min
    }

    fn max_size(&self) -> (f64, f64) {
        self.max
    }

    fn layout_priority(&self) -> f64 {
        self.priority
    }

    fn id(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_defaults() {
        let c = MockClansman::new(None);
        assert!(c.intrinsic_size().is_none());
        assert_eq!(c.min_size(), (0.0, 0.0));
        assert_eq!(c.layout_priority(), 0.0);
    }

    #[test]
    fn mock_with_intrinsic() {
        let c = MockClansman::new(Some((100.0, 50.0)));
        assert_eq!(c.intrinsic_size(), Some((100.0, 50.0)));
    }

    #[test]
    fn mock_named() {
        let c = MockClansman::named("header", Some((200.0, 44.0)));
        assert_eq!(c.id(), "header");
        assert_eq!(c.intrinsic_size(), Some((200.0, 44.0)));
    }

    #[test]
    fn trait_is_object_safe() {
        fn accepts_clansman(_c: &dyn Clansman) {}
        let c = MockClansman::new(None);
        accepts_clansman(&c);
    }
}
