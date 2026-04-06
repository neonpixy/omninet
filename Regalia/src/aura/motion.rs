use serde::{Deserialize, Serialize};

/// User preference for animation intensity. Accessibility-first: reduced-motion
/// users get simplified animations, none-motion users get zero animation.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MotionPreference {
    /// All animations play.
    #[default]
    Full,
    /// Simplified animations (large motion removed, subtle transitions preserved).
    Reduced,
    /// Zero animation — all state changes are instant.
    None,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_full() {
        assert_eq!(MotionPreference::default(), MotionPreference::Full);
    }

    #[test]
    fn equality() {
        assert_eq!(MotionPreference::Full, MotionPreference::Full);
        assert_eq!(MotionPreference::Reduced, MotionPreference::Reduced);
        assert_eq!(MotionPreference::None, MotionPreference::None);
        assert_ne!(MotionPreference::Full, MotionPreference::Reduced);
        assert_ne!(MotionPreference::Full, MotionPreference::None);
    }

    #[test]
    fn serde_roundtrip() {
        for pref in [
            MotionPreference::Full,
            MotionPreference::Reduced,
            MotionPreference::None,
        ] {
            let json = serde_json::to_string(&pref).unwrap();
            let decoded: MotionPreference = serde_json::from_str(&json).unwrap();
            assert_eq!(pref, decoded);
        }
    }

    #[test]
    fn hash_works() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(MotionPreference::Full);
        set.insert(MotionPreference::Reduced);
        set.insert(MotionPreference::None);
        assert_eq!(set.len(), 3);
        assert!(set.contains(&MotionPreference::Full));
    }
}
