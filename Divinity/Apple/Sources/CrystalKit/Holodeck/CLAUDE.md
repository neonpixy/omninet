# Holodeck/

Metal rendering pipeline for glass and iris surfaces. Two material renderers sharing shader infrastructure.

## Files

| File | Role |
|------|------|
| `GlassRenderer.swift` | Glass material: SDF shape descriptor building, per-shape material application, confluence (goo) rendering. Contains `buildShapeDescriptor()` and `ConfluenceShapeDescriptor` struct. |
| `GlassShader.swift` | **Glass Metal shader source** as a Swift string, compiled at runtime via `MTLDevice.makeLibrary(source:)`. Contains both standalone and goo composite fragment shaders. |
| `IrisRenderer.swift` | Iris (thin-film interference) material renderer for opaque/metallic surfaces. |
| `IrisShader.swift` | **Iris Metal shader source** as a Swift string, compiled at runtime. |
| `BlurRenderer.swift` | Kawase multi-pass blur (4-5 passes, ping-pong textures). |
| `ResonanceTintCache.swift` | Caches tint colors for interaction effects (resonance). |
| `GlassBackgroundCache.swift` | Caches background textures for glass rendering. |

## Shape Type Mapping in `buildShapeDescriptor()`

This is the single source of truth for how `ShapeDescriptor` maps to Metal `shapeType`:

| ShapeDescriptor | shapeType | Corner Radii |
|----------------|-----------|-------------|
| `.roundedRect` | 0 | From descriptor (or style override) |
| `.capsule` | 0 | `min(halfWidth, halfHeight)` uniform |
| `.circle`, `.ellipse` | 1 | N/A |
| `.polygon` | 2 | N/A |
| `.star` | 3 | N/A |
| `.custom` | 0 | Uses SDF texture instead |

**Capsule is shapeType 0 (roundedRect), not 1 (ellipse).** The ellipse SDF uses Newton iteration that is numerically fragile at wide aspect ratios. A capsule is geometrically a rounded rect with maximal corner radius.

## ConfluenceShapeDescriptor Struct Alignment

The Swift struct has **3 explicit padding fields** (`_pad0`, `_pad1`, `_pad2`) between `sdfTexelToPoint` and `tintColor`. These align the `SIMD4<Float> tintColor` to a 16-byte boundary matching the Metal struct layout.

```
offset 0:   position (float2)
offset 8:   halfSize (float2)
offset 16:  cornerRadii (float4)
offset 32:  rotation, shapeType, sides, innerRadius, outerRadius,
            polygonBorderRadius, starInnerBorderRadius, cornerSmoothing
offset 64:  sdfTextureIndex (int32)
        +4: implicit padding for float2 alignment
offset 72:  sdfMaskPadding (float2)
offset 80:  sdfTexelToPoint (float)
offset 84:  _pad0, _pad1, _pad2 (3 x float)  -- DO NOT REMOVE
offset 96:  tintColor (float4, 16-byte aligned)
offset 112: tintOpacity ... splayStrength (10 x float)
stride 160: (rounded up to 16-byte alignment)
```

Removing the padding fields silently corrupts every shape after the first in the buffer.

## Per-Shape Material

`applyMaterial(to:style:)` fills tint, refraction, frost, dispersion, depth, lighting, and edge properties on each `ConfluenceShapeDescriptor`. Called with the **per-node style** (not the group style). The goo shader blends materials from all shapes weighted by SDF proximity.

## Limits

- `cachedSDF[8]` in shader -- max 8 shapes per composite pass (no Swift-side guard)
- 8 SDF texture slots (indices 2-9) -- capped by `min(sdfTextures.count, 8)`
- `weights[8]` in material blend -- same 8-shape ceiling
