use super::Formation;
use crate::domain::Clansman;
use crate::insignia::Decree;

/// Flow-wrap formation: places children left-to-right, wrapping to next row
/// when bounds width is exceeded.
/// (→ CSS flex-wrap, Flutter Wrap)
pub struct Procession {
    pub horizontal_spacing: f64,
    pub vertical_spacing: f64,
}

impl Procession {
    /// Create a procession with the given horizontal and vertical spacing between items.
    pub fn new(horizontal_spacing: f64, vertical_spacing: f64) -> Self {
        Self {
            horizontal_spacing,
            vertical_spacing,
        }
    }
}

impl Default for Procession {
    fn default() -> Self {
        Self::new(8.0, 8.0)
    }
}

impl Formation for Procession {
    fn place_children(
        &self,
        bounds_x: f64,
        bounds_y: f64,
        bounds_width: f64,
        _bounds_height: f64,
        children: &[&dyn Clansman],
    ) -> Vec<Decree> {
        if children.is_empty() {
            return vec![];
        }

        let mut decrees = Vec::with_capacity(children.len());
        let mut x = bounds_x;
        let mut y = bounds_y;
        let mut row_height: f64 = 0.0;

        for child in children {
            let intrinsic = child.intrinsic_size();
            let (min_w, min_h) = child.min_size();
            let (max_w, max_h) = child.max_size();

            let w = intrinsic.map(|(iw, _)| iw).unwrap_or(min_w).clamp(min_w, max_w);
            let h = intrinsic.map(|(_, ih)| ih).unwrap_or(min_h).clamp(min_h, max_h);

            // Wrap to next row if this child doesn't fit
            if x + w > bounds_x + bounds_width && x > bounds_x {
                x = bounds_x;
                y += row_height + self.vertical_spacing;
                row_height = 0.0;
            }

            decrees.push(Decree::new(x, y, w, h));
            x += w + self.horizontal_spacing;
            row_height = row_height.max(h);
        }

        decrees
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::MockClansman;

    #[test]
    fn single_row() {
        let proc = Procession::new(10.0, 10.0);
        let c1 = MockClansman::new(Some((30.0, 20.0)));
        let c2 = MockClansman::new(Some((30.0, 20.0)));
        let children: Vec<&dyn Clansman> = vec![&c1, &c2];
        let result = proc.place_children(0.0, 0.0, 100.0, 100.0, &children);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].x, 0.0);
        assert_eq!(result[1].x, 40.0); // 30 + 10
        assert_eq!(result[0].y, 0.0);
        assert_eq!(result[1].y, 0.0);
    }

    #[test]
    fn wraps_to_next_row() {
        let proc = Procession::new(10.0, 10.0);
        let c1 = MockClansman::new(Some((50.0, 20.0)));
        let c2 = MockClansman::new(Some((50.0, 20.0)));
        let c3 = MockClansman::new(Some((50.0, 20.0)));
        let children: Vec<&dyn Clansman> = vec![&c1, &c2, &c3];
        // 100px wide: c1=50, then x=60, c2=50 would need x=60+50=110 > 100, wraps
        let result = proc.place_children(0.0, 0.0, 100.0, 200.0, &children);
        assert_eq!(result.len(), 3);
        // First row: c1 at x=0
        assert_eq!(result[0].x, 0.0);
        assert_eq!(result[0].y, 0.0);
        // c2 wraps to next row
        assert_eq!(result[1].x, 0.0);
        assert_eq!(result[1].y, 30.0); // 20 + 10 spacing
        // c3 wraps again (c2 is at x=0, width=50, next x=60, 60+50=110 > 100)
        assert_eq!(result[2].x, 0.0);
        assert_eq!(result[2].y, 60.0); // 30 + 20 + 10 spacing
    }

    #[test]
    fn empty_children() {
        let proc = Procession::default();
        let result = proc.place_children(0.0, 0.0, 100.0, 100.0, &[]);
        assert!(result.is_empty());
    }
}
