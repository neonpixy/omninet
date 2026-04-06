use serde::{de::DeserializeOwned, Serialize};

/// Universal material interface. Any visual material (glass, thin-film, neon,
/// fabric, etc.) implements this trait. Adding a new material to Omnidea =
/// implementing `Material` + `MaterialDelta`.
///
/// All materials are serializable (for `.excalibur` theme files), cloneable,
/// and thread-safe.
pub trait Material: Clone + Send + Sync + Serialize + DeserializeOwned + 'static {
    /// The delta type for additive modifications.
    type Delta: MaterialDelta;

    /// Apply a delta additively, returning a new style with clamped values.
    fn applying(&self, delta: &Self::Delta) -> Self;

    /// Material kind identifier (e.g., "facet", "iris").
    fn kind() -> &'static str;
}

/// Additive modifier for a material. All fields are optional — only set fields
/// are applied.
pub trait MaterialDelta:
    Clone + Send + Sync + Default + Serialize + DeserializeOwned + 'static
{
    /// Whether this delta would change anything (all fields are None/default).
    fn is_identity(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Verify the traits have the bounds we expect.
    fn _assert_material_bounds<M: Material>() {}
    fn _assert_delta_bounds<D: MaterialDelta>() {}

    #[test]
    fn delta_default_is_identity() {
        // Any MaterialDelta's Default should be identity.
        // We'll test this with concrete types in facet/iris tests.
    }
}
