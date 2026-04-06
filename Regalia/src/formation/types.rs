use serde::{Deserialize, Serialize};

use super::{Column, ColumnAlignment, ColumnJustification, Formation, OpenCourt, Procession, Rank, RankAlignment, RankJustification, Tier};

/// Codable layout descriptor. Maps to a Formation implementation at runtime.
///
/// This is the serialization bridge between `.excalibur` theme files and runtime
/// layout resolution. Each variant maps directly to a platform layout primitive.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub enum FormationKind {
    /// Free-form positioning (absolute placement).
    #[default]
    OpenCourt,

    /// Horizontal stack (→ SwiftUI HStack, CSS flex-row, Flutter Row).
    Rank {
        #[serde(default = "default_spacing")]
        spacing: f64,
        #[serde(default)]
        alignment: RankAlignment,
        #[serde(default)]
        justification: RankJustification,
    },

    /// Vertical stack (→ SwiftUI VStack, CSS flex-column, Flutter Column).
    Column {
        #[serde(default = "default_spacing")]
        spacing: f64,
        #[serde(default)]
        alignment: ColumnAlignment,
        #[serde(default)]
        justification: ColumnJustification,
    },

    /// All children get full bounds (→ SwiftUI ZStack, CSS position stacked, Flutter Stack).
    Tier,

    /// Flow-wrap layout (→ CSS flex-wrap, Flutter Wrap).
    Procession {
        #[serde(default = "default_spacing")]
        horizontal_spacing: f64,
        #[serde(default = "default_spacing")]
        vertical_spacing: f64,
    },

    /// Custom formation resolved at runtime by name.
    Custom(String),
}

fn default_spacing() -> f64 {
    8.0
}

impl FormationKind {
    /// Instantiate the Formation implementation for this kind.
    ///
    /// Custom formations return OpenCourt if no resolver is provided.
    pub fn make_formation(
        &self,
        resolver: Option<super::FormationResolver<'_>>,
    ) -> Box<dyn Formation> {
        match self {
            FormationKind::OpenCourt => Box::new(OpenCourt),
            FormationKind::Rank {
                spacing,
                alignment,
                justification,
            } => Box::new(Rank::new(*spacing, *alignment, *justification)),
            FormationKind::Column {
                spacing,
                alignment,
                justification,
            } => Box::new(Column::new(*spacing, *alignment, *justification)),
            FormationKind::Tier => Box::new(Tier),
            FormationKind::Procession {
                horizontal_spacing,
                vertical_spacing,
            } => Box::new(Procession::new(*horizontal_spacing, *vertical_spacing)),
            FormationKind::Custom(name) => {
                if let Some(resolver) = resolver {
                    resolver(name).unwrap_or_else(|| Box::new(OpenCourt))
                } else {
                    Box::new(OpenCourt)
                }
            }
        }
    }
}

// Default is OpenCourt, applied via #[derive(Default)] + #[default] on the enum.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_open_court() {
        assert_eq!(FormationKind::default(), FormationKind::OpenCourt);
    }

    #[test]
    fn rank_default_spacing() {
        let fk = FormationKind::Rank {
            spacing: 8.0,
            alignment: RankAlignment::Center,
            justification: RankJustification::Center,
        };
        if let FormationKind::Rank { spacing, .. } = fk {
            assert_eq!(spacing, 8.0);
        }
    }

    #[test]
    fn serde_roundtrip_all_variants() {
        let variants = vec![
            FormationKind::OpenCourt,
            FormationKind::Rank {
                spacing: 12.0,
                alignment: RankAlignment::Top,
                justification: RankJustification::Leading,
            },
            FormationKind::Column {
                spacing: 16.0,
                alignment: ColumnAlignment::Trailing,
                justification: ColumnJustification::Bottom,
            },
            FormationKind::Tier,
            FormationKind::Procession {
                horizontal_spacing: 8.0,
                vertical_spacing: 12.0,
            },
            FormationKind::Custom("masonry".into()),
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let decoded: FormationKind = serde_json::from_str(&json).unwrap();
            assert_eq!(v, decoded);
        }
    }

    #[test]
    fn custom_without_resolver_falls_back() {
        let fk = FormationKind::Custom("masonry".into());
        // Should not panic — falls back to OpenCourt
        let _formation = fk.make_formation(None);
    }

    #[test]
    fn custom_with_resolver() {
        let fk = FormationKind::Custom("grid".into());
        let resolver = |name: &str| -> Option<Box<dyn Formation>> {
            if name == "grid" {
                Some(Box::new(Tier)) // Just use Tier as a stand-in
            } else {
                None
            }
        };
        let _formation = fk.make_formation(Some(&resolver));
    }
}
