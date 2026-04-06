//! # Crown Jewels — Universal Material System
//!
//! The crown jewels of Regalia. A universal material system where any visual
//! material (glass, thin-film, neon, fabric, etc.) can be described, cascaded
//! via stylesheets, and composed with layout regions.
//!
//! Adding a new material = implementing `Material` + `MaterialDelta` traits,
//! then creating a subdirectory under `materials/`.
//!
//! ## Subsystems
//!
//! - **Material trait** — universal interface for any material type
//! - **Stylesheet<M>** — CSS-like cascade for any material (generic)
//! - **materials/facet** — glass material (frost, refraction, dispersion, etc.)
//! - **materials/iris** — thin-film interference (nacre, oil slick, beetle, etc.)
//! - **Shape** — SDF primitives (rounded rect, capsule, circle, polygon, star)
//! - **SDF** — signed distance field math (pure f64, no platform deps)
//! - **Color math** — HSL/HSB conversion, WCAG contrast, Fresnel weights
//! - **OneEuroFilter** — adaptive low-pass signal smoothing
//! - **Bridge** — CrownSanctum/CrownArbiter (layout + material composition)

mod bridge;
mod color_math;
mod crown_role;
mod filter;
mod material;
pub mod materials;
mod sdf;
mod shape;
mod stylesheet;

pub use bridge::{CrownAppointment, CrownArbiter, CrownDomain, CrownSanctum};
pub use color_math::{
    contrast_ratio, fresnel_spectral_weights, hsl_to_rgba, hsb_to_rgba, premultiply,
    relative_luminance, rgba_to_hsl, rgba_to_hsb,
};
pub use crown_role::CrownRole;
pub use filter::{OneEuroFilter, OneEuroFilterConfig};
pub use material::{Material, MaterialDelta};
pub use materials::facet::{FacetAppearance, FacetStyle, FacetStyleDelta, FacetVariant, LightSource};
pub use materials::iris::{IrisDimple, IrisStyle, IrisStyleDelta, DIMPLE_MAX_COUNT};
pub use sdf::{sdf_ellipse, sdf_polygon, sdf_rounded_rect, sdf_star, smooth_min};
pub use shape::{CornerRadii, ShapeDescriptor};
pub use stylesheet::Stylesheet;
