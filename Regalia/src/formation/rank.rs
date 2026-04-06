use serde::{Deserialize, Serialize};

use super::Formation;
use crate::domain::Clansman;
use crate::insignia::Decree;

/// Vertical alignment for horizontal (Rank) layout.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RankAlignment {
    /// Align children to the top of the row.
    Top,
    /// Center children vertically within the row.
    #[default]
    Center,
    /// Align children to the bottom of the row.
    Bottom,
}

/// Horizontal justification for Rank layout.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RankJustification {
    /// Pack children to the leading edge.
    Leading,
    /// Center children horizontally within the bounds.
    #[default]
    Center,
    /// Pack children to the trailing edge.
    Trailing,
}

/// Horizontal stack formation (→ SwiftUI HStack, CSS flex-row, Flutter Row).
pub struct Rank {
    pub spacing: f64,
    pub alignment: RankAlignment,
    pub justification: RankJustification,
}

impl Rank {
    /// Create a rank with the given spacing, cross-axis alignment, and justification.
    pub fn new(spacing: f64, alignment: RankAlignment, justification: RankJustification) -> Self {
        Self {
            spacing,
            alignment,
            justification,
        }
    }
}

impl Default for Rank {
    fn default() -> Self {
        Self::new(8.0, RankAlignment::Center, RankJustification::Center)
    }
}

impl Formation for Rank {
    fn place_children(
        &self,
        bounds_x: f64,
        bounds_y: f64,
        bounds_width: f64,
        bounds_height: f64,
        children: &[&dyn Clansman],
    ) -> Vec<Decree> {
        if children.is_empty() {
            return vec![];
        }

        let total_spacing = self.spacing * (children.len() as f64 - 1.0).max(0.0);
        let available = (bounds_width - total_spacing).max(0.0);

        // Allocate widths proportionally (priority-based in full impl,
        // even distribution here for correctness)
        let per_child = if children.is_empty() {
            0.0
        } else {
            available / children.len() as f64
        };

        // Measure each child
        let mut widths = Vec::with_capacity(children.len());
        let mut heights = Vec::with_capacity(children.len());
        for child in children {
            let (min_w, min_h) = child.min_size();
            let (max_w, max_h) = child.max_size();
            let intrinsic = child.intrinsic_size();
            let w = intrinsic.map(|(iw, _)| iw).unwrap_or(per_child);
            let h = intrinsic.map(|(_, ih)| ih).unwrap_or(bounds_height);
            widths.push(w.clamp(min_w, max_w).min(per_child));
            heights.push(h.clamp(min_h, max_h).min(bounds_height));
        }

        let total_width: f64 = widths.iter().sum::<f64>() + total_spacing;

        // Justification offset
        let x_offset = match self.justification {
            RankJustification::Leading => bounds_x,
            RankJustification::Center => bounds_x + (bounds_width - total_width).max(0.0) / 2.0,
            RankJustification::Trailing => bounds_x + (bounds_width - total_width).max(0.0),
        };

        let mut x = x_offset;
        let mut decrees = Vec::with_capacity(children.len());

        for i in 0..children.len() {
            let w = widths[i];
            let h = heights[i];

            // Vertical alignment
            let y = match self.alignment {
                RankAlignment::Top => bounds_y,
                RankAlignment::Center => bounds_y + (bounds_height - h) / 2.0,
                RankAlignment::Bottom => bounds_y + bounds_height - h,
            };

            decrees.push(Decree::new(x, y, w, h));
            x += w + self.spacing;
        }

        decrees
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::MockClansman;

    #[test]
    fn empty_children() {
        let rank = Rank::default();
        let result = rank.place_children(0.0, 0.0, 100.0, 50.0, &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn single_child_centered() {
        let rank = Rank::default();
        let child = MockClansman::new(Some((40.0, 30.0)));
        let children: Vec<&dyn Clansman> = vec![&child];
        let result = rank.place_children(0.0, 0.0, 100.0, 50.0, &children);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].width, 40.0);
        assert_eq!(result[0].height, 30.0);
        // Centered vertically
        assert!((result[0].y - 10.0).abs() < 0.01);
    }

    #[test]
    fn two_children_with_spacing() {
        let rank = Rank::new(10.0, RankAlignment::Top, RankJustification::Leading);
        let c1 = MockClansman::new(Some((30.0, 20.0)));
        let c2 = MockClansman::new(Some((30.0, 20.0)));
        let children: Vec<&dyn Clansman> = vec![&c1, &c2];
        let result = rank.place_children(0.0, 0.0, 100.0, 50.0, &children);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].x, 0.0);
        assert_eq!(result[1].x, 40.0); // 30 + 10 spacing
        // Top aligned
        assert_eq!(result[0].y, 0.0);
        assert_eq!(result[1].y, 0.0);
    }

    #[test]
    fn trailing_justification() {
        let rank = Rank::new(0.0, RankAlignment::Center, RankJustification::Trailing);
        let child = MockClansman::new(Some((20.0, 20.0)));
        let children: Vec<&dyn Clansman> = vec![&child];
        let result = rank.place_children(0.0, 0.0, 100.0, 50.0, &children);
        assert_eq!(result[0].x, 80.0); // 100 - 20
    }
}
