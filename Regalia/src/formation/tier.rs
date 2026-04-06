use super::Formation;
use crate::domain::Clansman;
use crate::insignia::Decree;

/// Depth-stacking formation: all children get full bounds, centered.
/// First child = back, last child = front.
/// (→ SwiftUI ZStack, CSS position stacked, Flutter Stack)
pub struct Tier;

impl Formation for Tier {
    fn place_children(
        &self,
        bounds_x: f64,
        bounds_y: f64,
        bounds_width: f64,
        bounds_height: f64,
        children: &[&dyn Clansman],
    ) -> Vec<Decree> {
        children
            .iter()
            .enumerate()
            .map(|(i, child)| {
                let intrinsic = child.intrinsic_size();
                let (min_w, min_h) = child.min_size();
                let (max_w, max_h) = child.max_size();

                let w = intrinsic
                    .map(|(iw, _)| iw)
                    .unwrap_or(bounds_width)
                    .clamp(min_w, max_w)
                    .min(bounds_width);
                let h = intrinsic
                    .map(|(_, ih)| ih)
                    .unwrap_or(bounds_height)
                    .clamp(min_h, max_h)
                    .min(bounds_height);

                // Center within bounds
                let x = bounds_x + (bounds_width - w) / 2.0;
                let y = bounds_y + (bounds_height - h) / 2.0;

                Decree::new(x, y, w, h).with_z_index(i as f64)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::MockClansman;

    #[test]
    fn all_children_centered() {
        let tier = Tier;
        let c1 = MockClansman::new(Some((40.0, 30.0)));
        let c2 = MockClansman::new(Some((60.0, 50.0)));
        let children: Vec<&dyn Clansman> = vec![&c1, &c2];
        let result = tier.place_children(0.0, 0.0, 100.0, 100.0, &children);
        assert_eq!(result.len(), 2);
        // Both centered
        assert!((result[0].x - 30.0).abs() < 0.01);
        assert!((result[1].x - 20.0).abs() < 0.01);
        // z-order: first=0, second=1
        assert_eq!(result[0].z_index, 0.0);
        assert_eq!(result[1].z_index, 1.0);
    }

    #[test]
    fn no_intrinsic_fills_bounds() {
        let tier = Tier;
        let child = MockClansman::new(None);
        let children: Vec<&dyn Clansman> = vec![&child];
        let result = tier.place_children(10.0, 20.0, 200.0, 100.0, &children);
        assert_eq!(result[0].x, 10.0);
        assert_eq!(result[0].y, 20.0);
        assert_eq!(result[0].width, 200.0);
        assert_eq!(result[0].height, 100.0);
    }
}
