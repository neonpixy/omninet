use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Lightweight spatial position for an idea.
///
/// Complex spatial types (orbits, multiverse, wormholes) go to Universe (U).
/// This is the bare minimum an .idea needs for spatial identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub coordinates: Coordinates,
    pub pinned: bool,
    pub modified: DateTime<Utc>,
}

/// A point in 3D space, used for spatial positioning of ideas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Coordinates {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Coordinates {
    /// The origin point (0, 0, 0).
    pub fn origin() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    /// Euclidean distance to another point.
    pub fn distance_to(&self, other: &Coordinates) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

impl Position {
    /// Creates a new Position at the given coordinates.
    pub fn new(coordinates: Coordinates, pinned: bool) -> Self {
        Position {
            coordinates,
            pinned,
            modified: Utc::now(),
        }
    }

    /// Creates an unpinned Position at the origin.
    pub fn at_origin() -> Self {
        Self::new(Coordinates::origin(), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin() {
        let c = Coordinates::origin();
        assert_eq!(c.x, 0.0);
        assert_eq!(c.y, 0.0);
        assert_eq!(c.z, 0.0);
    }

    #[test]
    fn distance() {
        let a = Coordinates { x: 0.0, y: 0.0, z: 0.0 };
        let b = Coordinates { x: 3.0, y: 4.0, z: 0.0 };
        assert!((a.distance_to(&b) - 5.0).abs() < 1e-10);
    }

    #[test]
    fn position_serde_round_trip() {
        let p = Position::new(Coordinates { x: 10.0, y: 20.0, z: 30.0 }, true);
        let json = serde_json::to_string(&p).unwrap();
        let rt: Position = serde_json::from_str(&json).unwrap();
        assert_eq!(rt.coordinates.x, 10.0);
        assert!(rt.pinned);
    }
}
