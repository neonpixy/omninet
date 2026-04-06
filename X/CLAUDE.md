# X -- Shared Utilities

The common ground. Shared types, CRDT infrastructure, geographic primitives, image placeholders, and common utilities that every other letter imports. Zero UI dependencies. Zero business logic. If it's used by three or more letters, it lives here.

## Modules

### `value` -- Polymorphic Value Type

- **`Value`** -- 9-variant enum: Null, Bool(bool), Int(i64), Double(f64), String(String), Date(DateTime<Utc>), Data(Vec<u8>), Array(Vec<Value>), Dictionary(HashMap<String, Value>).
- Custom serde: encodes as single-key JSON objects for type disambiguation (`{"string": "hello"}`, `{"int": 42}`, `{"data": "<base64>"}`, `{"date": "<RFC3339>"}`).
- Typed accessors: `as_bool()`, `as_int()`, `as_double()`, `as_str()`, `as_date()`, `as_data()`, `as_array()`, `as_dictionary()`, plus `is_null()`.
- `From` impls for bool, i64, f64, String, &str, Vec<u8>, Vec<Value>, DateTime<Utc>.

### `crdt` -- Conflict-Free Replicated Data Types

- **`VectorClock`** -- `HashMap<String, u64>`. Increment, merge, compare (Less/Greater/Equal/Concurrent). Author IDs truncated to 8 chars (strips `cpub1` prefix).
- **`CrdtOperation` trait** -- `id()`, `target_id()`, `vector()`, `timestamp()`, `author()`. Generic interface for any module's operations. Requires Clone + Serialize + DeserializeOwned + Send + Sync.
- **`CrdtEngine`** -- Generic idempotent apply, merge (deduplication + causal sort), last-writer-wins conflict resolution, conflict detection.
- **`OperationLog<T: CrdtOperation>`** -- Newline-delimited JSON append log, snapshot save/load.

### `geo` -- Geographic Coordinate Primitives

- **`GeoCoordinate`** -- Latitude/longitude with optional altitude and accuracy. Validated on construction (-90..90 lat, -180..180 lon). Serialize/Deserialize + Display.
- Methods: `distance_to()` (Haversine, WGS-84 mean radius), `is_within()` (radius check), `bearing_to()` (degrees, 0=north), `midpoint()`.
- Builder: `with_altitude()`, `with_accuracy()`.
- **`point_in_polygon()`** -- Ray-casting even-odd rule. Implicitly closed polygon.
- **`polygon_area()`** -- Spherical excess approximation in square meters.
- **`GeoError`** -- InvalidLatitude, InvalidLongitude, InvalidRadius, InvalidPolygon.
- Used by World/Physical and any crate needing geographic math.

### `blurhash` -- Compact Image Placeholders

- Pure-math implementation of the [BlurHash](https://blurha.sh) algorithm. No platform or image library dependencies.
- **`encode()`** -- RGBA pixels + dimensions + component count (1-9 per axis) -> blurhash string (typically 20-30 characters). DCT-based.
- **`decode()`** -- Blurhash string + desired dimensions -> RGBA pixel data.
- **`components()`** -- Extract component count from a hash string.
- **`is_valid()`** -- Validate hash length matches its component count.
- Internal: Base83 encoding, sRGB <-> linear color space conversion.

### `color` -- RGBA Color with HSL/HSB, WCAG, Blend Modes

- **`Color`** -- RGBA with conversions: HSL, HSB, hex, luminance, WCAG contrast ratio.
- **`BlendMode`** -- Normal, Multiply, Screen, Overlay, Darken, Lighten, etc.
- Ported from Swiftlight's CASColor.swift and Regalia's color_math.rs.

### `math` -- Pure Math Utilities

- Interpolation: `lerp`, `inverse_lerp`, `smoothstep`, `smooth_start`, `smooth_stop`.
- Clamping, angles (degrees/radians), bezier curves.
- Ported from Swiftlight's MathUtils.swift.

### `geometry` -- 2D Geometry Primitives

- **`Vector2`**, **`Point`**, **`Size`**, **`Rect`**, **`Transform`**, **`Matrix3`**, **`EdgeInsets`**, **`Anchor`**.
- Full arithmetic ops, intersection, union, contains, affine transforms.

## Public Re-exports

```rust
pub use color::{BlendMode, Color, ColorError};
pub use crdt::{CrdtEngine, CrdtOperation, OperationLog, SequenceAtom, SequenceId, SequenceOp, SequenceRga};
pub use crdt::vector_clock::{ClockComparison, VectorClock};
pub use geo::{GeoCoordinate, GeoError, point_in_polygon, polygon_area};
pub use geometry::{Anchor, EdgeInsets, Matrix3, Point, Rect, Size, Transform, Vector2};
pub use value::Value;
// blurhash is a public module (x::blurhash::encode, x::blurhash::decode, etc.)
```

## Dependencies

External only (X is the foundation -- no internal Omninet deps):

```toml
uuid, chrono, serde, serde_json, base64, log, thiserror
```

## Still To Come

- Structured logging (currently depends on `log` crate facade)

## Covenant Alignment

X itself is value-neutral infrastructure. Its Covenant duty is to be correct, stable, and never a point of coupling or control.
