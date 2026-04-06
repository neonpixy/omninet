use serde::{Deserialize, Serialize};

/// Coordinate origin / anchor point for positioning within a sanctum.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Seat {
    /// Origin at the center of the sanctum.
    #[default]
    Center,
    /// Origin at the top-leading corner.
    TopLeading,
    /// Origin at the top-trailing corner.
    TopTrailing,
    /// Origin at the bottom-leading corner.
    BottomLeading,
    /// Origin at the bottom-trailing corner.
    BottomTrailing,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_center() {
        assert_eq!(Seat::default(), Seat::Center);
    }

    #[test]
    fn serde_roundtrip() {
        for seat in [
            Seat::Center,
            Seat::TopLeading,
            Seat::TopTrailing,
            Seat::BottomLeading,
            Seat::BottomTrailing,
        ] {
            let json = serde_json::to_string(&seat).unwrap();
            let decoded: Seat = serde_json::from_str(&json).unwrap();
            assert_eq!(seat, decoded);
        }
    }
}
