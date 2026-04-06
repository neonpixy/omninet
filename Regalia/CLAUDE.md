# Regalia — Design Language

The royal garments. Regalia is the design vocabulary everything wears — tokens, layout, theming, animation, materials. Serializable, runtime-configurable, zero rendering dependencies. No hardcoded colors, spacing, or typography anywhere in Omninet.

## Subsystems

### Aura — Design Tokens (`aura/`)

All token types are `Serialize + Deserialize` and support custom `HashMap` dictionaries for extensibility.

| File | Type | What It Is |
|------|------|-----------|
| `ember.rs` | `Ember` | RGBA color. Hex-encoded serde (`#RRGGBB`/`#RRGGBBAA`). `lighten()`, `darken()`. Constants: BLACK, WHITE, CLEAR. |
| `flame.rs` | `Flame` | Shade/base/tint color ramp. Auto-generate from base color. |
| `crest.rs` | `Crest` | 11 semantic colors + families + custom HashMap. Light/dark defaults. |
| `span.rs` | `Span` | Spacing scale (xs through xxl) + custom HashMap. |
| `inscription.rs` | `Glyph`, `GlyphWeight`, `Inscription` | Typography: 6 type levels + custom. Glyph = family/size/weight. |
| `arch.rs` | `Arch` | Corner radii scale (sm through full) + custom. |
| `umbra.rs` | `Umbra`, `UmbraScale` | 4 shadow levels + custom. |
| `impulse.rs` | `Impulse` | Extensible animation preset names. |
| `gradient.rs` | `Gradient`, `GradientStop` | Linear/Radial/Angular gradients with color stops. `color_at()` interpolation, `reversed()`, presets (sunset/ocean/forest). |
| `image_style.rs` | `ImageStyle`, `ImageFitMode` | Image styling: fit mode (Fill/Fit/Stretch/Tile), corner radius, border, shadow, opacity. All decoration fields are string keys into Aura. |
| `motion.rs` | `MotionPreference` | Accessibility motion preference: Full, Reduced, None. |
| `container.rs` | `Aura` | Token container. Serializes to `.excalibur` theme files. Includes gradients, image styles, motion preference, minimum touch target (44pt), minimum font size (12pt). |

### Insignia — Layout Primitives (`insignia/`)

| File | Type | What It Is |
|------|------|-----------|
| `border.rs` | `Border` | Edge enum (Top/Bottom/Leading/Trailing). |
| `border_insets.rs` | `BorderInsets` | 4-edge padding/margin insets. |
| `seat.rs` | `Seat` | Coordinate origin / sizing (fixed/fill/hug). |
| `decree.rs` | `Decree` | Placement order in local space (alignment). |
| `petition.rs` | `Petition` | Size proposal / constraints. |
| `sanctum_id.rs` | `SanctumID` | String-based layout region identifier, flat serde. |

### Formation — Layout Algorithms (`formation/`)

`FormationKind` enum maps 1:1 to platform layout primitives:

| FormationKind | What It Does | SwiftUI | CSS |
|---|---|---|---|
| `Rank` | Horizontal flow | HStack | flex-row |
| `Column` | Vertical flow | VStack | flex-column |
| `Tier` | Depth stacking | ZStack | position stacked |
| `Procession` | Flow-wrap | LazyVGrid / flow | flex-wrap |
| `OpenCourt` | Free positioning | Canvas / GeometryReader | position: absolute |
| `Custom(String)` | User-defined | resolver-based | resolver-based |

Each formation implements the `Formation` trait. `FormationResolver` allows registering custom layout algorithms at runtime.

`Rank` and `Column` have alignment and justification options. `Procession` handles wrapping. `OpenCourt` allows absolute positioning. `Tier` stacks all children to full bounds.

### Domain — Layout Solver (`domain/`)

- **`Arbiter`** — layout solver: allocate sanctums, apply insets, recurse subsanctums (max 8 levels), run formations, produce Appointments
- **`Appointment`** — resolved layout node (position + size + z-order + sanctum ID). `Frame` type alias for `(f64, f64, f64, f64)`.
- **`Clansman`** trait — child protocol for formation participation. `MockClansman` for testing.
- **`Domain`** — complete layout result (sorted appointments + clip rects + sanctum bounds)

The Arbiter algorithm:
1. **Border carving** — subtract padding/margin from available bounds
2. **Recurse** — process nested Sanctums (max 8 levels, enforced)
3. **Formation** — run the Sanctum's Formation algorithm on its Clansmen
4. **Appointments** — output resolved frames (position + size + z-order)

### Sanctum (`sanctum.rs`)

Named layout region with edge attachment, formation, and nesting. The key insight: Sanctums are serializable layout declarations that map 1:1 to platform primitives. A Sanctum isn't a pixel rectangle — it's a specification that says "this region uses a Rank formation with 8px spacing." That declaration is enough for Magic's Projection to emit `HStack(spacing: 8)` or `flex-direction: row; gap: 8px`.

