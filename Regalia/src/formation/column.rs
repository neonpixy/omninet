use serde::{Deserialize, Serialize};

use super::Formation;
use crate::domain::Clansman;
use crate::insignia::Decree;

/// Horizontal alignment for vertical (Column) layout.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColumnAlignment {
    /// Align children to the leading edge.
    Leading,
    /// Center children horizontally within the column.
    #[default]
    Center,
    /// Align children to the trailing edge.
    Trailing,
}

/// Vertical justification for Column layout.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColumnJustification {
    /// Pack children to the top.
    Top,
    /// Center children vertically within the bounds.
    #[default]
    Center,
    /// Pack children to the bottom.
    Bottom,
}

/// Vertical stack formation (→ SwiftUI VStack, CSS flex-column, Flutter Column).
pub struct Column {
    pub spacing: f64,
    pub alignment: ColumnAlignment,
    pub justification: ColumnJustification,
}

impl Column {
    /// Create a column with the given spacing, cross-axis alignment, and justification.
    pub fn new(
        spacing: f64,
        alignment: ColumnAlignment,
        justification: ColumnJustification,
    ) -> Self {
        Self {
            spacing,
            alignment,
            justification,
        }
    }
}

impl Default for Column {
    fn default() -> Self {
        Self::new(8.0, ColumnAlignment::Center, ColumnJustification::Center)
    }
}

impl Formation for Column {
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
        let available = (bounds_height - total_spacing).max(0.0);
        let per_child = available / children.len() as f64;

        let mut widths = Vec::with_capacity(children.len());
        let mut heights = Vec::with_capacity(children.len());
        for child in children {
            let (min_w, min_h) = child.min_size();
            let (max_w, max_h) = child.max_size();
            let intrinsic = child.intrinsic_size();
            let w = intrinsic.map(|(iw, _)| iw).unwrap_or(bounds_width);
            let h = intrinsic.map(|(_, ih)| ih).unwrap_or(per_child);
            widths.push(w.clamp(min_w, max_w).min(bounds_width));
            heights.push(h.clamp(min_h, max_h).min(per_child));
        }

        let total_height: f64 = heights.iter().sum::<f64>() + total_spacing;

        let y_offset = match self.justification {
            ColumnJustification::Top => bounds_y,
            ColumnJustification::Center => {
                bounds_y + (bounds_height - total_height).max(0.0) / 2.0
            }
            ColumnJustification::Bottom => bounds_y + (bounds_height - total_height).max(0.0),
        };

        let mut y = y_offset;
        let mut decrees = Vec::with_capacity(children.len());

        for i in 0..children.len() {
            let w = widths[i];
            let h = heights[i];

            let x = match self.alignment {
                ColumnAlignment::Leading => bounds_x,
                ColumnAlignment::Center => bounds_x + (bounds_width - w) / 2.0,
                ColumnAlignment::Trailing => bounds_x + bounds_width - w,
            };

            decrees.push(Decree::new(x, y, w, h));
            y += h + self.spacing;
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
        let col = Column::default();
        let result = col.place_children(0.0, 0.0, 100.0, 200.0, &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn single_child_centered() {
        let col = Column::default();
        let child = MockClansman::new(Some((40.0, 30.0)));
        let children: Vec<&dyn Clansman> = vec![&child];
        let result = col.place_children(0.0, 0.0, 100.0, 200.0, &children);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].width, 40.0);
        assert_eq!(result[0].height, 30.0);
        // Centered horizontally
        assert!((result[0].x - 30.0).abs() < 0.01);
    }

    #[test]
    fn two_children_with_spacing() {
        let col = Column::new(10.0, ColumnAlignment::Leading, ColumnJustification::Top);
        let c1 = MockClansman::new(Some((40.0, 30.0)));
        let c2 = MockClansman::new(Some((40.0, 30.0)));
        let children: Vec<&dyn Clansman> = vec![&c1, &c2];
        let result = col.place_children(0.0, 0.0, 100.0, 200.0, &children);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].y, 0.0);
        assert_eq!(result[1].y, 40.0); // 30 + 10 spacing
        // Leading aligned
        assert_eq!(result[0].x, 0.0);
        assert_eq!(result[1].x, 0.0);
    }

    #[test]
    fn bottom_justification() {
        let col = Column::new(0.0, ColumnAlignment::Center, ColumnJustification::Bottom);
        let child = MockClansman::new(Some((20.0, 20.0)));
        let children: Vec<&dyn Clansman> = vec![&child];
        let result = col.place_children(0.0, 0.0, 100.0, 200.0, &children);
        assert_eq!(result[0].y, 180.0); // 200 - 20
    }
}
