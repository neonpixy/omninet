use serde::{Deserialize, Serialize};

/// Animation preset name. Extensible via custom string values.
///
/// Maps to a `Shift` (animation curve) via `to_shift()`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Impulse(pub String);

impl Impulse {
    /// Instant transition, no animation.
    pub fn snap() -> Self {
        Self("snap".into())
    }
    /// Short, snappy animation.
    pub fn quick() -> Self {
        Self("quick".into())
    }
    /// Balanced ease-in-out, the default feel.
    pub fn smooth() -> Self {
        Self("smooth".into())
    }
    /// Slow, intentional transition for large changes.
    pub fn deliberate() -> Self {
        Self("deliberate".into())
    }
    /// Playful spring overshoot.
    pub fn bouncy() -> Self {
        Self("bouncy".into())
    }
    /// Soft, relaxed motion for ambient effects.
    pub fn gentle() -> Self {
        Self("gentle".into())
    }
    /// User-defined animation preset.
    pub fn custom(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Get the name of this impulse preset.
    pub fn name(&self) -> &str {
        &self.0
    }
}

impl Default for Impulse {
    fn default() -> Self {
        Self::smooth()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_names() {
        assert_eq!(Impulse::snap().name(), "snap");
        assert_eq!(Impulse::smooth().name(), "smooth");
        assert_eq!(Impulse::bouncy().name(), "bouncy");
    }

    #[test]
    fn custom_preset() {
        let i = Impulse::custom("wiggle");
        assert_eq!(i.name(), "wiggle");
    }

    #[test]
    fn default_is_smooth() {
        assert_eq!(Impulse::default(), Impulse::smooth());
    }

    #[test]
    fn serde_roundtrip() {
        let i = Impulse::bouncy();
        let json = serde_json::to_string(&i).unwrap();
        let decoded: Impulse = serde_json::from_str(&json).unwrap();
        assert_eq!(i, decoded);
    }
}