### Surge — Animation Curves (`surge/`)

`Surge` trait with 5 implementations + `Shift` (boxed wrapper with presets):

| Type | Behavior |
|------|----------|
| `SpringSurge` | Underdamped, overshoots |
| `EaseSurge` | Cubic ease-in-out |
| `LinearSurge` | Constant rate |
| `DecaySurge` | Exponential decay |
| `SnapSurge` | Instant transition |

Surge trait: `value(t) -> f64`, `is_complete(t) -> bool`, `duration() -> f64`.

### Reign — Theming (`reign.rs`)

- **`Reign`** — complete theme (name + Aura tokens + Aspect appearance mode). Shortcut accessors for all Aura subsystems including gradients, image styles, motion preference, minimum touch target, and minimum font size.
- **`Aspect`** — appearance enum: Light, Dark, Custom(String)

### Component Style (`component_style.rs`)

- **`ComponentStyle`** — composite style that references Aura token keys by name (crest color, background, padding, radius, typography, shadow, gradient, material). Presets: `primary_button()`, `card()`, `input_field()`, `text_body()`.
- **`ComponentStyleRegistry`** — named registry of component styles. `register()`, `get()`, `list()`, `remove()`.

### Theme Collection (`theme_collection.rs`)

- **`ThemeCollection`** — multi-theme management with an active selection. `new()`, `add()`, `remove()`, `switch()`, `get()`, `get_mut()`, `list()`, `count()`. Error handling for theme-not-found, cannot-remove-active, cannot-remove-last, already-exists.

### Crown Jewels — Universal Material System (`crown_jewels/`)

A universal material system where any visual material can be described, cascaded via stylesheets, and composed with layout regions.

**Core abstractions:**
- **`Material`** trait — `applying(delta) -> Self`, `kind() -> &str`. Any material type implements this.
- **`MaterialDelta`** trait — partial updates to a material.
- **`Stylesheet<M: Material>`** — generic CSS-like cascade (base + role overrides + deltas). Works with any Material.
- **`CrownRole`** — extensible semantic role for material cascade (panel, sidebar, controlBar, tile, overlay).

**Built-in materials:**
- **`FacetStyle`** (glass) — 19 properties: frost, refraction, dispersion, depth, splay, light, tint, etc. `FacetVariant` presets. `FacetAppearance` for light/dark adaptation. `FacetStyleDelta` for partial updates. `LightSource` for directional lighting.
- **`IrisStyle`** (thin-film interference) — 15 properties + `IrisDimple` array (max `DIMPLE_MAX_COUNT`). Presets: nacre, oil slick, beetle, etc. `IrisStyleDelta` for partial updates.

**Geometry:**
- **`ShapeDescriptor`** — SDF primitives: RoundedRect, Capsule, Circle, Ellipse, Polygon, Star. `CornerRadii` for per-corner control.
- **SDF math** (`sdf.rs`) — `sdf_rounded_rect`, `sdf_ellipse`, `sdf_polygon`, `sdf_star`, `smooth_min`. Pure f64, no platform deps.

**Utilities:**
- **Color math** (`color_math.rs`) — HSL/HSB conversion, WCAG contrast ratio, relative luminance, Fresnel spectral weights, premultiply.
- **`OneEuroFilter`** (`filter.rs`) — adaptive low-pass signal smoothing for input/gesture data.

**Layout + Material bridge:**
- **`CrownSanctum`** — Sanctum + CrownRole + ShapeDescriptor (layout + material binding)
- **`CrownArbiter`** — resolves layout via Arbiter + material styles via Stylesheet
- **`CrownAppointment`** — resolved layout node with material
- **`CrownDomain`** — complete result (layout + materials)

## Extensibility Patterns

All token types support custom dictionaries:
- `Crest` has `custom: HashMap<String, Ember>` for brand colors beyond the 11 semantic slots
- `Span` has `custom: HashMap<String, f64>` for project-specific spacing
- `Inscription` has `custom: HashMap<String, Glyph>` for additional type styles
- `Aura` has `gradients: HashMap<String, Gradient>` and `image_styles: HashMap<String, ImageStyle>` for design extensions
- `FormationKind::Custom(String)` with a runtime resolver for user-defined layout algorithms
- `ComponentStyleRegistry` maps named component styles to Aura token key references
- `ThemeCollection` manages multiple `Reign` themes with active-theme switching

Adding a new material = implementing `Material` + `MaterialDelta` traits, then creating a subdirectory under `materials/`.

All types implement `Serialize + Deserialize` for `.excalibur` theme file persistence.

## Dependencies

```toml
x = { path = "../X" }   # Shared utilities
serde, serde_json, thiserror, uuid, log
```

Regalia has zero dependencies on any other Omninet crate — it's pure design vocabulary.

## Covenant Alignment

**Dignity** — consistent, beautiful design is a right. **Sovereignty** — themes are user-configurable; `.excalibur` files can be shared.
