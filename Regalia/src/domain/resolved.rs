use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::Appointment;
use crate::insignia::SanctumID;

/// Complete resolved layout: all appointments sorted back-to-front,
/// plus clip rects and sanctum bounds for rendering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Domain {
    /// All resolved nodes, sorted by composite_z_order (back-to-front).
    pub appointments: Vec<Appointment>,
    /// Clip rects for sanctums with clips=true.
    pub sanctum_clips: HashMap<String, (f64, f64, f64, f64)>,
    /// Bounds for ALL sanctums (for overlay lookup).
    pub sanctum_bounds: HashMap<String, (f64, f64, f64, f64)>,
    /// Total layout bounds.
    pub bounds: (f64, f64, f64, f64),
}

impl Domain {
    /// Create a new domain, sorting appointments by z-order (back-to-front).
    pub fn new(
        mut appointments: Vec<Appointment>,
        sanctum_clips: HashMap<String, (f64, f64, f64, f64)>,
        sanctum_bounds: HashMap<String, (f64, f64, f64, f64)>,
        bounds: (f64, f64, f64, f64),
    ) -> Self {
        // Sort back-to-front by composite z-order
        appointments.sort_by(|a, b| {
            a.composite_z_order
                .partial_cmp(&b.composite_z_order)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Self {
            appointments,
            sanctum_clips,
            sanctum_bounds,
            bounds,
        }
    }

    /// Look up the bounds of a specific sanctum.
    pub fn sanctum_bounds_for(&self, id: &SanctumID) -> Option<(f64, f64, f64, f64)> {
        self.sanctum_bounds.get(id.as_str()).copied()
    }

    /// Get all appointments belonging to a specific sanctum.
    pub fn appointments_for(&self, sanctum_id: &SanctumID) -> Vec<&Appointment> {
        self.appointments
            .iter()
            .filter(|a| a.sanctum_id == *sanctum_id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_by_z_order() {
        let a1 = Appointment::new("back", (0.0, 0.0, 10.0, 10.0), SanctumID::content(), 0, 0.0);
        let a2 = Appointment::new("front", (0.0, 0.0, 10.0, 10.0), SanctumID::overlay(), 200, 0.0);
        let domain = Domain::new(
            vec![a2, a1], // Intentionally out of order
            HashMap::new(),
            HashMap::new(),
            (0.0, 0.0, 100.0, 100.0),
        );
        assert_eq!(domain.appointments[0].id, "back");
        assert_eq!(domain.appointments[1].id, "front");
    }

    #[test]
    fn sanctum_bounds_lookup() {
        let mut bounds = HashMap::new();
        bounds.insert("content".into(), (0.0, 0.0, 800.0, 600.0));
        let domain = Domain::new(vec![], HashMap::new(), bounds, (0.0, 0.0, 800.0, 600.0));
        assert!(domain.sanctum_bounds_for(&SanctumID::content()).is_some());
        assert!(domain.sanctum_bounds_for(&SanctumID::sidebar()).is_none());
    }

    #[test]
    fn appointments_for_sanctum() {
        let a1 = Appointment::new("a", (0.0, 0.0, 10.0, 10.0), SanctumID::content(), 0, 0.0);
        let a2 = Appointment::new("b", (0.0, 0.0, 10.0, 10.0), SanctumID::sidebar(), 0, 0.0);
        let a3 = Appointment::new("c", (0.0, 0.0, 10.0, 10.0), SanctumID::content(), 0, 1.0);
        let domain = Domain::new(
            vec![a1, a2, a3],
            HashMap::new(),
            HashMap::new(),
            (0.0, 0.0, 100.0, 100.0),
        );
        let content = domain.appointments_for(&SanctumID::content());
        assert_eq!(content.len(), 2);
        let sidebar = domain.appointments_for(&SanctumID::sidebar());
        assert_eq!(sidebar.len(), 1);
    }
}
