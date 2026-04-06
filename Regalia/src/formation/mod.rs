mod column;
mod open_court;
mod procession;
mod rank;
mod tier;
mod types;

pub use column::{Column, ColumnAlignment, ColumnJustification};
pub use open_court::OpenCourt;
pub use procession::Procession;
pub use rank::{Rank, RankAlignment, RankJustification};
pub use tier::Tier;
pub use types::FormationKind;

use crate::domain::Clansman;
use crate::insignia::Decree;

/// Layout algorithm trait. Takes children and bounds, produces placement decrees.
pub trait Formation: Send + Sync {
    fn place_children(
        &self,
        bounds_x: f64,
        bounds_y: f64,
        bounds_width: f64,
        bounds_height: f64,
        children: &[&dyn Clansman],
    ) -> Vec<Decree>;
}

/// Resolver function type for custom formations.
pub type FormationResolver<'a> = &'a dyn Fn(&str) -> Option<Box<dyn Formation>>;
