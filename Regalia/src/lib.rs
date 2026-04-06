//! # Regalia — Design Language
//!
//! The royal garments. Regalia is the design vocabulary everything wears — tokens,
//! layout, theming, animation. Serializable, runtime-configurable, zero rendering
//! dependencies. No hardcoded colors, spacing, or typography anywhere in Omnidea.
//!
//! ## Subsystems
//!
//! - **Aura** — Design tokens: Ember (color), Flame (ramp), Crest (palette), Span (spacing),
//!   Inscription (typography), Arch (radii), UmbraScale (shadows), Impulse (animation presets),
//!   Gradient (linear/radial/angular), ImageStyle (fit/border/shadow), MotionPreference (accessibility).
//! - **Insignia** — Layout primitives: Border, BorderInsets, Seat, Decree, Petition, SanctumID.
//! - **Formation** — Layout algorithms: OpenCourt, Rank, Column, Tier, Procession. FormationKind
//!   enum maps 1:1 to platform primitives (HStack/VStack/ZStack/flex-row/flex-column/etc).
//! - **Domain** — Layout solver: Arbiter (carve → recurse → form → appoint), Appointment
//!   (resolved frame), Domain (complete layout result).
//! - **Sanctum** — Named layout regions with edge attachment, nesting, and formation.
//! - **Surge** — Animation curves: SpringSurge, EaseSurge, LinearSurge, DecaySurge, SnapSurge.
//! - **Reign** — Theming: Reign (theme) + Aspect (light/dark/custom).
//! - **Component Style** — Composite styles referencing Aura token keys. ComponentStyleRegistry.
//! - **Theme Collection** — Multi-theme management with active selection.
//! - **Crown Jewels** — Universal material system: Material trait, Stylesheet<M>,
//!   FacetStyle (glass), IrisStyle (thin-film), ShapeDescriptor (SDF primitives),
//!   color math, OneEuroFilter, CrownSanctum/CrownArbiter (layout + material bridge).
//!
//! ## Covenant Alignment
//!
//! **Dignity** — consistent, beautiful design is a right.
//! **Sovereignty** — themes are user-configurable; `.excalibur` files can be shared.

pub mod aura;
pub mod component_style;
pub mod crown_jewels;
pub mod domain;
pub mod error;
pub mod formation;
pub mod insignia;
pub mod reign;
pub mod sanctum;
pub mod surge;
pub mod theme_collection;

// Re-exports for convenience.
pub use aura::{
    Arch, Aura, Crest, Ember, Flame, Glyph, GlyphWeight, Gradient, GradientStop, ImageFitMode,
    ImageStyle, Impulse, Inscription, MotionPreference, Span, Umbra, UmbraScale,
};
pub use domain::{Appointment, Arbiter, Clansman, Domain, Frame, MockClansman};
pub use error::RegaliaError;
pub use formation::{
    Column, ColumnAlignment, ColumnJustification, Formation, FormationKind, FormationResolver,
    OpenCourt, Procession, Rank, RankAlignment, RankJustification, Tier,
};
pub use insignia::{Border, BorderInsets, Decree, Petition, SanctumID, Seat};
pub use component_style::{ComponentStyle, ComponentStyleRegistry};
pub use reign::{Aspect, Reign};
pub use sanctum::Sanctum;
pub use theme_collection::ThemeCollection;
pub use surge::{DecaySurge, EaseSurge, LinearSurge, Shift, SnapSurge, SpringSurge, Surge};

// Crown Jewels re-exports.
pub use crown_jewels::{
    CornerRadii, CrownAppointment, CrownArbiter, CrownDomain, CrownRole, CrownSanctum,
    FacetAppearance, FacetStyle, FacetStyleDelta, FacetVariant, IrisDimple, IrisStyle,
    IrisStyleDelta, LightSource, Material, MaterialDelta, OneEuroFilter, OneEuroFilterConfig,
    ShapeDescriptor, Stylesheet, DIMPLE_MAX_COUNT,
};
